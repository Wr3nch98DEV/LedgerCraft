//! US 1099 tracking: vendors flagged for 1099s, and the yearly contractor summary of what was
//! paid to them. Card payments are left out (the card processor reports those on a 1099-K).

use crate::{reports::{fmt_long, ReportParams}, util, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

/// The IRS reporting threshold for 1099-NEC, in cents.
pub const THRESHOLD: i64 = 60000;

impl Ledger {
    /// A customer's credit limit (cents; None = no limit) and credit hold.
    pub fn set_credit(&self, contact: i64, limit: Option<i64>, hold: bool) -> Result<()> {
        if limit.is_some_and(|l| l < 0) { return Err(crate::error::Error::Validation("the credit limit can't be negative".into())); }
        self.conn.execute("UPDATE contacts SET credit_limit = ?2, credit_hold = ?3 WHERE id = ?1", params![contact, limit, hold as i64])?;
        self.audit("UPDATE", "contact", Some(contact), None, Some(json!({"credit_limit": limit, "credit_hold": hold})))?;
        self.add_note("contact", contact, "HISTORY", &if hold { "Put on credit hold".to_string() } else { match limit { Some(l) => format!("Credit limit set to {}", crate::money::fmt_money(l)), None => "Credit limit removed".into() } })?;
        Ok(())
    }

    /// Refuses to approve an invoice for a customer on credit hold, or one that takes them over
    /// their credit limit (unless the user said to go ahead). The over-limit message starts
    /// "CREDIT_LIMIT:" so the app can offer "Save anyway".
    pub fn check_credit(&self, contact: i64, doc: Option<i64>, total: i64, allow_over: bool) -> Result<()> {
        let (name, limit, hold): (String, Option<i64>, i64) = self.conn.query_row("SELECT name, credit_limit, credit_hold FROM contacts WHERE id = ?1", [contact], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        if hold == 1 { return Err(crate::error::Error::Validation(format!("{name} is on credit hold. Take them off hold on their contact page before approving invoices."))); }
        let Some(limit) = limit.filter(|l| *l > 0) else { return Ok(()) };
        if allow_over { return Ok(()); }
        let owing: i64 = self.conn.query_row("SELECT COALESCE(SUM(total - amount_paid - amount_credited), 0) FROM documents WHERE type = 'ACCREC' AND status = 'AUTHORISED' AND contact_id = ?1 AND id <> COALESCE(?2, 0)",
            params![contact, doc], |r| r.get(0))?;
        if owing + total > limit {
            return Err(crate::error::Error::Validation(format!("CREDIT_LIMIT: This takes {name} to {} owing, over their credit limit of {}.", crate::money::fmt_money(owing + total), crate::money::fmt_money(limit))));
        }
        Ok(())
    }

    pub fn set_track_1099(&self, contact: i64, on: bool) -> Result<()> {
        self.conn.execute("UPDATE contacts SET track_1099 = ?2 WHERE id = ?1", params![contact, on as i64])?;
        self.audit("UPDATE", "contact", Some(contact), None, Some(json!({"track_1099": on})))?;
        Ok(())
    }

    /// Per flagged vendor: (contact id, name, tax id, paid other than by card, paid by card).
    pub fn payments_1099(&self, from: &str, to: &str) -> Result<Vec<(i64, String, String, i64, i64)>> {
        let vendors = util::query_json(&self.conn, "SELECT id, name, tax_number FROM contacts WHERE track_1099 = 1 ORDER BY name", [])?;
        let mut out = Vec::new();
        for v in vendors {
            let id = v["id"].as_i64().unwrap_or(0);
            // Bill payments (less refunds on vendor credits) and direct spends (less refunds), split by card or not.
            let (cash, card): (i64, i64) = self.conn.query_row(
                "SELECT COALESCE(SUM(CASE WHEN card = 0 THEN amt END), 0), COALESCE(SUM(CASE WHEN card = 1 THEN amt END), 0) FROM (
                   SELECT p.base_amount * CASE WHEN d.type = 'ACCPAY' THEN 1 ELSE -1 END AS amt, COALESCE(a.bank_type = 'CREDITCARD', 0) AS card
                     FROM payments p JOIN documents d ON d.id = p.document_id JOIN accounts a ON a.id = p.account_id
                    WHERE d.contact_id = ?1 AND d.type IN ('ACCPAY','ACCPAYCREDIT') AND p.status <> 'DELETED' AND p.date >= ?2 AND p.date <= ?3
                   UNION ALL
                   SELECT CASE WHEN b.type LIKE 'SPEND%' THEN b.total ELSE -b.total END, COALESCE(a.bank_type = 'CREDITCARD', 0)
                     FROM bank_transactions b JOIN accounts a ON a.id = b.account_id
                    WHERE b.contact_id = ?1 AND b.status <> 'DELETED' AND b.date >= ?2 AND b.date <= ?3)",
                params![id, from, to], |r| Ok((r.get(0)?, r.get(1)?)))?;
            out.push((id, v["name"].as_str().unwrap_or("").to_string(), v["tax_number"].as_str().unwrap_or("").to_string(), cash, card));
        }
        Ok(out)
    }

    /// "1099 Contractor Summary": vendors at or over the threshold, then those under it.
    /// W-2 Wage and Tax Summary (US): each employee's calendar-year wages and the federal income
    /// tax, social security and Medicare withheld, from posted pay runs. Social security and
    /// Medicare are the employee's 6.2% and 1.45% of each payslip; federal is the rest of the tax.
    pub fn report_w2(&self, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().or(p.date.clone()).unwrap_or_else(util::today);
        let year = to[..4].to_string();
        let (from, to) = (format!("{year}-01-01"), format!("{year}-12-31"));
        let slips = util::query_json(&self.conn,
            "SELECT e.id AS employee_id, e.first_name || ' ' || e.last_name AS name, e.tax_id, s.gross, s.tax FROM payslips s JOIN pay_runs r ON r.id = s.pay_run_id JOIN employees e ON e.id = s.employee_id
             WHERE r.status = 'POSTED' AND r.payment_date >= ?1 AND r.payment_date <= ?2 ORDER BY e.last_name, e.first_name", params![from, to])?;
        let mut rows: Vec<Value> = Vec::new();
        let mut tot = [0i64; 4];
        let mut cur: Option<(i64, String, [i64; 4])> = None;
        let flush = |c: Option<(i64, String, [i64; 4])>, rows: &mut Vec<Value>| if let Some((id, label, v)) = c { rows.push(json!({"label": label, "employee_id": id, "values": v})); };
        for s in &slips {
            let id = s["employee_id"].as_i64().unwrap_or(0);
            if cur.as_ref().map(|c| c.0) != Some(id) {
                flush(cur.take(), &mut rows);
                let tin = s["tax_id"].as_str().unwrap_or("");
                let name = s["name"].as_str().unwrap_or("");
                cur = Some((id, if tin.is_empty() { format!("{name} (no SSN on file)") } else { format!("{name} ({tin})") }, [0; 4]));
            }
            let (gross, tax) = (s["gross"].as_i64().unwrap_or(0), s["tax"].as_i64().unwrap_or(0));
            let ss = crate::money::div_round(gross as i128 * 620, 10_000) as i64;
            let med = crate::money::div_round(gross as i128 * 145, 10_000) as i64;
            let add = [gross, (tax - ss - med).max(0), ss, med];
            if let Some(c) = cur.as_mut() { for n in 0..4 { c.2[n] += add[n]; tot[n] += add[n]; } }
        }
        flush(cur.take(), &mut rows);
        Ok(json!({"title": "W-2 Wage and Tax Summary", "org": self.setting_or("org_name", "")?, "subtitle": format!("Calendar year {year}"),
            "columns": ["Wages (box 1)", "Federal tax withheld (box 2)", "Social security tax (box 4)", "Medicare tax (box 6)"],
            "sections": [{"title": "Employees", "rows": rows, "total": {"label": "Total", "values": tot}}],
            "notes": ["Figures to copy onto Forms W-2 and W-3; LedgerCraft doesn't file them. Pre-tax deductions and state boxes aren't included."]}))
    }

    pub fn report_1099(&self, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().or(p.date.clone()).unwrap_or_else(util::today);
        let from = p.from.clone().unwrap_or_else(|| format!("{}-01-01", &to[..4]));
        let mut over = Vec::new();
        let mut under = Vec::new();
        let (mut tot_over, mut tot_under) = ([0i64; 3], [0i64; 3]);
        for (id, name, tin, cash, card) in self.payments_1099(&from, &to)? {
            let label = if tin.is_empty() { format!("{name} (no tax ID on file)") } else { format!("{name} ({tin})") };
            let row = json!({"label": label, "contact_id": id, "values": [cash + card, card, cash]});
            let (rows, tot) = if cash >= THRESHOLD { (&mut over, &mut tot_over) } else { (&mut under, &mut tot_under) };
            rows.push(row);
            tot[0] += cash + card; tot[1] += card; tot[2] += cash;
        }
        Ok(json!({"title": "1099 Contractor Summary", "org": self.setting_or("org_name", "")?, "subtitle": format!("{} to {}", fmt_long(&from), fmt_long(&to)),
            "columns": ["Total paid", "Paid by card (excluded)", "Reportable on 1099-NEC"],
            "sections": [
                {"title": "Vendors to file a 1099 for ($600 or more)", "rows": over, "total": {"label": "Total", "values": tot_over}},
                {"title": "Below the threshold", "rows": under, "total": {"label": "Total", "values": tot_under}},
            ]}))
    }
}
