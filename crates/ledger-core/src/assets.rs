//! Fixed asset register, depreciation (book and tax), depreciation runs and disposals.

use crate::{error::invalid, journal::{JLine, NewJournal}, money, util, Error, Ledger, Result};
use chrono::Datelike;
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AssetInput {
    pub name: String,
    pub number: Option<String>,
    pub description: String,
    pub asset_type_id: Option<i64>,
    pub purchase_date: String,
    pub purchase_price: String,
    pub serial_number: String,
    pub warranty_expiry: Option<String>,
    pub method: Option<String>,
    pub rate: Option<String>,          // annual % e.g. "20"
    pub life_years: Option<String>,
    pub residual_value: Option<String>,
    pub dep_start_date: Option<String>,
    pub opening_accum_dep: Option<String>,
    pub tax_method: Option<String>,
    pub tax_rate: Option<String>,
    pub tax_life_years: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AssetTypeInput {
    pub name: String,
    pub asset_account_id: i64,
    pub accum_dep_account_id: i64,
    pub dep_expense_account_id: i64,
    pub method: String,
    pub rate: Option<String>,
    pub life_years: Option<String>,
}

pub const METHODS: &[&str] = &["STRAIGHTLINE", "DIMINISHING100", "DIMINISHING150", "DIMINISHING200", "FULL", "NONE"];

/// Annual depreciation rate in bp for a method given rate or effective life.
fn annual_rate_bp(method: &str, rate_bp: Option<i64>, life_months: Option<i64>) -> i64 {
    if let Some(r) = rate_bp.filter(|r| *r > 0) { return r; }
    let life = life_months.filter(|l| *l > 0).unwrap_or(0);
    if life == 0 { return 0; }
    let base = money::div_round(10_000 * 12, life as i128) as i64; // 100% / life_years
    match method { "DIMINISHING150" => base * 3 / 2, "DIMINISHING200" => base * 2, _ => base }
}

/// Depreciation for one calendar month. `days_fraction` (0..=1 as bp) pro-rates the first month.
pub fn month_depreciation(method: &str, cost: i64, residual: i64, accum: i64, rate_bp: i64, fraction_bp: i64) -> i64 {
    let depreciable = (cost - residual - accum).max(0);
    if depreciable == 0 { return 0; }
    let amt = match method {
        "STRAIGHTLINE" => money::div_round((cost - residual) as i128 * rate_bp as i128 * fraction_bp as i128, 10_000 * 12 * 10_000) as i64,
        "DIMINISHING100" | "DIMINISHING150" | "DIMINISHING200" => money::div_round((cost - accum) as i128 * rate_bp as i128 * fraction_bp as i128, 10_000 * 12 * 10_000) as i64,
        "FULL" => depreciable,
        _ => 0,
    };
    amt.min(depreciable).max(0)
}

fn pf(s: &Option<String>, dp: u32) -> Option<i64> { s.as_deref().filter(|x| !x.trim().is_empty()).and_then(|x| money::parse_fixed(x, dp)) }

impl Ledger {
    pub fn list_asset_types(&self) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT t.*, a.name AS asset_account, b.name AS accum_account, c.name AS expense_account FROM asset_types t
            JOIN accounts a ON a.id = t.asset_account_id JOIN accounts b ON b.id = t.accum_dep_account_id JOIN accounts c ON c.id = t.dep_expense_account_id ORDER BY t.name", [])
    }

    pub fn save_asset_type(&self, id: Option<i64>, t: &AssetTypeInput) -> Result<i64> {
        if t.name.trim().is_empty() { return invalid("asset type needs a name"); }
        if !METHODS.contains(&t.method.as_str()) { return invalid("unknown depreciation method"); }
        let rate = pf(&t.rate, 2); let life = pf(&t.life_years, 2).map(|y| y * 12 / 100);
        match id {
            None => { self.conn.execute("INSERT INTO asset_types(name, asset_account_id, accum_dep_account_id, dep_expense_account_id, method, rate_bp, life_months) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![t.name.trim(), t.asset_account_id, t.accum_dep_account_id, t.dep_expense_account_id, t.method, rate, life])?; Ok(self.conn.last_insert_rowid()) }
            Some(id) => { self.conn.execute("UPDATE asset_types SET name=?2, asset_account_id=?3, accum_dep_account_id=?4, dep_expense_account_id=?5, method=?6, rate_bp=?7, life_months=?8 WHERE id=?1",
                params![id, t.name.trim(), t.asset_account_id, t.accum_dep_account_id, t.dep_expense_account_id, t.method, rate, life])?; Ok(id) }
        }
    }

    /// Create sensible default asset types from the chart (office/computer/vehicle).
    pub fn ensure_asset_types(&self) -> Result<()> {
        let n: i64 = self.conn.query_row("SELECT COUNT(*) FROM asset_types", [], |r| r.get(0))?;
        if n > 0 { return Ok(()); }
        let dep = match self.account_by_code("416")? { Some(d) => d, None => return Ok(()) };
        // (name, asset account, accumulated depreciation account, method, effective life in years)
        for (name, a, b, method, life) in [("Office equipment", "710", "711", "DIMINISHING200", 5), ("Computer equipment", "720", "721", "DIMINISHING200", 4), ("Motor vehicles", "730", "731", "DIMINISHING200", 8)] {
            if let (Some(a), Some(b)) = (self.account_by_code(a)?, self.account_by_code(b)?) {
                self.save_asset_type(None, &AssetTypeInput { name: name.into(), asset_account_id: a, accum_dep_account_id: b, dep_expense_account_id: dep, method: method.into(),
                    life_years: Some(life.to_string()), rate: None })?;
            }
        }
        Ok(())
    }

    pub fn list_assets(&self, status: &str) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT f.*, t.name AS type_name, f.purchase_price - f.accum_dep - f.opening_accum_dep AS book_value FROM fixed_assets f LEFT JOIN asset_types t ON t.id = f.asset_type_id
            WHERE (?1 = 'ALL' OR f.status = ?1) ORDER BY f.number", [status])
    }

    pub fn get_asset(&self, id: i64) -> Result<Value> {
        let mut a = util::query_one_json(&self.conn, "SELECT f.*, t.name AS type_name, f.purchase_price - f.accum_dep - f.opening_accum_dep AS book_value FROM fixed_assets f LEFT JOIN asset_types t ON t.id = f.asset_type_id WHERE f.id = ?1", [id])?;
        a["schedule"] = self.depreciation_schedule(id, 5)?;
        a["history"] = json!(self.history("asset", id)?);
        Ok(a)
    }

    pub fn save_asset(&self, id: Option<i64>, a: &AssetInput) -> Result<i64> {
        if a.name.trim().is_empty() { return invalid("asset name is required"); }
        util::parse_date(&a.purchase_date)?;
        let price = money::parse_fixed(&a.purchase_price, 2).ok_or_else(|| Error::Validation("invalid purchase price".into()))?;
        let (tm, tr, tl): (Option<String>, Option<i64>, Option<i64>) = match a.asset_type_id {
            Some(t) => self.conn.query_row("SELECT method, rate_bp, life_months FROM asset_types WHERE id = ?1", [t], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?,
            None => (None, None, None),
        };
        let method = a.method.clone().filter(|m| !m.is_empty()).or(tm).unwrap_or_else(|| "STRAIGHTLINE".into());
        if !METHODS.contains(&method.as_str()) { return invalid("unknown depreciation method"); }
        let rate = pf(&a.rate, 2).or(if a.life_years.is_some() { None } else { tr });
        let life = pf(&a.life_years, 2).map(|y| y * 12 / 100).or(if rate.is_some() { None } else { tl });
        let residual = pf(&a.residual_value, 2).unwrap_or(0);
        let opening = pf(&a.opening_accum_dep, 2).unwrap_or(0);
        if residual + opening > price { return invalid("residual value plus opening depreciation can't exceed the cost"); }
        self.atomic(|| {
            let aid = match id {
                None => {
                    let number = a.number.clone().filter(|n| !n.is_empty()).map(Ok).unwrap_or_else(|| self.next_number("ASSET"))?;
                    self.conn.execute("INSERT INTO fixed_assets(number, name, purchase_date, purchase_price) VALUES (?1, ?2, ?3, ?4)", params![number, a.name.trim(), a.purchase_date, price])?;
                    self.conn.last_insert_rowid()
                }
                Some(id) => {
                    let st: String = self.conn.query_row("SELECT status FROM fixed_assets WHERE id = ?1", [id], |r| r.get(0))?;
                    let ran: i64 = self.conn.query_row("SELECT accum_dep FROM fixed_assets WHERE id = ?1", [id], |r| r.get(0))?;
                    if st == "DISPOSED" { return invalid("disposed assets can't be edited"); }
                    if st == "REGISTERED" && ran != 0 { self.conn.execute("UPDATE fixed_assets SET name = ?2, description = ?3, serial_number = ?4, warranty_expiry = ?5 WHERE id = ?1",
                        params![id, a.name.trim(), a.description, a.serial_number, a.warranty_expiry])?; return Ok(id); }
                    id
                }
            };
            self.conn.execute("UPDATE fixed_assets SET name=?2, description=?3, asset_type_id=?4, purchase_date=?5, purchase_price=?6, serial_number=?7, warranty_expiry=?8, method=?9,
                rate_bp=?10, life_months=?11, residual_value=?12, dep_start_date=?13, opening_accum_dep=?14, tax_method=?15, tax_rate_bp=?16, tax_life_months=?17 WHERE id=?1",
                params![aid, a.name.trim(), a.description, a.asset_type_id, a.purchase_date, price, a.serial_number, a.warranty_expiry.clone().filter(|s| !s.is_empty()), method,
                    rate, life, residual, a.dep_start_date.clone().filter(|s| !s.is_empty()).unwrap_or(a.purchase_date.clone()), opening,
                    a.tax_method.clone().filter(|s| !s.is_empty()), pf(&a.tax_rate, 2), pf(&a.tax_life_years, 2).map(|y| y * 12 / 100)])?;
            self.audit(if id.is_some() { "UPDATE" } else { "CREATE" }, "asset", Some(aid), None, Some(json!({"name": a.name})))?;
            self.index_search("asset", aid, &a.name, &a.serial_number)?;
            Ok(aid)
        })
    }

    /// Register a draft asset (starts depreciating). FULL method depreciates immediately.
    pub fn register_asset(&self, id: i64) -> Result<()> {
        let (st, type_id, method, date): (String, Option<i64>, String, String) = self.conn.query_row("SELECT status, asset_type_id, method, dep_start_date FROM fixed_assets WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        if st != "DRAFT" { return invalid("only draft assets can be registered"); }
        if type_id.is_none() { return invalid("choose an asset type (it sets the accounts used for depreciation)"); }
        self.atomic(|| {
            self.conn.execute("UPDATE fixed_assets SET status = 'REGISTERED' WHERE id = ?1", [id])?;
            self.add_note("asset", id, "HISTORY", "Asset registered")?;
            if method == "FULL" { self.run_depreciation_for(&[id], &date)?; }
            Ok(())
        })
    }

    /// Calculate depreciation for an asset from its last run to `to` (inclusive). Returns (book, tax, months).
    fn calc_depreciation(&self, id: i64, to: &str) -> Result<(i64, i64, String)> {
        let (cost, residual, accum, opening, method, rate, life, start, last, tax_m, tax_r, tax_l, tax_acc): (i64, i64, i64, i64, String, Option<i64>, Option<i64>, String, Option<String>, Option<String>, Option<i64>, Option<i64>, i64) =
            self.conn.query_row("SELECT purchase_price, residual_value, accum_dep, opening_accum_dep, method, rate_bp, life_months, COALESCE(dep_start_date, purchase_date), last_dep_date, tax_method, tax_rate_bp, tax_life_months, tax_accum_dep FROM fixed_assets WHERE id = ?1",
                [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?, r.get(12)?)))?;
        let rate_bp = annual_rate_bp(&method, rate, life);
        let tax_method = tax_m.clone().unwrap_or(method.clone());
        let tax_rate_bp = if tax_m.is_some() || tax_r.is_some() || tax_l.is_some() { annual_rate_bp(&tax_method, tax_r, tax_l) } else { rate_bp };
        let start_d = util::parse_date(&start)?;
        let from_d = match &last { Some(l) => util::parse_date(l)? + chrono::Duration::days(1), None => start_d };
        let to_d = util::parse_date(to)?;
        let (mut book, mut tax) = (0i64, 0i64);
        let mut acc = accum + opening;
        let mut tacc = tax_acc + opening;
        if method == "FULL" && last.is_none() { return Ok(((cost - residual - acc).max(0), (cost - residual - tacc).max(0), to.into())); }
        let mut m = chrono::NaiveDate::from_ymd_opt(from_d.year(), from_d.month(), 1).unwrap();
        while m <= to_d {
            let me = util::last_day_of_month(m.year(), m.month());
            if me > to_d { break; } // only complete months
            let first = m.year() == start_d.year() && m.month() == start_d.month();
            let frac = if first && start_d.day() > 1 { money::div_round(((me.day() - start_d.day() + 1) as i128) * 10_000, me.day() as i128) as i64 } else { 10_000 };
            let b = month_depreciation(&method, cost, residual, acc, rate_bp, frac);
            let t = month_depreciation(&tax_method, cost, residual, tacc, tax_rate_bp, frac);
            book += b; tax += t; acc += b; tacc += t;
            m = util::add_months(m, 1);
        }
        // Straight line over an effective life: use the exact cumulative target so rounding never drifts.
        let sl_exact = |m: &str, r: Option<i64>, life: Option<i64>, accum_now: i64| -> Option<i64> {
            if m != "STRAIGHTLINE" || r.is_some() { return None; }
            let life = life.filter(|l| *l > 0)?;
            let end = if util::last_day_of_month(to_d.year(), to_d.month()) == to_d { to_d } else { chrono::NaiveDate::from_ymd_opt(to_d.year(), to_d.month(), 1).unwrap() - chrono::Duration::days(1) };
            if end < start_d { return Some(0); }
            let months = (end.year() - start_d.year()) as i64 * 12 + end.month() as i64 - start_d.month() as i64; // full months after the first
            let first_last = util::last_day_of_month(start_d.year(), start_d.month()).day() as i64;
            let first_bp = if start_d.day() > 1 { money::div_round(((first_last - start_d.day() as i64 + 1) * 10_000) as i128, first_last as i128) as i64 } else { 10_000 };
            let elapsed_bp = months * 10_000 + first_bp;
            let target = money::div_round((cost - residual) as i128 * elapsed_bp as i128, (life * 10_000) as i128) as i64;
            Some((target.min(cost - residual - opening) - accum_now).max(0))
        };
        if let Some(b) = sl_exact(&method, rate, life, accum) { book = b; }
        if tax_m.is_some() || tax_r.is_some() || tax_l.is_some() {
            if let Some(t) = sl_exact(&tax_method, tax_r, tax_l, tax_acc) { tax = t; }
        } else if let Some(t) = sl_exact(&method, rate, life, tax_acc) { tax = t; }
        let last_month_end = util::last_day_of_month(to_d.year(), to_d.month());
        let through = if last_month_end == to_d { to.to_string() } else { util::fmt_date(chrono::NaiveDate::from_ymd_opt(to_d.year(), to_d.month(), 1).unwrap() - chrono::Duration::days(1)) };
        Ok((book, tax, through))
    }

    /// Run depreciation for all registered assets up to the end of the month containing `to`.
    pub fn run_depreciation(&self, to: &str) -> Result<Value> {
        let ids: Vec<i64> = { let mut st = self.conn.prepare("SELECT id FROM fixed_assets WHERE status = 'REGISTERED'")?; st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()? };
        let d = util::parse_date(to)?;
        let end = util::fmt_date(util::last_day_of_month(d.year(), d.month()));
        self.run_depreciation_for(&ids, &end)
    }

    fn run_depreciation_for(&self, ids: &[i64], to: &str) -> Result<Value> {
        let mut lines = Vec::new();
        let mut detail = Vec::new();
        let mut total = 0;
        for id in ids {
            let (book, tax, through) = self.calc_depreciation(*id, to)?;
            if book == 0 && tax == 0 { continue; }
            let (exp, accum, name): (i64, i64, String) = self.conn.query_row("SELECT t.dep_expense_account_id, t.accum_dep_account_id, f.name FROM fixed_assets f JOIN asset_types t ON t.id = f.asset_type_id WHERE f.id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            if book != 0 {
                lines.push(JLine::new(exp, book).desc(format!("Depreciation: {name}")));
                lines.push(JLine::new(accum, -book).desc(format!("Depreciation: {name}")));
            }
            detail.push(json!({"asset_id": id, "name": name, "amount": book, "tax_amount": tax, "through": through}));
            total += book;
        }
        if detail.is_empty() { return Ok(json!({"posted": false, "total": 0, "detail": []})); }
        self.atomic(|| {
            let jid = if lines.is_empty() { None } else {
                Some(self.post_journal(NewJournal { date: to.into(), source_type: "DEPRECIATION".into(), reference: "Depreciation".into(), narration: format!("Depreciation to {to}"), lines: lines.clone(), cash_basis: true, ..Default::default() })?)
            };
            for d in &detail {
                self.conn.execute("UPDATE fixed_assets SET accum_dep = accum_dep + ?2, tax_accum_dep = tax_accum_dep + ?3, last_dep_date = ?4 WHERE id = ?1",
                    params![d["asset_id"].as_i64(), d["amount"].as_i64(), d["tax_amount"].as_i64(), d["through"].as_str()])?;
            }
            self.conn.execute("INSERT INTO depreciation_runs(period_end, journal_id, total, detail) VALUES (?1, ?2, ?3, ?4)", params![to, jid, total, json!(detail).to_string()])?;
            Ok(json!({"posted": true, "journal_id": jid, "total": total, "detail": detail}))
        })
    }

    /// Roll back the most recent depreciation run.
    pub fn rollback_depreciation(&self) -> Result<()> {
        let (id, jid, detail): (i64, Option<i64>, String) = self.conn.query_row("SELECT id, journal_id, detail FROM depreciation_runs WHERE status = 'POSTED' ORDER BY id DESC LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|_| Error::Validation("there is no depreciation run to roll back".into()))?;
        let det: Vec<Value> = serde_json::from_str(&detail)?;
        self.atomic(|| {
            if let Some(j) = jid { self.reverse_journal(j, None, None)?; }
            for d in &det {
                let aid = d["asset_id"].as_i64().unwrap_or(0);
                let prev: Option<String> = self.conn.query_row("SELECT MAX(json_extract(value, '$.through')) FROM depreciation_runs r, json_each(r.detail) WHERE r.status = 'POSTED' AND r.id <> ?1 AND json_extract(value, '$.asset_id') = ?2",
                    params![id, aid], |r| r.get(0)).unwrap_or(None);
                self.conn.execute("UPDATE fixed_assets SET accum_dep = accum_dep - ?2, tax_accum_dep = tax_accum_dep - ?3, last_dep_date = ?4 WHERE id = ?1",
                    params![aid, d["amount"].as_i64(), d["tax_amount"].as_i64(), prev])?;
            }
            self.conn.execute("UPDATE depreciation_runs SET status = 'ROLLEDBACK' WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    pub fn list_depreciation_runs(&self) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT id, period_end, journal_id, total, status, created_at FROM depreciation_runs ORDER BY id DESC", [])
    }

    /// Dispose of (sell or scrap) an asset. Depreciation is brought up to the disposal month first.
    pub fn dispose_asset(&self, id: i64, date: &str, sale_price: &str, proceeds_account_id: Option<i64>, gain_loss_account_id: i64) -> Result<i64> {
        let price = if sale_price.trim().is_empty() { 0 } else { money::parse_fixed(sale_price, 2).ok_or_else(|| Error::Validation("invalid sale price".into()))? };
        if price > 0 && proceeds_account_id.is_none() { return invalid("choose where the sale proceeds were received"); }
        let st: String = self.conn.query_row("SELECT status FROM fixed_assets WHERE id = ?1", [id], |r| r.get(0))?;
        if st != "REGISTERED" { return invalid("only registered assets can be disposed"); }
        self.atomic(|| {
            let d = util::parse_date(date)?;
            let prev_month_end = chrono::NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap() - chrono::Duration::days(1);
            self.run_depreciation_for(&[id], &util::fmt_date(prev_month_end))?;
            let (cost, accum, opening, asset_acct, accum_acct, name): (i64, i64, i64, i64, i64, String) = self.conn.query_row(
                "SELECT f.purchase_price, f.accum_dep, f.opening_accum_dep, t.asset_account_id, t.accum_dep_account_id, f.name FROM fixed_assets f JOIN asset_types t ON t.id = f.asset_type_id WHERE f.id = ?1",
                [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;
            let total_accum = accum + opening;
            let mut lines = vec![JLine::new(accum_acct, total_accum).desc(format!("Disposal: {name}")), JLine::new(asset_acct, -cost).desc(format!("Disposal: {name}"))];
            if price != 0 { lines.push(JLine::new(proceeds_account_id.unwrap(), price).desc(format!("Sale of {name}"))); }
            let diff: i64 = lines.iter().map(|l| l.amount).sum();
            if diff != 0 { lines.push(JLine::new(gain_loss_account_id, -diff).desc(format!("{} on disposal of {name}", if diff > 0 { "Gain" } else { "Loss" }))); }
            let jid = self.post_journal(NewJournal { date: date.into(), source_type: "ASSETDISPOSAL".into(), source_id: Some(id), reference: name.clone(), narration: format!("Disposal of {name}"), lines, cash_basis: true, reverses: None })?;
            self.conn.execute("UPDATE fixed_assets SET status = 'DISPOSED', disposal_date = ?2, disposal_price = ?3, disposal_journal_id = ?4 WHERE id = ?1", params![id, date, price, jid])?;
            self.add_note("asset", id, "HISTORY", &format!("Disposed on {date} for {}", money::fmt_money_grouped(price)))?;
            Ok(jid)
        })
    }

    /// Projected depreciation for the next `years` financial years.
    pub fn depreciation_schedule(&self, id: i64, years: i64) -> Result<Value> {
        let (cost, residual, accum, opening, method, rate, life, status): (i64, i64, i64, i64, String, Option<i64>, Option<i64>, String) = self.conn.query_row(
            "SELECT purchase_price, residual_value, accum_dep, opening_accum_dep, method, rate_bp, life_months, status FROM fixed_assets WHERE id = ?1", [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)))?;
        let rate_bp = annual_rate_bp(&method, rate, life);
        let mut acc = accum + opening;
        let mut out = Vec::new();
        if status == "DISPOSED" { return Ok(json!(out)); }
        let mut fy = util::parse_date(&self.fy_start(&util::today())?)?;
        for _ in 0..years {
            let mut y = 0;
            for _ in 0..12 { let b = month_depreciation(&method, cost, residual, acc, rate_bp, 10_000); y += b; acc += b; }
            out.push(json!({"year_start": util::fmt_date(fy), "depreciation": y, "closing_book_value": cost - acc}));
            fy = util::add_months(fy, 12);
        }
        Ok(json!(out))
    }

    pub fn asset_register_report(&self, to: &str) -> Result<Value> {
        let rows = util::query_json(&self.conn,
            "SELECT f.id, f.number, f.name, t.name AS type_name, f.purchase_date, f.purchase_price, f.accum_dep + f.opening_accum_dep AS accum, f.purchase_price - f.accum_dep - f.opening_accum_dep AS book,
               f.tax_accum_dep + f.opening_accum_dep AS tax_accum, f.status FROM fixed_assets f LEFT JOIN asset_types t ON t.id = f.asset_type_id WHERE f.status <> 'DRAFT' AND f.purchase_date <= ?1 ORDER BY t.name, f.number", [to])?;
        let tot = rows.iter().fold((0i64, 0i64, 0i64), |a, r| (a.0 + r["purchase_price"].as_i64().unwrap_or(0), a.1 + r["accum"].as_i64().unwrap_or(0), a.2 + r["book"].as_i64().unwrap_or(0)));
        let out: Vec<Value> = rows.iter().map(|r| json!({"label": format!("{} · {}", r["number"].as_str().unwrap_or(""), r["name"].as_str().unwrap_or("")), "flag": if r["status"] == "DISPOSED" { "Disposed" } else { "" },
            "values": [r["purchase_price"], r["accum"], r["book"], r["tax_accum"]]})).collect();
        Ok(json!({"title": "Fixed Asset Register", "org": self.setting_or("org_name", "")?, "subtitle": format!("As at {}", crate::reports::fmt_long(to)),
            "columns": ["Cost", "Accumulated depreciation", "Book value", "Tax depreciation"], "sections": [{"title": "", "rows": out, "total": {"label": "Total", "values": [tot.0, tot.1, tot.2, Value::Null]}}]}))
    }

    pub fn asset_reconciliation_report(&self, from: &str, to: &str) -> Result<Value> {
        let types = util::query_json(&self.conn, "SELECT id, name, asset_account_id, accum_dep_account_id FROM asset_types ORDER BY name", [])?;
        let mut sections = Vec::new();
        for t in types {
            let bal = |acct: i64, d: &str| -> i64 { self.conn.query_row("SELECT COALESCE(SUM(jl.amount),0) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE j.posted = 1 AND jl.account_id = ?1 AND j.date <= ?2", params![acct, d], |r| r.get(0)).unwrap_or(0) };
            let a = t["asset_account_id"].as_i64().unwrap(); let b = t["accum_dep_account_id"].as_i64().unwrap();
            let before = util::add_days(from, -1)?;
            let (ao, ac, bo, bc) = (bal(a, &before), bal(a, to), -bal(b, &before), -bal(b, to));
            sections.push(json!({"title": t["name"], "rows": [
                {"label": "Cost — opening", "values": [ao]}, {"label": "Additions / disposals", "values": [ac - ao]}, {"label": "Cost — closing", "values": [ac]},
                {"label": "Accumulated depreciation — opening", "values": [bo]}, {"label": "Depreciation / disposals", "values": [bc - bo]}, {"label": "Accumulated depreciation — closing", "values": [bc]}],
                "total": {"label": "Net book value", "values": [ac - bc]}}));
        }
        Ok(json!({"title": "Fixed Asset Reconciliation", "org": self.setting_or("org_name", "")?, "subtitle": format!("{} to {}", crate::reports::fmt_long(from), crate::reports::fmt_long(to)), "columns": ["Amount"], "sections": sections}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn methods() {
        // $12,000, 5 year straight line = 200/month
        assert_eq!(month_depreciation("STRAIGHTLINE", 1_200_000, 0, 0, annual_rate_bp("STRAIGHTLINE", None, Some(60)), 10_000), 20_000);
        // 200% DV over 5 years = 40% p.a.; first month on 12,000 = 400
        assert_eq!(month_depreciation("DIMINISHING200", 1_200_000, 0, 0, annual_rate_bp("DIMINISHING200", None, Some(60)), 10_000), 40_000);
        // 150% DV 4 years = 37.5%; 10,000 -> 312.50
        assert_eq!(month_depreciation("DIMINISHING150", 1_000_000, 0, 0, annual_rate_bp("DIMINISHING150", None, Some(48)), 10_000), 31_250);
        // 100% DV at given rate 25% on book value 8,000 after 2,000 accumulated -> 166.67
        assert_eq!(month_depreciation("DIMINISHING100", 1_000_000, 0, 200_000, 2_500, 10_000), 16_667);
        // never beyond residual
        assert_eq!(month_depreciation("STRAIGHTLINE", 1_000, 100, 850, 10_000, 10_000), 50);
        assert_eq!(month_depreciation("FULL", 5_000, 0, 0, 0, 10_000), 5_000);
        assert_eq!(month_depreciation("NONE", 5_000, 0, 0, 2_000, 10_000), 0);
    }
}
