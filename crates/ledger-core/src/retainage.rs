//! Retainage / retentions (QuickBooks Contractor, Acumatica construction): part of each progress
//! invoice is held back until the job is finished. The held amount is a negative invoice line
//! coded to Retainage Receivable (an asset), so it's out of what the customer owes now; a later
//! "retainage release" invoice bills it, coded back to the same account.

use crate::{accounts::AccountInput, documents::{DocInput, LineInput}, error::invalid, money, util, Ledger, Result};
use rusqlite::OptionalExtension;
use serde_json::Value;

pub const RETAINAGE: &str = "Retainage Receivable";

impl Ledger {
    /// The Retainage Receivable account, created the first time it's needed.
    pub fn retainage_account(&self) -> Result<i64> {
        if let Some(id) = self.conn.query_row("SELECT id FROM accounts WHERE name = ?1 COLLATE NOCASE AND status = 'ACTIVE'", [RETAINAGE], |r| r.get(0)).optional()? { return Ok(id); }
        self.create_account(&AccountInput { name: RETAINAGE.into(), kind: "CURRENT".into(), description: "Amounts held back on progress invoices until the work is finished".into(), ..Default::default() })
    }

    /// Retainage held per customer (positive = owed to us).
    pub fn retainage_balances(&self) -> Result<Vec<Value>> {
        let acct = self.retainage_account()?;
        util::query_json(&self.conn,
            "SELECT c.id AS contact_id, c.name, SUM(jl.amount) AS held FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id JOIN contacts c ON c.id = jl.contact_id
             WHERE j.posted = 1 AND jl.account_id = ?1 GROUP BY c.id HAVING SUM(jl.amount) <> 0 ORDER BY c.name", [acct])
    }

    /// A draft invoice releasing a customer's retainage (all of it unless `amount` is given).
    pub fn retainage_release(&self, contact: i64, amount: Option<i64>, date: Option<&str>) -> Result<i64> {
        let acct = self.retainage_account()?;
        let held: i64 = self.conn.query_row("SELECT COALESCE(SUM(jl.amount), 0) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE j.posted = 1 AND jl.account_id = ?1 AND jl.contact_id = ?2",
            rusqlite::params![acct, contact], |r| r.get(0))?;
        let amount = amount.unwrap_or(held);
        if amount <= 0 { return invalid("there's no retainage held for this customer"); }
        if amount > held { return invalid(format!("only {} is held for this customer", money::fmt_money(held))); }
        self.save_document(None, &DocInput { kind: "ACCREC".into(), contact_id: Some(contact), date: date.map(String::from).unwrap_or_else(util::today), line_amount_type: Some("NOTAX".into()),
            lines: vec![LineInput { description: "Retainage released".into(), quantity: Some("1".into()), unit_price: Some(money::fmt_money(amount)), account_id: Some(acct), ..Default::default() }],
            status: Some("DRAFT".into()), ..Default::default() })
    }
}
