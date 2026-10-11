use crate::{util, Ledger, Result};
use rusqlite::params;
use serde_json::Value;

impl Ledger {
    pub fn index_search(&self, entity: &str, id: i64, title: &str, body: &str) -> Result<()> {
        self.conn.execute("DELETE FROM search_index WHERE entity = ?1 AND entity_id = ?2", params![entity, id])?;
        self.conn.execute("INSERT INTO search_index(entity, entity_id, title, body) VALUES (?1, ?2, ?3, ?4)", params![entity, id, title, body])?;
        Ok(())
    }

    pub fn unindex_search(&self, entity: &str, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM search_index WHERE entity = ?1 AND entity_id = ?2", params![entity, id])?;
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
