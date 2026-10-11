use crate::Result;
use rusqlite::Connection;

/// Ordered list of migrations. Never edit a shipped migration; append a new one.
pub const MIGRATIONS: &[(&str, &str)] = &[
    ("v1_core", include_str!("schema_v1.sql")),
    ("v2_modules", include_str!("schema_v2.sql")),
];

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations (name TEXT PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')))")?;
    for (name, sql) in MIGRATIONS {
        let done: bool = conn.query_row("SELECT COUNT(*) FROM schema_migrations WHERE name = ?1", [name], |r| r.get::<_, i64>(0))? > 0;
        if done { continue; }
        conn.execute_batch("BEGIN")?;
        let r = conn.execute_batch(sql).and_then(|_| conn.execute("INSERT INTO schema_migrations(name) VALUES (?1)", [name]).map(|_| ()));
        match r {
            Ok(_) => conn.execute_batch("COMMIT")?,
            Err(e) => { let _ = conn.execute_batch("ROLLBACK"); return Err(e.into()); }
        }
    }
    Ok(())
}

pub fn schema_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))?)
}
