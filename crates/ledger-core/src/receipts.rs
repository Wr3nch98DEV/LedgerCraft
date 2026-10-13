//! Item receipts (QuickBooks Enhanced Inventory Receiving, Acumatica purchase receipts): stock
//! arrives before its bill. Receiving posts Dr Inventory, Cr Goods Received Not Invoiced (GRNI);
//! the bill entered later for the receipt posts Dr GRNI, Cr Accounts Payable, so stock isn't
//! counted twice.

use crate::{accounts::AccountInput, documents::{DocInput, LineInput}, error::invalid, journal::{JLine, NewJournal}, money, util, Ledger, Result};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

pub const GRNI: &str = "Goods Received Not Invoiced";

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ReceiptLine { pub item_id: i64, pub quantity: String, pub unit_price: String, pub lot: String }

impl Ledger {
    fn grni_account(&self) -> Result<i64> {
        if let Some(id) = self.conn.query_row("SELECT id FROM accounts WHERE name = ?1 COLLATE NOCASE AND status = 'ACTIVE'", [GRNI], |r| r.get(0)).optional()? { return Ok(id); }
        self.create_account(&AccountInput { name: GRNI.into(), kind: "CURRLIAB".into(), description: "Stock received but not yet billed".into(), ..Default::default() })
    }

    /// Receive stock from a supplier without a bill.
    pub fn receive_items(&self, contact: i64, date: &str, reference: &str, location: Option<i64>, lines: &[ReceiptLine]) -> Result<i64> {
        util::parse_date(date)?;
        let lines: Vec<&ReceiptLine> = lines.iter().filter(|l| l.item_id > 0).collect();
        if lines.is_empty() { return invalid("add the items received"); }
        let grni = self.grni_account()?;
        let mut parsed = Vec::new();
        for l in &lines {
            let (tracked, inv, _) = self.item_tracking(l.item_id)?;
            let code: String = self.conn.query_row("SELECT code FROM items WHERE id = ?1", [l.item_id], |r| r.get(0))?;
            if !tracked { return invalid(format!("{code} doesn't track quantity, so it can't be received into stock")); }
            let q = money::parse_fixed(&l.quantity, 4).filter(|q| *q > 0).ok_or_else(|| crate::Error::Validation(format!("{code}: enter the quantity received")))?;
            let u = money::parse_fixed(&l.unit_price, 4).filter(|u| *u >= 0).ok_or_else(|| crate::Error::Validation(format!("{code}: enter the unit cost")))?;
            let amount = money::div_round(q as i128 * u as i128, 1_000_000) as i64;
            parsed.push((l.item_id, inv.unwrap_or(self.sys_account("INVENTORY")?), q, u, amount, l.lot.trim().to_string(), code));
        }
        let total: i64 = parsed.iter().map(|p| p.4).sum();
        self.atomic(|| {
            let mut jl: Vec<JLine> = parsed.iter().map(|p| JLine::new(p.1, p.4).desc(format!("Received: {}", p.6)).contact(Some(contact))).collect();
            jl.push(JLine::new(grni, -total).desc("Items received, not yet billed").contact(Some(contact)));
            let j = self.post_journal(NewJournal { date: date.into(), source_type: "RECEIPT".into(), source_id: None, reference: reference.into(),
                narration: "Item receipt".into(), lines: jl, cash_basis: false, reverses: None })?;
            self.conn.execute("INSERT INTO item_receipts(contact_id, date, reference, location_id, total, journal_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![contact, date, reference, location, total, j])?;
            let id = self.conn.last_insert_rowid();
            for (item, _, q, u, amount, lot, code) in &parsed {
                self.conn.execute("INSERT INTO item_receipt_lines(receipt_id, item_id, description, qty_e4, unit_e2, amount, lot) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![id, item, code, q, u, amount, lot])?;
                for (lq, lc, l) in self.split_lots(*item, *q, *amount, lot)? { self.record_movement_lot(*item, date, lq, lc, "RECEIPT", Some(id), Some(j), location, &l)?; }
            }
            Ok(id)
        })
    }

    /// The bill for a receipt: a draft bill whose lines are against GRNI (the stock is already in).
    pub fn bill_receipt(&self, receipt: i64, date: Option<&str>) -> Result<i64> {
        let r = util::query_one_json(&self.conn, "SELECT * FROM item_receipts WHERE id = ?1", [receipt])?;
        if r.is_null() { return invalid("receipt not found"); }
        if r["status"] != "OPEN" { return invalid("this receipt was voided"); }
        if let Some(b) = r["bill_id"].as_i64() {
            let st: String = self.conn.query_row("SELECT status FROM documents WHERE id = ?1", [b], |x| x.get(0))?;
            if !matches!(st.as_str(), "DELETED" | "VOIDED") { return invalid("this receipt already has a bill"); }
        }
        let grni = self.grni_account()?;
        let lines: Vec<LineInput> = util::query_json(&self.conn, "SELECT l.*, i.name, i.purchase_tax_rate_id FROM item_receipt_lines l JOIN items i ON i.id = l.item_id WHERE l.receipt_id = ?1", [receipt])?
            .iter().map(|l| LineInput { description: format!("{} (received {})", l["name"].as_str().unwrap_or(""), r["date"].as_str().unwrap_or("")),
                quantity: Some(money::fmt_fixed(l["qty_e4"].as_i64().unwrap_or(0), 4)), unit_price: Some(money::fmt_fixed(l["unit_e2"].as_i64().unwrap_or(0), 4)),
                account_id: Some(grni), tax_rate_id: l["purchase_tax_rate_id"].as_i64(), ..Default::default() }).collect();
        let bill = self.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_id: r["contact_id"].as_i64(), reference: r["reference"].as_str().unwrap_or("").into(),
            date: date.map(String::from).unwrap_or_else(util::today), line_amount_type: Some("EXCLUSIVE".into()), lines, status: Some("DRAFT".into()), ..Default::default() })?;
        self.conn.execute("UPDATE item_receipts SET bill_id = ?2 WHERE id = ?1", params![receipt, bill])?;
        self.add_note("document", bill, "HISTORY", &format!("Bill for item receipt {receipt}"))?;
        Ok(bill)
    }

    /// Void an unbilled receipt: its journal and stock are reversed.
    pub fn void_receipt(&self, receipt: i64) -> Result<()> {
        let (status, bill, j, date): (String, Option<i64>, Option<i64>, String) = self.conn.query_row("SELECT status, bill_id, journal_id, date FROM item_receipts WHERE id = ?1", [receipt], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        if status != "OPEN" { return invalid("already voided"); }
        if let Some(b) = bill {
            let st: String = self.conn.query_row("SELECT status FROM documents WHERE id = ?1", [b], |x| x.get(0))?;
            if !matches!(st.as_str(), "DELETED" | "VOIDED") { return invalid("delete or void its bill first"); }
        }
        self.atomic(|| {
            if let Some(j) = j {
                let rid = self.reverse_journal(j, None, None)?;
                let moves: Vec<(i64, i64, i64, Option<i64>, String)> = {
                    let mut st = self.conn.prepare("SELECT item_id, qty_e4, cost, location_id, lot FROM inventory_movements WHERE source_type = 'RECEIPT' AND source_id = ?1")?;
                    st.query_map([receipt], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
                };
                for (item, q, c, loc, lot) in moves {
                    self.check_stock_at(item, loc, q)?;
                    self.record_movement_lot(item, &date, -q, -c, "REVERSAL", Some(receipt), Some(rid), loc, &lot)?;
                }
            }
            self.conn.execute("UPDATE item_receipts SET status = 'VOIDED' WHERE id = ?1", [receipt])?;
            Ok(())
        })
    }

    pub fn list_item_receipts(&self) -> Result<Vec<Value>> {
        let mut v = util::query_json(&self.conn,
            "SELECT r.*, c.name AS contact_name, d.number AS bill_number, d.status AS bill_status FROM item_receipts r JOIN contacts c ON c.id = r.contact_id
             LEFT JOIN documents d ON d.id = r.bill_id ORDER BY r.date DESC, r.id DESC LIMIT 500", [])?;
        for r in v.iter_mut() {
            let id = r["id"].as_i64().unwrap_or(0);
            r["lines"] = json!(util::query_json(&self.conn, "SELECT l.*, i.code FROM item_receipt_lines l JOIN items i ON i.id = l.item_id WHERE l.receipt_id = ?1", [id])?);
            let billed = r["bill_id"].is_i64() && !matches!(r["bill_status"].as_str(), Some("DELETED" | "VOIDED"));
            r["state"] = json!(if r["status"] == "VOIDED" { "Voided" } else if billed { "Billed" } else { "Not billed" });
        }
        Ok(v)
    }
}
