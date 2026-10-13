//! Custom fields: extra named fields on customers & vendors, items and employees.

use crate::{error::invalid, util, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

pub const ENTITIES: [&str; 3] = ["contact", "item", "employee"];
pub const KINDS: [&str; 4] = ["TEXT", "NUMBER", "DATE", "YESNO"];

impl Ledger {
    pub fn list_custom_fields(&self, entity: Option<&str>) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT id, entity, name, kind, sort FROM custom_fields WHERE active = 1 AND (?1 IS NULL OR entity = ?1) ORDER BY entity, sort, id", [entity])
    }

    pub fn save_custom_field(&self, id: Option<i64>, entity: &str, name: &str, kind: &str) -> Result<i64> {
        let name = name.trim();
        if !ENTITIES.contains(&entity) { return invalid("custom fields go on customers & vendors, items or employees"); }
        if !KINDS.contains(&kind) { return invalid("a custom field is text, a number, a date or yes/no"); }
        if name.is_empty() { return invalid("the custom field needs a name"); }
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM custom_fields WHERE entity = ?1 AND active = 1 AND id IS NOT ?2", params![entity, id], |r| r.get(0))?;
        if id.is_none() && count >= 30 { return invalid("there can be up to 30 custom fields of each kind"); }
        let clash: i64 = self.conn.query_row("SELECT COUNT(*) FROM custom_fields WHERE entity = ?1 AND name = ?2 COLLATE NOCASE AND active = 1 AND id IS NOT ?3", params![entity, name, id], |r| r.get(0))?;
        if clash > 0 { return invalid("there's already a custom field with that name"); }
        let id = match id {
            Some(id) => { self.conn.execute("UPDATE custom_fields SET name = ?2, kind = ?3 WHERE id = ?1", params![id, name, kind])?; id }
            None => {
                // A removed field with the same name comes back (with its old values).
                if let Ok(old) = self.conn.query_row("SELECT id FROM custom_fields WHERE entity = ?1 AND name = ?2 COLLATE NOCASE AND active = 0", params![entity, name], |r| r.get::<_, i64>(0)) {
                    self.conn.execute("UPDATE custom_fields SET active = 1, kind = ?2 WHERE id = ?1", params![old, kind])?;
                    old
                } else {
                    self.conn.execute("INSERT INTO custom_fields(entity, name, kind, sort) VALUES (?1, ?2, ?3, ?4)", params![entity, name, kind, count])?;
                    self.conn.last_insert_rowid()
                }
            }
        };
        self.audit("UPDATE", "custom_field", Some(id), None, Some(json!({"entity": entity, "name": name, "kind": kind})))?;
        Ok(id)
    }

    pub fn delete_custom_field(&self, id: i64) -> Result<()> {
        self.conn.execute("UPDATE custom_fields SET active = 0 WHERE id = ?1", [id])?;
        Ok(())
    }

    /// A record's custom fields with their values.
    pub fn custom_values(&self, entity: &str, entity_id: i64) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT f.id, f.name, f.kind, COALESCE(v.value, '') AS value FROM custom_fields f LEFT JOIN custom_values v ON v.field_id = f.id AND v.entity_id = ?2
            WHERE f.entity = ?1 AND f.active = 1 ORDER BY f.sort, f.id", params![entity, entity_id])
    }

    /// Set a record's custom field values (field id → value as text). Values are checked by kind.
    pub fn set_custom_values(&self, entity: &str, entity_id: i64, values: &serde_json::Map<String, Value>) -> Result<()> {
        self.atomic(|| {
            for (k, v) in values {
                let fid: i64 = k.parse().map_err(|_| crate::Error::Validation("unknown custom field".into()))?;
                let (fe, kind, name): (String, String, String) = self.conn.query_row("SELECT entity, kind, name FROM custom_fields WHERE id = ?1", [fid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
                if fe != entity { return invalid("that custom field belongs to another kind of record"); }
                let text = v.as_str().map(str::trim).unwrap_or("").to_string();
                if !text.is_empty() {
                    match kind.as_str() {
                        "NUMBER" if text.replace(',', "").parse::<f64>().is_err() => return invalid(format!("{name} must be a number")),
                        "DATE" => { util::parse_date(&text)?; }
                        "YESNO" if !matches!(text.as_str(), "Yes" | "No") => return invalid(format!("{name} must be Yes or No")),
                        _ => {}
                    }
                }
                self.conn.execute("INSERT INTO custom_values(field_id, entity_id, value) VALUES (?1, ?2, ?3) ON CONFLICT(field_id, entity_id) DO UPDATE SET value = excluded.value",
                    params![fid, entity_id, text])?;
            }
            self.audit("UPDATE", entity, Some(entity_id), None, Some(json!({"custom_fields": values})))?;
            Ok(())
        })
    }
}
