//! Multi-entity consolidation (Xero, Sage, Acumatica, QuickBooks): several company files'
//! profit and loss and balance sheets side by side, matched by account code, with chosen
//! intercompany accounts eliminated, and a consolidated total.

use crate::{util, Ledger, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

struct Acct { key: String, label: String, class: String }

fn accounts(l: &Ledger) -> Result<Vec<(i64, Acct)>> {
    Ok(util::query_json(&l.conn, "SELECT id, code, name, class FROM accounts", [])?.into_iter().map(|a| {
        let code = a["code"].as_str().unwrap_or("").trim().to_string();
        let name = a["name"].as_str().unwrap_or("").to_string();
        let key = if code.is_empty() { name.to_lowercase() } else { code.clone() };
        (a["id"].as_i64().unwrap_or(0), Acct { key, label: format!("{code} {name}").trim().to_string(), class: a["class"].as_str().unwrap_or("").to_string() })
    }).collect())
}

/// Consolidated profit and loss (`from`..`to`) and balance sheet (at `to`). Accounts are matched
/// by code (by name when there's no code). Rows for `eliminate` codes go to the Eliminations
/// column so they net to nothing in the consolidated total.
pub fn consolidate(entities: &[(String, &Ledger)], from: &str, to: &str, eliminate: &[String]) -> Result<Value> {
    util::parse_date(from)?;
    util::parse_date(to)?;
    let n = entities.len();
    // (section, key) -> (label, values per entity)
    let mut rows: BTreeMap<(u8, String), (String, Vec<i64>)> = BTreeMap::new();
    let mut profit = vec![0i64; n];
    let mut retained = vec![0i64; n];
    let mut currencies: Vec<String> = Vec::new();
    for (e, (_, l)) in entities.iter().enumerate() {
        let cur = l.base_currency()?;
        if !currencies.contains(&cur) { currencies.push(cur); }
        let accts = accounts(l)?;
        let period = l.account_movements(Some(from), to, false, None)?;
        let to_date = l.account_movements(None, to, false, None)?;
        for (id, a) in &accts {
            let (section, value) = match a.class.as_str() {
                "REVENUE" => (0u8, -period.get(id).copied().unwrap_or(0)),
                "EXPENSE" => (1, period.get(id).copied().unwrap_or(0)),
                "ASSET" => (2, to_date.get(id).copied().unwrap_or(0)),
                "LIABILITY" => (3, -to_date.get(id).copied().unwrap_or(0)),
                "EQUITY" => (4, -to_date.get(id).copied().unwrap_or(0)),
                _ => continue,
            };
            match a.class.as_str() {
                "REVENUE" => { profit[e] += value; retained[e] -= to_date.get(id).copied().unwrap_or(0); }
                "EXPENSE" => { profit[e] -= value; retained[e] -= to_date.get(id).copied().unwrap_or(0); }
                _ => {}
            }
            if value == 0 { continue; }
            let entry = rows.entry((section, a.key.clone())).or_insert_with(|| (a.label.clone(), vec![0; n]));
            entry.1[e] += value;
        }
    }
    let width = n + 2;
    let mut sections: Vec<Value> = Vec::new();
    let titles = ["Income", "Expenses", "Assets", "Liabilities", "Equity"];
    let mut totals: Vec<Vec<i64>> = vec![vec![0; width]; 5];
    for (sec, title) in titles.iter().enumerate() {
        let mut out = Vec::new();
        for ((s, key), (label, vals)) in rows.iter().filter(|((s, _), _)| *s as usize == sec) {
            let _ = s;
            let elim = eliminate.iter().any(|c| c.trim() == key);
            let sum: i64 = vals.iter().sum();
            let mut v = vals.clone();
            v.push(if elim { -sum } else { 0 });
            v.push(if elim { 0 } else { sum });
            for (t, x) in totals[sec].iter_mut().zip(&v) { *t += x; }
            out.push(json!({"label": if elim { format!("{label} (eliminated)") } else { label.clone() }, "values": v}));
        }
        if sec == 4 {
            // Profit not yet closed to retained earnings, per company.
            let mut v = retained.clone();
            let sum: i64 = v.iter().sum();
            v.push(0);
            v.push(sum);
            for (t, x) in totals[sec].iter_mut().zip(&v) { *t += x; }
            out.push(json!({"label": "Current and retained earnings", "values": v}));
        }
        sections.push(json!({"title": title, "rows": out, "total": {"label": format!("Total {}", title.to_lowercase()), "values": totals[sec]}}));
    }
    let net: Vec<i64> = (0..width).map(|i| totals[0][i] - totals[1][i]).collect();
    sections.insert(2, json!({"title": "", "rows": [], "total": {"label": "Net profit", "values": net}}));
    let mut columns: Vec<String> = entities.iter().map(|e| e.0.clone()).collect();
    columns.push("Eliminations".into());
    columns.push("Consolidated".into());
    let mut notes = vec![format!("Profit and loss {} to {}; balance sheet at {}. Accounts are matched by code.", crate::reports::fmt_long(from), crate::reports::fmt_long(to), crate::reports::fmt_long(to))];
    if currencies.len() > 1 { notes.push(format!("These companies use different currencies ({}); the figures are added as they are, not translated.", currencies.join(", "))); }
    let _ = profit;
    Ok(json!({"title": "Consolidated Financial Statements", "subtitle": format!("{} companies", n), "org": "Consolidation", "columns": columns, "sections": sections, "notes": notes}))
}
