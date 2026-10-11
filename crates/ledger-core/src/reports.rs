//! Financial reports built from the posted general ledger.
//! Report output shape (generic, rendered by the UI):
//! { title, subtitle, columns: [..], sections: [{ title, rows: [{label, account_id?, values: [..]}], total? }] }
//! All SQL here is parameterised; string formatting is only used for display labels.

use crate::{money, util, Ledger, Result};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ReportParams {
    pub from: Option<String>,
    pub to: Option<String>,
    pub date: Option<String>,
    pub basis: Option<String>,       // ACCRUAL | CASH
    pub compare: Option<i64>,        // number of comparative periods
    pub period: Option<String>,      // MONTH | QUARTER | YEAR
    pub monthly: Option<bool>,       // monthly columns across the range
    pub tracking: Option<i64>,       // tracking option filter
    pub account_id: Option<i64>,
    pub contact_id: Option<i64>,
    pub ageing_by: Option<String>,   // DUE | INVOICE
    pub show_zero: Option<bool>,
    pub budget_id: Option<i64>,
}

#[derive(Clone)]
struct Acct { id: i64, code: String, name: String, kind: String, class: String, sys: Option<String> }

pub struct Period { pub label: String, pub from: String, pub to: String }

fn ymd(d: chrono::NaiveDate) -> (i32, u32, u32) { use chrono::Datelike; (d.year(), d.month(), d.day()) }

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const MONTHS_LONG: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

fn month_label(d: chrono::NaiveDate) -> String { let (y, m, _) = ymd(d); String::from(MONTHS[m as usize - 1]) + " " + &y.to_string() }

impl Ledger {
    fn accounts_meta(&self) -> Result<Vec<Acct>> {
        let mut st = self.conn.prepare("SELECT id, code, name, type, class, system_account FROM accounts ORDER BY code")?;
        let v = st.query_map([], |r| Ok(Acct { id: r.get(0)?, code: r.get(1)?, name: r.get(2)?, kind: r.get(3)?, class: r.get(4)?, sys: r.get(5)? }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Net movement per account (debit +) over a date range, on an accrual or cash basis.
    pub fn account_movements(&self, from: Option<&str>, to: &str, cash: bool, tracking: Option<i64>) -> Result<HashMap<i64, i64>> {
        let from = from.unwrap_or("0000-01-01");
        let mut m: HashMap<i64, i64> = HashMap::new();
        if !cash {
            let mut st = self.conn.prepare_cached(
                "SELECT jl.account_id, SUM(jl.amount) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id
                 WHERE j.posted = 1 AND j.date >= ?1 AND j.date <= ?2 AND (?3 IS NULL OR jl.tracking1 = ?3 OR jl.tracking2 = ?3) GROUP BY jl.account_id")?;
            for r in st.query_map(params![from, to, tracking], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? { let (a, v) = r?; m.insert(a, v); }
            return Ok(m);
        }
        // Cash basis: non-document journals as-is, minus the AR/AP leg of settlement journals,
        // plus document lines recognised in proportion to settlements.
        let mut st = self.conn.prepare_cached(
            "SELECT jl.account_id, SUM(jl.amount) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id
             WHERE j.posted = 1 AND j.date >= ?1 AND j.date <= ?2 AND (?3 IS NULL OR jl.tracking1 = ?3 OR jl.tracking2 = ?3)
               AND j.source_type NOT IN ('ACCREC','ACCPAY','ACCRECCREDIT','ACCPAYCREDIT')
               AND NOT (j.reverses_journal_id IS NOT NULL AND (SELECT source_type FROM journals o WHERE o.id = j.reverses_journal_id) IN ('ACCREC','ACCPAY','ACCRECCREDIT','ACCPAYCREDIT'))
               AND j.cash_basis = 1
             GROUP BY jl.account_id")?;
        for r in st.query_map(params![from, to, tracking], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? { let (a, v) = r?; *m.entry(a).or_default() += v; }
        let ar = self.sys_account("AR")?; let ap = self.sys_account("AP")?;
        m.remove(&ar); m.remove(&ap);
        let mut st = self.conn.prepare_cached(
            "SELECT document_id, amount FROM payments WHERE status <> 'DELETED' AND date >= ?1 AND date <= ?2
             UNION ALL SELECT document_id, amount FROM credit_allocations WHERE status <> 'DELETED' AND date >= ?1 AND date <= ?2
             UNION ALL SELECT credit_note_id, amount FROM credit_allocations WHERE status <> 'DELETED' AND date >= ?1 AND date <= ?2
             UNION ALL SELECT document_id, amount FROM prepayment_allocations WHERE status <> 'DELETED' AND date >= ?1 AND date <= ?2")?;
        let settlements = st.query_map(params![from, to], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        for (doc, amt) in settlements {
            let (total, jid): (i64, Option<i64>) = self.conn.query_row("SELECT total, journal_id FROM documents WHERE id = ?1", [doc], |r| Ok((r.get(0)?, r.get(1)?)))?;
            let Some(jid) = jid else { continue };
            if total == 0 { continue; }
            let mut st2 = self.conn.prepare_cached("SELECT account_id, amount, tracking1, tracking2 FROM journal_lines WHERE journal_id = ?1")?;
            for r in st2.query_map([jid], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<i64>>(2)?, r.get::<_, Option<i64>>(3)?)))? {
                let (a, v, t1, t2) = r?;
                if a == ar || a == ap { continue; }
                if let Some(t) = tracking { if t1 != Some(t) && t2 != Some(t) { continue; } }
                *m.entry(a).or_default() += money::div_round(v as i128 * amt as i128, total as i128) as i64;
            }
        }
        Ok(m)
    }

    fn balances_at(&self, date: &str, tracking: Option<i64>) -> Result<HashMap<i64, i64>> { self.account_movements(None, date, false, tracking) }

    /// Build comparison periods ending at `to`.
    pub fn periods(&self, from: &str, to: &str, compare: i64, period: &str, monthly: bool) -> Result<Vec<Period>> {
        let mut out = Vec::new();
        let f = util::parse_date(from)?; let t = util::parse_date(to)?;
        if monthly {
            let (fy, fm, _) = ymd(f);
            let mut cur = chrono::NaiveDate::from_ymd_opt(fy, fm, 1).unwrap();
            while cur <= t {
                let (cy, cm, _) = ymd(cur);
                let end = util::last_day_of_month(cy, cm).min(t);
                let start = cur.max(f);
                out.push(Period { label: month_label(cur), from: util::fmt_date(start), to: util::fmt_date(end) });
                cur = util::add_months(cur, 1);
                if out.len() > 36 { break; }
            }
            return Ok(out);
        }
        out.push(Period { label: label_for(from, to), from: from.into(), to: to.into() });
        let months = match period { "QUARTER" => 3, "YEAR" => 12, _ => 1 };
        for i in 1..=compare.min(12) {
            let pf = util::add_months(f, -(months * i as i32));
            let pt = if is_month_end(t) { let x = util::add_months(t, -(months * i as i32)); let (y, m, _) = ymd(x); util::last_day_of_month(y, m) }
                else { util::add_months(t, -(months * i as i32)) };
            out.push(Period { label: label_for(&util::fmt_date(pf), &util::fmt_date(pt)), from: util::fmt_date(pf), to: util::fmt_date(pt) });
        }
        Ok(out)
    }

    pub fn profit_and_loss(&self, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().unwrap_or_else(util::today);
        let from = p.from.clone().unwrap_or(self.fy_start(&to)?);
        let cash = p.basis.as_deref() == Some("CASH");
        let periods = self.periods(&from, &to, p.compare.unwrap_or(0), p.period.as_deref().unwrap_or("MONTH"), p.monthly.unwrap_or(false))?;
        let accts = self.accounts_meta()?;
        let moves: Vec<HashMap<i64, i64>> = periods.iter().map(|pr| self.account_movements(Some(&pr.from), &pr.to, cash, p.tracking)).collect::<Result<_>>()?;
        let budget: Option<HashMap<i64, i64>> = match p.budget_id { Some(b) => Some(self.budget_amounts(b, &from, &to)?), None => None };
        let groups: [(&str, &[&str], i64); 4] = [
            ("Trading Income", &["REVENUE", "SALES"], -1), ("Cost of Sales", &["DIRECTCOSTS"], 1),
            ("Other Income", &["OTHERINCOME"], -1), ("Operating Expenses", &["EXPENSE", "OVERHEADS", "DEPRECIATN"], 1),
        ];
        let n = periods.len();
        let mut sections = Vec::new();
        let mut totals: HashMap<&str, Vec<i64>> = HashMap::new();
        for (title, kinds, sign) in groups.iter() {
            let mut rows = Vec::new();
            let mut tot = vec![0i64; n];
            let mut bt = 0;
            for a in accts.iter().filter(|a| kinds.contains(&a.kind.as_str())) {
                let vals: Vec<i64> = moves.iter().map(|m| sign * m.get(&a.id).copied().unwrap_or(0)).collect();
                let b = budget.as_ref().map(|bm| sign * bm.get(&a.id).copied().unwrap_or(0));
                if vals.iter().all(|v| *v == 0) && b.unwrap_or(0) == 0 && !p.show_zero.unwrap_or(false) { continue; }
                for (i, v) in vals.iter().enumerate() { tot[i] += v; }
                bt += b.unwrap_or(0);
                let mut row = json!({"label": a.name, "code": a.code, "account_id": a.id, "values": vals});
                if let Some(b) = b { row["budget"] = json!(b); row["variance"] = json!(vals[0] - b); }
                rows.push(row);
            }
            totals.insert(title, tot.clone());
            let mut total = json!({"label": String::from("Total ") + title, "values": tot});
            if budget.is_some() { total["budget"] = json!(bt); total["variance"] = json!(tot[0] - bt); }
            sections.push(json!({"title": title, "rows": rows, "total": total}));
        }
        let get = |k: &str| totals.get(k).cloned().unwrap_or(vec![0; n]);
        let gross: Vec<i64> = (0..n).map(|i| get("Trading Income")[i] - get("Cost of Sales")[i]).collect();
        let net: Vec<i64> = (0..n).map(|i| gross[i] + get("Other Income")[i] - get("Operating Expenses")[i]).collect();
        let mut out_sections = Vec::new();
        for (i, s) in sections.into_iter().enumerate() {
            out_sections.push(s);
            if i == 1 { out_sections.push(json!({"title": "", "summary": true, "total": {"label": "Gross Profit", "values": gross}})); }
        }
        out_sections.push(json!({"title": "", "summary": true, "grand": true, "total": {"label": "Net Profit", "values": net}}));
        Ok(json!({
            "title": "Profit and Loss", "org": self.setting_or("org_name", "")?,
            "subtitle": String::from("For the period ") + &fmt_long(&from) + " to " + &fmt_long(&to),
            "basis": if cash { "Cash" } else { "Accrual" },
            "columns": periods.iter().map(|p| p.label.clone()).collect::<Vec<_>>(),
            "income": get("Trading Income"), "has_budget": budget.is_some(),
            "sections": out_sections,
        }))
    }

    pub fn balance_sheet(&self, p: &ReportParams) -> Result<Value> {
        let date = p.date.clone().or(p.to.clone()).unwrap_or_else(util::today);
        let compare = p.compare.unwrap_or(0);
        let months = match p.period.as_deref() { Some("QUARTER") => 3, Some("MONTH") => 1, _ => 12 };
        let d = util::parse_date(&date)?;
        let mut dates = vec![date.clone()];
        for i in 1..=compare.min(12) { dates.push(util::fmt_date(util::add_months(d, -(months * i as i32)))); }
        let accts = self.accounts_meta()?;
        let n = dates.len();
        let mut bals = Vec::new();
        let mut cye = Vec::new();
        let mut prior_pl = Vec::new();
        let is_pl = |a: &Acct| a.class == "REVENUE" || a.class == "EXPENSE";
        for dt in &dates {
            bals.push(self.balances_at(dt, p.tracking)?);
            let fy = self.fy_start(dt)?;
            let pl_this = self.account_movements(Some(&fy), dt, false, p.tracking)?;
            let pl_before = self.account_movements(None, &util::add_days(&fy, -1)?, false, p.tracking)?;
            cye.push(-accts.iter().filter(|a| is_pl(a)).map(|a| pl_this.get(&a.id).copied().unwrap_or(0)).sum::<i64>());
            prior_pl.push(-accts.iter().filter(|a| is_pl(a)).map(|a| pl_before.get(&a.id).copied().unwrap_or(0)).sum::<i64>());
        }
        let groups: [(&str, &str, &[&str], i64); 7] = [
            ("Assets", "Bank", &["BANK"], 1), ("Assets", "Current Assets", &["CURRENT", "INVENTORY", "PREPAYMENT"], 1),
            ("Assets", "Fixed Assets", &["FIXED"], 1), ("Assets", "Non-current Assets", &["NONCURRENT"], 1),
            ("Liabilities", "Current Liabilities", &["CURRLIAB"], -1), ("Liabilities", "Non-current Liabilities", &["TERMLIAB", "LIABILITY"], -1),
            ("Equity", "Equity", &["EQUITY"], -1),
        ];
        let mut sections = Vec::new();
        let mut class_tot: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
        for (class, title, kinds, sign) in groups.iter() {
            let mut rows = Vec::new();
            let mut tot = vec![0i64; n];
            for a in accts.iter().filter(|a| kinds.contains(&a.kind.as_str())) {
                let mut vals: Vec<i64> = bals.iter().map(|m| sign * m.get(&a.id).copied().unwrap_or(0)).collect();
                if a.sys.as_deref() == Some("RETAINED") { for i in 0..n { vals[i] += prior_pl[i]; } }
                if vals.iter().all(|v| *v == 0) && !p.show_zero.unwrap_or(false) { continue; }
                for i in 0..n { tot[i] += vals[i]; }
                rows.push(json!({"label": a.name, "code": a.code, "account_id": a.id, "values": vals}));
            }
            if *title == "Equity" {
                rows.push(json!({"label": "Current Year Earnings", "values": cye.clone()}));
                for i in 0..n { tot[i] += cye[i]; }
            }
            let ct = class_tot.entry(class).or_insert(vec![0; n]);
            for i in 0..n { ct[i] += tot[i]; }
            if rows.is_empty() { continue; }
            sections.push(json!({"title": title, "group": class, "rows": rows, "total": {"label": String::from("Total ") + title, "values": tot}}));
        }
        let zero = vec![0i64; n];
        let assets = class_tot.get("Assets").cloned().unwrap_or(zero.clone());
        let liabs = class_tot.get("Liabilities").cloned().unwrap_or(zero.clone());
        let equity = class_tot.get("Equity").cloned().unwrap_or(zero.clone());
        let net_assets: Vec<i64> = (0..n).map(|i| assets[i] - liabs[i]).collect();
        let mut out = Vec::new();
        let mut last_group = String::new();
        let close = |out: &mut Vec<Value>, g: &str| {
            if g.is_empty() { return; }
            let t = class_tot.get(g).cloned().unwrap_or(zero.clone());
            out.push(json!({"title": "", "summary": true, "total": {"label": String::from("Total ") + g, "values": t}}));
            if g == "Liabilities" { out.push(json!({"title": "", "summary": true, "grand": true, "total": {"label": "Net Assets", "values": net_assets.clone()}})); }
        };
        for s in sections {
            let g = s["group"].as_str().unwrap_or("").to_string();
            if g != last_group { close(&mut out, &last_group); }
            last_group = g;
            out.push(s);
        }
        if let Some(last) = out.last_mut() { if last["group"] == "Equity" { last["grand"] = json!(true); } }
        let _ = equity;
        Ok(json!({
            "title": "Balance Sheet", "org": self.setting_or("org_name", "")?, "subtitle": String::from("As at ") + &fmt_long(&date),
            "columns": dates.iter().map(|d| fmt_long(d)).collect::<Vec<_>>(), "sections": out,
            "check": (0..n).map(|i| net_assets[i] - equity[i]).collect::<Vec<_>>(),
        }))
    }

    pub fn trial_balance(&self, p: &ReportParams) -> Result<Value> {
        let date = p.date.clone().or(p.to.clone()).unwrap_or_else(util::today);
        let fy = self.fy_start(&date)?;
        let accts = self.accounts_meta()?;
        let all = self.balances_at(&date, p.tracking)?;
        let ytd = self.account_movements(Some(&fy), &date, false, p.tracking)?;
        let before_fy = self.account_movements(None, &util::add_days(&fy, -1)?, false, p.tracking)?;
        let prior_pl: i64 = accts.iter().filter(|a| a.class == "REVENUE" || a.class == "EXPENSE").map(|a| before_fy.get(&a.id).copied().unwrap_or(0)).sum();
        let mut rows = Vec::new();
        let (mut td, mut tc, mut yd, mut yc) = (0i64, 0i64, 0i64, 0i64);
        for a in &accts {
            let pl = a.class == "REVENUE" || a.class == "EXPENSE";
            let mut bal = if pl { ytd.get(&a.id).copied().unwrap_or(0) } else { all.get(&a.id).copied().unwrap_or(0) };
            if a.sys.as_deref() == Some("RETAINED") { bal += prior_pl; }
            let y = ytd.get(&a.id).copied().unwrap_or(0);
            if bal == 0 && y == 0 { continue; }
            let (d, c) = if bal >= 0 { (bal, 0) } else { (0, -bal) };
            let (ydd, ycc) = if y >= 0 { (y, 0) } else { (0, -y) };
            td += d; tc += c; yd += ydd; yc += ycc;
            rows.push(json!({"label": a.name.clone() + " (" + &a.code + ")", "code": a.code, "account_id": a.id, "type": a.kind, "values": [d, c, ydd, ycc]}));
        }
        Ok(json!({
            "title": "Trial Balance", "org": self.setting_or("org_name", "")?, "subtitle": String::from("As at ") + &fmt_long(&date),
            "columns": ["Debit", "Credit", "YTD Debit", "YTD Credit"],
            "sections": [{"title": "", "rows": rows, "total": {"label": "Total", "values": [td, tc, yd, yc]}}],
            "balanced": td == tc,
        }))
    }

    /// Account transactions / general ledger detail (one account, or all when `account_id` is None).
    pub fn account_transactions(&self, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().unwrap_or_else(util::today);
        let from = p.from.clone().unwrap_or(self.fy_start(&to)?);
        let accts = self.accounts_meta()?;
        let mut sections = Vec::new();
        let opening = self.account_movements(None, &util::add_days(&from, -1)?, false, p.tracking)?;
        for a in accts.iter().filter(|a| p.account_id.map(|x| x == a.id).unwrap_or(true)) {
            let rows = util::query_json(&self.conn,
                "SELECT j.date, j.source_type, j.source_id, j.reference, j.id AS journal_id, j.number AS journal_number, jl.description, jl.amount, jl.tax_amount,
                   c.name AS contact, jl.contact_id
                 FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id LEFT JOIN contacts c ON c.id = jl.contact_id
                 WHERE jl.account_id = ?1 AND j.posted = 1 AND j.date >= ?2 AND j.date <= ?3 AND (?4 IS NULL OR jl.tracking1 = ?4 OR jl.tracking2 = ?4)
                   AND (?5 IS NULL OR jl.contact_id = ?5)
                 ORDER BY j.date, j.id, jl.id", params![a.id, from, to, p.tracking, p.contact_id])?;
            let pl = a.class == "REVENUE" || a.class == "EXPENSE";
            let open = if pl { 0 } else { opening.get(&a.id).copied().unwrap_or(0) };
            if rows.is_empty() && open == 0 { continue; }
            let mut bal = open;
            let mut out = vec![json!({"label": "Opening Balance", "date": from, "values": [Value::Null, Value::Null, bal]})];
            let (mut dr, mut cr) = (0i64, 0i64);
            for r in rows {
                let amt = r["amount"].as_i64().unwrap_or(0);
                bal += amt;
                if amt >= 0 { dr += amt } else { cr -= amt }
                out.push(json!({"label": r["description"], "date": r["date"], "source_type": r["source_type"], "source_label": source_label(r["source_type"].as_str().unwrap_or("")),
                    "source_id": r["source_id"], "journal_id": r["journal_id"], "reference": r["reference"], "contact": r["contact"],
                    "values": [if amt >= 0 { json!(amt) } else { Value::Null }, if amt < 0 { json!(-amt) } else { Value::Null }, bal]}));
            }
            sections.push(json!({"title": a.name.clone() + " (" + &a.code + ")", "account_id": a.id, "rows": out,
                "total": {"label": String::from("Total ") + &a.name, "values": [dr, cr, bal]}}));
        }
        Ok(json!({"title": if p.account_id.is_some() { "Account Transactions" } else { "General Ledger Detail" }, "org": self.setting_or("org_name", "")?,
            "subtitle": fmt_long(&from) + " to " + &fmt_long(&to), "columns": ["Debit", "Credit", "Running Balance"], "detail": true, "sections": sections}))
    }

    pub fn general_ledger_summary(&self, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().unwrap_or_else(util::today);
        let from = p.from.clone().unwrap_or(self.fy_start(&to)?);
        let accts = self.accounts_meta()?;
        let opening = self.account_movements(None, &util::add_days(&from, -1)?, false, p.tracking)?;
        let mut rows = Vec::new();
        let mut st = self.conn.prepare("SELECT jl.account_id, SUM(CASE WHEN jl.amount > 0 THEN jl.amount ELSE 0 END), SUM(CASE WHEN jl.amount < 0 THEN -jl.amount ELSE 0 END)
            FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE j.posted = 1 AND j.date >= ?1 AND j.date <= ?2 AND (?3 IS NULL OR jl.tracking1 = ?3 OR jl.tracking2 = ?3) GROUP BY jl.account_id")?;
        let mv: HashMap<i64, (i64, i64)> = st.query_map(params![from, to, p.tracking], |r| Ok((r.get::<_, i64>(0)?, (r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))))?.collect::<rusqlite::Result<_>>()?;
        let (mut td, mut tc) = (0, 0);
        for a in &accts {
            let pl = a.class == "REVENUE" || a.class == "EXPENSE";
            let open = if pl { 0 } else { opening.get(&a.id).copied().unwrap_or(0) };
            let (d, c) = mv.get(&a.id).copied().unwrap_or((0, 0));
            if open == 0 && d == 0 && c == 0 { continue; }
            td += d; tc += c;
            rows.push(json!({"label": a.name.clone() + " (" + &a.code + ")", "account_id": a.id, "values": [open, d, c, d - c, open + d - c]}));
        }
        Ok(json!({"title": "General Ledger Summary", "org": self.setting_or("org_name", "")?, "subtitle": fmt_long(&from) + " to " + &fmt_long(&to),
            "columns": ["Opening", "Debits", "Credits", "Net Movement", "Closing"], "sections": [{"title": "", "rows": rows, "total": {"label": "Total", "values": [Value::Null, td, tc, td - tc, Value::Null]}}]}))
    }

    pub fn journal_report(&self, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().unwrap_or_else(util::today);
        let from = p.from.clone().unwrap_or(self.fy_start(&to)?);
        let journals = util::query_json(&self.conn, "SELECT id, number, date, source_type, source_id, reference, narration, reverses_journal_id, created_at FROM journals
            WHERE posted = 1 AND date >= ?1 AND date <= ?2 ORDER BY date, number LIMIT 2000", params![from, to])?;
        let mut sections = Vec::new();
        for j in journals {
            let lines = util::query_json(&self.conn, "SELECT a.code, a.name, jl.description, jl.amount, jl.tax_amount, t.name AS tax_rate FROM journal_lines jl JOIN accounts a ON a.id = jl.account_id
                LEFT JOIN tax_rates t ON t.id = jl.tax_rate_id WHERE jl.journal_id = ?1 ORDER BY jl.id", [j["id"].as_i64().unwrap()])?;
            let rows: Vec<Value> = lines.iter().map(|l| {
                let a = l["amount"].as_i64().unwrap_or(0);
                json!({"label": String::from(l["code"].as_str().unwrap_or("")) + " - " + l["name"].as_str().unwrap_or(""), "description": l["description"], "tax_rate": l["tax_rate"],
                    "values": [if a >= 0 { json!(a) } else { Value::Null }, if a < 0 { json!(-a) } else { Value::Null }]})
            }).collect();
            let dr: i64 = lines.iter().map(|l| l["amount"].as_i64().unwrap_or(0).max(0)).sum();
            let title = String::from("#") + &j["number"].to_string() + " · " + j["date"].as_str().unwrap_or("") + " · " + source_label(j["source_type"].as_str().unwrap_or("")) + " " + j["reference"].as_str().unwrap_or("");
            sections.push(json!({"title": title, "journal_id": j["id"], "source_type": j["source_type"], "source_id": j["source_id"], "rows": rows, "total": {"label": "Total", "values": [dr, dr]}}));
        }
        Ok(json!({"title": "Journal Report", "org": self.setting_or("org_name", "")?, "subtitle": fmt_long(&from) + " to " + &fmt_long(&to), "columns": ["Debit", "Credit"], "sections": sections}))
    }

    /// Aged receivables (sales = true) or payables, in base currency, as at a date.
    pub fn aged(&self, sales: bool, p: &ReportParams) -> Result<Value> {
        let date = p.date.clone().or(p.to.clone()).unwrap_or_else(util::today);
        let by_invoice = p.ageing_by.as_deref() == Some("INVOICE");
        let types = if sales { ("ACCREC", "ACCRECCREDIT") } else { ("ACCPAY", "ACCPAYCREDIT") };
        let docs = util::query_json(&self.conn,
            "SELECT d.id, d.type, d.number, d.reference, d.date, d.due_date, d.total, d.base_total, d.currency, d.contact_id, c.name AS contact,
               d.total - COALESCE((SELECT SUM(amount) FROM payments WHERE document_id = d.id AND status <> 'DELETED' AND date <= ?3), 0)
                 - COALESCE((SELECT SUM(amount) FROM credit_allocations WHERE (document_id = d.id OR credit_note_id = d.id) AND status <> 'DELETED' AND date <= ?3), 0)
                 - COALESCE((SELECT SUM(amount) FROM prepayment_allocations WHERE document_id = d.id AND status <> 'DELETED' AND date <= ?3), 0) AS due_at
             FROM documents d JOIN contacts c ON c.id = d.contact_id
             WHERE d.type IN (?1, ?2) AND d.date <= ?3 AND d.status IN ('AUTHORISED','PAID') AND (?4 IS NULL OR d.contact_id = ?4)
             ORDER BY c.name, d.date", params![types.0, types.1, date, p.contact_id])?;
        let d0 = util::parse_date(&date)?;
        let mut by_contact: BTreeMap<String, (i64, Vec<i64>, Vec<Value>)> = BTreeMap::new();
        let mut grand = vec![0i64; 6];
        for d in docs {
            let due_at = d["due_at"].as_i64().unwrap_or(0);
            if due_at == 0 { continue; }
            let total = d["total"].as_i64().unwrap_or(1).max(1);
            let credit = d["type"].as_str().unwrap_or("").ends_with("CREDIT");
            let base = money::div_round(d["base_total"].as_i64().unwrap_or(0) as i128 * due_at as i128, total as i128) as i64;
            let base = if credit { -base } else { base };
            let refd = if by_invoice { d["date"].as_str().unwrap_or(&date).to_string() } else { d["due_date"].as_str().or(d["date"].as_str()).unwrap_or(&date).to_string() };
            let rd = util::parse_date(&refd)?;
            let bucket = if rd >= d0 { 0 } else { (months_between(rd, d0) + 1).min(4) as usize };
            let e = by_contact.entry(d["contact"].as_str().unwrap_or("").to_string()).or_insert((d["contact_id"].as_i64().unwrap_or(0), vec![0; 6], vec![]));
            e.1[bucket] += base; e.1[5] += base;
            grand[bucket] += base; grand[5] += base;
            let mut vals = vec![0i64; 6]; vals[bucket] = base; vals[5] = base;
            let kind = if credit { "Credit Note " } else if sales { "Invoice " } else { "Bill " };
            e.2.push(json!({"label": String::from(kind) + d["number"].as_str().unwrap_or(""), "document_id": d["id"], "date": d["date"], "due_date": d["due_date"], "reference": d["reference"], "values": vals}));
        }
        let rows: Vec<Value> = by_contact.into_iter().map(|(name, (cid, v, docs))| json!({"label": name, "contact_id": cid, "values": v, "children": docs})).collect();
        Ok(json!({"title": if sales { "Aged Receivables" } else { "Aged Payables" }, "org": self.setting_or("org_name", "")?,
            "subtitle": String::from("As at ") + &fmt_long(&date) + " · ageing by " + if by_invoice { "invoice date" } else { "due date" },
            "columns": ["Current", "< 1 Month", "1 Month", "2 Months", "Older", "Total"], "sections": [{"title": "", "rows": rows, "total": {"label": "Total", "values": grand}}]}))
    }

    /// Cash summary: cash received/spent grouped by account category.
    pub fn cash_summary(&self, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().unwrap_or_else(util::today);
        let from = p.from.clone().unwrap_or(self.fy_start(&to)?);
        let periods = self.periods(&from, &to, p.compare.unwrap_or(0), p.period.as_deref().unwrap_or("MONTH"), p.monthly.unwrap_or(false))?;
        let accts = self.accounts_meta()?;
        let n = periods.len();
        let moves: Vec<HashMap<i64, i64>> = periods.iter().map(|pr| self.account_movements(Some(&pr.from), &pr.to, true, p.tracking)).collect::<Result<_>>()?;
        let groups: [(&str, &[&str], i64, i64); 4] = [
            ("Income", &["REVENUE", "SALES", "OTHERINCOME"], -1, 1), ("Less Cost of Sales", &["DIRECTCOSTS"], 1, -1),
            ("Less Operating Expenses", &["EXPENSE", "OVERHEADS", "DEPRECIATN"], 1, -1),
            ("Plus Other Cash Movements", &["CURRENT", "FIXED", "NONCURRENT", "INVENTORY", "PREPAYMENT", "CURRLIAB", "LIABILITY", "TERMLIAB", "EQUITY"], -1, 1),
        ];
        let mut sections = Vec::new();
        let mut net = vec![0i64; n];
        for (title, kinds, sign, effect) in groups.iter() {
            let mut rows = Vec::new(); let mut tot = vec![0i64; n];
            for a in accts.iter().filter(|a| kinds.contains(&a.kind.as_str())) {
                let vals: Vec<i64> = moves.iter().map(|m| sign * m.get(&a.id).copied().unwrap_or(0)).collect();
                if vals.iter().all(|v| *v == 0) { continue; }
                for i in 0..n { tot[i] += vals[i]; }
                rows.push(json!({"label": a.name, "account_id": a.id, "values": vals}));
            }
            for i in 0..n { net[i] += effect * tot[i]; }
            let short = title.trim_start_matches("Less ").trim_start_matches("Plus ");
            sections.push(json!({"title": title, "rows": rows, "total": {"label": String::from("Total ") + short, "values": tot}}));
        }
        sections.push(json!({"title": "", "summary": true, "grand": true, "total": {"label": "Net Cash Movement", "values": net}}));
        let mut bank_rows = Vec::new();
        let closing: Vec<HashMap<i64, i64>> = periods.iter().map(|pr| self.balances_at(&pr.to, None)).collect::<Result<_>>()?;
        for a in accts.iter().filter(|a| a.kind == "BANK") {
            let vals: Vec<i64> = closing.iter().map(|m| m.get(&a.id).copied().unwrap_or(0)).collect();
            bank_rows.push(json!({"label": a.name, "account_id": a.id, "values": vals}));
        }
        sections.push(json!({"title": "Closing Bank Balances", "rows": bank_rows}));
        Ok(json!({"title": "Cash Summary", "org": self.setting_or("org_name", "")?, "subtitle": fmt_long(&from) + " to " + &fmt_long(&to),
            "columns": periods.iter().map(|p| p.label.clone()).collect::<Vec<_>>(), "sections": sections}))
    }

    pub fn bank_summary(&self, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().unwrap_or_else(util::today);
        let from = p.from.clone().unwrap_or(self.fy_start(&to)?);
        let rows = util::query_json(&self.conn,
            "SELECT a.id AS account_id, a.name AS label, a.currency,
               COALESCE((SELECT SUM(COALESCE(jl.fx_amount, jl.amount)) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE jl.account_id = a.id AND j.posted = 1 AND j.date < ?1), 0) AS opening,
               COALESCE((SELECT SUM(COALESCE(jl.fx_amount, jl.amount)) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE jl.account_id = a.id AND j.posted = 1 AND j.date >= ?1 AND j.date <= ?2 AND COALESCE(jl.fx_amount, jl.amount) > 0), 0) AS received,
               COALESCE((SELECT -SUM(COALESCE(jl.fx_amount, jl.amount)) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE jl.account_id = a.id AND j.posted = 1 AND j.date >= ?1 AND j.date <= ?2 AND COALESCE(jl.fx_amount, jl.amount) < 0), 0) AS spent
             FROM accounts a WHERE a.type = 'BANK' ORDER BY a.code", params![from, to])?;
        let rows: Vec<Value> = rows.into_iter().map(|r| {
            let o = r["opening"].as_i64().unwrap_or(0); let rc = r["received"].as_i64().unwrap_or(0); let sp = r["spent"].as_i64().unwrap_or(0);
            json!({"label": r["label"], "account_id": r["account_id"], "values": [o, rc, sp, o + rc - sp]})
        }).collect();
        Ok(json!({"title": "Bank Summary", "org": self.setting_or("org_name", "")?, "subtitle": fmt_long(&from) + " to " + &fmt_long(&to),
            "columns": ["Opening Balance", "Cash Received", "Cash Spent", "Closing Balance"], "sections": [{"title": "", "rows": rows}]}))
    }

    pub fn sales_by_contact(&self, sales: bool, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().unwrap_or_else(util::today);
        let from = p.from.clone().unwrap_or(self.fy_start(&to)?);
        let (a, b) = if sales { ("ACCREC", "ACCRECCREDIT") } else { ("ACCPAY", "ACCPAYCREDIT") };
        let rows = util::query_json(&self.conn,
            "SELECT c.id AS contact_id, c.name AS label,
               SUM(CASE WHEN d.type = ?1 THEN 1 ELSE 0 END) AS count,
               SUM(CASE WHEN d.type = ?1 THEN d.base_total * d.subtotal / MAX(d.total, 1) ELSE -d.base_total * d.subtotal / MAX(d.total, 1) END) AS net,
               SUM(CASE WHEN d.type = ?1 THEN d.base_total ELSE -d.base_total END) AS gross
             FROM documents d JOIN contacts c ON c.id = d.contact_id
             WHERE d.type IN (?1, ?2) AND d.status IN ('AUTHORISED','PAID') AND d.date >= ?3 AND d.date <= ?4
             GROUP BY c.id ORDER BY net DESC", params![a, b, from, to])?;
        let tot: (i64, i64) = rows.iter().fold((0, 0), |acc, r| (acc.0 + r["net"].as_i64().unwrap_or(0), acc.1 + r["gross"].as_i64().unwrap_or(0)));
        let rows: Vec<Value> = rows.into_iter().map(|r| json!({"label": r["label"], "contact_id": r["contact_id"], "values": [r["count"], r["net"], r["gross"]]})).collect();
        Ok(json!({"title": if sales { "Sales by Contact" } else { "Purchases by Contact" }, "org": self.setting_or("org_name", "")?, "subtitle": fmt_long(&from) + " to " + &fmt_long(&to),
            "columns": ["Transactions", "Net", "Gross"], "count_cols": [0], "sections": [{"title": "", "rows": rows, "total": {"label": "Total", "values": [Value::Null, tot.0, tot.1]}}]}))
    }

    pub fn sales_by_item(&self, sales: bool, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().unwrap_or_else(util::today);
        let from = p.from.clone().unwrap_or(self.fy_start(&to)?);
        let (a, b) = if sales { ("ACCREC", "ACCRECCREDIT") } else { ("ACCPAY", "ACCPAYCREDIT") };
        let rows = util::query_json(&self.conn,
            "SELECT i.id AS item_id, i.code || ' - ' || i.name AS label,
               SUM(CASE WHEN d.type = ?1 THEN li.qty_e4 ELSE -li.qty_e4 END) AS qty,
               SUM(CASE WHEN d.type = ?1 THEN li.net_amount ELSE -li.net_amount END * d.base_total / MAX(d.total, 1)) AS net
             FROM line_items li JOIN documents d ON d.id = li.parent_id AND li.parent_kind = 'DOC' JOIN items i ON i.id = li.item_id
             WHERE d.type IN (?1, ?2) AND d.status IN ('AUTHORISED','PAID') AND d.date >= ?3 AND d.date <= ?4
             GROUP BY i.id ORDER BY net DESC", params![a, b, from, to])?;
        let total: i64 = rows.iter().map(|r| r["net"].as_i64().unwrap_or(0)).sum();
        let rows: Vec<Value> = rows.into_iter().map(|r| {
            let q = r["qty"].as_i64().unwrap_or(0); let n = r["net"].as_i64().unwrap_or(0);
            json!({"label": r["label"], "item_id": r["item_id"], "values": [q, if q != 0 { money::div_round(n as i128 * 10_000, q as i128) as i64 } else { 0 }, n]})
        }).collect();
        Ok(json!({"title": if sales { "Sales by Item" } else { "Purchases by Item" }, "org": self.setting_or("org_name", "")?, "subtitle": fmt_long(&from) + " to " + &fmt_long(&to),
            "columns": ["Quantity", "Average Price", "Total"], "qty_cols": [0], "sections": [{"title": "", "rows": rows, "total": {"label": "Total", "values": [Value::Null, Value::Null, total]}}]}))
    }

    /// Tax transactions summarised by tax rate for a period (basis for tax returns). Sales positive.
    pub fn tax_summary(&self, from: &str, to: &str, cash: bool) -> Result<Vec<Value>> {
        let mut agg: BTreeMap<i64, (String, String, i64, i64, i64)> = BTreeMap::new();
        let mut add = |id: i64, n: String, rt: String, net: i64, tax: i64| { let e = agg.entry(id).or_insert((n, rt, 0, 0, 0)); e.2 += net; e.3 += tax; e.4 += 1; };
        let sql = if cash {
            "SELECT t.id, t.name, t.report_type, -jl.amount, -jl.tax_amount FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id JOIN tax_rates t ON t.id = jl.tax_rate_id
             WHERE j.posted = 1 AND j.date >= ?1 AND j.date <= ?2 AND j.cash_basis = 1 AND j.source_type NOT IN ('ACCREC','ACCPAY','ACCRECCREDIT','ACCPAYCREDIT')
               AND NOT (j.reverses_journal_id IS NOT NULL AND (SELECT source_type FROM journals o WHERE o.id = j.reverses_journal_id) IN ('ACCREC','ACCPAY','ACCRECCREDIT','ACCPAYCREDIT'))"
        } else {
            "SELECT t.id, t.name, t.report_type, -jl.amount, -jl.tax_amount FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id JOIN tax_rates t ON t.id = jl.tax_rate_id
             WHERE j.posted = 1 AND j.date >= ?1 AND j.date <= ?2"
        };
        let mut st = self.conn.prepare(sql)?;
        for r in st.query_map(params![from, to], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?)))? {
            let (id, n, rt, net, tax) = r?; add(id, n, rt, net, tax);
        }
        if cash {
            let mut st = self.conn.prepare(
                "SELECT document_id, amount FROM payments WHERE status <> 'DELETED' AND date >= ?1 AND date <= ?2
                 UNION ALL SELECT document_id, amount FROM credit_allocations WHERE status <> 'DELETED' AND date >= ?1 AND date <= ?2
                 UNION ALL SELECT credit_note_id, amount FROM credit_allocations WHERE status <> 'DELETED' AND date >= ?1 AND date <= ?2
                 UNION ALL SELECT document_id, amount FROM prepayment_allocations WHERE status <> 'DELETED' AND date >= ?1 AND date <= ?2")?;
            let settles = st.query_map(params![from, to], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            for (doc, amt) in settles {
                let (total, jid): (i64, Option<i64>) = self.conn.query_row("SELECT total, journal_id FROM documents WHERE id = ?1", [doc], |r| Ok((r.get(0)?, r.get(1)?)))?;
                let Some(jid) = jid else { continue };
                let mut st2 = self.conn.prepare_cached("SELECT t.id, t.name, t.report_type, -jl.amount, -jl.tax_amount FROM journal_lines jl JOIN tax_rates t ON t.id = jl.tax_rate_id WHERE jl.journal_id = ?1")?;
                for r in st2.query_map([jid], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?)))? {
                    let (id, n, rt, net, tax) = r?;
                    add(id, n, rt, money::div_round(net as i128 * amt as i128, total.max(1) as i128) as i64, money::div_round(tax as i128 * amt as i128, total.max(1) as i128) as i64);
                }
            }
        }
        Ok(agg.into_iter().map(|(id, (n, rt, net, tax, c))| json!({"tax_rate_id": id, "name": n, "report_type": rt, "net": net, "tax": tax, "lines": c})).collect())
    }

    /// Tax reconciliation report (net & tax by rate) with a GL tie-out.
    pub fn tax_report(&self, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().unwrap_or_else(util::today);
        let from = p.from.clone().unwrap_or_else(|| util::fmt_date(util::add_months(util::parse_date(&to).unwrap(), -3) + chrono::Duration::days(1)));
        let cash = p.basis.as_deref() == Some("CASH") || (p.basis.is_none() && self.setting_or("tax_basis", "ACCRUAL")? == "CASH");
        let sum = self.tax_summary(&from, &to, cash)?;
        let rows: Vec<Value> = sum.iter().map(|r| json!({"label": r["name"], "tax_rate_id": r["tax_rate_id"], "report_type": r["report_type"],
            "values": [r["net"], r["tax"], r["net"].as_i64().unwrap_or(0) + r["tax"].as_i64().unwrap_or(0)]})).collect();
        let gst = self.sys_account("GST")?;
        let gl: i64 = self.conn.query_row("SELECT COALESCE(-SUM(jl.amount), 0) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE jl.account_id = ?1 AND j.posted = 1 AND j.date >= ?2 AND j.date <= ?3 AND j.source_type <> 'TAXRETURN'",
            params![gst, from, to], |r| r.get(0))?;
        let tax_total: i64 = sum.iter().filter(|r| r["report_type"] != "REVERSECHARGE").map(|r| r["tax"].as_i64().unwrap_or(0)).sum();
        let label = self.setting_or("tax_label", "Tax")?;
        Ok(json!({"title": label.clone() + " Reconciliation", "org": self.setting_or("org_name", "")?,
            "subtitle": fmt_long(&from) + " to " + &fmt_long(&to) + " · " + if cash { "Cash" } else { "Accrual" } + " basis · positive = collected, negative = paid",
            "columns": ["Net", label.clone(), "Gross"], "sections": [{"title": "", "rows": rows, "total": {"label": String::from("Net ") + &label, "values": [Value::Null, tax_total, Value::Null]}}],
            "gl_movement": gl, "difference": if cash { 0 } else { gl - tax_total }}))
    }

    pub fn executive_summary(&self, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().unwrap_or_else(util::today);
        let d = util::parse_date(&to)?;
        let mut cols = Vec::new();
        let names = ["Cash received", "Cash spent", "Closing bank balance", "Income", "Direct costs", "Gross profit", "Other income", "Expenses", "Net profit",
            "Gross margin %", "Net margin %", "Debtors (receivables)", "Creditors (payables)", "Debtor days", "Creditor days", "Current ratio %", "Number of invoices", "Average invoice"];
        let formats = ["money", "money", "money", "money", "money", "money", "money", "money", "money", "pct", "pct", "money", "money", "num", "num", "pct", "num", "money"];
        let mut metrics: Vec<Vec<i64>> = vec![vec![]; names.len()];
        let accts = self.accounts_meta()?;
        let ar_id = self.sys_account("AR")?; let ap_id = self.sys_account("AP")?;
        for i in (0..3).rev() {
            let end_m = util::add_months(d, -i);
            let (y, m, _) = ymd(end_m);
            let start = chrono::NaiveDate::from_ymd_opt(y, m, 1).unwrap();
            let end = util::last_day_of_month(y, m);
            let (s, e) = (util::fmt_date(start), util::fmt_date(end));
            cols.push(month_label(start));
            let mv = self.account_movements(Some(&s), &e, false, None)?;
            let bal = self.balances_at(&e, None)?;
            let sum_kind = |kinds: &[&str], mm: &HashMap<i64, i64>| -> i64 { accts.iter().filter(|a| kinds.contains(&a.kind.as_str())).map(|a| mm.get(&a.id).copied().unwrap_or(0)).sum() };
            let income = -sum_kind(&["REVENUE", "SALES"], &mv);
            let cogs = sum_kind(&["DIRECTCOSTS"], &mv);
            let other = -sum_kind(&["OTHERINCOME"], &mv);
            let exp = sum_kind(&["EXPENSE", "OVERHEADS", "DEPRECIATN"], &mv);
            let gp = income - cogs; let np = gp + other - exp;
            let (recv, spent): (i64, i64) = self.conn.query_row(
                "SELECT COALESCE(SUM(CASE WHEN jl.amount > 0 THEN jl.amount END), 0), COALESCE(-SUM(CASE WHEN jl.amount < 0 THEN jl.amount END), 0)
                 FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id JOIN accounts a ON a.id = jl.account_id
                 WHERE a.type = 'BANK' AND j.posted = 1 AND j.date >= ?1 AND j.date <= ?2 AND j.source_type <> 'TRANSFER'", params![s, e], |r| Ok((r.get(0)?, r.get(1)?)))?;
            let bank = sum_kind(&["BANK"], &bal);
            let ar = bal.get(&ar_id).copied().unwrap_or(0);
            let ap = -bal.get(&ap_id).copied().unwrap_or(0);
            let cur_assets = sum_kind(&["BANK", "CURRENT", "INVENTORY", "PREPAYMENT"], &bal);
            let cur_liab = -sum_kind(&["CURRLIAB"], &bal);
            let (ninv, invtot): (i64, i64) = self.conn.query_row("SELECT COUNT(*), COALESCE(SUM(base_total), 0) FROM documents WHERE type = 'ACCREC' AND status IN ('AUTHORISED','PAID') AND date >= ?1 AND date <= ?2", params![s, e], |r| Ok((r.get(0)?, r.get(1)?)))?;
            let days = (end - start).num_days() + 1;
            let purchases: i64 = cogs + exp;
            let vals = [recv, spent, bank, income, cogs, gp, other, exp, np, pct(gp, income), pct(np, income), ar, ap,
                if income > 0 { ar * days / income } else { 0 }, if purchases > 0 { ap * days / purchases } else { 0 },
                pct(cur_assets, cur_liab), ninv, if ninv > 0 { invtot / ninv } else { 0 }];
            for (k, v) in vals.iter().enumerate() { metrics[k].push(*v); }
        }
        let rows: Vec<Value> = names.iter().zip(formats.iter()).zip(metrics).map(|((l, f), v)| json!({"label": l, "values": v, "format": f})).collect();
        Ok(json!({"title": "Executive Summary", "org": self.setting_or("org_name", "")?, "subtitle": String::from("For the 3 months ending ") + &fmt_long(&to), "columns": cols,
            "sections": [{"title": "", "rows": rows}]}))
    }

    pub fn budget_amounts(&self, budget_id: i64, from: &str, to: &str) -> Result<HashMap<i64, i64>> {
        let mut st = self.conn.prepare("SELECT account_id, SUM(amount) FROM budget_lines WHERE budget_id = ?1 AND period >= substr(?2, 1, 7) AND period <= substr(?3, 1, 7) GROUP BY account_id")?;
        let m = st.query_map(params![budget_id, from, to], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?.collect::<rusqlite::Result<HashMap<_, _>>>()?;
        Ok(m)
    }
}

fn pct(a: i64, b: i64) -> i64 { if b == 0 { 0 } else { money::div_round(a as i128 * 10_000, b as i128) as i64 } }

fn months_between(a: chrono::NaiveDate, b: chrono::NaiveDate) -> i64 {
    let (ay, am, ad) = ymd(a); let (by, bm, bd) = ymd(b);
    let mut m = (by - ay) as i64 * 12 + bm as i64 - am as i64;
    if bd < ad { m -= 1; }
    m.max(0)
}

fn is_month_end(d: chrono::NaiveDate) -> bool { let (y, m, _) = ymd(d); util::last_day_of_month(y, m) == d }

fn short(d: chrono::NaiveDate) -> String { let (y, m, dd) = ymd(d); dd.to_string() + " " + MONTHS[m as usize - 1] + " " + &(y % 100).to_string() }

fn label_for(from: &str, to: &str) -> String {
    let (f, t) = (util::parse_date(from).unwrap(), util::parse_date(to).unwrap());
    let (fy, fm, fd) = ymd(f); let (ty, tm, _) = ymd(t);
    if fd == 1 && is_month_end(t) && fy == ty && fm == tm { return month_label(f); }
    short(f) + " - " + &short(t)
}

pub fn fmt_long(d: &str) -> String {
    match util::parse_date(d) { Ok(x) => { let (y, m, dd) = ymd(x); dd.to_string() + " " + MONTHS_LONG[m as usize - 1] + " " + &y.to_string() } Err(_) => d.to_string() }
}

pub fn source_label(s: &str) -> &'static str {
    match s {
        "ACCREC" => "Invoice", "ACCPAY" => "Bill", "ACCRECCREDIT" => "Credit Note", "ACCPAYCREDIT" => "Supplier Credit",
        "PAYMENT" => "Payment", "CASHREC" => "Receive Money", "CASHPAID" => "Spend Money", "TRANSFER" => "Bank Transfer", "MANJOURNAL" => "Manual Journal",
        "DEPRECIATION" => "Depreciation", "PAYRUN" => "Pay Run", "INVENTORY" => "Inventory Adjustment", "CONVERSION" => "Conversion Balances",
        "FXREVAL" => "Currency Revaluation", "EXPCLAIM" => "Expense Claim", "CREDITALLOC" => "Credit Allocation", "PREPAYALLOC" => "Credit Allocation",
        "ASSETDISPOSAL" => "Asset Disposal", "TAXRETURN" => "Tax Return", _ => "Journal",
    }
}
