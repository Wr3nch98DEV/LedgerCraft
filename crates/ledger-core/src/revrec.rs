//! Revenue recognition (Acumatica deferral schedules, IFRS 15 / ASC 606 over-time revenue): an
//! invoice's income is moved to a Deferred Revenue liability on the invoice date, then
//! recognised back into income in equal monthly parts as the months pass.

use crate::{accounts::AccountInput, error::invalid, journal::{JLine, NewJournal}, money, util, Ledger, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

pub const DEFERRED_REVENUE: &str = "Deferred Revenue";

impl Ledger {
    /// The Deferred Revenue liability account, created the first time it's needed.
    fn deferred_revenue_account(&self) -> Result<i64> {
        if let Some(id) = self.conn.query_row("SELECT id FROM accounts WHERE name = ?1 COLLATE NOCASE AND status = 'ACTIVE'", [DEFERRED_REVENUE], |r| r.get(0)).optional()? { return Ok(id); }
        self.create_account(&AccountInput { name: DEFERRED_REVENUE.into(), kind: "CURRLIAB".into(), description: "Income received in advance, recognised over time".into(), ..Default::default() })
    }

    /// Defer an approved invoice's income and recognise it over `months` months from `start`.
    pub fn create_revenue_schedule(&self, document: i64, start: &str, months: i64) -> Result<i64> {
        let start_d = util::parse_date(start)?;
        if !(1..=120).contains(&months) { return invalid("recognise over 1 to 120 months"); }
        let d = self.get_document(document)?;
        if d["type"] != "ACCREC" || !matches!(d["status"].as_str(), Some("AUTHORISED" | "PAID")) { return invalid("choose an approved invoice"); }
        let existing: i64 = self.conn.query_row("SELECT COUNT(*) FROM revenue_schedules WHERE document_id = ?1 AND status = 'ACTIVE'", [document], |r| r.get(0))?;
        if existing > 0 { return invalid("this invoice already has a revenue schedule"); }
        // Income per revenue account on the invoice, in the base currency (from its posted journal).
        let jid = d["journal_id"].as_i64().ok_or_else(|| crate::Error::Validation("the invoice isn't posted".into()))?;
        let parts: Vec<(i64, i64)> = {
            let mut st = self.conn.prepare("SELECT jl.account_id, -SUM(jl.amount) FROM journal_lines jl JOIN accounts a ON a.id = jl.account_id
                WHERE jl.journal_id = ?1 AND a.class = 'REVENUE' GROUP BY jl.account_id HAVING SUM(jl.amount) < 0")?;
            st.query_map([jid], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let total: i64 = parts.iter().map(|p| p.1).sum();
        if total <= 0 { return invalid("this invoice has no income to defer"); }
        let deferral = self.deferred_revenue_account()?;
        let number = d["number"].as_str().unwrap_or("").to_string();
        let date = d["date"].as_str().unwrap_or("").to_string();
        self.atomic(|| {
            // Move the income to Deferred Revenue on the invoice date.
            let mut lines: Vec<JLine> = parts.iter().map(|(a, amt)| JLine::new(*a, *amt).desc(format!("Deferred: invoice {number}"))).collect();
            lines.push(JLine::new(deferral, -total).desc(format!("Deferred: invoice {number}")));
            let j = self.post_journal(NewJournal { date: date.clone(), source_type: "REVREC".into(), source_id: Some(document), reference: number.clone(),
                narration: format!("Revenue deferred over {months} months"), lines, cash_basis: false, reverses: None })?;
            self.conn.execute("INSERT INTO revenue_schedules(document_id, deferral_account_id, start_date, months, total, journal_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![document, deferral, start, months, total, j])?;
            let sid = self.conn.last_insert_rowid();
            // Equal monthly parts per account; the last month takes the rounding.
            for (acct, amt) in &parts {
                let each = money::div_round(*amt as i128, months as i128) as i64;
                for n in 0..months {
                    let a = if n + 1 == months { amt - each * (months - 1) } else { each };
                    let on = util::add_months(start_d, n as i32).to_string();
                    self.conn.execute("INSERT INTO revenue_entries(schedule_id, period, date, account_id, amount) VALUES (?1, ?2, ?3, ?4, ?5)", params![sid, n + 1, on, acct, a])?;
                }
            }
            self.add_note("document", document, "HISTORY", &format!("Revenue of {} deferred, recognised over {months} months from {start}", money::fmt_money(total)))?;
            Ok(sid)
        })
    }

    /// Post every scheduled month due on or before `to`. Returns how many months were posted.
    pub fn recognise_revenue(&self, to: &str) -> Result<i64> {
        util::parse_date(to)?;
        let due: Vec<(i64, String, i64, String, i64)> = {
            let mut st = self.conn.prepare("SELECT e.schedule_id, e.date, s.deferral_account_id, d.number, e.period FROM revenue_entries e JOIN revenue_schedules s ON s.id = e.schedule_id
                JOIN documents d ON d.id = s.document_id WHERE e.journal_id IS NULL AND s.status = 'ACTIVE' AND e.date <= ?1 GROUP BY e.schedule_id, e.period ORDER BY e.date")?;
            st.query_map([to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
        };
        self.atomic(|| {
            for (sid, date, deferral, number, period) in &due {
                let entries: Vec<(i64, i64, i64)> = {
                    let mut st = self.conn.prepare("SELECT id, account_id, amount FROM revenue_entries WHERE schedule_id = ?1 AND period = ?2 AND journal_id IS NULL")?;
                    st.query_map(params![sid, period], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
                };
                let total: i64 = entries.iter().map(|e| e.2).sum();
                let mut lines: Vec<JLine> = entries.iter().map(|(_, a, amt)| JLine::new(*a, -amt).desc(format!("Revenue recognised: invoice {number}, month {period}"))).collect();
                lines.push(JLine::new(*deferral, total).desc(format!("Revenue recognised: invoice {number}, month {period}")));
                let j = self.post_journal(NewJournal { date: date.clone(), source_type: "REVREC".into(), source_id: Some(*sid), reference: number.clone(),
                    narration: format!("Revenue recognised, month {period}"), lines, cash_basis: false, reverses: None })?;
                for (id, _, _) in &entries { self.conn.execute("UPDATE revenue_entries SET journal_id = ?2 WHERE id = ?1", params![id, j])?; }
            }
            Ok(due.len() as i64)
        })
    }

    pub fn list_revenue_schedules(&self) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT s.*, d.number, c.name AS contact_name,
               COALESCE((SELECT SUM(amount) FROM revenue_entries e WHERE e.schedule_id = s.id AND e.journal_id IS NOT NULL), 0) AS recognised,
               (SELECT MIN(date) FROM revenue_entries e WHERE e.schedule_id = s.id AND e.journal_id IS NULL) AS next_date
             FROM revenue_schedules s JOIN documents d ON d.id = s.document_id JOIN contacts c ON c.id = d.contact_id WHERE s.status = 'ACTIVE' ORDER BY s.start_date DESC", [])
    }

    /// Report: deferred revenue still to recognise, by schedule.
    pub fn deferred_revenue_report(&self) -> Result<Value> {
        let rows: Vec<Value> = self.list_revenue_schedules()?.into_iter().map(|s| {
            let (total, done) = (s["total"].as_i64().unwrap_or(0), s["recognised"].as_i64().unwrap_or(0));
            json!({"label": format!("Invoice {} — {}", s["number"].as_str().unwrap_or(""), s["contact_name"].as_str().unwrap_or("")), "values": [total, done, total - done]})
        }).collect();
        let tot = rows.iter().fold([0i64; 3], |mut t, r| { for n in 0..3 { t[n] += r["values"][n].as_i64().unwrap_or(0); } t });
        Ok(json!({"title": "Deferred Revenue", "org": self.setting_or("org_name", "")?, "subtitle": "Revenue recognition schedules", "columns": ["Deferred", "Recognised", "Still to recognise"],
            "sections": [{"title": "", "rows": rows, "total": {"label": "Total", "values": tot}}]}))
    }
}
