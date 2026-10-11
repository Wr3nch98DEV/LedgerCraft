//! Custom report builder (rows of accounts / account types / formulas / headings / text across
//! period columns, with publish-to-lock) and management report packs.

use crate::{error::invalid, reports::ReportParams, util, Error, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Evaluate an arithmetic expression over row references (r1, r2 …). Supports + - * / ( ) and numbers.
pub fn eval_formula(expr: &str, refs: &HashMap<String, f64>) -> Result<f64> {
    struct P<'a> { s: &'a [u8], i: usize, refs: &'a HashMap<String, f64> }
    impl<'a> P<'a> {
        fn ws(&mut self) { while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() { self.i += 1; } }
        fn expr(&mut self) -> Result<f64> { let mut v = self.term()?; loop { self.ws(); match self.s.get(self.i) { Some(b'+') => { self.i += 1; v += self.term()?; } Some(b'-') => { self.i += 1; v -= self.term()?; } _ => return Ok(v) } } }
        fn term(&mut self) -> Result<f64> { let mut v = self.factor()?; loop { self.ws(); match self.s.get(self.i) { Some(b'*') => { self.i += 1; v *= self.factor()?; } Some(b'/') => { self.i += 1; let d = self.factor()?; v = if d == 0.0 { 0.0 } else { v / d }; } _ => return Ok(v) } } }
        fn factor(&mut self) -> Result<f64> {
            self.ws();
            match self.s.get(self.i) {
                Some(b'(') => { self.i += 1; let v = self.expr()?; self.ws(); if self.s.get(self.i) != Some(&b')') { return invalid("missing ')' in formula"); } self.i += 1; Ok(v) }
                Some(b'-') => { self.i += 1; Ok(-self.factor()?) }
                Some(c) if c.is_ascii_digit() || *c == b'.' => { let st = self.i; while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.') { self.i += 1; } std::str::from_utf8(&self.s[st..self.i]).unwrap().parse::<f64>().map_err(|_| Error::Validation("bad number in formula".into())) }
                Some(c) if c.is_ascii_alphabetic() => { let st = self.i; while self.i < self.s.len() && self.s[self.i].is_ascii_alphanumeric() { self.i += 1; } let k = std::str::from_utf8(&self.s[st..self.i]).unwrap().to_lowercase();
                    self.refs.get(&k).copied().ok_or_else(|| Error::Validation(format!("unknown row '{k}' in formula"))) }
                _ => invalid("formula is incomplete"),
            }
        }
    }
    let mut p = P { s: expr.as_bytes(), i: 0, refs };
    let v = p.expr()?; p.ws();
    if p.i != p.s.len() { return invalid("unexpected text in formula"); }
    Ok(v)
}

impl Ledger {
    pub fn list_layouts(&self) -> Result<Vec<Value>> {
        let mut v = util::query_json(&self.conn, "SELECT id, name, kind, base_report, definition, published, updated_at FROM report_layouts ORDER BY kind, name", [])?;
        for r in v.iter_mut() { r["definition"] = serde_json::from_str(r["definition"].as_str().unwrap_or("{}")).unwrap_or(json!({})); }
        Ok(v)
    }

    pub fn save_layout(&self, id: Option<i64>, name: &str, kind: &str, def: &Value) -> Result<i64> {
        if name.trim().is_empty() { return invalid("give the report a name"); }
        if !["CUSTOM", "PACK", "SAVED"].contains(&kind) { return invalid("unknown layout kind"); }
        match id {
            None => { self.conn.execute("INSERT INTO report_layouts(name, kind, definition) VALUES (?1, ?2, ?3)", params![name.trim(), kind, def.to_string()])?; Ok(self.conn.last_insert_rowid()) }
            Some(id) => {
                let published: i64 = self.conn.query_row("SELECT published FROM report_layouts WHERE id = ?1", [id], |r| r.get(0))?;
                if published != 0 { return invalid("this report is published; unpublish it to make changes"); }
                self.conn.execute("UPDATE report_layouts SET name = ?2, definition = ?3, updated_at = datetime('now') WHERE id = ?1", params![id, name.trim(), def.to_string()])?; Ok(id)
            }
        }
    }

    pub fn delete_layout(&self, id: i64) -> Result<()> { self.conn.execute("DELETE FROM report_layouts WHERE id = ?1", [id])?; Ok(()) }

    /// Publishing freezes the current output so it can't change if transactions are edited later.
    pub fn publish_layout(&self, id: i64, publish: bool) -> Result<()> {
        if publish {
            let out = self.run_layout(id, None)?;
            self.conn.execute("UPDATE report_layouts SET published = 1, published_snapshot = ?2 WHERE id = ?1", params![id, out.to_string()])?;
        } else {
            self.conn.execute("UPDATE report_layouts SET published = 0, published_snapshot = NULL WHERE id = ?1", [id])?;
        }
        Ok(())
    }

    pub fn run_layout(&self, id: i64, override_params: Option<&Value>) -> Result<Value> {
        let (name, kind, def, published, snap): (String, String, String, i64, Option<String>) = self.conn.query_row(
            "SELECT name, kind, definition, published, published_snapshot FROM report_layouts WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
        if published != 0 && override_params.is_none() { if let Some(s) = snap { let mut v: Value = serde_json::from_str(&s)?; v["published"] = json!(true); return Ok(v); } }
        let def: Value = serde_json::from_str(&def)?;
        match kind.as_str() {
            "CUSTOM" => self.run_custom(&name, &def, override_params),
            "PACK" => self.run_pack(&name, &def),
            _ => invalid("unknown layout"),
        }
    }

    /// Definition: { from, to, compare, period, monthly, basis, balance: bool, rows: [ {type, label, account_ids|types|expr|text, sign} ] }
    pub fn run_custom(&self, name: &str, def: &Value, ov: Option<&Value>) -> Result<Value> {
        let g = |k: &str| ov.and_then(|o| o.get(k)).filter(|v| !v.is_null()).unwrap_or(&def[k]).clone();
        let to = g("to").as_str().map(String::from).unwrap_or_else(util::today);
        let from = g("from").as_str().map(String::from).unwrap_or(self.fy_start(&to)?);
        let balance = def["balance"].as_bool().unwrap_or(false);
        let cash = g("basis").as_str() == Some("CASH");
        let periods = self.periods(&from, &to, g("compare").as_i64().unwrap_or(0), g("period").as_str().unwrap_or("MONTH"), g("monthly").as_bool().unwrap_or(false))?;
        let moves: Vec<HashMap<i64, i64>> = periods.iter().map(|p| if balance { self.account_movements(None, &p.to, false, None) } else { self.account_movements(Some(&p.from), &p.to, cash, None) }).collect::<Result<_>>()?;
        let accts = util::query_json(&self.conn, "SELECT id, type, class FROM accounts", [])?;
        let n = periods.len();
        let mut values: Vec<HashMap<String, f64>> = vec![HashMap::new(); n];
        let mut rows = Vec::new();
        for (idx, r) in def["rows"].as_array().cloned().unwrap_or_default().iter().enumerate() {
            let key = format!("r{}", idx + 1);
            let sign = r["sign"].as_i64().unwrap_or(1);
            let label = r["label"].as_str().unwrap_or("").to_string();
            match r["type"].as_str().unwrap_or("") {
                "heading" => rows.push(json!({"label": label, "heading": true, "values": []})),
                "text" => rows.push(json!({"label": r["text"].as_str().unwrap_or(&label), "text": true, "values": []})),
                "account" | "types" => {
                    let ids: Vec<i64> = if r["type"] == "account" { r["account_ids"].as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default() }
                        else { let ts: Vec<String> = r["types"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                            accts.iter().filter(|a| ts.iter().any(|t| a["type"].as_str() == Some(t) || a["class"].as_str() == Some(t))).filter_map(|a| a["id"].as_i64()).collect() };
                    let vals: Vec<i64> = moves.iter().map(|m| sign * ids.iter().map(|i| m.get(i).copied().unwrap_or(0)).sum::<i64>()).collect();
                    for i in 0..n { values[i].insert(key.clone(), vals[i] as f64); }
                    rows.push(json!({"label": label, "ref": key, "values": vals, "bold": r["bold"]}));
                }
                "formula" => {
                    let expr = r["expr"].as_str().unwrap_or("");
                    let mut vals = Vec::new();
                    for i in 0..n { let v = eval_formula(expr, &values[i])?; values[i].insert(key.clone(), v); vals.push(v.round() as i64); }
                    let pct = r["percent"].as_bool().unwrap_or(false);
                    rows.push(json!({"label": label, "ref": key, "values": if pct { vals.iter().map(|v| json!(*v)).collect::<Vec<_>>() } else { vals.iter().map(|v| json!(v)).collect() }, "format": if pct { "pctx" } else { "money" }, "bold": true}));
                }
                _ => {}
            }
        }
        Ok(json!({"title": name, "org": self.setting_or("org_name", "")?, "subtitle": format!("{} to {}", crate::reports::fmt_long(&from), crate::reports::fmt_long(&to)),
            "columns": periods.iter().map(|p| p.label.clone()).collect::<Vec<_>>(), "custom": true, "sections": [{"title": "", "rows": rows}]}))
    }

    /// Definition: { cover_title, cover_subtitle, commentary, items: [{report, params, title?} | {layout_id}] }
    pub fn run_pack(&self, name: &str, def: &Value) -> Result<Value> {
        let mut reports = Vec::new();
        for it in def["items"].as_array().cloned().unwrap_or_default() {
            let r = if let Some(lid) = it["layout_id"].as_i64() { self.run_layout(lid, None)? } else {
                let p: ReportParams = serde_json::from_value(it["params"].clone()).unwrap_or_default();
                crate::extra_reports::run_any(self, it["report"].as_str().unwrap_or(""), &p)?
            };
            reports.push(r);
        }
        Ok(json!({"pack": true, "title": def["cover_title"].as_str().unwrap_or(name), "subtitle": def["cover_subtitle"], "commentary": def["commentary"], "org": self.setting_or("org_name", "")?,
            "prepared": util::today(), "reports": reports}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formulas() {
        let refs = HashMap::from([("r1".to_string(), 1000.0), ("r2".to_string(), 400.0)]);
        assert_eq!(eval_formula("r1 - r2", &refs).unwrap(), 600.0);
        assert_eq!(eval_formula("(r1 - r2) / r1 * 100", &refs).unwrap(), 60.0);
        assert_eq!(eval_formula("-r2 + 2*3", &refs).unwrap(), -394.0);
        assert!(eval_formula("r9", &refs).is_err());
        assert!(eval_formula("r1 +", &refs).is_err());
        assert_eq!(eval_formula("r1 / 0", &refs).unwrap(), 0.0);
    }
}
