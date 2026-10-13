//! Inventory locations (QuickBooks Advanced Inventory sites, Acumatica warehouses, Sage stock
//! locations). Every stock movement belongs to a location; no location means the main one.
//! Quantities are tracked per location, cost is one average per item.

use crate::{error::invalid, money, util, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

/// The name shown for stock with no location.
pub const MAIN: &str = "Main location";

impl Ledger {
    pub fn list_locations(&self) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT id, name, address FROM locations WHERE status = 'ACTIVE' ORDER BY name COLLATE NOCASE", [])
    }

    pub fn save_location(&self, id: Option<i64>, name: &str, address: &str) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() { return invalid("the location needs a name"); }
        if name.eq_ignore_ascii_case(MAIN) { return invalid(format!("\"{MAIN}\" is already the name for stock with no location")); }
        let clash: i64 = self.conn.query_row("SELECT COUNT(*) FROM locations WHERE name = ?1 COLLATE NOCASE AND id IS NOT ?2", params![name, id], |r| r.get(0))?;
        if clash > 0 { return invalid("there's already a location with that name"); }
        let id = match id {
            Some(id) => { self.conn.execute("UPDATE locations SET name = ?2, address = ?3 WHERE id = ?1", params![id, name, address])?; id }
            None => { self.conn.execute("INSERT INTO locations(name, address) VALUES (?1, ?2)", params![name, address])?; self.conn.last_insert_rowid() }
        };
        self.audit("UPDATE", "location", Some(id), None, Some(json!({"name": name})))?;
        Ok(id)
    }

    /// A location can be removed once it holds no stock.
    pub fn delete_location(&self, id: i64) -> Result<()> {
        let held: i64 = self.conn.query_row("SELECT COUNT(*) FROM (SELECT item_id FROM inventory_movements WHERE location_id = ?1 GROUP BY item_id HAVING SUM(qty_e4) <> 0)", [id], |r| r.get(0))?;
        if held > 0 { return invalid("move the stock out of this location before removing it"); }
        self.conn.execute("UPDATE locations SET status = 'DELETED' WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Quantity of an item at a location (None = the main location).
    pub fn stock_at(&self, item: i64, location: Option<i64>) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COALESCE(SUM(qty_e4), 0) FROM inventory_movements WHERE item_id = ?1 AND location_id IS ?2", params![item, location], |r| r.get(0))?)
    }

    /// Refuses to take more of an item out of a location than it holds there.
    pub fn check_stock_at(&self, item: i64, location: Option<i64>, qty_e4: i64) -> Result<()> {
        let has_locations: i64 = self.conn.query_row("SELECT COUNT(*) FROM locations WHERE status = 'ACTIVE'", [], |r| r.get(0))?;
        if has_locations == 0 { return Ok(()); }
        let at = self.stock_at(item, location)?;
        if qty_e4 > at {
            let code: String = self.conn.query_row("SELECT code FROM items WHERE id = ?1", [item], |r| r.get(0))?;
            let place = match location { Some(l) => self.conn.query_row("SELECT name FROM locations WHERE id = ?1", [l], |r| r.get::<_, String>(0))?, None => MAIN.to_string() };
            return invalid(format!("not enough stock of {code} at {place}: {} there", qty_text(at)));
        }
        Ok(())
    }

    /// Move stock between locations. Cost moves with it (at the average), so totals don't change
    /// and nothing is posted to the ledger.
    pub fn transfer_stock(&self, item: i64, from: Option<i64>, to: Option<i64>, qty_e4: i64, date: &str) -> Result<()> {
        if from == to { return invalid("choose two different locations"); }
        if qty_e4 <= 0 { return invalid("enter a quantity to move"); }
        util::parse_date(date)?;
        let (tracked, _, _) = self.item_tracking(item)?;
        if !tracked { return invalid("only tracked stock items have locations"); }
        if self.lot_tracking(item)? != "NONE" { return invalid("moving lot- or serial-tracked stock between locations isn't supported yet"); }
        let at = self.stock_at(item, from)?;
        if qty_e4 > at { return invalid(format!("only {} there to move", qty_text(at))); }
        let cost = self.inventory_cost_out(item, qty_e4)?;
        self.atomic(|| {
            self.record_movement_at(item, date, -qty_e4, -cost, "TRANSFER", None, None, from)?;
            self.record_movement_at(item, date, qty_e4, cost, "TRANSFER", None, None, to)?;
            self.audit("UPDATE", "item", Some(item), None, Some(json!({"transfer": {"from": from, "to": to, "qty_e4": qty_e4}})))?;
            Ok(())
        })
    }

    /// Stock on hand by item and location.
    pub fn stock_by_location(&self) -> Result<Value> {
        let locs = self.list_locations()?;
        let rows = util::query_json(&self.conn,
            "SELECT i.id, i.code, i.name, m.location_id, SUM(m.qty_e4) AS qty_e4 FROM inventory_movements m JOIN items i ON i.id = m.item_id
             WHERE i.is_tracked = 1 GROUP BY i.id, m.location_id ORDER BY i.code", [])?;
        Ok(json!({"locations": locs, "rows": rows}))
    }

    /// Report: stock on hand per item, a column per location.
    pub fn stock_by_location_report(&self) -> Result<Value> {
        let s = self.stock_by_location()?;
        let locs = s["locations"].as_array().cloned().unwrap_or_default();
        let mut cols: Vec<String> = vec![MAIN.into()];
        cols.extend(locs.iter().map(|l| l["name"].as_str().unwrap_or("").to_string()));
        cols.push("Total".into());
        let mut rows: Vec<Value> = Vec::new();
        let mut cur: Option<(i64, String, Vec<i64>)> = None;
        let idx = |loc: Option<i64>| -> usize { loc.and_then(|l| locs.iter().position(|x| x["id"].as_i64() == Some(l))).map(|p| p + 1).unwrap_or(0) };
        let width = cols.len();
        let push = |c: Option<(i64, String, Vec<i64>)>, rows: &mut Vec<Value>| if let Some((_, label, v)) = c { rows.push(json!({"label": label, "values": v, "qty": true})); };
        for r in s["rows"].as_array().cloned().unwrap_or_default() {
            let id = r["id"].as_i64().unwrap_or(0);
            if cur.as_ref().map(|c| c.0) != Some(id) {
                push(cur.take(), &mut rows);
                cur = Some((id, format!("{} {}", r["code"].as_str().unwrap_or(""), r["name"].as_str().unwrap_or("")).trim().to_string(), vec![0; width]));
            }
            if let Some(c) = cur.as_mut() {
                let q = r["qty_e4"].as_i64().unwrap_or(0);
                c.2[idx(r["location_id"].as_i64())] += q;
                c.2[width - 1] += q;
            }
        }
        push(cur.take(), &mut rows);
        Ok(json!({"title": "Stock by Location", "org": self.setting_or("org_name", "")?, "subtitle": "Quantity on hand", "columns": cols, "qty": true,
            "sections": [{"title": "", "rows": rows}]}))
    }
}

fn qty_text(q: i64) -> String { money::fmt_fixed(q, 4).trim_end_matches('0').trim_end_matches('.').to_string() }
