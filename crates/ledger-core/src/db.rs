use crate::Result;
use rusqlite::Connection;

/// Ordered list of migrations. Never edit a shipped migration; append a new one.
pub const MIGRATIONS: &[(&str, &str)] = &[
    ("v1_core", include_str!("schema_v1.sql")),
    ("v2_modules", include_str!("schema_v2.sql")),
    ("v3_payroll_projects", include_str!("schema_v3.sql")),
    ("v4_performance", include_str!("schema_v4.sql")),
    ("v5_quickbooks_workflows", include_str!("schema_v5.sql")),
    ("v6_tickoff_reconcile", include_str!("schema_v6.sql")),
    ("v7_account_detail_types", include_str!("schema_v7.sql")),
    ("v8_1099_tracking", include_str!("schema_v8.sql")),
    ("v9_sales_tax_agencies", include_str!("schema_v9.sql")),
    ("v10_price_levels", include_str!("schema_v10.sql")),
    ("v11_assemblies", include_str!("schema_v11.sql")),
    ("v12_credit_hold", include_str!("schema_v12.sql")),
    ("v13_price_rules", include_str!("schema_v13.sql")),
    ("v14_locations", include_str!("schema_v14.sql")),
    ("v15_lots", include_str!("schema_v15.sql")),
    ("v16_revenue_recognition", include_str!("schema_v16.sql")),
    ("v17_custom_fields", include_str!("schema_v17.sql")),
    ("v18_landed_costs", include_str!("schema_v18.sql")),
    ("v19_item_receipts", include_str!("schema_v19.sql")),
    ("v20_approval_rules", include_str!("schema_v20.sql")),
    ("v21_order_deposits", include_str!("schema_v21.sql")),
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
