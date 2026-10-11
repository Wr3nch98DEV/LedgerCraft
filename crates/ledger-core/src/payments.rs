//! Payments against invoices/bills/credit notes, credit allocations and batch payments.

use crate::{documents::{entity_name, is_sales}, error::invalid, journal::{JLine, NewJournal}, money, util, Error, Ledger, Result};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct PaymentInput {
    pub document_id: i64,
    pub account_id: i64,
    pub date: String,
    pub amount: String,
    pub rate: Option<String>,
    pub reference: String,
    pub batch_id: Option<i64>,
}

impl Ledger {
    /// Record a payment (or refund for credit notes). Handles realised FX gains/losses.
    pub fn add_payment(&self, p: &PaymentInput) -> Result<i64> {
        util::parse_date(&p.date)?;
        let amount = money::parse_fixed(&p.amount, 2).filter(|a| *a > 0).ok_or_else(|| Error::Validation("payment amount must be greater than zero".into()))?;
        let (kind, status, contact, currency, doc_rate, total, paid, credited, base_total, number): (String, String, i64, String, i64, i64, i64, i64, i64, String) = self.conn.query_row(
            "SELECT type, status, contact_id, currency, rate_e8, total, amount_paid, amount_credited, base_total, number FROM documents WHERE id = ?1", [p.document_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?)))?;
        if status != "AUTHORISED" { return invalid(format!("only approved {}s can be paid", entity_name(&kind))); }
        let due = total - paid - credited;
        if amount > due { return invalid(format!("payment of {} exceeds the amount due of {}", money::fmt_money(amount), money::fmt_money(due))); }
        let (bank_type, bank_cur): (String, Option<String>) = self.conn.query_row("SELECT type, currency FROM accounts WHERE id = ?1", [p.account_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let enable: i64 = self.conn.query_row("SELECT enable_payments FROM accounts WHERE id = ?1", [p.account_id], |r| r.get(0))?;
        if bank_type != "BANK" && enable == 0 { return invalid("payments can only be made to bank accounts or accounts with payments enabled"); }
        let base = self.base_currency()?;
        let bank_cur = bank_cur.unwrap_or(base.clone());
        if bank_cur != base && bank_cur != currency { return invalid(format!("a {bank_cur} account can't be used to pay a {currency} {}", entity_name(&kind))); }
        let rate = if currency == base { 100_000_000 } else {
            match p.rate.as_deref().filter(|s| !s.trim().is_empty()) {
                Some(r) => money::parse_rate_e8(r).filter(|r| *r > 0).ok_or_else(|| Error::Validation("invalid exchange rate".into()))?,
                None => self.rate_for(&currency, &p.date).unwrap_or(doc_rate),
            }
        };
        let bank_base = self.to_base_amount(amount, &currency, rate)?;
        // Portion of the control account cleared, at the document's rate. Final payment clears the remainder.
        let cleared_base: i64 = if amount == due {
            base_total - self.base_cleared(p.document_id)?
        } else {
            money::div_round(base_total as i128 * amount as i128, total.max(1) as i128) as i64
        };
        let control = if is_sales(&kind) { self.sys_account("AR")? } else { self.sys_account("AP")? };
        // Money in for invoices & supplier credit refunds; out for bills & customer credit refunds.
        let money_in = matches!(kind.as_str(), "ACCREC" | "ACCPAYCREDIT");
        let s = if money_in { 1 } else { -1 };
        let mut lines = vec![
            JLine { account_id: p.account_id, amount: s * bank_base, description: format!("Payment: {number}"), contact_id: Some(contact),
                currency: Some(bank_cur.clone()), fx_amount: Some(s * if bank_cur == currency { amount } else { bank_base }), ..Default::default() },
            JLine { account_id: control, amount: -s * cleared_base, description: format!("Payment: {number}"), contact_id: Some(contact),
                currency: Some(currency.clone()), fx_amount: Some(-s * amount), ..Default::default() },
        ];
        let diff = s * bank_base - s * cleared_base;
        if diff != 0 {
            lines.push(JLine::new(self.sys_account("FXGAIN")?, -diff).desc(format!("Realised FX on {number}")).contact(Some(contact)));
        }
        self.atomic(|| {
            let jid = self.post_journal(NewJournal { date: p.date.clone(), source_type: "PAYMENT".into(), source_id: Some(p.document_id),
                reference: if p.reference.is_empty() { number.clone() } else { p.reference.clone() }, narration: format!("Payment for {number}"),
                lines, cash_basis: true, reverses: None })?;
            self.conn.execute("INSERT INTO payments(document_id, account_id, date, amount, rate_e8, base_amount, control_base, reference, kind, batch_id, journal_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?11, ?7, ?8, ?9, ?10)",
                params![p.document_id, p.account_id, p.date, amount, rate, bank_base, p.reference, if kind.ends_with("CREDIT") { "REFUND" } else { "PAYMENT" }, p.batch_id, jid, cleared_base])?;
            let pid = self.conn.last_insert_rowid();
            self.conn.execute("UPDATE journals SET source_id = ?2 WHERE id = ?1", params![jid, pid])?;
            self.refresh_paid(p.document_id)?;
            self.add_note("document", p.document_id, "HISTORY", &format!("Payment of {} {} received on {}", currency, money::fmt_money(amount), p.date))?;
            self.audit("CREATE", "payment", Some(pid), None, Some(json!({"document_id": p.document_id, "amount": amount})))?;
            Ok(pid)
        })
    }

    /// Base-currency amount already cleared from AR/AP for a document by payments and allocations.
    pub fn base_cleared(&self, doc: i64) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE((SELECT SUM(control_base) FROM payments WHERE document_id = ?1 AND status <> 'DELETED'), 0)
                  + COALESCE((SELECT SUM(doc_base) FROM credit_allocations WHERE document_id = ?1 AND status <> 'DELETED'), 0)
                  + COALESCE((SELECT SUM(cn_base) FROM credit_allocations WHERE credit_note_id = ?1 AND status <> 'DELETED'), 0)
                  + COALESCE((SELECT SUM(doc_base) FROM prepayment_allocations WHERE document_id = ?1 AND status <> 'DELETED'), 0)", [doc], |r| r.get(0))?)
    }

    /// Recalculate amount paid/credited and PAID status for a document.
    pub fn refresh_paid(&self, doc: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE documents SET
               amount_paid = COALESCE((SELECT SUM(amount) FROM payments WHERE document_id = ?1 AND status <> 'DELETED'), 0),
               amount_credited = COALESCE((SELECT SUM(amount) FROM credit_allocations WHERE (document_id = ?1 OR credit_note_id = ?1) AND status <> 'DELETED'), 0)
                 + COALESCE((SELECT SUM(amount) FROM prepayment_allocations WHERE document_id = ?1 AND status <> 'DELETED'), 0)
             WHERE id = ?1", [doc])?;
        let (status, total, paid, credited): (String, i64, i64, i64) = self.conn.query_row("SELECT status, total, amount_paid, amount_credited FROM documents WHERE id = ?1", [doc], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        if status == "AUTHORISED" && paid + credited >= total {
            let last: Option<String> = self.conn.query_row("SELECT MAX(d) FROM (SELECT MAX(date) AS d FROM payments WHERE document_id = ?1 AND status <> 'DELETED' UNION ALL SELECT MAX(date) FROM credit_allocations WHERE (document_id = ?1 OR credit_note_id = ?1) AND status <> 'DELETED')", [doc], |r| r.get(0))?;
            self.conn.execute("UPDATE documents SET status = 'PAID', fully_paid_on = ?2 WHERE id = ?1", params![doc, last])?;
        } else if status == "PAID" && paid + credited < total {
            self.conn.execute("UPDATE documents SET status = 'AUTHORISED', fully_paid_on = NULL WHERE id = ?1", [doc])?;
        }
        Ok(())
    }

    pub fn delete_payment(&self, id: i64) -> Result<()> {
        let (doc, jid, reconciled, status, date): (i64, Option<i64>, i64, String, String) = self.conn.query_row(
            "SELECT document_id, journal_id, reconciled, status, date FROM payments WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
        if status == "DELETED" { return Ok(()); }
        if reconciled != 0 { return invalid("this payment is reconciled; unreconcile it first"); }
        self.check_lock(&date)?;
        self.atomic(|| {
            if let Some(j) = jid {
                let r = self.reverse_journal(j, None, None)?;
                self.conn.execute("UPDATE payments SET void_journal_id = ?2 WHERE id = ?1", params![id, r])?;
            }
            self.conn.execute("UPDATE payments SET status = 'DELETED' WHERE id = ?1", [id])?;
            self.refresh_paid(doc)?;
            self.add_note("document", doc, "HISTORY", "Payment removed")?;
            self.audit("DELETE", "payment", Some(id), None, None)?;
            Ok(())
        })
    }

    /// Allocate a credit note to an invoice/bill of the same contact & currency.
    pub fn allocate_credit(&self, credit_id: i64, doc_id: i64, amount: &str, date: Option<&str>) -> Result<i64> {
        let amount = money::parse_fixed(amount, 2).filter(|a| *a > 0).ok_or_else(|| Error::Validation("amount must be greater than zero".into()))?;
        let cn: (String, String, i64, String, i64, i64, i64, i64, String) = self.conn.query_row(
            "SELECT type, status, contact_id, currency, total, amount_paid + amount_credited, base_total, rate_e8, number FROM documents WHERE id = ?1", [credit_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?)))?;
        let doc: (String, String, i64, String, i64, i64, i64, i64, String) = self.conn.query_row(
            "SELECT type, status, contact_id, currency, total, amount_paid + amount_credited, base_total, rate_e8, number FROM documents WHERE id = ?1", [doc_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?)))?;
        let ok_pair = (cn.0 == "ACCRECCREDIT" && doc.0 == "ACCREC") || (cn.0 == "ACCPAYCREDIT" && doc.0 == "ACCPAY");
        if !ok_pair { return invalid("credit notes can only be allocated to invoices/bills of the same side"); }
        if cn.1 != "AUTHORISED" || doc.1 != "AUTHORISED" { return invalid("both the credit note and the document must be approved and unpaid"); }
        if cn.3 != doc.3 { return invalid("credit note and document must be in the same currency"); }
        if amount > cn.4 - cn.5 { return invalid("amount exceeds the remaining credit"); }
        if amount > doc.4 - doc.5 { return invalid("amount exceeds the amount due"); }
        let date = date.map(String::from).unwrap_or_else(util::today);
        self.atomic(|| {
            // FX difference between the two documents' rates goes to realised FX gains.
            let cn_base = money::div_round(cn.6 as i128 * amount as i128, cn.4.max(1) as i128) as i64;
            let doc_base = money::div_round(doc.6 as i128 * amount as i128, doc.4.max(1) as i128) as i64;
            let mut jid = None;
            if cn_base != doc_base {
                let control = if cn.0 == "ACCRECCREDIT" { self.sys_account("AR")? } else { self.sys_account("AP")? };
                let s = if cn.0 == "ACCRECCREDIT" { 1 } else { -1 };
                // Sales: clear AR on invoice (credit doc_base), restore AR on credit (debit cn_base)
                let lines = vec![
                    JLine::new(control, s * cn_base).contact(Some(cn.2)).desc(format!("Credit {} applied", cn.8)),
                    JLine::new(control, -s * doc_base).contact(Some(doc.2)).desc(format!("Credit applied to {}", doc.8)),
                    JLine::new(self.sys_account("FXGAIN")?, s * (doc_base - cn_base)).desc("Realised FX on credit allocation"),
                ];
                jid = Some(self.post_journal(NewJournal { date: date.clone(), source_type: "CREDITALLOC".into(), source_id: Some(credit_id), reference: cn.8.clone(),
                    narration: "Credit note allocation".into(), lines, cash_basis: true, reverses: None })?);
            } else { self.check_lock(&date)?; }
            self.conn.execute("INSERT INTO credit_allocations(credit_note_id, document_id, amount, cn_base, doc_base, date, journal_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![credit_id, doc_id, amount, cn_base, doc_base, date, jid])?;
            let aid = self.conn.last_insert_rowid();
            self.refresh_paid(credit_id)?;
            self.refresh_paid(doc_id)?;
            self.add_note("document", doc_id, "HISTORY", &format!("Credit note {} applied for {}", cn.8, money::fmt_money(amount)))?;
            self.add_note("document", credit_id, "HISTORY", &format!("Allocated {} to {}", money::fmt_money(amount), doc.8))?;
            Ok(aid)
        })
    }

    pub fn delete_allocation(&self, id: i64) -> Result<()> {
        let (cn, doc, jid, date): (i64, i64, Option<i64>, String) = self.conn.query_row("SELECT credit_note_id, document_id, journal_id, date FROM credit_allocations WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        self.check_lock(&date)?;
        self.atomic(|| {
            if let Some(j) = jid { self.reverse_journal(j, None, None)?; }
            self.conn.execute("UPDATE credit_allocations SET status = 'DELETED' WHERE id = ?1", [id])?;
            self.refresh_paid(cn)?;
            self.refresh_paid(doc)?;
            Ok(())
        })
    }

    /// Pay several invoices or bills in one bank transaction.
    pub fn batch_payment(&self, account_id: i64, date: &str, reference: &str, items: &[(i64, String)]) -> Result<i64> {
        if items.is_empty() { return invalid("select at least one document"); }
        self.atomic(|| {
            let kind: String = self.conn.query_row("SELECT type FROM documents WHERE id = ?1", [items[0].0], |r| r.get(0))?;
            self.conn.execute("INSERT INTO batch_payments(type, account_id, date, reference) VALUES (?1, ?2, ?3, ?4)",
                params![if kind == "ACCREC" { "RECBATCH" } else { "PAYBATCH" }, account_id, date, reference])?;
            let bid = self.conn.last_insert_rowid();
            let mut total = 0;
            for (doc, amt) in items {
                let k: String = self.conn.query_row("SELECT type FROM documents WHERE id = ?1", [doc], |r| r.get(0))?;
                if k != kind { return invalid("a batch can only contain invoices or only bills"); }
                self.add_payment(&PaymentInput { document_id: *doc, account_id, date: date.into(), amount: amt.clone(), rate: None, reference: reference.into(), batch_id: Some(bid) })?;
                total += money::parse_fixed(amt, 2).unwrap_or(0);
            }
            self.conn.execute("UPDATE batch_payments SET total = ?2 WHERE id = ?1", params![bid, total])?;
            Ok(bid)
        })
    }

    pub fn list_batch_payments(&self) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT b.*, a.name AS account_name, (SELECT COUNT(*) FROM payments p WHERE p.batch_id = b.id AND p.status <> 'DELETED') AS payment_count
            FROM batch_payments b JOIN accounts a ON a.id = b.account_id ORDER BY b.date DESC", [])
    }

    pub fn batch_payment_ids(&self, batch_id: i64) -> Result<Vec<i64>> {
        let mut st = self.conn.prepare("SELECT id FROM payments WHERE batch_id = ?1 AND status <> 'DELETED' ORDER BY id")?;
        let ids = st.query_map([batch_id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<i64>>>()?;
        Ok(ids)
    }

    pub fn batch_payment_detail(&self, id: i64) -> Result<Value> {
        let mut b = util::query_one_json(&self.conn, "SELECT b.*, a.name AS account_name, a.bank_account_number FROM batch_payments b JOIN accounts a ON a.id = b.account_id WHERE b.id = ?1", [id])?;
        b["payments"] = json!(util::query_json(&self.conn,
            "SELECT p.*, d.number, d.type, c.name AS contact_name, c.bank_account, c.account_number FROM payments p JOIN documents d ON d.id = p.document_id JOIN contacts c ON c.id = d.contact_id
             WHERE p.batch_id = ?1 AND p.status <> 'DELETED'", [id])?);
        Ok(b)
    }
}
