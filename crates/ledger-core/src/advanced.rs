//! Tracking categories, currencies & exchange rates, manual journals, repeating templates,
//! conversion balances, budgets, find & recode, FX revaluation, year-end.

use crate::{documents::{DocInput, LineInput}, error::invalid, journal::{JLine, NewJournal}, money, tax::Lat, util, Error, Ledger, Result};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ManualJournalInput {
    pub date: String,
    pub narration: String,
    pub line_amount_type: Option<String>,
    pub auto_reverse_date: Option<String>,
    pub show_on_cash_basis: Option<bool>,
    pub lines: Vec<MjLine>,
    pub status: Option<String>, // DRAFT | POSTED
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct MjLine {
    pub description: String,
    pub account_id: Option<i64>,
    pub tax_rate_id: Option<i64>,
    pub debit: Option<String>,
    pub credit: Option<String>,
    pub tracking1: Option<i64>,
    pub tracking2: Option<i64>,
}

impl Ledger {
    // ---------------- Tracking ----------------
    pub fn list_tracking(&self) -> Result<Vec<Value>> {
        let mut cats = util::query_json(&self.conn, "SELECT * FROM tracking_categories ORDER BY id", [])?;
        for c in cats.iter_mut() {
            c["options"] = json!(util::query_json(&self.conn, "SELECT * FROM tracking_options WHERE category_id = ?1 ORDER BY name", [c["id"].as_i64().unwrap()])?);
        }
        Ok(cats)
    }
    pub fn save_tracking_category(&self, id: Option<i64>, name: &str, options: &[String]) -> Result<i64> {
        if name.trim().is_empty() { return invalid("category name is required"); }
        self.atomic(|| {
            let cid = match id {
                Some(id) => { self.conn.execute("UPDATE tracking_categories SET name = ?2 WHERE id = ?1", params![id, name.trim()])?; id }
                None => {
                    let active: i64 = self.conn.query_row("SELECT COUNT(*) FROM tracking_categories WHERE status = 'ACTIVE'", [], |r| r.get(0))?;
                    if active >= 2 { return invalid("you can have at most two active tracking categories"); }
                    self.conn.execute("INSERT INTO tracking_categories(name) VALUES (?1)", [name.trim()])?; self.conn.last_insert_rowid()
                }
            };
            for o in options.iter().map(|o| o.trim()).filter(|o| !o.is_empty()) {
                let exists: i64 = self.conn.query_row("SELECT COUNT(*) FROM tracking_options WHERE category_id = ?1 AND name = ?2", params![cid, o], |r| r.get(0))?;
                if exists == 0 { self.conn.execute("INSERT INTO tracking_options(category_id, name) VALUES (?1, ?2)", params![cid, o])?; }
            }
            Ok(cid)
        })
    }
    pub fn set_tracking_option_status(&self, id: i64, status: &str) -> Result<()> {
        self.conn.execute("UPDATE tracking_options SET status = ?2 WHERE id = ?1", params![id, status])?; Ok(())
    }
    pub fn rename_tracking_option(&self, id: i64, name: &str) -> Result<()> {
        self.conn.execute("UPDATE tracking_options SET name = ?2 WHERE id = ?1", params![id, name])?; Ok(())
    }
    pub fn set_tracking_category_status(&self, id: i64, status: &str) -> Result<()> {
        if status == "ACTIVE" {
            let active: i64 = self.conn.query_row("SELECT COUNT(*) FROM tracking_categories WHERE status = 'ACTIVE' AND id <> ?1", [id], |r| r.get(0))?;
            if active >= 2 { return invalid("you can have at most two active tracking categories"); }
        }
        self.conn.execute("UPDATE tracking_categories SET status = ?2 WHERE id = ?1", params![id, status])?; Ok(())
    }

    // ---------------- Currencies ----------------
    pub fn list_currencies(&self) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT c.*, (SELECT rate_e8 FROM exchange_rates r WHERE r.currency = c.code ORDER BY date DESC LIMIT 1) AS latest_rate_e8,
            (SELECT date FROM exchange_rates r WHERE r.currency = c.code ORDER BY date DESC LIMIT 1) AS latest_rate_date FROM currencies c ORDER BY c.enabled DESC, c.code", [])
    }
    pub fn enable_currency(&self, code: &str, enabled: bool) -> Result<()> {
        if code == self.base_currency()? && !enabled { return invalid("the base currency can't be removed"); }
        self.conn.execute("UPDATE currencies SET enabled = ?2 WHERE code = ?1", params![code, enabled as i64])?; Ok(())
    }
    pub fn set_rate(&self, code: &str, date: &str, rate: &str, source: &str) -> Result<()> {
        util::parse_date(date)?;
        let r = money::parse_rate_e8(rate).filter(|r| *r > 0).ok_or_else(|| Error::Validation("invalid rate".into()))?;
        self.conn.execute("INSERT INTO exchange_rates(currency, date, rate_e8, source) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(currency, date) DO UPDATE SET rate_e8 = excluded.rate_e8, source = excluded.source",
            params![code, date, r, source])?;
        Ok(())
    }
    pub fn list_rates(&self, code: &str) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT * FROM exchange_rates WHERE currency = ?1 ORDER BY date DESC LIMIT 200", [code])
    }

    /// Unrealised FX revaluation of open foreign-currency documents and foreign bank accounts
    /// at `date`, auto-reversed on the following day.
    pub fn fx_revalue(&self, date: &str) -> Result<Value> {
        let base = self.base_currency()?;
        let next = util::add_days(date, 1)?;
        let mut lines = Vec::new();
        let mut detail = Vec::new();
        let unreal = self.sys_account("UNREALISEDFX")?;
        let docs = util::query_json(&self.conn,
            "SELECT id, type, number, currency, total, base_total, total - amount_paid - amount_credited AS due, contact_id FROM documents
             WHERE status = 'AUTHORISED' AND currency <> ?1 AND date <= ?2 AND type IN ('ACCREC','ACCPAY','ACCRECCREDIT','ACCPAYCREDIT')", params![base, date])?;
        for d in docs {
            let cur = d["currency"].as_str().unwrap();
            let rate = match self.rate_for(cur, date) { Ok(r) => r, Err(_) => continue };
            let due = d["due"].as_i64().unwrap_or(0);
            let total = d["total"].as_i64().unwrap_or(1).max(1);
            let carried = money::div_round(d["base_total"].as_i64().unwrap_or(0) as i128 * due as i128, total as i128) as i64 ;
            let carried = carried - 0; // remaining base at document rate
            let revalued = self.to_base_amount(due, cur, rate)?;
            let diff = revalued - carried;
            if diff == 0 { continue; }
            let kind = d["type"].as_str().unwrap();
            let sign = match kind { "ACCREC" | "ACCPAYCREDIT" => 1, _ => -1 };
            let control = if crate::documents::is_sales(kind) { self.sys_account("AR")? } else { self.sys_account("AP")? };
            lines.push(JLine::new(control, sign * diff).contact(d["contact_id"].as_i64()).desc(format!("Revaluation {}", d["number"].as_str().unwrap_or(""))));
            lines.push(JLine::new(unreal, -sign * diff).desc(format!("Revaluation {}", d["number"].as_str().unwrap_or(""))));
            detail.push(json!({"document_id": d["id"], "number": d["number"], "currency": cur, "carried": carried, "revalued": revalued, "difference": sign * diff}));
        }
        // Foreign currency bank accounts
        let banks = util::query_json(&self.conn, "SELECT id, name, currency FROM accounts WHERE type = 'BANK' AND currency IS NOT NULL AND currency <> ?1", [&base])?;
        let reval = self.sys_account("BANKREVAL")?;
        for b in banks {
            let cur = b["currency"].as_str().unwrap();
            let rate = match self.rate_for(cur, date) { Ok(r) => r, Err(_) => continue };
            let (fx, carried): (i64, i64) = self.conn.query_row("SELECT COALESCE(SUM(COALESCE(jl.fx_amount, 0)),0), COALESCE(SUM(jl.amount),0) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE jl.account_id = ?1 AND j.posted = 1 AND j.date <= ?2",
                params![b["id"].as_i64().unwrap(), date], |r| Ok((r.get(0)?, r.get(1)?)))?;
            let revalued = self.to_base_amount(fx, cur, rate)?;
            let diff = revalued - carried;
            if diff == 0 { continue; }
            lines.push(JLine::new(b["id"].as_i64().unwrap(), diff).desc(format!("Bank revaluation {cur}")));
            lines.push(JLine::new(reval, -diff).desc(format!("Bank revaluation {cur}")));
            detail.push(json!({"account": b["name"], "currency": cur, "carried": carried, "revalued": revalued, "difference": diff}));
        }
        if lines.is_empty() { return Ok(json!({"posted": false, "detail": detail})); }
        self.atomic(|| {
            let jid = self.post_journal(NewJournal { date: date.into(), source_type: "FXREVAL".into(), reference: "Currency revaluation".into(), narration: format!("Unrealised currency gains as at {date}"), lines: lines.clone(), cash_basis: false, ..Default::default() })?;
            let rid = self.reverse_journal(jid, Some(&next), Some("FXREVAL"))?;
            self.conn.execute("INSERT INTO fx_revaluations(date, journal_id, reversal_journal_id, detail) VALUES (?1, ?2, ?3, ?4)", params![date, jid, rid, json!(detail).to_string()])?;
            Ok(json!({"posted": true, "journal_id": jid, "reversal_journal_id": rid, "detail": detail}))
        })
    }

    // ---------------- Manual journals ----------------
    pub fn save_manual_journal(&self, id: Option<i64>, m: &ManualJournalInput) -> Result<i64> {
        util::parse_date(&m.date)?;
        if m.narration.trim().is_empty() { return invalid("a narration is required"); }
        let lat = Lat::parse(m.line_amount_type.as_deref().unwrap_or("NOTAX"))?;
        let mut computed = Vec::new();
        for (i, l) in m.lines.iter().enumerate() {
            let d = l.debit.as_deref().filter(|s| !s.trim().is_empty()).map(|s| money::parse_fixed(s, 2).ok_or_else(|| Error::Validation(format!("line {}: invalid debit", i + 1)))).transpose()?.unwrap_or(0);
            let c = l.credit.as_deref().filter(|s| !s.trim().is_empty()).map(|s| money::parse_fixed(s, 2).ok_or_else(|| Error::Validation(format!("line {}: invalid credit", i + 1)))).transpose()?.unwrap_or(0);
            if d == 0 && c == 0 && l.account_id.is_none() { continue; }
            let acct = l.account_id.ok_or_else(|| Error::Validation(format!("line {} needs an account", i + 1)))?;
            let signed = d - c;
            let (rate, comps) = match (lat, l.tax_rate_id) { (Lat::NoTax, _) | (_, None) => (0, vec![]), (_, Some(t)) => { let (r, cs, _) = self.tax_rate(t)?; (r, cs) } };
            let (_, net_abs, tax_abs) = crate::tax::split_amount(lat, signed.abs(), rate, &comps);
            let sgn = signed.signum();
            computed.push(crate::documents::CLine { description: l.description.clone(), account_id: Some(acct), tax_rate_id: l.tax_rate_id, line_amount: signed,
                net: sgn * net_abs, tax: sgn * tax_abs, tracking1: l.tracking1, tracking2: l.tracking2, qty_e4: 10_000, ..Default::default() });
        }
        if computed.len() < 2 { return invalid("a manual journal needs at least two lines"); }
        let total: i64 = computed.iter().map(|c| c.net + c.tax).sum();
        if total != 0 { return invalid(format!("debits and credits must balance (difference {})", money::fmt_money(total))); }
        let lat_s = match lat { Lat::Exclusive => "EXCLUSIVE", Lat::Inclusive => "INCLUSIVE", Lat::NoTax => "NOTAX" };
        self.atomic(|| {
            let mid = match id {
                None => { self.conn.execute("INSERT INTO manual_journals(date) VALUES (?1)", [&m.date])?; self.conn.last_insert_rowid() }
                Some(id) => {
                    let (st, jid, rjid): (String, Option<i64>, Option<i64>) = self.conn.query_row("SELECT status, journal_id, reversal_journal_id FROM manual_journals WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
                    if st == "VOIDED" { return invalid("voided journals can't be edited"); }
                    if st == "POSTED" {
                        if let Some(j) = jid { self.reverse_journal(j, None, None)?; }
                        if let Some(j) = rjid { self.reverse_journal(j, None, None)?; }
                        self.conn.execute("UPDATE manual_journals SET journal_id = NULL, reversal_journal_id = NULL, status = 'DRAFT' WHERE id = ?1", [id])?;
                    }
                    id
                }
            };
            self.conn.execute("UPDATE manual_journals SET date=?2, narration=?3, line_amount_type=?4, auto_reverse_date=?5, show_on_cash_basis=?6 WHERE id=?1",
                params![mid, m.date, m.narration, lat_s, m.auto_reverse_date.clone().filter(|s| !s.is_empty()), m.show_on_cash_basis.unwrap_or(true) as i64])?;
            self.write_lines("MJ", mid, &computed)?;
            if m.status.as_deref() == Some("POSTED") { self.post_manual_journal(mid)?; }
            self.audit(if id.is_some() { "UPDATE" } else { "CREATE" }, "manual_journal", Some(mid), None, Some(json!({"narration": m.narration})))?;
            let posted = m.status.as_deref() == Some("POSTED");
            let what = match (id.is_some(), posted) { (false, true) => "Journal created and posted", (false, false) => "Draft journal created", (true, true) => "Journal edited and reposted", (true, false) => "Journal edited (saved as draft)" };
            self.add_note("manual_journal", mid, "HISTORY", &format!("{} — total {}", what, money::fmt_money(computed.iter().filter(|c| c.net + c.tax > 0).map(|c| c.net + c.tax).sum())))?;
            self.index_search("manual_journal", mid, &format!("Manual journal {}", m.narration), "")?;
            Ok(mid)
        })
    }

    pub fn post_manual_journal(&self, id: i64) -> Result<()> {
        let (date, narration, rev, cash, st): (String, String, Option<String>, i64, String) = self.conn.query_row(
            "SELECT date, narration, auto_reverse_date, show_on_cash_basis, status FROM manual_journals WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
        if st != "DRAFT" { return invalid("only draft journals can be posted"); }
        let gst = self.sys_account("GST")?;
        let mut jl = Vec::new();
        for l in self.read_lines("MJ", id)? {
            jl.push(JLine { account_id: l.account_id.unwrap(), amount: l.net, description: l.description.clone(), tax_rate_id: l.tax_rate_id, tax_amount: l.tax, tracking1: l.tracking1, tracking2: l.tracking2, ..Default::default() });
            if l.tax != 0 { jl.push(JLine::new(gst, l.tax).desc(l.description)); }
        }
        self.atomic(|| {
            let jid = self.post_journal(NewJournal { date: date.clone(), source_type: "MANJOURNAL".into(), source_id: Some(id), reference: narration.clone(), narration: narration.clone(), lines: jl, cash_basis: cash != 0, reverses: None })?;
            let rid = match rev.filter(|s| !s.is_empty()) { Some(d) => Some(self.reverse_journal(jid, Some(&d), Some("MANJOURNAL"))?), None => None };
            self.conn.execute("UPDATE manual_journals SET status = 'POSTED', journal_id = ?2, reversal_journal_id = ?3 WHERE id = ?1", params![id, jid, rid])?;
            Ok(())
        })
    }

    pub fn void_manual_journal(&self, id: i64) -> Result<()> {
        let (st, jid, rjid): (String, Option<i64>, Option<i64>) = self.conn.query_row("SELECT status, journal_id, reversal_journal_id FROM manual_journals WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        self.atomic(|| {
            if st == "POSTED" {
                if let Some(j) = jid { self.reverse_journal(j, None, None)?; }
                if let Some(j) = rjid { self.reverse_journal(j, None, None)?; }
            }
            self.conn.execute("UPDATE manual_journals SET status = ?2 WHERE id = ?1", params![id, if st == "DRAFT" { "DELETED" } else { "VOIDED" }])?;
            self.add_note("manual_journal", id, "HISTORY", if st == "DRAFT" { "Draft journal deleted" } else { "Journal voided (reversed in the ledger)" })?;
            self.audit(if st == "DRAFT" { "DELETE" } else { "VOID" }, "manual_journal", Some(id), None, None)?;
            Ok(())
        })
    }

    pub fn list_manual_journals(&self, status: &str) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT m.*, (SELECT SUM(CASE WHEN net_amount + tax_amount > 0 THEN net_amount + tax_amount ELSE 0 END) FROM line_items WHERE parent_kind = 'MJ' AND parent_id = m.id) AS debit_total
            FROM manual_journals m WHERE (?1 = 'ALL' AND m.status <> 'DELETED') OR m.status = ?1 ORDER BY m.date DESC, m.id DESC", [status])
    }

    pub fn get_manual_journal(&self, id: i64) -> Result<Value> {
        let mut m = util::query_one_json(&self.conn, "SELECT * FROM manual_journals WHERE id = ?1", [id])?;
        m["lines"] = json!(util::query_json(&self.conn, "SELECT li.*, a.code AS account_code, a.name AS account_name, t.name AS tax_rate_name FROM line_items li
            LEFT JOIN accounts a ON a.id = li.account_id LEFT JOIN tax_rates t ON t.id = li.tax_rate_id WHERE parent_kind = 'MJ' AND parent_id = ?1 ORDER BY sort", [id])?);
        m["history"] = json!(self.history("manual_journal", id)?);
        Ok(m)
    }

    // ---------------- Repeating templates ----------------
    pub fn save_repeating(&self, id: Option<i64>, p: &Value) -> Result<i64> {
        let kind = p["kind"].as_str().unwrap_or("ACCREC");
        if !["ACCREC", "ACCPAY", "MJ"].contains(&kind) { return invalid("unsupported repeating type"); }
        let start = p["start_date"].as_str().unwrap_or("");
        util::parse_date(start)?;
        let template = p["template"].clone();
        let unit = if p["period_unit"] == "WEEKLY" { "WEEKLY" } else { "MONTHLY" };
        let count = p["period_count"].as_i64().unwrap_or(1).max(1);
        let save_as = p["save_as"].as_str().unwrap_or("DRAFT");
        let next = p["next_date"].as_str().filter(|s| !s.is_empty()).unwrap_or(start);
        match id {
            None => {
                self.conn.execute("INSERT INTO repeating_templates(kind, template, period_unit, period_count, start_date, next_date, end_date, due_terms_type, due_terms_days, save_as)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![kind, template.to_string(), unit, count, start, next, p["end_date"].as_str().filter(|s| !s.is_empty()),
                        p["due_terms_type"].as_str().unwrap_or("DAYSAFTERBILLDATE"), p["due_terms_days"].as_i64().unwrap_or(14), save_as])?;
                Ok(self.conn.last_insert_rowid())
            }
            Some(id) => {
                self.conn.execute("UPDATE repeating_templates SET kind=?2, template=?3, period_unit=?4, period_count=?5, start_date=?6, next_date=?7, end_date=?8, due_terms_type=?9, due_terms_days=?10, save_as=?11 WHERE id=?1",
                    params![id, kind, template.to_string(), unit, count, start, next, p["end_date"].as_str().filter(|s| !s.is_empty()),
                        p["due_terms_type"].as_str().unwrap_or("DAYSAFTERBILLDATE"), p["due_terms_days"].as_i64().unwrap_or(14), save_as])?;
                Ok(id)
            }
        }
    }

    pub fn list_repeating(&self, kind: &str) -> Result<Vec<Value>> {
        let mut v = util::query_json(&self.conn, "SELECT * FROM repeating_templates WHERE (?1 = 'ALL' OR kind = ?1) AND status <> 'DELETED' ORDER BY next_date", [kind])?;
        for r in v.iter_mut() {
            let t: Value = serde_json::from_str(r["template"].as_str().unwrap_or("{}")).unwrap_or(json!({}));
            let cname = t["contact_id"].as_i64().and_then(|c| self.conn.query_row("SELECT name FROM contacts WHERE id = ?1", [c], |r| r.get::<_, String>(0)).ok())
                .or_else(|| t["contact_name"].as_str().map(String::from));
            r["contact_name"] = json!(cname);
            r["reference"] = t["reference"].clone();
            r["estimated_total"] = json!(crate::ext::estimate_template_total(self, &t).unwrap_or(0));
            r["template"] = t;
        }
        Ok(v)
    }

    pub fn set_repeating_status(&self, id: i64, status: &str) -> Result<()> {
        self.conn.execute("UPDATE repeating_templates SET status = ?2 WHERE id = ?1", params![id, status])?; Ok(())
    }

    /// Generate all repeating transactions due on or before `as_of`. Returns created ids.
    pub fn run_repeating(&self, as_of: &str) -> Result<Vec<Value>> {
        let due: Vec<(i64, String, String, String, i64, String, Option<String>, String, i64, String)> = {
            let mut st = self.conn.prepare("SELECT id, kind, template, period_unit, period_count, next_date, end_date, due_terms_type, due_terms_days, save_as FROM repeating_templates WHERE status = 'ACTIVE' AND next_date <= ?1")?;
            st.query_map([as_of], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut created = Vec::new();
        for (id, kind, tmpl, unit, count, mut next, end, tt, td, save_as) in due {
            let mut guard = 0;
            while next.as_str() <= as_of && guard < 120 {
                if end.as_deref().map(|e| next.as_str() > e).unwrap_or(false) { self.set_repeating_status(id, "ENDED")?; break; }
                let mut t: Value = serde_json::from_str(&tmpl)?;
                let date = next.clone();
                substitute_placeholders(&mut t, &date);
                let r = self.atomic(|| {
                    if kind == "MJ" {
                        let mut m: ManualJournalInput = serde_json::from_value(t.clone())?;
                        m.date = date.clone(); m.status = Some(if save_as == "DRAFT" { "DRAFT".into() } else { "POSTED".into() });
                        let mid = self.save_manual_journal(None, &m)?;
                        self.conn.execute("UPDATE manual_journals SET repeating_id = ?2 WHERE id = ?1", params![mid, id])?;
                        Ok(json!({"kind": "MJ", "id": mid}))
                    } else {
                        let mut d: DocInput = serde_json::from_value(t.clone())?;
                        d.kind = kind.clone(); d.date = date.clone(); d.due_date = Some(util::due_date(&date, &tt, td)?); d.repeating_id = Some(id);
                        d.status = Some(if save_as == "DRAFT" { "DRAFT".into() } else { "AUTHORISED".into() });
                        let did = self.save_document(None, &d)?;
                        if save_as == "AUTHORISED_SEND" && kind == "ACCREC" { let _ = crate::email::email_document(self, did, None, None, None); }
                        Ok(json!({"kind": kind, "id": did}))
                    }
                });
                match r { Ok(v) => created.push(v), Err(e) => { created.push(json!({"template_id": id, "error": e.to_string()})); break; } }
                next = crate::ext::advance_date(&next, &unit, count)?;
                self.conn.execute("UPDATE repeating_templates SET next_date = ?2, last_run = ?3 WHERE id = ?1", params![id, next, date])?;
                guard += 1;
            }
        }
        Ok(created)
    }

    // ---------------- Conversion balances ----------------
    /// Opening balances as at the conversion date. `lines`: [{account_id, debit, credit}].
    /// AR/AP balances should be entered as outstanding invoices/bills dated before the conversion date.
    pub fn save_conversion_balances(&self, date: &str, lines: &[Value]) -> Result<i64> {
        util::parse_date(date)?;
        let hist = self.sys_account("HISTORICAL")?;
        let mut jl = Vec::new();
        for l in lines {
            let a = l["account_id"].as_i64().ok_or_else(|| Error::Validation("account required".into()))?;
            let d = l["debit"].as_str().and_then(|s| money::parse_fixed(s, 2)).unwrap_or(0);
            let c = l["credit"].as_str().and_then(|s| money::parse_fixed(s, 2)).unwrap_or(0);
            if d - c != 0 { jl.push(JLine::new(a, d - c).desc("Conversion balance")); }
        }
        let diff: i64 = jl.iter().map(|l| l.amount).sum();
        if diff != 0 { jl.push(JLine::new(hist, -diff).desc("Conversion balance adjustment")); }
        self.atomic(|| {
            // Replace previous conversion journal
            let prev: Option<i64> = self.conn.query_row("SELECT id FROM journals WHERE source_type = 'CONVERSION' AND reversed = 0 AND reverses_journal_id IS NULL ORDER BY id DESC LIMIT 1", [], |r| r.get(0)).optional()?;
            if let Some(p) = prev { self.reverse_journal(p, None, None)?; }
            let jid = if jl.is_empty() { 0 } else {
                self.post_journal(NewJournal { date: date.into(), source_type: "CONVERSION".into(), reference: "Conversion balances".into(), narration: "Opening balances".into(), lines: jl, cash_basis: false, ..Default::default() })?
            };
            self.set_setting("conversion_date", date)?;
            Ok(jid)
        })
    }

    pub fn conversion_balances(&self) -> Result<Value> {
        let jid: Option<i64> = self.conn.query_row("SELECT id FROM journals WHERE source_type = 'CONVERSION' AND reversed = 0 AND reverses_journal_id IS NULL ORDER BY id DESC LIMIT 1", [], |r| r.get(0)).optional()?;
        let lines = match jid { Some(j) => util::query_json(&self.conn, "SELECT jl.account_id, a.code, a.name, jl.amount FROM journal_lines jl JOIN accounts a ON a.id = jl.account_id WHERE jl.journal_id = ?1", [j])?, None => vec![] };
        Ok(json!({"date": self.setting("conversion_date")?, "lines": lines}))
    }

    // ---------------- Budgets ----------------
    pub fn list_budgets(&self) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT b.*, o.name AS tracking_name FROM budgets b LEFT JOIN tracking_options o ON o.id = b.tracking_option_id ORDER BY b.id", [])
    }
    pub fn save_budget(&self, id: Option<i64>, name: &str, tracking: Option<i64>, lines: &[Value]) -> Result<i64> {
        if name.trim().is_empty() { return invalid("budget name required"); }
        self.atomic(|| {
            let bid = match id {
                Some(id) => { self.conn.execute("UPDATE budgets SET name = ?2, tracking_option_id = ?3, type = ?4 WHERE id = ?1", params![id, name, tracking, if tracking.is_some() { "TRACKING" } else { "OVERALL" }])?; id }
                None => { self.conn.execute("INSERT INTO budgets(name, tracking_option_id, type) VALUES (?1, ?2, ?3)", params![name, tracking, if tracking.is_some() { "TRACKING" } else { "OVERALL" }])?; self.conn.last_insert_rowid() }
            };
            for l in lines {
                let a = l["account_id"].as_i64().unwrap_or(0);
                let period = l["period"].as_str().unwrap_or("");
                let amt = l["amount"].as_str().and_then(|s| money::parse_fixed(s, 2)).or(l["amount"].as_i64()).unwrap_or(0);
                let class = self.account_class(a)?;
                let signed = if class == "REVENUE" || class == "LIABILITY" || class == "EQUITY" { -amt } else { amt };
                self.conn.execute("INSERT INTO budget_lines(budget_id, account_id, period, amount) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(budget_id, account_id, period) DO UPDATE SET amount = excluded.amount",
                    params![bid, a, period, signed])?;
            }
            Ok(bid)
        })
    }
    pub fn get_budget(&self, id: i64) -> Result<Value> {
        let mut b = util::query_one_json(&self.conn, "SELECT * FROM budgets WHERE id = ?1", [id])?;
        b["lines"] = json!(util::query_json(&self.conn, "SELECT bl.account_id, bl.period, CASE WHEN a.class IN ('REVENUE','LIABILITY','EQUITY') THEN -bl.amount ELSE bl.amount END AS amount FROM budget_lines bl JOIN accounts a ON a.id = bl.account_id WHERE bl.budget_id = ?1", [id])?);
        Ok(b)
    }

    // ---------------- Find & recode ----------------
    /// Find posted document/bank lines matching filters.
    pub fn find_lines(&self, p: &Value) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT li.id AS line_id, li.parent_kind, li.parent_id, li.description, li.net_amount, li.tax_amount, li.account_id, a.code AS account_code, a.name AS account_name,
               li.tax_rate_id, t.name AS tax_rate_name, li.tracking1, li.tracking2,
               COALESCE(d.date, b.date) AS date, COALESCE(d.number, b.reference) AS number, COALESCE(c1.name, c2.name) AS contact, COALESCE(d.type, b.type) AS type
             FROM line_items li
             LEFT JOIN documents d ON li.parent_kind = 'DOC' AND d.id = li.parent_id
             LEFT JOIN bank_transactions b ON li.parent_kind = 'BANKTX' AND b.id = li.parent_id
             LEFT JOIN contacts c1 ON c1.id = d.contact_id LEFT JOIN contacts c2 ON c2.id = b.contact_id
             LEFT JOIN accounts a ON a.id = li.account_id LEFT JOIN tax_rates t ON t.id = li.tax_rate_id
             WHERE li.parent_kind IN ('DOC','BANKTX')
               AND ((li.parent_kind = 'DOC' AND d.status IN ('AUTHORISED','PAID') AND d.type IN ('ACCREC','ACCPAY','ACCRECCREDIT','ACCPAYCREDIT')) OR (li.parent_kind = 'BANKTX' AND b.status = 'AUTHORISED'))
               AND (?1 IS NULL OR li.account_id = ?1) AND (?2 IS NULL OR li.tax_rate_id = ?2) AND (?3 IS NULL OR COALESCE(d.contact_id, b.contact_id) = ?3)
               AND (?4 IS NULL OR COALESCE(d.date, b.date) >= ?4) AND (?5 IS NULL OR COALESCE(d.date, b.date) <= ?5)
               AND (?6 IS NULL OR li.description LIKE '%' || ?6 || '%')
             ORDER BY date DESC LIMIT 1000",
            params![p["account_id"].as_i64(), p["tax_rate_id"].as_i64(), p["contact_id"].as_i64(), p["from"].as_str(), p["to"].as_str(), p["q"].as_str().filter(|s| !s.is_empty())])
    }

    /// Recode lines: changes account/tax/tracking; affected transactions are reversed and re-posted.
    pub fn recode_lines(&self, line_ids: &[i64], account_id: Option<i64>, tax_rate_id: Option<i64>, tracking1: Option<i64>, tracking2: Option<i64>) -> Result<i64> {
        self.atomic(|| {
            let mut parents: Vec<(String, i64)> = Vec::new();
            for lid in line_ids {
                let (kind, pid): (String, i64) = self.conn.query_row("SELECT parent_kind, parent_id FROM line_items WHERE id = ?1", [lid], |r| Ok((r.get(0)?, r.get(1)?)))?;
                if let Some(a) = account_id { self.conn.execute("UPDATE line_items SET account_id = ?2 WHERE id = ?1", params![lid, a])?; }
                if let Some(t) = tracking1 { self.conn.execute("UPDATE line_items SET tracking1 = ?2 WHERE id = ?1", params![lid, t])?; }
                if let Some(t) = tracking2 { self.conn.execute("UPDATE line_items SET tracking2 = ?2 WHERE id = ?1", params![lid, t])?; }
                if let Some(t) = tax_rate_id { self.conn.execute("UPDATE line_items SET tax_rate_id = ?2 WHERE id = ?1", params![lid, t])?; }
                if !parents.contains(&(kind.clone(), pid)) { parents.push((kind, pid)); }
            }
            for (kind, pid) in &parents {
                if kind == "DOC" { self.repost_document(*pid, tax_rate_id.is_some())?; } else { self.repost_bank_transaction(*pid, tax_rate_id.is_some())?; }
            }
            self.audit("RECODE", "lines", None, None, Some(json!({"lines": line_ids, "account_id": account_id, "tax_rate_id": tax_rate_id})))?;
            Ok(parents.len() as i64)
        })
    }

    fn repost_document(&self, id: i64, _retax: bool) -> Result<()> {
        // Re-save the document through the normal path (reverses and re-posts journal), preserving payments.
        let d = self.get_document(id)?;
        let date = d["date"].as_str().unwrap_or("").to_string();
        self.check_lock(&date)?;
        let jid = d["journal_id"].as_i64();
        let lines: Vec<LineInput> = d["lines"].as_array().unwrap().iter().map(|l| LineInput {
            item_id: l["item_id"].as_i64(), description: l["description"].as_str().unwrap_or("").into(),
            quantity: Some(money::fmt_fixed(l["qty_e4"].as_i64().unwrap_or(0), 4)), unit_price: Some(money::fmt_fixed(l["unit_e2"].as_i64().unwrap_or(0), 4)),
            discount: Some(money::fmt_fixed(l["discount_bp"].as_i64().unwrap_or(0), 2)), account_id: l["account_id"].as_i64(), tax_rate_id: l["tax_rate_id"].as_i64(),
            tracking1: l["tracking1"].as_i64(), tracking2: l["tracking2"].as_i64(), amount: None }).collect();
        let lat = Lat::parse(d["line_amount_type"].as_str().unwrap_or(""))?;
        let computed = self.compute_lines(lat, &lines, crate::documents::is_sales(d["type"].as_str().unwrap_or("")))?;
        let total: i64 = computed.iter().map(|l| l.net).sum::<i64>() + computed.iter().filter(|l| l.report_type.as_deref() != Some("REVERSECHARGE")).map(|l| l.tax).sum::<i64>();
        if total != d["total"].as_i64().unwrap_or(0) && (d["amount_paid"].as_i64().unwrap_or(0) > 0 || d["amount_credited"].as_i64().unwrap_or(0) > 0) {
            return invalid(format!("recoding {} would change its total; it has payments applied", d["number"].as_str().unwrap_or("")));
        }
        self.write_lines("DOC", id, &computed)?;
        if let Some(j) = jid {
            // Reverse only the document journal (payments are untouched) and post a fresh one.
            let _ = j;
            self.unpost_document(id)?;
            let nid = self.post_document(id)?;
            self.conn.execute("UPDATE documents SET journal_id = ?2, total_tax = ?3, subtotal = ?4, total = ?5 WHERE id = ?1", params![id, nid,
                computed.iter().filter(|l| l.report_type.as_deref() != Some("REVERSECHARGE")).map(|l| l.tax).sum::<i64>(), computed.iter().map(|l| l.net).sum::<i64>(), total])?;
        }
        self.add_note("document", id, "HISTORY", "Recoded via Find & Recode")?;
        Ok(())
    }

    fn repost_bank_transaction(&self, id: i64, _retax: bool) -> Result<()> {
        let t = self.get_bank_transaction(id)?;
        let lines: Vec<LineInput> = t["lines"].as_array().unwrap().iter().map(|l| LineInput {
            description: l["description"].as_str().unwrap_or("").into(), quantity: Some(money::fmt_fixed(l["qty_e4"].as_i64().unwrap_or(0), 4)),
            unit_price: Some(money::fmt_fixed(l["unit_e2"].as_i64().unwrap_or(0), 4)), account_id: l["account_id"].as_i64(), tax_rate_id: l["tax_rate_id"].as_i64(),
            tracking1: l["tracking1"].as_i64(), tracking2: l["tracking2"].as_i64(), ..Default::default() }).collect();
        let rec = t["reconciled"].as_i64().unwrap_or(0);
        let old_total = t["total"].as_i64().unwrap_or(0);
        self.conn.execute("UPDATE bank_transactions SET reconciled = 0 WHERE id = ?1", [id])?;
        self.save_bank_transaction(Some(id), &crate::banking::BankTxInput { kind: t["type"].as_str().unwrap_or("").into(), account_id: t["account_id"].as_i64().unwrap_or(0),
            contact_id: t["contact_id"].as_i64(), date: t["date"].as_str().unwrap_or("").into(), reference: t["reference"].as_str().unwrap_or("").into(),
            line_amount_type: t["line_amount_type"].as_str().map(String::from), lines, ..Default::default() })?;
        let new_total: i64 = self.conn.query_row("SELECT total FROM bank_transactions WHERE id = ?1", [id], |r| r.get(0))?;
        if rec != 0 && new_total != old_total { return invalid("recoding would change the total of a reconciled transaction"); }
        self.conn.execute("UPDATE bank_transactions SET reconciled = ?2 WHERE id = ?1", params![id, rec])?;
        Ok(())
    }
}

fn substitute_placeholders(v: &mut Value, date: &str) {
    let d = util::parse_date(date).unwrap();
    use chrono::Datelike;
    const M: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let month = M[d.month0() as usize];
    let next = util::add_months(d, 1);
    let prev = util::add_months(d, -1);
    let rep = |s: &str| s.replace("[Month]", month).replace("[Year]", &d.year().to_string())
        .replace("[Month+1]", M[next.month0() as usize]).replace("[Month-1]", M[prev.month0() as usize])
        .replace("[Week]", &d.iso_week().week().to_string());
    match v {
        Value::String(s) => *s = rep(s),
        Value::Array(a) => a.iter_mut().for_each(|x| substitute_placeholders(x, date)),
        Value::Object(o) => o.values_mut().for_each(|x| substitute_placeholders(x, date)),
        _ => {}
    }
}
