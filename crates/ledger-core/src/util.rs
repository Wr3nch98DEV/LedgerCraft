use crate::{error::invalid, Result};
use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::types::ValueRef;
use serde_json::{Map, Value};

pub fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").or_else(|_| invalid(String::from("invalid date '") + s + "' (expected YYYY-MM-DD)"))
}

pub fn fmt_date(d: NaiveDate) -> String { d.format("%Y-%m-%d").to_string() }

pub fn today() -> String { fmt_date(chrono::Local::now().date_naive()) }

pub fn add_days(s: &str, n: i64) -> Result<String> { Ok(fmt_date(parse_date(s)? + Duration::days(n))) }

pub fn last_day_of_month(y: i32, m: u32) -> NaiveDate {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    NaiveDate::from_ymd_opt(ny, nm, 1).unwrap() - Duration::days(1)
}

/// Add months, clamping the day to the end of the target month.
pub fn add_months(d: NaiveDate, n: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + n;
    let (y, m) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
    let last = last_day_of_month(y, m).day();
    NaiveDate::from_ymd_opt(y, m, d.day().min(last)).unwrap()
}

/// Due date from payment terms (Xero-style term types).
pub fn due_date(date: &str, terms_type: &str, days: i64) -> Result<String> {
    let d = parse_date(date)?;
    let r = match terms_type {
        "DAYSAFTERBILLMONTH" => last_day_of_month(d.year(), d.month()) + Duration::days(days),
        "OFCURRENTMONTH" => {
            let last = last_day_of_month(d.year(), d.month()).day();
            NaiveDate::from_ymd_opt(d.year(), d.month(), (days.max(1) as u32).min(last)).unwrap()
        }
        "OFFOLLOWINGMONTH" => {
            let n = add_months(NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap(), 1);
            let last = last_day_of_month(n.year(), n.month()).day();
            NaiveDate::from_ymd_opt(n.year(), n.month(), (days.max(1) as u32).min(last)).unwrap()
        }
        _ => d + Duration::days(days),
    };
    Ok(fmt_date(r))
}

/// Convert the current row into a JSON object using column names.
pub fn row_to_json(row: &rusqlite::Row) -> rusqlite::Result<Value> {
    let stmt = row.as_ref();
    let mut m = Map::new();
    for i in 0..stmt.column_count() {
        let name = stmt.column_name(i)?.to_string();
        let v = match row.get_ref(i)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(n) => Value::from(n),
            ValueRef::Real(f) => Value::from(f),
            ValueRef::Text(t) => Value::from(String::from_utf8_lossy(t).to_string()),
            ValueRef::Blob(b) => Value::from(b.len().to_string() + " bytes"),
        };
        m.insert(name, v);
    }
    Ok(Value::Object(m))
}

pub fn query_json(conn: &rusqlite::Connection, sql: &str, params: impl rusqlite::Params) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params, row_to_json)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn query_one_json(conn: &rusqlite::Connection, sql: &str, params: impl rusqlite::Params) -> Result<Value> {
    let mut v = query_json(conn, sql, params)?;
    if v.is_empty() { return Err(crate::Error::NotFound("record".into())); }
    Ok(v.swap_remove(0))
}

pub fn random_token(len: usize) -> String {
    use rand::RngExt;
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789";
    let mut rng = rand::rng();
    (0..len).map(|_| CHARS[rng.random_range(0..CHARS.len())] as char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terms() {
        assert_eq!(due_date("2026-01-15", "DAYSAFTERBILLDATE", 14).unwrap(), "2026-01-29");
        assert_eq!(due_date("2026-01-15", "DAYSAFTERBILLMONTH", 20).unwrap(), "2026-02-20");
        assert_eq!(due_date("2026-01-15", "OFFOLLOWINGMONTH", 31).unwrap(), "2026-02-28");
        assert_eq!(due_date("2026-01-15", "OFCURRENTMONTH", 28).unwrap(), "2026-01-28");
        assert_eq!(fmt_date(add_months(parse_date("2026-01-31").unwrap(), 1)), "2026-02-28");
        assert_eq!(fmt_date(add_months(parse_date("2026-03-15").unwrap(), -3)), "2025-12-15");
    }
}
