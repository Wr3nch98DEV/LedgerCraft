use crate::{util, Ledger, Result};
use rusqlite::params;
use serde_json::Value;

impl Ledger {
    pub fn audit(&self, action: &str, entity: &str, entity_id: Option<i64>, before: Option<Value>, after: Option<Value>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO audit_log(user_id, action, entity, entity_id, before, after) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![self.user_id, action, entity, entity_id, before.map(|v| v.to_string()), after.map(|v| v.to_string())],
        )?;
        Ok(())
    }

    /// Add a history entry or user note to any object (shown in "History & Notes").
    pub fn add_note(&self, entity: &str, entity_id: i64, kind: &str, text: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO notes(entity, entity_id, user_id, kind, text) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![entity, entity_id, self.user_id, kind, text],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn history(&self, entity: &str, entity_id: i64) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT n.id, n.at, n.kind, n.text, u.name AS user FROM notes n LEFT JOIN users u ON u.id = n.user_id
             WHERE n.entity = ?1 AND n.entity_id = ?2 ORDER BY n.at DESC, n.id DESC", params![entity, entity_id])
    }

    pub fn audit_log(&self, limit: i64, entity: Option<&str>) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT a.id, a.at, a.action, a.entity, a.entity_id, a.before, a.after, u.name AS user
             FROM audit_log a LEFT JOIN users u ON u.id = a.user_id
             WHERE (?2 IS NULL OR a.entity = ?2) ORDER BY a.id DESC LIMIT ?1", params![limit, entity])
    }
}
