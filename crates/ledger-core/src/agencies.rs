//! US sales tax agencies (QuickBooks): each sales tax rate is made of components (state, county,
//! city…) and each component is owed to an agency. Tax collected is reported, filed and paid per
//! agency, each on its own filing frequency.

use crate::tax::{Component, TaxRateInput};
use crate::{error::invalid, reports::{fmt_long, ReportParams}, util, Ledger, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

pub const FREQUENCIES: [&str; 3] = ["MONTHLY", "QUARTERLY", "ANNUALLY"];

impl Ledger {
    /// Find an agency by name, or create it.
    pub fn agency(&self, name: &str, frequency: Option<&str>) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() { return invalid("the agency needs a name"); }
        if let Some(id) = self.conn.query_row("SELECT id FROM tax_agencies WHERE name = ?1 COLLATE NOCASE", [name], |r| r.get(0)).optional()? { return Ok(id); }
        let freq = frequency.unwrap_or("QUARTERLY");
        if !FREQUENCIES.contains(&freq) { return invalid("filing frequency must be monthly, quarterly or annually"); }
        self.conn.execute("INSERT INTO tax_agencies(name, frequency) VALUES (?1, ?2)", params![name, freq])?;
        let id = self.conn.last_insert_rowid();
        self.audit("CREATE", "tax_agency", Some(id), None, Some(json!({"name": name, "frequency": freq})))?;
        Ok(id)
    }

    pub fn set_agency_frequency(&self, id: i64, frequency: &str) -> Result<()> {
        if !FREQUENCIES.contains(&frequency) { return invalid("filing frequency must be monthly, quarterly or annually"); }
        self.conn.execute("UPDATE tax_agencies SET frequency = ?2 WHERE id = ?1", params![id, frequency])?;
        Ok(())
    }

    /// A sales tax rate built from (agency, component name, rate) parts, e.g. state 6% + county 1.25%.
    pub fn create_sales_tax_rate(&self, name: &str, parts: &[(String, String, i64)]) -> Result<i64> {
        if parts.is_empty() { return invalid("add at least one agency and rate"); }
        self.atomic(|| {
            let comps: Vec<Component> = parts.iter().map(|(agency, comp, rate)| Component { name: if comp.trim().is_empty() { agency.clone() } else { comp.clone() }, rate_bp: *rate, compound: false }).collect();
            let id = self.create_tax_rate(&TaxRateInput { name: name.into(), report_type: "OUTPUT".into(), components: comps, rate_bp: None })?;
            let comp_ids: Vec<i64> = {
                let mut st = self.conn.prepare("SELECT id FROM tax_components WHERE tax_rate_id = ?1 ORDER BY id")?;
                st.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
            };
            for ((agency, _, _), cid) in parts.iter().zip(comp_ids) {
                let a = self.agency(agency, None)?;
                self.conn.execute("UPDATE tax_components SET agency_id = ?2 WHERE id = ?1", params![cid, a])?;
            }
            Ok(id)
        })
    }

    /// Tax collected per agency in a period: each rate's tax is split across its components in
    /// proportion to their rates. Returns rows of {id, name, frequency, taxable, tax}; components
    /// without an agency are grouped under id 0.
    pub fn tax_by_agency(&self, from: &str, to: &str, cash: bool) -> Result<Vec<Value>> {
        let agencies = util::query_json(&self.conn, "SELECT id, name, frequency FROM tax_agencies WHERE status = 'ACTIVE' ORDER BY name", [])?;
        let mut totals: Vec<(i64, i64, i64)> = agencies.iter().map(|a| (a["id"].as_i64().unwrap_or(0), 0, 0)).collect();
        let mut none = (0i64, 0i64);
        for r in self.tax_summary(from, to, cash)? {
            if r["report_type"] != "OUTPUT" { continue; }
            let rate = r["tax_rate_id"].as_i64().unwrap_or(0);
            let (net, tax) = (r["net"].as_i64().unwrap_or(0), r["tax"].as_i64().unwrap_or(0));
            let comps: Vec<(Option<i64>, i64)> = {
                let mut st = self.conn.prepare_cached("SELECT agency_id, rate_bp FROM tax_components WHERE tax_rate_id = ?1 ORDER BY id")?;
                st.query_map([rate], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?
            };
            let sum: i64 = comps.iter().map(|c| c.1).sum();
            let mut left = tax;
            for (n, (agency, bp)) in comps.iter().enumerate() {
                let share = if n + 1 == comps.len() { left } else if sum == 0 { 0 } else { crate::money::div_round(tax as i128 * *bp as i128, sum as i128) as i64 };
                left -= share;
                match agency.and_then(|a| totals.iter_mut().find(|t| t.0 == a)) {
                    Some(t) => { t.1 += net; t.2 += share; }
                    None => { none.0 += net; none.1 += share; }
                }
            }
            if comps.is_empty() { none.0 += net; none.1 += tax; }
        }
        let mut out: Vec<Value> = agencies.iter().zip(totals).map(|(a, (_, net, tax))| json!({"id": a["id"], "name": a["name"], "frequency": a["frequency"], "taxable": net, "tax": tax})).collect();
        if none.1 != 0 { out.push(json!({"id": 0, "name": "No agency assigned", "frequency": "", "taxable": none.0, "tax": none.1})); }
        Ok(out)
    }

    /// Current filing period for an agency (the period containing `today`).
    pub fn agency_period(&self, frequency: &str, today: &str) -> Result<(String, String)> {
        let d = util::parse_date(today)?;
        use chrono::Datelike;
        let months = match frequency { "MONTHLY" => 1, "ANNUALLY" => 12, _ => 3 };
        let m0 = (d.month0() / months) * months + 1;
        let start = chrono::NaiveDate::from_ymd_opt(d.year(), m0, 1).unwrap();
        let end = util::add_months(start, months as i32) - chrono::Duration::days(1);
        Ok((util::fmt_date(start), util::fmt_date(end)))
    }

    /// Agencies with their current period and the tax collected so far in it.
    pub fn list_agencies(&self) -> Result<Vec<Value>> {
        let today = util::today();
        let cash = self.setting_or("tax_basis", "ACCRUAL")? == "CASH";
        let mut out = Vec::new();
        for a in util::query_json(&self.conn, "SELECT * FROM tax_agencies WHERE status = 'ACTIVE' ORDER BY name", [])? {
            let (from, to) = self.agency_period(a["frequency"].as_str().unwrap_or("QUARTERLY"), &today)?;
            let row = self.tax_by_agency(&from, &to, cash)?.into_iter().find(|r| r["id"] == a["id"]).unwrap_or(json!({}));
            let rates = util::query_json(&self.conn, "SELECT t.name, c.rate_bp FROM tax_components c JOIN tax_rates t ON t.id = c.tax_rate_id WHERE c.agency_id = ?1 AND t.status = 'ACTIVE'", [a["id"].as_i64()])?;
            out.push(json!({"id": a["id"], "name": a["name"], "frequency": a["frequency"], "period_from": from, "period_to": to, "taxable": row["taxable"], "tax": row["tax"], "rates": rates}));
        }
        Ok(out)
    }

    /// "Sales Tax Liability by Agency" report.
    pub fn report_tax_by_agency(&self, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().or(p.date.clone()).unwrap_or_else(util::today);
        let from = p.from.clone().unwrap_or_else(|| format!("{}-01-01", &to[..4]));
        let cash = p.basis.as_deref() == Some("CASH");
        let rows: Vec<Value> = self.tax_by_agency(&from, &to, cash)?.into_iter().map(|r| json!({"label": r["name"], "values": [r["taxable"], r["tax"]]})).collect();
        let tot = |i: usize| rows.iter().map(|r| r["values"][i].as_i64().unwrap_or(0)).sum::<i64>();
        let total = json!({"label": "Total", "values": [tot(0), tot(1)]});
        Ok(json!({"title": "Sales Tax Liability by Agency", "org": self.setting_or("org_name", "")?, "subtitle": format!("{} to {}", fmt_long(&from), fmt_long(&to)),
            "basis": if cash { "Cash" } else { "Accrual" }, "columns": ["Taxable sales", "Tax collected"], "sections": [{"title": "", "rows": rows, "total": total}]}))
    }
}
