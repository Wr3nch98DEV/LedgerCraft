#![allow(deprecated)]
use crate::{error::invalid, util, Error, Ledger, Result};
use argon2::{password_hash::{PasswordHash, PasswordHasher, PasswordVerifier}, Argon2};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};

pub const AREAS: &[&str] = &["settings", "users", "accounting", "sales", "purchases", "banking", "reports", "contacts", "payroll", "inventory", "projects", "expenses", "assets"];

/// Permissions for a role. Each entry is "area:read" or "area:write".
pub fn role_permissions(role: &str, custom: Option<&str>) -> Vec<String> {
    let all_rw = || AREAS.iter().flat_map(|a| [format!("{a}:read"), format!("{a}:write")]).collect::<Vec<_>>();
    let rw = |areas: &[&str]| areas.iter().flat_map(|a| [format!("{a}:read"), format!("{a}:write")]).collect::<Vec<_>>();
    match role {
        "ADVISER" => { let mut v = all_rw(); v.push("lockdates:write".into()); v }
        "STANDARD" => all_rw().into_iter().filter(|p| !p.starts_with("users:write") && !p.starts_with("payroll")).collect(),
        "INVOICE_ONLY" => { let mut v = rw(&["sales", "contacts"]); v.push("inventory:read".into()); v }
        "READONLY" => AREAS.iter().filter(|a| **a != "payroll" && **a != "users").map(|a| format!("{a}:read")).collect(),
        "CASHBOOK" => { let mut v = rw(&["banking", "contacts", "expenses"]); v.extend(["reports:read".into(), "accounting:read".into()]); v }
        "PAYROLL_ADMIN" => { let mut v = rw(&["payroll"]); v.extend(["reports:read".into(), "contacts:read".into()]); v }
        "CUSTOM" => custom.and_then(|c| serde_json::from_str::<Vec<String>>(c).ok()).unwrap_or_default(),
        _ => vec![],
    }
}

#[derive(Debug, Deserialize)]
pub struct UserInput {
    pub name: String,
    pub email: String,
    #[serde(default = "default_role")]
    pub role: String,
    #[serde(default)]
    pub permissions: Option<Vec<String>>,
    #[serde(default)]
    pub password: Option<String>,
}
fn default_role() -> String { "STANDARD".into() }

pub fn hash_password(pw: &str) -> Result<String> {
    let salt = rand_bytes(16);
    Argon2::default().hash_password_with_salt(pw.as_bytes(), &salt).map(|h| h.to_string()).map_err(|e| Error::Validation(e.to_string()))
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    PasswordHash::new(hash).map(|h| Argon2::default().verify_password(pw.as_bytes(), &h).is_ok()).unwrap_or(false)
}

fn rand_bytes(n: usize) -> Vec<u8> {
    use rand::RngExt;
    let mut rng = rand::rng();
    (0..n).map(|_| rng.random::<u8>()).collect()
}

impl Ledger {
    pub fn list_users(&self) -> Result<Vec<Value>> {
        let mut users = util::query_json(&self.conn, "SELECT id, name, email, role, permissions, active, created_at, password_hash IS NOT NULL AS has_password, totp_secret IS NOT NULL AS has_2fa FROM users ORDER BY id", [])?;
        for u in users.iter_mut() {
            let perms = role_permissions(u["role"].as_str().unwrap_or(""), u["permissions"].as_str());
            u["effective_permissions"] = json!(perms);
        }
        Ok(users)
    }

    pub fn create_user(&self, u: &UserInput) -> Result<i64> {
        if u.name.trim().is_empty() || !u.email.contains('@') { return invalid("name and a valid email are required"); }
        if role_permissions(&u.role, Some("[]")).is_empty() && u.role != "CUSTOM" { return invalid("unknown role"); }
        let hash = match &u.password { Some(p) if !p.is_empty() => { check_password_strength(p)?; Some(hash_password(p)?) } _ => None };
        self.conn.execute("INSERT INTO users(name, email, role, permissions, password_hash) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![u.name.trim(), u.email.trim().to_lowercase(), u.role, u.permissions.as_ref().map(|p| json!(p).to_string()), hash])?;
        let id = self.conn.last_insert_rowid();
        self.audit("CREATE", "user", Some(id), None, Some(json!({"name": u.name, "email": u.email, "role": u.role})))?;
        Ok(id)
    }

    pub fn update_user(&self, id: i64, u: &UserInput) -> Result<()> {
        if id == 1 && u.role != "ADVISER" { return invalid("the organisation owner must remain an adviser"); }
        self.conn.execute("UPDATE users SET name = ?2, email = ?3, role = ?4, permissions = ?5 WHERE id = ?1",
            params![id, u.name.trim(), u.email.trim().to_lowercase(), u.role, u.permissions.as_ref().map(|p| json!(p).to_string())])?;
        if let Some(p) = u.password.as_ref().filter(|p| !p.is_empty()) { self.set_password(id, Some(p))?; }
        self.audit("UPDATE", "user", Some(id), None, Some(json!({"name": u.name, "role": u.role})))?;
        Ok(())
    }

    pub fn set_user_active(&self, id: i64, active: bool) -> Result<()> {
        if id == 1 && !active { return invalid("the owner cannot be deactivated"); }
        self.conn.execute("UPDATE users SET active = ?2 WHERE id = ?1", params![id, active as i64])?;
        Ok(())
    }

    pub fn set_password(&self, id: i64, pw: Option<&str>) -> Result<()> {
        let hash = match pw { Some(p) if !p.is_empty() => { check_password_strength(p)?; Some(hash_password(p)?) } _ => None };
        self.conn.execute("UPDATE users SET password_hash = ?2 WHERE id = ?1", params![id, hash])?;
        self.audit("PASSWORD", "user", Some(id), None, None)?;
        Ok(())
    }

    /// Returns (user_id, role, permissions) when credentials are valid.
    pub fn authenticate(&self, email: &str, pw: &str) -> Result<(i64, String, Vec<String>)> {
        let row: (i64, String, Option<String>, Option<String>, i64) = self.conn.query_row(
            "SELECT id, role, permissions, password_hash, active FROM users WHERE email = ?1", [email.trim().to_lowercase()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .map_err(|_| Error::Forbidden("invalid email or password".into()))?;
        if row.4 == 0 { return Err(Error::Forbidden("user is inactive".into())); }
        match &row.3 {
            Some(h) if verify_password(pw, h) => {}
            None => {}
            _ => return Err(Error::Forbidden("invalid email or password".into())),
        }
        Ok((row.0, row.1.clone(), role_permissions(&row.1, row.2.as_deref())))
    }

    pub fn user_permissions(&self, id: i64) -> Result<(String, Vec<String>)> {
        let (role, perms): (String, Option<String>) = self.conn.query_row("SELECT role, permissions FROM users WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let p = role_permissions(&role, perms.as_deref());
        Ok((role, p))
    }

    pub fn any_password_set(&self) -> Result<bool> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM users WHERE password_hash IS NOT NULL AND active = 1", [], |r| r.get::<_, i64>(0))? > 0)
    }
}

fn check_password_strength(p: &str) -> Result<()> {
    if p.len() < 8 { return invalid("password must be at least 8 characters"); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hashing() {
        let h = hash_password("correct horse").unwrap();
        assert!(h.starts_with("$argon2"));
        assert!(verify_password("correct horse", &h));
        assert!(!verify_password("wrong", &h));
    }
    #[test]
    fn roles() {
        assert!(role_permissions("READONLY", None).iter().all(|p| p.ends_with(":read")));
        assert!(role_permissions("ADVISER", None).contains(&"lockdates:write".to_string()));
        assert!(!role_permissions("INVOICE_ONLY", None).contains(&"banking:read".to_string()));
    }
}
