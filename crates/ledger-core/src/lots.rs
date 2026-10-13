//! Lot and serial numbers (QuickBooks Enterprise, Sage, Acumatica). An item is tracked by lot
//! (a batch: one lot per line) or by serial number (one per unit). Stock in, stock out and
//! what's left are kept per lot/serial; lots can carry an expiry date.

use crate::{error::invalid, money, util, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

pub const MODES: [&str; 3] = ["NONE", "LOT", "SERIAL"];

/// Serial numbers as typed: separated by commas, spaces or new lines.
pub fn serials(text: &str) -> Vec<String> {
    text.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

impl Ledger {
    pub fn lot_tracking(&self, item: i64) -> Result<String> {
        Ok(self.conn.query_row("SELECT lot_tracking FROM items WHERE id = ?1", [item], |r| r.get(0))?)
    }

    pub fn set_lot_tracking(&self, item: i64, mode: &str) -> Result<()> {
        if !MODES.contains(&mode) { return invalid("track by NONE, LOT or SERIAL"); }
        let (tracked, _, _) = self.item_tracking(item)?;
        if !tracked && mode != "NONE" { return invalid("only items that track quantity can have lot or serial numbers"); }
        self.conn.execute("UPDATE items SET lot_tracking = ?2 WHERE id = ?1", params![item, mode])?;
        self.audit("UPDATE", "item", Some(item), None, Some(json!({"lot_tracking": mode})))?;
        Ok(())
    }

    /// Quantity on hand of one lot or serial number.
    pub fn lot_on_hand(&self, item: i64, lot: &str) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COALESCE(SUM(qty_e4), 0) FROM inventory_movements WHERE item_id = ?1 AND lot = ?2", params![item, lot], |r| r.get(0))?)
    }

    /// Splits a line's stock movement by lot/serial, checking the numbers: a lot is required, one
    /// serial per unit, no serial twice, and stock going out must be on hand in that lot/serial.
    pub(crate) fn split_lots(&self, item: i64, qty_e4: i64, cost: i64, lot: &str) -> Result<Vec<(i64, i64, String)>> {
        let mode = self.lot_tracking(item)?;
        let code: String = self.conn.query_row("SELECT code FROM items WHERE id = ?1", [item], |r| r.get(0))?;
        match mode.as_str() {
            "LOT" => {
                let lot = lot.trim();
                if lot.is_empty() { return invalid(format!("enter the lot number for {code}")); }
                if qty_e4 < 0 && self.lot_on_hand(item, lot)? < -qty_e4 { return invalid(format!("lot {lot} of {code} doesn't have that many on hand")); }
                Ok(vec![(qty_e4, cost, lot.to_string())])
            }
            "SERIAL" => {
                let list = serials(lot);
                if qty_e4 % 10_000 != 0 { return invalid(format!("{code} is tracked by serial number, so the quantity must be whole units")); }
                let units = (qty_e4.abs() / 10_000) as usize;
                if list.len() != units { return invalid(format!("enter {units} serial number(s) for {code} ({} entered)", list.len())); }
                let mut seen = std::collections::HashSet::new();
                let mut out = Vec::new();
                let each = money::div_round(cost as i128, units.max(1) as i128) as i64;
                for (n, s) in list.iter().enumerate() {
                    if !seen.insert(s.clone()) { return invalid(format!("serial number {s} is entered twice")); }
                    let on_hand = self.lot_on_hand(item, s)?;
                    if qty_e4 > 0 && on_hand > 0 { return invalid(format!("serial number {s} of {code} is already in stock")); }
                    if qty_e4 < 0 && on_hand < 10_000 { return invalid(format!("serial number {s} of {code} isn't in stock")); }
                    // The last serial takes any rounding so the costs add up.
                    let c = if n + 1 == units { cost - each * (units as i64 - 1) } else { each };
                    out.push((qty_e4.signum() * 10_000, c, s.clone()));
                }
                Ok(out)
            }
            _ => Ok(vec![(qty_e4, cost, String::new())]),
        }
    }

    /// Lots and serial numbers with stock on hand (or all, with `all`).
    pub fn list_lots(&self, all: bool) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT l.item_id, i.code, i.name, i.lot_tracking, l.lot, l.expiry, COALESCE((SELECT SUM(m.qty_e4) FROM inventory_movements m WHERE m.item_id = l.item_id AND m.lot = l.lot), 0) AS qty_e4
             FROM lots l JOIN items i ON i.id = l.item_id WHERE ?1 OR COALESCE((SELECT SUM(m.qty_e4) FROM inventory_movements m WHERE m.item_id = l.item_id AND m.lot = l.lot), 0) <> 0
             ORDER BY i.code, COALESCE(l.expiry, '9999'), l.lot", [all])
    }

    pub fn set_lot_expiry(&self, item: i64, lot: &str, expiry: Option<&str>) -> Result<()> {
        if let Some(d) = expiry { util::parse_date(d)?; }
        self.conn.execute("INSERT INTO lots(item_id, lot, expiry) VALUES (?1, ?2, ?3) ON CONFLICT(item_id, lot) DO UPDATE SET expiry = excluded.expiry", params![item, lot.trim(), expiry])?;
        Ok(())
    }

    /// Report: lots and serial numbers on hand, with expiry.
    pub fn lots_report(&self) -> Result<Value> {
        let rows: Vec<Value> = self.list_lots(false)?.into_iter().map(|l| {
            let label = format!("{} {} — {}{}", l["code"].as_str().unwrap_or(""), l["name"].as_str().unwrap_or(""), l["lot"].as_str().unwrap_or(""),
                l["expiry"].as_str().map(|e| format!(" (expires {})", crate::reports::fmt_long(e))).unwrap_or_default());
            json!({"label": label, "values": [l["qty_e4"]], "qty": true})
        }).collect();
        Ok(json!({"title": "Lots and Serial Numbers", "org": self.setting_or("org_name", "")?, "subtitle": "On hand", "columns": ["Quantity"], "qty": true,
            "sections": [{"title": "", "rows": rows}]}))
    }
}
