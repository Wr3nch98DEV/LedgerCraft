//! Landed costs (QuickBooks Enterprise, Acumatica): a freight, duty or insurance bill's cost is
//! spread over the stock received on other bills, by value or by quantity, and added to those
//! items' cost (Dr Inventory, Cr the freight bill's expense accounts).

use crate::{error::invalid, journal::{JLine, NewJournal}, money, util, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

impl Ledger {
    /// Apply an approved bill's (non-stock) cost to the stock received on `receipts`.
    pub fn apply_landed_cost(&self, source_bill: i64, receipts: &[i64], method: &str) -> Result<i64> {
        if !matches!(method, "VALUE" | "QTY") { return invalid("spread the cost by VALUE or QTY"); }
        if receipts.is_empty() { return invalid("choose the bills for the stock this cost belongs to"); }
        let src = self.get_document(source_bill)?;
        if src["type"] != "ACCPAY" || !matches!(src["status"].as_str(), Some("AUTHORISED" | "PAID")) { return invalid("the landed cost must be an approved bill"); }
        let done: i64 = self.conn.query_row("SELECT COUNT(*) FROM landed_costs WHERE source_bill_id = ?1", [source_bill], |r| r.get(0))?;
        if done > 0 { return invalid("this bill's cost has already been added to stock"); }
        // The cost to spread: the bill's posted non-stock lines, by account (base currency).
        let jid = src["journal_id"].as_i64().ok_or_else(|| crate::Error::Validation("the bill isn't posted".into()))?;
        let inv_accounts: Vec<i64> = util::query_json(&self.conn, "SELECT DISTINCT inventory_account_id AS id FROM items WHERE inventory_account_id IS NOT NULL", [])?
            .iter().filter_map(|r| r["id"].as_i64()).collect();
        let sources: Vec<(i64, i64)> = {
            let mut st = self.conn.prepare("SELECT jl.account_id, SUM(jl.amount) FROM journal_lines jl JOIN accounts a ON a.id = jl.account_id
                WHERE jl.journal_id = ?1 AND a.class IN ('EXPENSE', 'ASSET') AND a.type <> 'BANK' AND jl.item_id IS NULL AND jl.amount > 0 GROUP BY jl.account_id")?;
            st.query_map([jid], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
        }.into_iter().filter(|(a, _)| !inv_accounts.contains(a)).collect();
        let amount: i64 = sources.iter().map(|s| s.1).sum();
        if amount <= 0 { return invalid("this bill has no cost to add (its lines are stock items or tax)"); }
        // The stock received on the chosen bills: their receipt movements.
        let mut targets: Vec<(i64, i64, i64, i64, i64)> = Vec::new(); // movement, bill, item, qty, cost
        for b in receipts {
            let mut st = self.conn.prepare("SELECT id, source_id, item_id, qty_e4, cost FROM inventory_movements WHERE source_type = 'ACCPAY' AND source_id = ?1 AND qty_e4 > 0")?;
            let rows = st.query_map([b], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            if rows.is_empty() { return invalid(format!("bill {} didn't bring in any stock", self.get_document(*b)?["number"].as_str().unwrap_or(""))); }
            targets.extend(rows);
        }
        let weight = |t: &(i64, i64, i64, i64, i64)| if method == "QTY" { t.3 } else { t.4 };
        let total_w: i64 = targets.iter().map(weight).sum();
        if total_w <= 0 { return invalid("the received stock has no value or quantity to spread by"); }
        let mut shares: Vec<i64> = targets.iter().map(|t| money::div_round(amount as i128 * weight(t) as i128, total_w as i128) as i64).collect();
        let diff = amount - shares.iter().sum::<i64>();
        if let Some(last) = shares.last_mut() { *last += diff; }
        let date = src["date"].as_str().unwrap_or("").to_string();
        let number = src["number"].as_str().unwrap_or("").to_string();
        self.atomic(|| {
            let mut lines: Vec<JLine> = Vec::new();
            for (t, share) in targets.iter().zip(&shares) {
                let (_, inv, _) = self.item_tracking(t.2)?;
                let inv = inv.unwrap_or(self.sys_account("INVENTORY")?);
                lines.push(JLine::new(inv, *share).desc(format!("Landed cost from bill {number}")));
            }
            for (acct, amt) in &sources { lines.push(JLine::new(*acct, -amt).desc(format!("Landed cost moved to stock: bill {number}"))); }
            let j = self.post_journal(NewJournal { date: date.clone(), source_type: "LANDEDCOST".into(), source_id: Some(source_bill), reference: number.clone(),
                narration: "Landed cost".into(), lines, cash_basis: false, reverses: None })?;
            self.conn.execute("INSERT INTO landed_costs(source_bill_id, method, amount, journal_id, date) VALUES (?1, ?2, ?3, ?4, ?5)", params![source_bill, method, amount, j, date])?;
            let lc = self.conn.last_insert_rowid();
            for (t, share) in targets.iter().zip(&shares) {
                // The cost joins the stock it came in with (so FIFO layers carry it too).
                self.conn.execute("UPDATE inventory_movements SET cost = cost + ?2 WHERE id = ?1", params![t.0, share])?;
                self.conn.execute("UPDATE items SET total_cost = total_cost + ?2 WHERE id = ?1", params![t.2, share])?;
                self.conn.execute("INSERT INTO landed_cost_lines(landed_cost_id, bill_id, item_id, movement_id, amount) VALUES (?1, ?2, ?3, ?4, ?5)", params![lc, t.1, t.2, t.0, share])?;
            }
            self.add_note("document", source_bill, "HISTORY", &format!("Added to stock as a landed cost ({})", money::fmt_money(amount)))?;
            Ok(lc)
        })
    }

    pub fn list_landed_costs(&self) -> Result<Vec<Value>> {
        let mut v = util::query_json(&self.conn,
            "SELECT l.*, d.number, c.name AS contact_name FROM landed_costs l JOIN documents d ON d.id = l.source_bill_id JOIN contacts c ON c.id = d.contact_id ORDER BY l.date DESC, l.id DESC", [])?;
        for l in v.iter_mut() {
            let id = l["id"].as_i64().unwrap_or(0);
            l["lines"] = json!(util::query_json(&self.conn, "SELECT x.*, i.code, i.name, d.number AS bill_number FROM landed_cost_lines x JOIN items i ON i.id = x.item_id JOIN documents d ON d.id = x.bill_id WHERE x.landed_cost_id = ?1", [id])?);
        }
        Ok(v)
    }
}
