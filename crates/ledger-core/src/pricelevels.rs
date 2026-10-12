//! Price levels (QuickBooks Desktop): a named percentage that raises or lowers item prices for the
//! customers it's assigned to. Stored in basis points (−10% = −1000).

use crate::{error::invalid, util, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

impl Ledger {
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
