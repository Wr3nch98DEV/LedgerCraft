//! The general ledger posting engine. The single choke point for writes to `journals`.

use crate::{error::invalid, util, Error, Ledger, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JLine {
    pub account_id: i64,
    /// Base currency; debit positive, credit negative.
    pub amount: i64,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tax_rate_id: Option<i64>,
    #[serde(default)]
    pub tax_amount: i64,
    #[serde(default)]
    pub contact_id: Option<i64>,
    #[serde(default)]
    pub tracking1: Option<i64>,
    #[serde(default)]
    pub tracking2: Option<i64>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub fx_amount: Option<i64>,
    #[serde(default)]
    pub item_id: Option<i64>,
    #[serde(default)]
    pub qty_e4: Option<i64>,
}

impl JLine {
    pub fn new(account_id: i64, amount: i64) -> JLine { JLine { account_id, amount, ..Default::default() } }
    pub fn desc(mut self, d: impl Into<String>) -> JLine { self.description = d.into(); self }
    pub fn contact(mut self, c: Option<i64>) -> JLine { self.contact_id = c; self }
    pub fn tax(mut self, rate: Option<i64>, amount: i64) -> JLine { self.tax_rate_id = rate; self.tax_amount = amount; self }
    pub fn tracking(mut self, t1: Option<i64>, t2: Option<i64>) -> JLine { self.tracking1 = t1; self.tracking2 = t2; self }
}

#[derive(Debug, Clone, Default)]
pub struct NewJournal {
    pub date: String,
    pub source_type: String,
    pub source_id: Option<i64>,
    pub reference: String,
    pub narration: String,
    pub lines: Vec<JLine>,
    pub cash_basis: bool,
    pub reverses: Option<i64>,
}

impl Ledger {
    /// Post a balanced journal. Fails on imbalance, lock dates or archived accounts.
    pub fn post_journal(&self, mut j: NewJournal) -> Result<i64> {
        util::parse_date(&j.date)?;
        self.check_lock(&j.date)?;
        j.lines.retain(|l| l.amount != 0 || l.tax_amount != 0);
        if j.lines.is_empty() {
            return invalid("journal has no lines");
        }
        let sum: i64 = j.lines.iter().map(|l| l.amount).sum();
        if sum != 0 {
            return invalid(format!("journal does not balance (out by {})", crate::money::fmt_money(sum)));
        }
        self.atomic(|| {
            let number: i64 = self.conn.query_row("SELECT COALESCE(MAX(number), 0) + 1 FROM journals", [], |r| r.get(0))?;
            self.conn.execute(
                "INSERT INTO journals(number, date, source_type, source_id, reference, narration, reverses_journal_id, cash_basis, created_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![number, j.date, j.source_type, j.source_id, j.reference, j.narration, j.reverses, j.cash_basis as i64, self.user_id],
            )?;
            let jid = self.conn.last_insert_rowid();
            let mut stmt = self.conn.prepare_cached(
                "INSERT INTO journal_lines(journal_id, account_id, amount, description, tax_rate_id, tax_amount, tax_report_type,
                   contact_id, tracking1, tracking2, currency, fx_amount, item_id, qty_e4)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )?;
            for l in &j.lines {
                let status: Option<String> = self.conn.query_row("SELECT status FROM accounts WHERE id = ?1", [l.account_id], |r| r.get(0)).optional()?;
                match status.as_deref() {
                    None => return Err(Error::NotFound(format!("account {}", l.account_id))),
                    Some("ARCHIVED") if j.reverses.is_none() => return invalid("cannot post to an archived account"),
                    _ => {}
                }
                let report_type: Option<String> = match l.tax_rate_id {
                    Some(t) => self.conn.query_row("SELECT report_type FROM tax_rates WHERE id = ?1", [t], |r| r.get(0)).optional()?,
                    None => None,
                };
                stmt.execute(params![jid, l.account_id, l.amount, l.description, l.tax_rate_id, l.tax_amount, report_type,
                    l.contact_id, l.tracking1, l.tracking2, l.currency, l.fx_amount, l.item_id, l.qty_e4])?;
            }
            self.conn.execute("UPDATE journals SET posted = 1 WHERE id = ?1", [jid])?;
            Ok(jid)
        })
    }

    /// Reverse a posted journal on `date` (defaults to the original date).
    pub fn reverse_journal(&self, journal_id: i64, date: Option<&str>, source_type: Option<&str>) -> Result<i64> {
        let (orig_date, st, sid, reference, reversed): (String, String, Option<i64>, String, i64) = self.conn.query_row(
            "SELECT date, source_type, source_id, reference, reversed FROM journals WHERE id = ?1 AND posted = 1",
            [journal_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
        if reversed != 0 { return invalid("journal is already reversed"); }
        let lines = self.journal_lines(journal_id)?;
        let date = date.map(|s| s.to_string()).unwrap_or(orig_date);
        self.atomic(|| {
            let id = self.post_journal(NewJournal {
                date,
                source_type: source_type.unwrap_or(&st).to_string(),
                source_id: sid,
                reference,
                narration: format!("Reversal of journal #{journal_id}"),
                lines: lines.into_iter().map(|mut l| { l.amount = -l.amount; l.tax_amount = -l.tax_amount; l.fx_amount = l.fx_amount.map(|x| -x); l }).collect(),
                cash_basis: true,
                reverses: Some(journal_id),
            })?;
            self.conn.execute("UPDATE journals SET reversed = 1 WHERE id = ?1", [journal_id])?;
            Ok(id)
        })
    }

    pub fn journal_lines(&self, journal_id: i64) -> Result<Vec<JLine>> {
        let mut stmt = self.conn.prepare(
            "SELECT account_id, amount, description, tax_rate_id, tax_amount, contact_id, tracking1, tracking2, currency, fx_amount, item_id, qty_e4
             FROM journal_lines WHERE journal_id = ?1 ORDER BY id")?;
        let rows = stmt.query_map([journal_id], |r| Ok(JLine {
            account_id: r.get(0)?, amount: r.get(1)?, description: r.get(2)?, tax_rate_id: r.get(3)?, tax_amount: r.get(4)?,
            contact_id: r.get(5)?, tracking1: r.get(6)?, tracking2: r.get(7)?, currency: r.get(8)?, fx_amount: r.get(9)?,
            item_id: r.get(10)?, qty_e4: r.get(11)?,
        }))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn journal_json(&self, journal_id: i64) -> Result<Value> {
        let mut j = util::query_one_json(&self.conn, "SELECT * FROM journals WHERE id = ?1", [journal_id])?;
        let lines = util::query_json(&self.conn,
            "SELECT jl.*, a.code AS account_code, a.name AS account_name, t.name AS tax_rate_name
             FROM journal_lines jl JOIN accounts a ON a.id = jl.account_id LEFT JOIN tax_rates t ON t.id = jl.tax_rate_id
             WHERE jl.journal_id = ?1 ORDER BY jl.id", [journal_id])?;
        j["lines"] = Value::from(lines);
        Ok(j)
    }

    /// Sum of all posted journal lines must be zero. Used by tests and the assurance dashboard.
    pub fn ledger_imbalance(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COALESCE(SUM(jl.amount),0) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE j.posted = 1", [], |r| r.get(0))?)
    }
}
