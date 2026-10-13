use crate::{util, Ledger, Result};
use chrono::Datelike;
use rusqlite::{params, OptionalExtension};
use serde_json::{Map, Value};

/// Keys an org may set via the settings API (others are internal).
pub const PUBLIC_KEYS: &[&str] = &[
    "org_name", "legal_name", "country", "base_currency", "fy_end_month", "fy_end_day", "tax_basis",
    "tax_number", "tax_registered", "tax_period", "tax_label", "address", "phone", "email", "website",
    "lock_date_all", "lock_date_others", "conversion_date", "industry", "timezone",
    "sales_terms_type", "sales_terms_days", "bill_terms_type", "bill_terms_days",
    "default_sales_lat", "default_purchases_lat", "smtp_host", "smtp_port", "smtp_user", "smtp_from",
    "smtp_tls", "company_number", "invoice_reminders_enabled", "payroll_jurisdiction",
    "payroll_brackets", "payroll_retirement_bp", "ocr_command", "public_url", "inventory_costing",
];

impl Ledger {
    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn setting_or(&self, key: &str, default: &str) -> Result<String> {
        Ok(self.setting(key)?.unwrap_or_else(|| default.to_string()))
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let before = self.setting(key)?;
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        if before.as_deref() != Some(value) {
            self.audit("UPDATE", "setting", None, before.map(Value::from), Some(Value::from(format!("{key}={value}"))))?;
        }
        Ok(())
    }

    pub fn settings_json(&self) -> Result<Value> {
        let mut m = Map::new();
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (k, v) = row?;
            if k.starts_with("smtp_pass") { continue; }
            m.insert(k, Value::from(v));
        }
        Ok(Value::Object(m))
    }

    pub fn update_settings(&self, v: &Value) -> Result<()> {
        let obj = v.as_object().ok_or_else(|| crate::Error::Validation("expected object".into()))?;
        self.atomic(|| {
            for (k, val) in obj {
                if !PUBLIC_KEYS.contains(&k.as_str()) && k != "smtp_pass" { continue; }
                let s = match val { Value::String(s) => s.clone(), Value::Null => String::new(), other => other.to_string() };
                if k.starts_with("lock_date") && !s.is_empty() { util::parse_date(&s)?; }
                if k.starts_with("lock_date") && !self.is_adviser() {
                    return Err(crate::Error::Forbidden("only advisers can change lock dates".into()));
                }
                self.set_setting(k, &s)?;
            }
            Ok(())
        })
    }

    pub fn base_currency(&self) -> Result<String> { self.setting_or("base_currency", "AUD") }

    /// Start date of the financial year containing `date`.
    pub fn fy_start(&self, date: &str) -> Result<String> {
        let m: u32 = self.setting_or("fy_end_month", "6")?.parse().unwrap_or(6);
        let d = util::parse_date(date)?;
        // FY ends on last day of month m. Start = day after previous end.
        let start_month = if m == 12 { 1 } else { m + 1 };
        let y = if d.month() >= start_month { d.year() } else { d.year() - 1 };
        Ok(format!("{y:04}-{start_month:02}-01"))
    }

    pub fn fy_end(&self, date: &str) -> Result<String> {
        let start = util::parse_date(&self.fy_start(date)?)?;
        let end = util::add_months(start, 12) - chrono::Duration::days(1);
        Ok(util::fmt_date(end))
    }

    pub fn is_adviser(&self) -> bool { self.role == "ADVISER" }

    /// Check a date against lock dates for the current user.
    pub fn check_lock(&self, date: &str) -> Result<()> {
        if let Some(all) = self.setting("lock_date_all")?.filter(|s| !s.is_empty()) {
            if date <= all.as_str() {
                return Err(crate::Error::Locked(format!("The date {date} is on or before the lock date ({all}) for all users")));
            }
        }
        if !self.is_adviser() {
            if let Some(o) = self.setting("lock_date_others")?.filter(|s| !s.is_empty()) {
                if date <= o.as_str() {
                    return Err(crate::Error::Locked(format!("The date {date} is on or before the lock date ({o}); only advisers can make changes")));
                }
            }
        }
        Ok(())
    }

    /// Next number from a named sequence (INV, BILL, CN, QU, PO, MJ ...).
    pub fn next_number(&self, name: &str) -> Result<String> {
        let row: Option<(String, i64, i64)> = self.conn.query_row(
            "SELECT prefix, next, width FROM sequences WHERE name = ?1", [name], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).optional()?;
        let (prefix, next, width) = match row {
            Some(r) => r,
            None => {
                self.conn.execute("INSERT INTO sequences(name, prefix, next, width) VALUES (?1, ?2, 1, 4)", params![name, format!("{name}-")])?;
                (format!("{name}-"), 1, 4)
            }
        };
        self.conn.execute("UPDATE sequences SET next = next + 1 WHERE name = ?1", [name])?;
        Ok(format!("{prefix}{next:0width$}", width = width as usize))
    }

    /// Document numbering (Settings → Invoice settings): prefix and next number per sequence.
    pub fn list_sequences(&self) -> Result<Vec<Value>> {
        for (name, prefix) in [("INV", "INV-"), ("CN", "CN-"), ("QU", "QU-"), ("PO", "PO-")] {
            self.conn.execute("INSERT OR IGNORE INTO sequences(name, prefix, next, width) VALUES (?1, ?2, 1, 4)", params![name, prefix])?;
        }
        crate::util::query_json(&self.conn, "SELECT name, prefix, next, width FROM sequences ORDER BY CASE name WHEN 'INV' THEN 0 WHEN 'CN' THEN 1 WHEN 'QU' THEN 2 WHEN 'PO' THEN 3 ELSE 9 END, name", [])
    }

    pub fn set_sequence(&self, name: &str, prefix: &str, next: i64) -> Result<()> {
        if next < 1 { return crate::error::invalid("the next number must be 1 or more"); }
        if prefix.chars().count() > 20 { return crate::error::invalid("the prefix can be at most 20 characters"); }
        let n = self.conn.execute("UPDATE sequences SET prefix = ?2, next = ?3 WHERE name = ?1", params![name, prefix, next])?;
        if n == 0 { return crate::error::invalid("unknown number sequence"); }
        self.audit("UPDATE", "sequence", None, None, Some(serde_json::json!({"name": name, "prefix": prefix, "next": next})))
    }
}
