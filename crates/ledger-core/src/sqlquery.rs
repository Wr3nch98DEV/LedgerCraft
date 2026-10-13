//! Read-only SQL over the company file for reporting (QuickBooks offers ODBC for this). Only
//! single read-only statements run, on a connection switched to query-only while they do; and
//! nothing that names passwords, tokens or secrets.

use crate::{error::invalid, Ledger, Result};
use serde_json::{json, Value};

pub const MAX_ROWS: usize = 1000;

impl Ledger {
    pub fn run_sql(&self, sql: &str) -> Result<Value> {
        if self.role != "ADVISER" { return invalid("only advisers can run SQL queries"); }
        let sql = sql.trim().trim_end_matches(';').trim();
        if sql.is_empty() { return invalid("type a SELECT query"); }
        let lower = sql.to_lowercase();
        if !(lower.starts_with("select") || lower.starts_with("with")) { return invalid("only SELECT queries can be run here"); }
        if ["password", "token", "secret", "totp", "api_key"].iter().any(|w| lower.contains(w)) { return invalid("queries can't read passwords, tokens or secrets"); }
        if sql.contains(';') { return invalid("run one query at a time"); }
        let mut stmt = self.conn.prepare(sql)?;
        if !stmt.readonly() { return invalid("only read-only queries can be run here"); }
        let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
        self.conn.execute_batch("PRAGMA query_only = ON;")?;
        let result = (|| -> Result<Vec<Vec<Value>>> {
            let n = columns.len();
            let mut rows = stmt.query([])?;
            let mut out = Vec::new();
            while let Some(r) = rows.next()? {
                if out.len() >= MAX_ROWS { break; }
                let mut row = Vec::with_capacity(n);
                for i in 0..n {
                    use rusqlite::types::ValueRef::*;
                    row.push(match r.get_ref(i)? { Null => Value::Null, Integer(v) => json!(v), Real(v) => json!(v), Text(t) => json!(String::from_utf8_lossy(t)), Blob(b) => json!(format!("<{} bytes>", b.len())) });
                }
                out.push(row);
            }
            Ok(out)
        })();
        self.conn.execute_batch("PRAGMA query_only = OFF;")?;
        let rows = result?;
        self.audit("QUERY", "sql", None, None, Some(json!({"sql": sql, "rows": rows.len()})))?;
        Ok(json!({"columns": columns, "rows": rows, "truncated": rows.len() >= MAX_ROWS}))
    }
}
