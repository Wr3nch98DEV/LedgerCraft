//! Customer deposits on sales orders (QuickBooks): money received for an order before it's
//! invoiced. It's a prepayment from the customer (a credit on their account) linked to the order,
//! and it's applied to the order's invoice automatically when that's approved.

use crate::{banking::BankTxInput, error::invalid, money, util, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

impl Ledger {
    /// Record a deposit received into `bank_account` for a sales order.
    pub fn record_order_deposit(&self, order: i64, bank_account: i64, amount: &str, date: &str) -> Result<i64> {
        let d = self.get_document(order)?;
        if d["type"] != "SALESORDER" { return invalid("deposits are taken on sales orders"); }
        if matches!(d["status"].as_str(), Some("INVOICED" | "DELETED" | "DECLINED")) { return invalid("this order is closed"); }
        let amt = money::parse_fixed(amount, 2).filter(|a| *a > 0).ok_or_else(|| crate::Error::Validation("enter the deposit amount".into()))?;
        if amt > d["total"].as_i64().unwrap_or(0) { return invalid("the deposit is more than the order total"); }
        let number = d["number"].as_str().unwrap_or("").to_string();
        self.atomic(|| {
            let tx = self.save_bank_transaction(None, &BankTxInput { kind: "RECEIVE-PREPAYMENT".into(), account_id: bank_account, contact_id: d["contact_id"].as_i64(),
                date: date.into(), reference: format!("Deposit on {number}"), amount: Some(money::fmt_money(amt)), ..Default::default() })?;
            self.conn.execute("INSERT INTO order_deposits(order_id, bank_transaction_id) VALUES (?1, ?2)", params![order, tx])?;
            self.add_note("document", order, "HISTORY", &format!("Deposit of {} received", money::fmt_money_grouped(amt)))?;
            Ok(tx)
        })
    }

    pub fn order_deposits(&self, order: i64) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT b.id, b.date, b.total, b.remaining_credit, a.name AS account_name FROM order_deposits o JOIN bank_transactions b ON b.id = o.bank_transaction_id
            JOIN accounts a ON a.id = b.account_id WHERE o.order_id = ?1 AND b.status = 'AUTHORISED' ORDER BY b.date", [order])
    }

    /// Apply an order's unused deposits to an approved invoice made from it.
    pub(crate) fn apply_order_deposits(&self, invoice: i64) -> Result<()> {
        let source: Option<i64> = self.conn.query_row("SELECT source_document_id FROM documents WHERE id = ?1", [invoice], |r| r.get(0))?;
        let Some(order) = source else { return Ok(()) };
        let kind: String = self.conn.query_row("SELECT type FROM documents WHERE id = ?1", [order], |r| r.get(0))?;
        if kind != "SALESORDER" { return Ok(()); }
        for dep in self.order_deposits(order)? {
            let due: i64 = self.conn.query_row("SELECT total - amount_paid - amount_credited FROM documents WHERE id = ?1", [invoice], |r| r.get(0))?;
            let left = dep["remaining_credit"].as_i64().unwrap_or(0);
            let amt = left.min(due);
            if amt <= 0 { continue; }
            self.allocate_prepayment(dep["id"].as_i64().unwrap_or(0), invoice, &money::fmt_money(amt), None)?;
        }
        Ok(())
    }
}

pub fn deposits_json(l: &Ledger, order: i64) -> Result<Value> { Ok(json!(l.order_deposits(order)?)) }
