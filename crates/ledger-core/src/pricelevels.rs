//! Price levels (QuickBooks Desktop): a named percentage that raises or lowers item prices for the
//! customers it's assigned to. Stored in basis points (−10% = −1000).

use crate::{error::invalid, util, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

impl Ledger {
    pub fn list_price_rules(&self) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT r.*, i.name AS item_name, p.name AS level_name FROM price_rules r LEFT JOIN items i ON i.id = r.item_id LEFT JOIN price_levels p ON p.id = r.price_level_id
             WHERE r.active = 1 ORDER BY r.name COLLATE NOCASE", [])
    }

    /// Create a price rule. `percent_bp` changes the item's price (negative = discount).
    pub fn save_price_rule(&self, name: &str, item: Option<i64>, level: Option<i64>, min_qty_e4: i64, percent_bp: i64, start: Option<&str>, end: Option<&str>) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() { return invalid("the price rule needs a name"); }
        if !(-10_000..=100_000).contains(&percent_bp) || percent_bp == 0 { return invalid("enter a percentage to raise or lower prices by, e.g. -10 or 5"); }
        if min_qty_e4 < 0 { return invalid("the minimum quantity can't be negative"); }
        for d in [start, end].into_iter().flatten() { util::parse_date(d)?; }
        if let (Some(s), Some(e)) = (start, end) { if e < s { return invalid("the end date is before the start date"); } }
        self.conn.execute("INSERT INTO price_rules(name, item_id, price_level_id, min_qty_e4, percent_bp, start_date, end_date) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![name, item, level, min_qty_e4, percent_bp, start, end])?;
        let id = self.conn.last_insert_rowid();
        self.audit("CREATE", "price_rule", Some(id), None, Some(json!({"name": name, "percent_bp": percent_bp})))?;
        Ok(id)
    }

    pub fn delete_price_rule(&self, id: i64) -> Result<()> {
        self.conn.execute("UPDATE price_rules SET active = 0 WHERE id = ?1", [id])?;
        self.audit("DELETE", "price_rule", Some(id), None, None)?;
        Ok(())
    }

    pub fn list_price_levels(&self) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT p.id, p.name, p.percent_bp, p.status, (SELECT COUNT(*) FROM contacts c WHERE c.price_level_id = p.id AND c.merged_into IS NULL) AS customers
             FROM price_levels p WHERE p.status = 'ACTIVE' ORDER BY p.name COLLATE NOCASE", [])
    }

    /// Create or update a price level. `percent_bp` is between −100% and +1000%.
    pub fn save_price_level(&self, id: Option<i64>, name: &str, percent_bp: i64) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() { return invalid("the price level needs a name"); }
        if !(-10_000..=100_000).contains(&percent_bp) || percent_bp == 0 { return invalid("enter a percentage to raise or lower prices by, e.g. -10 or 5"); }
        let clash: i64 = self.conn.query_row("SELECT COUNT(*) FROM price_levels WHERE name = ?1 COLLATE NOCASE AND status = 'ACTIVE' AND id IS NOT ?2", params![name, id], |r| r.get(0))?;
        if clash > 0 { return invalid("there's already a price level with that name"); }
        let id = match id {
            Some(id) => { self.conn.execute("UPDATE price_levels SET name = ?2, percent_bp = ?3 WHERE id = ?1", params![id, name, percent_bp])?; id }
            None => {
                // A deleted level with the same name is replaced.
                self.conn.execute("DELETE FROM price_levels WHERE name = ?1 COLLATE NOCASE AND status <> 'ACTIVE'", [name])?;
                self.conn.execute("INSERT INTO price_levels(name, percent_bp) VALUES (?1, ?2)", params![name, percent_bp])?;
                self.conn.last_insert_rowid()
            }
        };
        self.audit("UPDATE", "price_level", Some(id), None, Some(json!({"name": name, "percent_bp": percent_bp})))?;
        Ok(id)
    }

    /// Delete a price level; its customers go back to standard prices.
    pub fn delete_price_level(&self, id: i64) -> Result<()> {
        self.conn.execute("UPDATE contacts SET price_level_id = NULL WHERE price_level_id = ?1", [id])?;
        self.conn.execute("UPDATE price_levels SET status = 'DELETED' WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn set_contact_price_level(&self, contact_id: i64, level: Option<i64>) -> Result<()> {
        if let Some(l) = level {
            let ok: i64 = self.conn.query_row("SELECT COUNT(*) FROM price_levels WHERE id = ?1 AND status = 'ACTIVE'", [l], |r| r.get(0))?;
            if ok == 0 { return invalid("that price level doesn't exist"); }
        }
        self.conn.execute("UPDATE contacts SET price_level_id = ?2 WHERE id = ?1", params![contact_id, level])?;
        Ok(())
    }
}

/// The best (lowest-price) price rule for a sale: rules for this item or every item, for this
/// customer's price level or everyone, at or above their minimum quantity, within their dates.
pub fn rule_bp(rules: &[Value], item: i64, level: Option<i64>, qty_e4: i64, date: &str) -> Option<i64> {
    rules.iter().filter(|r| {
        r["item_id"].as_i64().is_none_or(|i| i == item) && r["price_level_id"].as_i64().is_none_or(|l| Some(l) == level)
            && qty_e4 >= r["min_qty_e4"].as_i64().unwrap_or(0)
            && r["start_date"].as_str().is_none_or(|s| date >= s) && r["end_date"].as_str().is_none_or(|e| date <= e)
    }).filter_map(|r| r["percent_bp"].as_i64()).min()
}

/// An item price (in e4 units) at a price level, rounded to the cent.
pub fn apply_level(price_e4: i64, percent_bp: i64) -> i64 {
    let v = price_e4 as i128 * (10_000 + percent_bp as i128);
    let cents = (v as f64 / 10_000.0 / 100.0).round() as i64;
    cents * 100
}

#[cfg(test)]
mod tests {
    use super::apply_level;
    #[test]
    fn level_math() {
        assert_eq!(apply_level(1_000_000, -1000), 900_000); // 100.00 −10% = 90.00
        assert_eq!(apply_level(333_300, 500), 350_000); // 33.33 +5% = 34.9965 → 35.00
    }
}
