//! Tax returns per jurisdiction (BAS, GST101, VAT100/MTD, VAT3, GST/HST, US sales tax, generic).
//! Each jurisdiction implements [`TaxReturnForm`]. Returns are computed from the tax summary of
//! the ledger, so their totals always reconcile to the Tax Reconciliation report.

use crate::{error::invalid, money, util, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Net/tax by report type for a period (sales positive, purchases negative).
pub struct TaxTotals { pub by_type: HashMap<String, (i64, i64)>, pub wages: (i64, i64) }

impl TaxTotals {
    fn net(&self, t: &[&str]) -> i64 { t.iter().map(|k| self.by_type.get(*k).map(|x| x.0).unwrap_or(0)).sum() }
    fn tax(&self, t: &[&str]) -> i64 { t.iter().map(|k| self.by_type.get(*k).map(|x| x.1).unwrap_or(0)).sum() }
    fn gross(&self, t: &[&str]) -> i64 { self.net(t) + self.tax(t) }
}

pub trait TaxReturnForm {
    fn code(&self) -> &'static str;
    fn title(&self) -> &'static str;
    /// (box, label, amount in cents, is_total)
    fn boxes(&self, t: &TaxTotals) -> Vec<(&'static str, &'static str, i64, bool)>;
}

struct AuBas;
impl TaxReturnForm for AuBas {
    fn code(&self) -> &'static str { "AU-BAS" }
    fn title(&self) -> &'static str { "Business Activity Statement" }
    fn boxes(&self, t: &TaxTotals) -> Vec<(&'static str, &'static str, i64, bool)> {
        let whole = |v: i64| (v / 100) * 100; // ATO reports whole dollars (cents dropped)
        let g1 = whole(t.gross(&["OUTPUT", "EXEMPTOUTPUT", "EXPORTS", "ZERORATED"]));
        let g2 = whole(t.net(&["EXPORTS"]));
        let g3 = whole(t.net(&["EXEMPTOUTPUT", "ZERORATED"]));
        let g10 = whole(-t.gross(&["CAPEXINPUT", "EXEMPTCAPITAL"]));
        let g11 = whole(-t.gross(&["INPUT", "EXEMPTINPUT", "INPUTTAXED", "GSTONIMPORTS"]));
        let a1 = whole(t.tax(&["OUTPUT"]));
        let b1 = whole(-t.tax(&["INPUT", "CAPEXINPUT", "GSTONIMPORTS"]));
        let w1 = whole(t.wages.0);
        let w2 = whole(t.wages.1);
        vec![("G1", "Total sales (including any GST)", g1, false), ("G2", "Export sales", g2, false), ("G3", "Other GST-free sales", g3, false),
            ("G10", "Capital purchases (including any GST)", g10, false), ("G11", "Non-capital purchases (including any GST)", g11, false),
            ("1A", "GST on sales", a1, false), ("1B", "GST on purchases", b1, false),
            ("W1", "Total salary, wages and other payments", w1, false), ("W2", "Amounts withheld from payments shown at W1", w2, false),
            ("9", if a1 + w2 - b1 >= 0 { "Amount you owe the ATO" } else { "Amount the ATO owes you" }, a1 + w2 - b1, true)]
    }
}

struct NzGst101;
impl TaxReturnForm for NzGst101 {
    fn code(&self) -> &'static str { "NZ-GST101" }
    fn title(&self) -> &'static str { "GST Return (GST101A)" }
    fn boxes(&self, t: &TaxTotals) -> Vec<(&'static str, &'static str, i64, bool)> {
        let b5 = t.gross(&["OUTPUT", "ZERORATED"]);
        let b6 = t.net(&["ZERORATED"]);
        let b7 = b5 - b6;
        let b8 = money::div_round(b7 as i128 * 3, 23) as i64;
        let b10 = b8;
        let b11 = -t.gross(&["INPUT", "CAPEXINPUT", "GSTONIMPORTS"]);
        let b12 = money::div_round(b11 as i128 * 3, 23) as i64;
        let b14 = b12;
        vec![("5", "Total sales and income (including GST and zero-rated)", b5, false), ("6", "Zero-rated supplies", b6, false), ("7", "Box 5 minus box 6", b7, false),
            ("8", "Multiply box 7 by 3 and divide by 23", b8, false), ("9", "Adjustments from calculation sheet", 0, false), ("10", "Total GST collected on sales and income", b10, false),
            ("11", "Total purchases and expenses (including GST)", b11, false), ("12", "Multiply box 11 by 3 and divide by 23", b12, false), ("13", "Credit adjustments", 0, false),
            ("14", "Total GST credit for purchases and expenses", b14, false), ("15", if b10 - b14 >= 0 { "GST to pay" } else { "GST refund" }, b10 - b14, true)]
    }
}

struct UkVat100;
impl TaxReturnForm for UkVat100 {
    fn code(&self) -> &'static str { "UK-VAT100" }
    fn title(&self) -> &'static str { "VAT Return (VAT100)" }
    fn boxes(&self, t: &TaxTotals) -> Vec<(&'static str, &'static str, i64, bool)> {
        let rc = t.tax(&["REVERSECHARGE"]).abs();
        let b1 = t.tax(&["OUTPUT", "ECOUTPUT"]) + rc;
        let b2 = 0;
        let b3 = b1 + b2;
        let b4 = -t.tax(&["INPUT", "CAPEXINPUT", "ECINPUT"]) + rc;
        let b5 = b3 - b4;
        let pounds = |v: i64| (v / 100) * 100;
        let b6 = pounds(t.net(&["OUTPUT", "ZERORATED", "EXEMPTOUTPUT", "ECOUTPUT"]));
        let b7 = pounds(-t.net(&["INPUT", "ZERORATEDINPUT", "EXEMPTINPUT", "CAPEXINPUT", "REVERSECHARGE", "ECINPUT"]));
        vec![("1", "VAT due on sales and other outputs", b1, false), ("2", "VAT due on acquisitions from EU member states (NI)", b2, false), ("3", "Total VAT due", b3, false),
            ("4", "VAT reclaimed on purchases and other inputs", b4, false), ("5", if b5 >= 0 { "Net VAT to pay HMRC" } else { "Net VAT to reclaim from HMRC" }, b5, true),
            ("6", "Total value of sales excluding VAT", b6, false), ("7", "Total value of purchases excluding VAT", b7, false),
            ("8", "Total value of supplies to EU member states (NI)", 0, false), ("9", "Total value of acquisitions from EU member states (NI)", 0, false)]
    }
}

struct IeVat3;
impl TaxReturnForm for IeVat3 {
    fn code(&self) -> &'static str { "IE-VAT3" }
    fn title(&self) -> &'static str { "VAT3 Return" }
    fn boxes(&self, t: &TaxTotals) -> Vec<(&'static str, &'static str, i64, bool)> {
        let rc = t.tax(&["REVERSECHARGE"]).abs();
        let t1 = t.tax(&["OUTPUT"]) + rc;
        let t2 = -t.tax(&["INPUT", "CAPEXINPUT"]) + rc;
        vec![("T1", "VAT on sales", t1, false), ("T2", "VAT on purchases", t2, false), ("T3", "Net payable", (t1 - t2).max(0), true), ("T4", "Net repayable", (t2 - t1).max(0), true),
            ("E1", "Intra-EU dispatches of goods", 0, false), ("E2", "Intra-EU acquisitions of goods", 0, false)]
    }
}

struct CaGst;
impl TaxReturnForm for CaGst {
    fn code(&self) -> &'static str { "CA-GST34" }
    fn title(&self) -> &'static str { "GST/HST Return" }
    fn boxes(&self, t: &TaxTotals) -> Vec<(&'static str, &'static str, i64, bool)> {
        let l101 = (t.net(&["OUTPUT", "ZERORATED", "EXEMPTOUTPUT"]) / 100) * 100;
        let l103 = t.tax(&["OUTPUT"]);
        let l106 = -t.tax(&["INPUT", "CAPEXINPUT"]);
        vec![("101", "Sales and other revenue", l101, false), ("103", "GST/HST collected or collectible", l103, false), ("105", "Total GST/HST and adjustments", l103, false),
            ("106", "Input tax credits (ITCs)", l106, false), ("108", "Total ITCs and adjustments", l106, false), ("109", "Net tax", l103 - l106, true)]
    }
}

struct UsSalesTax;
impl TaxReturnForm for UsSalesTax {
    fn code(&self) -> &'static str { "US-SALESTAX" }
    fn title(&self) -> &'static str { "Sales Tax Liability" }
    fn boxes(&self, t: &TaxTotals) -> Vec<(&'static str, &'static str, i64, bool)> {
        let total = t.net(&["OUTPUT", "NONE", "EXEMPTOUTPUT", "ZERORATED"]);
        let taxable = t.net(&["OUTPUT"]);
        vec![("A", "Total sales", total, false), ("B", "Non-taxable sales", total - taxable, false), ("C", "Taxable sales", taxable, false), ("D", "Sales tax collected", t.tax(&["OUTPUT"]), true)]
    }
}

struct Generic;
impl TaxReturnForm for Generic {
    fn code(&self) -> &'static str { "GENERIC" }
    fn title(&self) -> &'static str { "VAT/GST Return" }
    fn boxes(&self, t: &TaxTotals) -> Vec<(&'static str, &'static str, i64, bool)> {
        let out = t.tax(&["OUTPUT"]) + t.tax(&["REVERSECHARGE"]).abs();
        let inp = -t.tax(&["INPUT", "CAPEXINPUT", "GSTONIMPORTS"]) + t.tax(&["REVERSECHARGE"]).abs();
        vec![("S", "Taxable sales (net)", t.net(&["OUTPUT"]), false), ("P", "Taxable purchases (net)", -t.net(&["INPUT", "CAPEXINPUT"]), false),
            ("O", "Output tax", out, false), ("I", "Input tax", inp, false), ("N", "Net tax payable / (refundable)", out - inp, true)]
    }
}

pub fn form_for(code: &str) -> Box<dyn TaxReturnForm> {
    match code { "AU-BAS" => Box::new(AuBas), "NZ-GST101" => Box::new(NzGst101), "UK-VAT100" => Box::new(UkVat100), "IE-VAT3" => Box::new(IeVat3),
        "CA-GST34" => Box::new(CaGst), "US-SALESTAX" => Box::new(UsSalesTax), _ => Box::new(Generic) }
}

pub fn default_form(country: &str) -> &'static str {
    match country { "AU" => "AU-BAS", "NZ" => "NZ-GST101", "GB" | "UK" => "UK-VAT100", "IE" => "IE-VAT3", "CA" => "CA-GST34", "US" => "US-SALESTAX", _ => "GENERIC" }
}

impl Ledger {
    pub fn tax_totals(&self, from: &str, to: &str, cash: bool) -> Result<TaxTotals> {
        let mut by_type: HashMap<String, (i64, i64)> = HashMap::new();
        for r in self.tax_summary(from, to, cash)? {
            let e = by_type.entry(r["report_type"].as_str().unwrap_or("").to_string()).or_default();
            e.0 += r["net"].as_i64().unwrap_or(0); e.1 += r["tax"].as_i64().unwrap_or(0);
        }
        let wages: (i64, i64) = self.conn.query_row("SELECT COALESCE(SUM(wages),0), COALESCE(SUM(tax),0) FROM pay_runs WHERE status = 'POSTED' AND payment_date >= ?1 AND payment_date <= ?2",
            params![from, to], |r| Ok((r.get(0)?, r.get(1)?))).unwrap_or((0, 0));
        Ok(TaxTotals { by_type, wages })
    }

    pub fn compute_return(&self, form: &str, from: &str, to: &str, basis: &str) -> Result<Value> {
        let f = form_for(form);
        let t = self.tax_totals(from, to, basis == "CASH")?;
        let boxes: Vec<Value> = f.boxes(&t).into_iter().map(|(b, l, a, tot)| json!({"box": b, "label": l, "amount": a, "total": tot})).collect();
        Ok(json!({"form": f.code(), "title": f.title(), "from": from, "to": to, "basis": basis, "boxes": boxes}))
    }

    pub fn list_tax_returns(&self) -> Result<Vec<Value>> {
        let mut v = util::query_json(&self.conn, "SELECT * FROM tax_returns ORDER BY period_end DESC", [])?;
        for r in v.iter_mut() { r["data"] = serde_json::from_str(r["data"].as_str().unwrap_or("{}")).unwrap_or(json!({})); }
        Ok(v)
    }

    /// Suggested next return period from the org's tax period setting.
    pub fn next_tax_period(&self) -> Result<(String, String)> {
        let months: i32 = match self.setting_or("tax_period", "QUARTERLY")?.as_str() { "MONTHLY" => 1, "TWOMONTHLY" => 2, "SIXMONTHLY" => 6, "ANNUALLY" => 12, _ => 3 };
        let last: Option<String> = self.conn.query_row("SELECT MAX(period_end) FROM tax_returns WHERE status = 'FILED'", [], |r| r.get(0))?;
        let start = match last { Some(l) => util::parse_date(&util::add_days(&l, 1)?)?, None => {
            let fy = util::parse_date(&self.fy_start(&util::today())?)?;
            let t = util::parse_date(&util::today())?;
            let mut s = fy;
            while util::add_months(s, months) <= t { s = util::add_months(s, months); }
            util::add_months(s, -months)
        } };
        let end = util::add_months(start, months) - chrono::Duration::days(1);
        Ok((util::fmt_date(start), util::fmt_date(end)))
    }

    pub fn create_tax_return(&self, form: Option<&str>, from: &str, to: &str, basis: Option<&str>) -> Result<i64> {
        util::parse_date(from)?; util::parse_date(to)?;
        if from > to { return invalid("the period end must be after the start"); }
        let form = form.map(String::from).unwrap_or_else(|| default_form(&self.setting_or("country", "AU").unwrap_or_default()).to_string());
        let basis = basis.map(String::from).unwrap_or(self.setting_or("tax_basis", "ACCRUAL")?);
        let data = self.compute_return(&form, from, to, &basis)?;
        self.conn.execute("INSERT INTO tax_returns(jurisdiction, period_start, period_end, basis, data) VALUES (?1, ?2, ?3, ?4, ?5)", params![form, from, to, basis, data.to_string()])?;
        let id = self.conn.last_insert_rowid();
        self.audit("CREATE", "tax_return", Some(id), None, Some(json!({"form": form, "from": from, "to": to})))?;
        Ok(id)
    }

    pub fn get_tax_return(&self, id: i64) -> Result<Value> {
        let mut r = util::query_one_json(&self.conn, "SELECT * FROM tax_returns WHERE id = ?1", [id])?;
        if r["status"] == "DRAFT" {
            // drafts are always recalculated so they reflect the latest transactions
            let data = self.compute_return(r["jurisdiction"].as_str().unwrap(), r["period_start"].as_str().unwrap(), r["period_end"].as_str().unwrap(), r["basis"].as_str().unwrap())?;
            self.conn.execute("UPDATE tax_returns SET data = ?2 WHERE id = ?1", params![id, data.to_string()])?;
            r["data"] = data;
        } else {
            r["data"] = serde_json::from_str(r["data"].as_str().unwrap_or("{}"))?;
        }
        r["transactions"] = json!(self.tax_summary(r["period_start"].as_str().unwrap(), r["period_end"].as_str().unwrap(), r["basis"] == "CASH")?);
        r["history"] = json!(self.history("tax_return", id)?);
        Ok(r)
    }

    /// Mark a return as filed: freezes its figures and locks the period for non-advisers.
    pub fn file_tax_return(&self, id: i64) -> Result<()> {
        let r = self.get_tax_return(id)?;
        if r["status"] != "DRAFT" { return invalid("this return is already filed"); }
        let end = r["period_end"].as_str().unwrap().to_string();
        self.atomic(|| {
            self.conn.execute("UPDATE tax_returns SET status = 'FILED', filed_at = datetime('now'), data = ?2 WHERE id = ?1", params![id, r["data"].to_string()])?;
            let cur = self.setting("lock_date_others")?.unwrap_or_default();
            if cur.as_str() < end.as_str() {
                self.conn.execute("INSERT INTO settings(key, value) VALUES ('lock_date_others', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value", [&end])?;
            }
            self.add_note("tax_return", id, "HISTORY", &format!("Marked as filed; transactions to {end} are now locked for non-adviser users"))?;
            self.audit("FILE", "tax_return", Some(id), None, None)?;
            Ok(())
        })
    }

    pub fn delete_tax_return(&self, id: i64) -> Result<()> {
        let st: String = self.conn.query_row("SELECT status FROM tax_returns WHERE id = ?1", [id], |r| r.get(0))?;
        if st != "DRAFT" && !self.is_adviser() { return invalid("only advisers can delete a filed return"); }
        self.conn.execute("DELETE FROM tax_returns WHERE id = ?1", [id])?;
        Ok(())
    }

    /// UK Making Tax Digital VAT return body (submission requires HMRC developer registration).
    pub fn mtd_payload(&self, id: i64) -> Result<Value> {
        let r = self.get_tax_return(id)?;
        let b = |k: &str| r["data"]["boxes"].as_array().unwrap().iter().find(|x| x["box"] == k).and_then(|x| x["amount"].as_i64()).unwrap_or(0);
        let dec = |v: i64| json!(v as f64 / 100.0);
        Ok(json!({
            "periodKey": "#001", "vatDueSales": dec(b("1")), "vatDueAcquisitions": dec(b("2")), "totalVatDue": dec(b("3")), "vatReclaimedCurrPeriod": dec(b("4")),
            "netVatDue": dec(b("5").abs()), "totalValueSalesExVAT": b("6") / 100, "totalValuePurchasesExVAT": b("7") / 100, "totalValueGoodsSuppliedExVAT": b("8") / 100,
            "totalAcquisitionsExVAT": b("9") / 100, "finalised": true,
            "_note": "Generated by LedgerCraft. Submitting to HMRC requires software registered with HMRC MTD — export file only."
        }))
    }
}
