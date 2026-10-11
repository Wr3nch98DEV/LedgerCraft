use crate::{util, Ledger, Result};
use rusqlite::params;
use serde_json::Value;

impl Ledger {
    pub fn index_search(&self, entity: &str, id: i64, title: &str, body: &str) -> Result<()> {
        self.unindex_search(entity, id)?;
        self.conn.execute("INSERT INTO search_index(entity, entity_id, title, body) VALUES (?1, ?2, ?3, ?4)", params![entity, id, title, body])?;
        let rowid = self.conn.last_insert_rowid();
        self.conn.execute("INSERT OR REPLACE INTO search_keys(entity, entity_id, fts_rowid) VALUES (?1, ?2, ?3)", params![entity, id, rowid])?;
        Ok(())
    }

    pub fn unindex_search(&self, entity: &str, id: i64) -> Result<()> {
        let rowid: Option<i64> = self.conn.query_row("SELECT fts_rowid FROM search_keys WHERE entity = ?1 AND entity_id = ?2", params![entity, id], |r| r.get(0)).ok();
        if let Some(r) = rowid {
            self.conn.execute("DELETE FROM search_index WHERE rowid = ?1", [r])?;
            self.conn.execute("DELETE FROM search_keys WHERE entity = ?1 AND entity_id = ?2", params![entity, id])?;
        }
        Ok(())
    }

    /// Global search. Each whitespace-separated term is prefix-matched.
    pub fn search(&self, q: &str, limit: i64) -> Result<Vec<Value>> {
        let terms: Vec<String> = q.split_whitespace()
            .map(|t| t.chars().filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-').collect::<String>())
            .filter(|t| !t.is_empty())
            .map(|t| format!("\"{}\"*", t.replace('"', "")))
            .collect();
        if terms.is_empty() { return Ok(vec![]); }
        let fts = terms.join(" ");
        util::query_json(&self.conn,
            "SELECT entity, entity_id AS id, title, snippet(search_index, 3, '[', ']', '…', 8) AS snippet
             FROM search_index WHERE search_index MATCH ?1 ORDER BY rank LIMIT ?2", params![fts, limit])
    }
}
