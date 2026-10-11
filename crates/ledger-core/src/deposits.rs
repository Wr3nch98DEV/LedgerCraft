//! Undeposited Funds and bank deposits (QuickBooks workflow).
//!
//! Customer payments can be received into the Undeposited Funds clearing account instead of a
//! bank account. A bank deposit later moves a group of them into the bank in one entry, so the
//! ledger shows a single amount that matches the single line on the bank statement.

use crate::journal::{JLine, NewJournal};
use crate::{error::invalid, util, Ledger, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

impl Ledger {
    /// The Undeposited Funds account, created on first use.
    pub fn undeposited_funds_account(&self) -> Result<i64> {
        if let Some(id) = self.conn.query_row("SELECT id FROM accounts WHERE system_account = 'UNDEPOSITED'", [], |r| r.get(0)).optional()? { return Ok(id); }
        let code = ["095", "096", "097", "098", "099", "0950", "0951"].into_iter()
            .find(|c| self.account_by_code(c).ok().flatten().is_none())
            .ok_or_else(|| crate::Error::Validation("no free account code for Undeposited Funds".into()))?;
        self.conn.execute(
            "INSERT INTO accounts(code, name, type, class, description, system_account, enable_payments) VALUES (?1, 'Undeposited Funds', 'CURRENT', 'ASSET', 'Payments received but not yet deposited to the bank', 'UNDEPOSITED', 1)",
            [code])?;
        let id = self.conn.last_insert_rowid();
        self.audit("CREATE", "account", Some(id), None, Some(json!({"system_account": "UNDEPOSITED"})))?;
        Ok(id)
    }

    /// Payments sitting in Undeposited Funds that haven't been deposited yet.
    pub fn undeposited_payments(&self) -> Result<Vec<Value>> {
        let uf = self.undeposited_funds_account()?;
        util::query_json(&self.conn,
            "SELECT p.id, p.date, p.reference, d.number, d.type, d.subtype, c.name AS contact,
                    p.base_amount * CASE WHEN d.type IN ('ACCREC','ACCPAYCREDIT') THEN 1 ELSE -1 END AS amount
             FROM payments p JOIN documents d ON d.id = p.document_id JOIN contacts c ON c.id = d.contact_id
             WHERE p.account_id = ?1 AND p.status <> 'DELETED' AND p.deposit_id IS NULL ORDER BY p.date, p.id", [uf])
    }

    /// Deposit the given Undeposited Funds payments into a bank account as one entry.
    pub fn bank_deposit(&self, bank: i64, date: &str, reference: &str, payment_ids: &[i64]) -> Result<i64> {
        util::parse_date(date)?;
        if payment_ids.is_empty() { return invalid("choose at least one payment to deposit"); }
        let kind: String = self.conn.query_row("SELECT type FROM accounts WHERE id = ?1", [bank], |r| r.get(0)).optional()?.ok_or_else(|| crate::Error::NotFound("bank account".into()))?;
        if kind != "BANK" { return invalid("deposits go into a bank account"); }
        let uf = self.undeposited_funds_account()?;
        let mut total = 0i64;
        for id in payment_ids {
            let row: Option<(i64, Option<i64>, String, i64, String)> = self.conn.query_row(
                "SELECT p.account_id, p.deposit_id, p.status, p.base_amount, d.type FROM payments p JOIN documents d ON d.id = p.document_id WHERE p.id = ?1", [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).optional()?;
            let Some((acc, dep, status, base, dtype)) = row else { return invalid(format!("payment {id} not found")) };
            if acc != uf { return invalid(format!("payment {id} isn't in Undeposited Funds")); }
            if dep.is_some() { return invalid(format!("payment {id} has already been deposited")); }
            if status == "DELETED" { return invalid(format!("payment {id} was deleted")); }
            total += if matches!(dtype.as_str(), "ACCREC" | "ACCPAYCREDIT") { base } else { -base };
        }
        if total == 0 { return invalid("the deposit total is zero"); }
        self.atomic(|| {
            self.conn.execute("INSERT INTO deposits(account_id, date, reference, total) VALUES (?1, ?2, ?3, ?4)", params![bank, date, reference, total])?;
            let dep = self.conn.last_insert_rowid();
            let jid = self.post_journal(NewJournal {
                date: date.into(), source_type: "DEPOSIT".into(), source_id: Some(dep), reference: reference.into(), narration: "Bank deposit".into(),
                lines: vec![JLine::new(bank, total).desc("Bank deposit"), JLine::new(uf, -total).desc("Bank deposit")], cash_basis: true, reverses: None,
            })?;
            self.conn.execute("UPDATE deposits SET journal_id = ?2 WHERE id = ?1", params![dep, jid])?;
            for id in payment_ids { self.conn.execute("UPDATE payments SET deposit_id = ?2 WHERE id = ?1", params![id, dep])?; }
            self.audit("CREATE", "deposit", Some(dep), None, Some(json!({"total": total, "payments": payment_ids.len()})))?;
            Ok(dep)
        })
    }

    /// Undo a deposit: reverse it and put its payments back into Undeposited Funds.
    pub fn delete_deposit(&self, id: i64) -> Result<()> {
        let (jid, reconciled, status): (Option<i64>, i64, String) = self.conn.query_row("SELECT journal_id, reconciled, status FROM deposits WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        if status == "DELETED" { return Ok(()); }
        if reconciled != 0 { return invalid("this deposit is reconciled — unreconcile it first"); }
        self.atomic(|| {
            if let Some(j) = jid { self.reverse_journal(j, None, None)?; }
            self.conn.execute("UPDATE deposits SET status = 'DELETED' WHERE id = ?1", [id])?;
            self.conn.execute("UPDATE payments SET deposit_id = NULL WHERE deposit_id = ?1", [id])?;
            self.audit("DELETE", "deposit", Some(id), None, None)?;
            Ok(())
        })
    }

    pub fn list_deposits(&self, bank: Option<i64>) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT d.*, a.name AS account_name, (SELECT COUNT(*) FROM payments p WHERE p.deposit_id = d.id) AS payment_count
             FROM deposits d JOIN accounts a ON a.id = d.account_id WHERE d.status <> 'DELETED' AND (?1 IS NULL OR d.account_id = ?1) ORDER BY d.date DESC, d.id DESC", [bank])
    }
}
