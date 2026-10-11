//! Bank statement import (CSV, OFX/QFX, QIF, CAMT.053, MT940) and reconciliation.

use crate::{banking::{BankTxInput, TransferInput}, documents::LineInput, error::invalid, money, payments::PaymentInput, util, Error, Ledger, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StmtLine {
    pub date: String,
    pub amount: i64,
    #[serde(default)]
    pub payee: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub txn_type: String,
    #[serde(default)]
    pub fitid: Option<String>,
    #[serde(default)]
    pub balance: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CsvMapping {
    pub has_header: bool,
    pub date_col: usize,
    pub date_format: String, // DMY MDY YMD
    pub amount_col: Option<usize>,
    pub debit_col: Option<usize>,
    pub credit_col: Option<usize>,
    pub payee_col: Option<usize>,
    pub description_col: Option<usize>,
    pub reference_col: Option<usize>,
    pub balance_col: Option<usize>,
    pub negate: bool,
}

// ---------------- Parsers ----------------

pub fn parse_csv_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_q = false;
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = chars.next() {
        if in_q {
            if c == '"' {
                if chars.peek() == Some(&'"') { field.push('"'); chars.next(); } else { in_q = false; }
            } else { field.push(c); }
        } else {
            match c {
                '"' => in_q = true,
                ',' => { row.push(std::mem::take(&mut field)); }
                '\r' => {}
                '\n' => { row.push(std::mem::take(&mut field)); if row.iter().any(|f| !f.trim().is_empty()) { rows.push(std::mem::take(&mut row)); } else { row.clear(); } }
                _ => field.push(c),
            }
        }
    }
    if !field.is_empty() || !row.is_empty() { row.push(field); if row.iter().any(|f| !f.trim().is_empty()) { rows.push(row); } }
    rows
}

/// Parse a date in one of several common layouts. `order` = DMY / MDY / YMD.
pub fn parse_flex_date(s: &str, order: &str) -> Option<String> {
    let s = s.trim();
    if s.len() >= 8 && s.chars().take(8).all(|c| c.is_ascii_digit()) {
        let (y, m, d) = (&s[0..4], &s[4..6], &s[6..8]);
        return Some(format!("{y}-{m}-{d}"));
    }
    let months = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let parts: Vec<&str> = s.split(|c: char| c == '/' || c == '-' || c == '.' || c == ' ' || c == '\'').filter(|p| !p.is_empty()).collect();
    if parts.len() < 3 { return None; }
    let p: Vec<String> = parts.iter().take(3).map(|x| x.to_lowercase()).collect();
    let num = |x: &str| -> Option<u32> {
        x.parse().ok().or_else(|| months.iter().position(|m| x.starts_with(m)).map(|i| i as u32 + 1))
    };
    let (y, m, d) = if p[0].len() == 4 { (num(&p[0])?, num(&p[1])?, num(&p[2])?) }
        else if months.iter().any(|m| p[1].starts_with(m)) { (num(&p[2])?, num(&p[1])?, num(&p[0])?) }
        else if order == "MDY" { (num(&p[2])?, num(&p[0])?, num(&p[1])?) }
        else if order == "YMD" { (num(&p[0])?, num(&p[1])?, num(&p[2])?) }
        else { (num(&p[2])?, num(&p[1])?, num(&p[0])?) };
    let y = if y < 100 { 2000 + y } else { y };
    chrono::NaiveDate::from_ymd_opt(y as i32, m, d).map(util::fmt_date)
}

pub fn parse_csv(text: &str, m: &CsvMapping) -> Result<Vec<StmtLine>> {
    let rows = parse_csv_rows(text);
    let mut out = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        if i == 0 && m.has_header { continue; }
        let get = |c: Option<usize>| c.and_then(|c| r.get(c)).map(|s| s.trim().to_string()).unwrap_or_default();
        let date = match parse_flex_date(&get(Some(m.date_col)), if m.date_format.is_empty() { "DMY" } else { &m.date_format }) {
            Some(d) => d, None => return invalid(format!("row {}: can't read date '{}'", i + 1, get(Some(m.date_col)))),
        };
        let amount = if m.amount_col.is_some() {
            money::parse_fixed(&get(m.amount_col), 2).ok_or_else(|| Error::Validation(format!("row {}: invalid amount", i + 1)))?
        } else {
            let d = money::parse_fixed(&get(m.debit_col), 2).unwrap_or(0).abs();
            let c = money::parse_fixed(&get(m.credit_col), 2).unwrap_or(0).abs();
            c - d
        };
        let amount = if m.negate { -amount } else { amount };
        if amount == 0 { continue; }
        out.push(StmtLine { date, amount, payee: get(m.payee_col), description: get(m.description_col), reference: get(m.reference_col),
            balance: m.balance_col.and_then(|_| money::parse_fixed(&get(m.balance_col), 2)), ..Default::default() });
    }
    Ok(out)
}

fn tag_value(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let i = block.find(&open)? + open.len();
    let rest = &block[i..];
    let end = rest.find(|c| c == '<' || c == '\n' || c == '\r').unwrap_or(rest.len());
    let v = rest[..end].trim();
    if v.is_empty() { None } else { Some(xml_unescape(v)) }
}

fn xml_unescape(s: &str) -> String { s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'") }

pub fn parse_ofx(text: &str) -> Result<Vec<StmtLine>> {
    let upper = text.to_string();
    let mut out = Vec::new();
    for block in upper.split("<STMTTRN>").skip(1) {
        let block = block.split("</STMTTRN>").next().unwrap_or(block);
        let date = tag_value(block, "DTPOSTED").and_then(|d| parse_flex_date(&d[..d.len().min(8)], "YMD")).ok_or_else(|| Error::Validation("OFX transaction missing date".into()))?;
        let amount = tag_value(block, "TRNAMT").and_then(|a| money::parse_fixed(&a, 2)).ok_or_else(|| Error::Validation("OFX transaction missing amount".into()))?;
        out.push(StmtLine {
            date, amount, payee: tag_value(block, "NAME").or_else(|| tag_value(block, "PAYEE")).unwrap_or_default(),
            description: tag_value(block, "MEMO").unwrap_or_default(),
            reference: tag_value(block, "CHECKNUM").or_else(|| tag_value(block, "REFNUM")).unwrap_or_default(),
            txn_type: tag_value(block, "TRNTYPE").unwrap_or_default(), fitid: tag_value(block, "FITID"), balance: None,
        });
    }
    if out.is_empty() { return invalid("no transactions found in OFX file"); }
    Ok(out)
}

pub fn parse_qif(text: &str, order: &str) -> Result<Vec<StmtLine>> {
    let mut out = Vec::new();
    let mut cur = StmtLine::default();
    let mut has = false;
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('!') { continue; }
        let (k, v) = line.split_at(1);
        match k {
            "D" => { cur.date = parse_flex_date(v, order).ok_or_else(|| Error::Validation(format!("QIF: can't read date '{v}'")))?; has = true; }
            "T" | "U" => { cur.amount = money::parse_fixed(v, 2).ok_or_else(|| Error::Validation("QIF: invalid amount".into()))?; }
            "P" => cur.payee = v.trim().into(),
            "M" => cur.description = v.trim().into(),
            "N" => cur.reference = v.trim().into(),
            "^" => { if has && cur.amount != 0 { out.push(std::mem::take(&mut cur)); } cur = StmtLine::default(); has = false; }
            _ => {}
        }
    }
    if has && cur.amount != 0 { out.push(cur); }
    if out.is_empty() { return invalid("no transactions found in QIF file"); }
    Ok(out)
}

pub fn parse_camt053(text: &str) -> Result<Vec<StmtLine>> {
    let mut out = Vec::new();
    for block in text.split("<Ntry>").skip(1) {
        let block = block.split("</Ntry>").next().unwrap_or(block);
        let amt = block.find("<Amt").and_then(|i| { let r = &block[i..]; let s = r.find('>')? + 1; let e = r.find("</Amt>")?; Some(r[s..e].to_string()) })
            .and_then(|a| money::parse_fixed(&a, 2)).ok_or_else(|| Error::Validation("CAMT entry missing amount".into()))?;
        let sign = if tag_value(block, "CdtDbtInd").as_deref() == Some("DBIT") { -1 } else { 1 };
        let date = block.find("<BookgDt>").and_then(|i| tag_value(&block[i..], "Dt"))
            .or_else(|| block.find("<ValDt>").and_then(|i| tag_value(&block[i..], "Dt"))).ok_or_else(|| Error::Validation("CAMT entry missing date".into()))?;
        let payee = if sign < 0 { block.find("<Cdtr>").and_then(|i| tag_value(&block[i..], "Nm")) } else { block.find("<Dbtr>").and_then(|i| tag_value(&block[i..], "Nm")) };
        out.push(StmtLine { date: date[..10.min(date.len())].to_string(), amount: sign * amt, payee: payee.unwrap_or_default(),
            description: tag_value(block, "Ustrd").or_else(|| tag_value(block, "AddtlNtryInf")).unwrap_or_default(),
            reference: tag_value(block, "EndToEndId").filter(|s| s != "NOTPROVIDED").or_else(|| tag_value(block, "NtryRef")).unwrap_or_default(),
            fitid: tag_value(block, "AcctSvcrRef"), ..Default::default() });
    }
    if out.is_empty() { return invalid("no entries found in CAMT.053 file"); }
    Ok(out)
}

pub fn parse_mt940(text: &str) -> Result<Vec<StmtLine>> {
    let mut out: Vec<StmtLine> = Vec::new();
    let mut in86 = false;
    for raw in text.lines() {
        let line = raw.trim_end();
        if let Some(rest) = line.strip_prefix(":61:") {
            in86 = false;
            if rest.len() < 12 { continue; }
            let date = format!("20{}-{}-{}", &rest[0..2], &rest[2..4], &rest[4..6]);
            let mut i = 6;
            if rest[i..].len() >= 4 && rest[i..i + 4].chars().all(|c| c.is_ascii_digit()) { i += 4; }
            let mut sign = 1;
            let r = &rest[i..];
            let (skip, s) = if r.starts_with("RC") { (2, 1) } else if r.starts_with("RD") { (2, -1) } else if r.starts_with('C') { (1, 1) } else if r.starts_with('D') { (1, -1) } else { (0, 1) };
            sign *= s; i += skip;
            if rest[i..].chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) { i += 1; } // funds code
            let amt_str: String = rest[i..].chars().take_while(|c| c.is_ascii_digit() || *c == ',').collect();
            let amount = money::parse_fixed(&amt_str.replace(',', "."), 2).ok_or_else(|| Error::Validation("MT940: invalid amount".into()))?;
            let after = &rest[i + amt_str.len()..];
            let reference = after.get(4..).unwrap_or("").split("//").next().unwrap_or("").to_string();
            out.push(StmtLine { date, amount: sign * amount, reference, ..Default::default() });
        } else if let Some(rest) = line.strip_prefix(":86:") {
            in86 = true;
            if let Some(l) = out.last_mut() { l.description = rest.to_string(); }
        } else if line.starts_with(':') {
            in86 = false;
        } else if in86 {
            if let Some(l) = out.last_mut() { l.description.push(' '); l.description.push_str(line); }
        }
    }
    if out.is_empty() { return invalid("no :61: statement lines found in MT940 file"); }
    for l in out.iter_mut() {
        // Many banks put the counterparty name after ?32 / /NAME/ in :86:
        if let Some(i) = l.description.find("/NAME/") { l.payee = l.description[i + 6..].split('/').next().unwrap_or("").trim().to_string(); }
        else if let Some(i) = l.description.find("?32") { l.payee = l.description[i + 3..].split('?').next().unwrap_or("").trim().to_string(); }
    }
    Ok(out)
}

/// Detect format from filename/content.
pub fn detect_format(filename: &str, text: &str) -> &'static str {
    let f = filename.to_lowercase();
    if f.ends_with(".ofx") || f.ends_with(".qfx") || text.contains("<OFX>") || text.contains("OFXHEADER") { "OFX" }
    else if f.ends_with(".qif") || text.starts_with("!Type") { "QIF" }
    else if text.contains("camt.053") || text.contains("<BkToCstmrStmt>") { "CAMT053" }
    else if f.ends_with(".sta") || f.ends_with(".mt940") || text.contains(":61:") { "MT940" }
    else { "CSV" }
}

// ---------------- Import & reconciliation ----------------

impl Ledger {
    pub fn import_statement(&self, account_id: i64, filename: &str, text: &str, format: Option<&str>, mapping: Option<&CsvMapping>, date_order: Option<&str>) -> Result<Value> {
        let fmt = format.filter(|f| !f.is_empty()).unwrap_or_else(|| detect_format(filename, text));
        let lines = match fmt {
            "OFX" | "QFX" => parse_ofx(text)?,
            "QIF" => parse_qif(text, date_order.unwrap_or("DMY"))?,
            "CAMT053" => parse_camt053(text)?,
            "MT940" => parse_mt940(text)?,
            _ => {
                let m = mapping.cloned().ok_or_else(|| Error::Validation("CSV files need a column mapping".into()))?;
                parse_csv(text, &m)?
            }
        };
        self.import_lines(account_id, filename, fmt, &lines)
    }

    pub fn import_lines(&self, account_id: i64, filename: &str, fmt: &str, lines: &[StmtLine]) -> Result<Value> {
        let ty: String = self.conn.query_row("SELECT type FROM accounts WHERE id = ?1", [account_id], |r| r.get(0))?;
        if ty != "BANK" { return invalid("statements can only be imported into bank accounts"); }
        self.atomic(|| {
            self.conn.execute("INSERT INTO statement_imports(account_id, filename, format) VALUES (?1, ?2, ?3)", params![account_id, filename, fmt])?;
            let imp = self.conn.last_insert_rowid();
            let (mut added, mut dupes) = (0, 0);
            let mut seen: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
            for l in lines {
                let base_key = match &l.fitid { Some(f) => format!("F:{f}"), None => format!("H:{}|{}|{}|{}|{}", l.date, l.amount, l.payee.to_lowercase(), l.description.to_lowercase(), l.reference.to_lowercase()) };
                let n = seen.entry(base_key.clone()).or_insert(0); *n += 1;
                let key = if l.fitid.is_some() { base_key } else { format!("{base_key}#{n}") };
                let r = self.conn.execute(
                    "INSERT OR IGNORE INTO statement_lines(account_id, import_id, date, amount, payee, description, reference, txn_type, balance, dedupe_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![account_id, imp, l.date, l.amount, l.payee, l.description, l.reference, l.txn_type, l.balance, key])?;
                if r == 1 { added += 1; } else { dupes += 1; }
            }
            let (s, e): (Option<String>, Option<String>) = self.conn.query_row("SELECT MIN(date), MAX(date) FROM statement_lines WHERE import_id = ?1", [imp], |r| Ok((r.get(0)?, r.get(1)?)))?;
            self.conn.execute("UPDATE statement_imports SET line_count = ?2, start_date = ?3, end_date = ?4 WHERE id = ?1", params![imp, added, s, e])?;
            self.audit("IMPORT", "statement", Some(imp), None, Some(json!({"account_id": account_id, "added": added, "duplicates": dupes})))?;
            Ok(json!({"import_id": imp, "added": added, "duplicates": dupes, "format": fmt}))
        })
    }

    pub fn save_csv_mapping(&self, account_id: Option<i64>, name: &str, m: &CsvMapping) -> Result<i64> {
        self.conn.execute("DELETE FROM csv_mappings WHERE account_id IS ?1 AND name = ?2", params![account_id, name])?;
        self.conn.execute("INSERT INTO csv_mappings(account_id, name, mapping) VALUES (?1, ?2, ?3)", params![account_id, name, serde_json::to_string(m)?])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn csv_mappings(&self, account_id: i64) -> Result<Vec<Value>> {
        let mut v = util::query_json(&self.conn, "SELECT * FROM csv_mappings WHERE account_id = ?1 OR account_id IS NULL ORDER BY id DESC", [account_id])?;
        for m in v.iter_mut() { m["mapping"] = serde_json::from_str(m["mapping"].as_str().unwrap_or("{}")).unwrap_or(json!({})); }
        Ok(v)
    }

    pub fn statement_lines(&self, account_id: i64, status: &str, limit: i64) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT s.* FROM statement_lines s WHERE s.account_id = ?1 AND (?2 = 'ALL' AND s.status <> 'DELETED' OR s.status = ?2) ORDER BY s.date, s.id LIMIT ?3",
            params![account_id, status, limit])
    }

    /// Signed amount (in bank account currency) of a ledger cash item.
    fn target_amount(&self, kind: &str, id: i64) -> Result<(i64, String, i64)> {
        Ok(match kind {
            "PAYMENT" => self.conn.query_row(
                "SELECT CASE WHEN a.currency = d.currency THEN p.amount ELSE p.base_amount END * CASE WHEN d.type IN ('ACCREC','ACCPAYCREDIT') THEN 1 ELSE -1 END, p.date, p.reconciled
                 FROM payments p JOIN documents d ON d.id = p.document_id JOIN accounts a ON a.id = p.account_id WHERE p.id = ?1 AND p.status <> 'DELETED'", [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?,
            "BANKTX" => self.conn.query_row("SELECT CASE WHEN type LIKE 'RECEIVE%' THEN total ELSE -total END, date, reconciled FROM bank_transactions WHERE id = ?1 AND status <> 'DELETED'", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?,
            "TRANSFER_FROM" => self.conn.query_row("SELECT -amount, date, from_reconciled FROM transfers WHERE id = ?1 AND status <> 'DELETED'", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?,
            "TRANSFER_TO" => self.conn.query_row("SELECT to_amount, date, to_reconciled FROM transfers WHERE id = ?1 AND status <> 'DELETED'", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?,
            _ => return invalid("unknown match kind"),
        })
    }

    fn set_target_reconciled(&self, kind: &str, id: i64, v: i64) -> Result<()> {
        let sql = match kind {
            "PAYMENT" => "UPDATE payments SET reconciled = ?2 WHERE id = ?1",
            "BANKTX" => "UPDATE bank_transactions SET reconciled = ?2 WHERE id = ?1",
            "TRANSFER_FROM" => "UPDATE transfers SET from_reconciled = ?2 WHERE id = ?1",
            "TRANSFER_TO" => "UPDATE transfers SET to_reconciled = ?2 WHERE id = ?1",
            _ => return invalid("unknown match kind"),
        };
        self.conn.execute(sql, params![id, v])?;
        Ok(())
    }

    /// Reconcile a statement line against one or more existing ledger transactions.
    /// `adjustment` (optional) posts a bank-fee/rounding difference to an account.
    pub fn reconcile_match(&self, line_id: i64, targets: &[(String, i64)]) -> Result<()> {
        let (amount, status, _account): (i64, String, i64) = self.conn.query_row("SELECT amount, status, account_id FROM statement_lines WHERE id = ?1", [line_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        if status != "UNRECONCILED" { return invalid("this statement line is already reconciled"); }
        if targets.is_empty() { return invalid("select at least one transaction"); }
        let mut sum = 0;
        let mut amounts = Vec::new();
        for (k, id) in targets {
            let (a, _d, rec) = self.target_amount(k, *id)?;
            if rec != 0 { return invalid("one of the selected transactions is already reconciled"); }
            sum += a; amounts.push(a);
        }
        if sum != amount { return invalid(format!("the selected transactions total {} but the statement line is {}", money::fmt_money(sum), money::fmt_money(amount))); }
        self.atomic(|| {
            for ((k, id), a) in targets.iter().zip(amounts) {
                self.set_target_reconciled(k, *id, 1)?;
                self.conn.execute("INSERT INTO reconciliation_matches(statement_line_id, kind, target_id, amount) VALUES (?1, ?2, ?3, ?4)", params![line_id, k, id, a])?;
            }
            self.conn.execute("UPDATE statement_lines SET status = 'RECONCILED', reconciled_at = datetime('now') WHERE id = ?1", [line_id])?;
            Ok(())
        })
    }

    /// Create a spend/receive money transaction from a statement line and reconcile it.
    pub fn reconcile_create(&self, line_id: i64, mut tx: BankTxInput) -> Result<i64> {
        let (amount, date, account, payee, reference): (i64, String, i64, String, String) = self.conn.query_row(
            "SELECT amount, date, account_id, payee, reference FROM statement_lines WHERE id = ?1 AND status = 'UNRECONCILED'", [line_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).map_err(|_| Error::Validation("statement line not found or already reconciled".into()))?;
        tx.account_id = account;
        tx.date = date;
        if tx.kind.is_empty() || !tx.kind.contains("PAYMENT") { tx.kind = if amount < 0 { "SPEND".into() } else { "RECEIVE".into() }; }
        else { tx.kind = format!("{}-{}", if amount < 0 { "SPEND" } else { "RECEIVE" }, if tx.kind.contains("PREPAYMENT") { "PREPAYMENT" } else { "OVERPAYMENT" }); tx.amount = Some(money::fmt_money(amount.abs())); }
        if tx.reference.is_empty() { tx.reference = reference; }
        if tx.contact_id.is_none() && tx.contact_name.as_deref().unwrap_or("").is_empty() && !payee.is_empty() { tx.contact_name = Some(payee); }
        // Single line without amount → whole statement amount
        if tx.lines.len() == 1 && tx.lines[0].amount.as_deref().unwrap_or("").is_empty() && tx.lines[0].unit_price.as_deref().unwrap_or("").is_empty() {
            tx.lines[0].amount = Some(money::fmt_money(amount.abs()));
        }
        self.atomic(|| {
            let id = self.save_bank_transaction(None, &tx)?;
            let total: i64 = self.conn.query_row("SELECT total FROM bank_transactions WHERE id = ?1", [id], |r| r.get(0))?;
            if total != amount.abs() { return invalid(format!("the transaction total {} must equal the statement amount {}", money::fmt_money(total), money::fmt_money(amount.abs()))); }
            self.reconcile_match(line_id, &[("BANKTX".into(), id)])?;
            Ok(id)
        })
    }

    /// Apply full/partial payments to invoices/bills from a statement line (amount per document).
    pub fn reconcile_pay_documents(&self, line_id: i64, docs: &[(i64, String)], extra: &[(String, i64)]) -> Result<()> {
        let (date, account, reference): (String, i64, String) = self.conn.query_row(
            "SELECT date, account_id, reference FROM statement_lines WHERE id = ?1 AND status = 'UNRECONCILED'", [line_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|_| Error::Validation("statement line not found or already reconciled".into()))?;
        self.atomic(|| {
            let mut targets: Vec<(String, i64)> = extra.to_vec();
            for (doc, amt) in docs {
                let pid = self.add_payment(&PaymentInput { document_id: *doc, account_id: account, date: date.clone(), amount: amt.clone(), rate: None, reference: reference.clone(), batch_id: None })?;
                targets.push(("PAYMENT".into(), pid));
            }
            self.reconcile_match(line_id, &targets)
        })
    }

    pub fn reconcile_transfer(&self, line_id: i64, other_account: i64, other_amount: Option<String>) -> Result<i64> {
        let (amount, date, account, reference): (i64, String, i64, String) = self.conn.query_row(
            "SELECT amount, date, account_id, reference FROM statement_lines WHERE id = ?1 AND status = 'UNRECONCILED'", [line_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        self.atomic(|| {
            let (from, to) = if amount < 0 { (account, other_account) } else { (other_account, account) };
            let a = money::fmt_money(amount.abs());
            let (amt, to_amt) = if amount < 0 { (a.clone(), other_amount.clone()) } else { (other_amount.clone().unwrap_or(a.clone()), Some(a.clone())) };
            let id = self.save_transfer(&TransferInput { from_account_id: from, to_account_id: to, date, amount: amt, to_amount: to_amt, reference })?;
            self.reconcile_match(line_id, &[(if amount < 0 { "TRANSFER_FROM".into() } else { "TRANSFER_TO".into() }, id)])?;
            Ok(id)
        })
    }

    pub fn unreconcile(&self, line_id: i64) -> Result<()> {
        self.atomic(|| {
            let matches: Vec<(String, i64)> = {
                let mut st = self.conn.prepare("SELECT kind, target_id FROM reconciliation_matches WHERE statement_line_id = ?1")?;
                st.query_map([line_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
            };
            for (k, id) in &matches { self.set_target_reconciled(k, *id, 0)?; }
            self.conn.execute("DELETE FROM reconciliation_matches WHERE statement_line_id = ?1", [line_id])?;
            self.conn.execute("UPDATE statement_lines SET status = 'UNRECONCILED', reconciled_at = NULL WHERE id = ?1", [line_id])?;
            self.audit("UNRECONCILE", "statement_line", Some(line_id), None, None)?;
            Ok(())
        })
    }

    pub fn delete_statement_line(&self, line_id: i64) -> Result<()> {
        let st: String = self.conn.query_row("SELECT status FROM statement_lines WHERE id = ?1", [line_id], |r| r.get(0))?;
        if st == "RECONCILED" { return invalid("unreconcile the line before deleting it"); }
        self.conn.execute("UPDATE statement_lines SET status = 'DELETED' WHERE id = ?1", [line_id])?;
        self.audit("DELETE", "statement_line", Some(line_id), None, None)?;
        Ok(())
    }

    pub fn set_statement_note(&self, line_id: i64, note: &str) -> Result<()> {
        self.conn.execute("UPDATE statement_lines SET notes = ?2 WHERE id = ?1", params![line_id, note])?;
        Ok(())
    }

    /// Reconciliation suggestions for a statement line: matching ledger transactions,
    /// open invoices/bills, and the bank rule (if any) that applies.
    pub fn reconcile_suggestions(&self, line_id: i64) -> Result<Value> {
        let (account, amount, date, payee, desc, reference): (i64, i64, String, String, String, String) = self.conn.query_row(
            "SELECT account_id, amount, date, payee, description, reference FROM statement_lines WHERE id = ?1", [line_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;
        let mut cands = self.unreconciled_ledger_items(account)?;
        let words: Vec<String> = format!("{payee} {desc} {reference}").to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 2).map(String::from).collect();
        let d0 = util::parse_date(&date)?;
        for c in cands.iter_mut() {
            let a = c["amount"].as_i64().unwrap_or(0);
            let cd = util::parse_date(c["date"].as_str().unwrap_or(&date)).unwrap_or(d0);
            let days = (cd - d0).num_days().abs();
            let name = format!("{} {}", c["contact"].as_str().unwrap_or(""), c["reference"].as_str().unwrap_or("")).to_lowercase();
            let name_hit = words.iter().any(|w| name.contains(w.as_str()));
            let mut score = 0i64;
            if a == amount { score += 60; }
            score += (30 - days.min(30)) as i64;
            if name_hit { score += 25; }
            c["score"] = json!(score);
        }
        cands.retain(|c| c["amount"].as_i64() == Some(amount) || c["score"].as_i64().unwrap_or(0) >= 50);
        cands.sort_by_key(|c| -c["score"].as_i64().unwrap_or(0));
        cands.truncate(10);
        // open documents
        let sales = amount > 0;
        let mut docs = util::query_json(&self.conn,
            "SELECT d.id, d.type, d.number, d.reference, d.date, d.due_date, c.name AS contact, d.currency, d.total - d.amount_paid - d.amount_credited AS amount_due
             FROM documents d JOIN contacts c ON c.id = d.contact_id
             WHERE d.status = 'AUTHORISED' AND d.type IN (?1, ?2) ORDER BY d.due_date LIMIT 500",
            params![if sales { "ACCREC" } else { "ACCPAY" }, if sales { "ACCPAYCREDIT" } else { "ACCRECCREDIT" }])?;
        for d in docs.iter_mut() {
            let due = d["amount_due"].as_i64().unwrap_or(0);
            let name = format!("{} {} {}", d["contact"].as_str().unwrap_or(""), d["number"].as_str().unwrap_or(""), d["reference"].as_str().unwrap_or("")).to_lowercase();
            let mut score = 0;
            if due == amount.abs() { score += 60; }
            if words.iter().any(|w| name.contains(w.as_str())) { score += 30; }
            let num = d["number"].as_str().unwrap_or("").to_lowercase();
            if !num.is_empty() && format!("{payee} {desc} {reference}").to_lowercase().contains(&num) { score += 40; }
            d["score"] = json!(score);
        }
        docs.retain(|d| d["score"].as_i64().unwrap_or(0) >= 60);
        docs.sort_by_key(|d| -d["score"].as_i64().unwrap_or(0));
        docs.truncate(5);
        let rule = self.match_rule(line_id)?.map(|(id, r)| json!({"id": id, "rule": r}));
        Ok(json!({"matches": cands, "documents": docs, "rule": rule}))
    }

    /// Unreconciled cash transactions on a bank account (for "find & match").
    pub fn unreconciled_ledger_items(&self, account: i64) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT 'PAYMENT' AS kind, p.id, p.date, CASE WHEN a.currency = d.currency THEN p.amount ELSE p.base_amount END * CASE WHEN d.type IN ('ACCREC','ACCPAYCREDIT') THEN 1 ELSE -1 END AS amount,
               c.name AS contact, COALESCE(NULLIF(p.reference, ''), d.number) AS reference, 'Payment: ' || d.number AS description
             FROM payments p JOIN documents d ON d.id = p.document_id JOIN contacts c ON c.id = d.contact_id JOIN accounts a ON a.id = p.account_id
             WHERE p.account_id = ?1 AND p.status <> 'DELETED' AND p.reconciled = 0
             UNION ALL
             SELECT 'BANKTX', b.id, b.date, CASE WHEN b.type LIKE 'RECEIVE%' THEN b.total ELSE -b.total END, c.name, b.reference, CASE WHEN b.type LIKE 'RECEIVE%' THEN 'Receive money' ELSE 'Spend money' END
             FROM bank_transactions b LEFT JOIN contacts c ON c.id = b.contact_id WHERE b.account_id = ?1 AND b.status <> 'DELETED' AND b.reconciled = 0
             UNION ALL
             SELECT 'TRANSFER_FROM', t.id, t.date, -t.amount, (SELECT name FROM accounts WHERE id = t.to_account_id), t.reference, 'Transfer out'
             FROM transfers t WHERE t.from_account_id = ?1 AND t.status <> 'DELETED' AND t.from_reconciled = 0
             UNION ALL
             SELECT 'TRANSFER_TO', t.id, t.date, t.to_amount, (SELECT name FROM accounts WHERE id = t.from_account_id), t.reference, 'Transfer in'
             FROM transfers t WHERE t.to_account_id = ?1 AND t.status <> 'DELETED' AND t.to_reconciled = 0
             ORDER BY date", [account])
    }

    /// Bulk "OK" all lines that have exactly one strong suggested match.
    pub fn auto_reconcile(&self, account: i64) -> Result<i64> {
        let lines: Vec<i64> = {
            let mut st = self.conn.prepare("SELECT id FROM statement_lines WHERE account_id = ?1 AND status = 'UNRECONCILED' ORDER BY date")?;
            st.query_map([account], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut n = 0;
        for l in lines {
            let s = self.reconcile_suggestions(l)?;
            let strong: Vec<&Value> = s["matches"].as_array().unwrap().iter().filter(|m| m["score"].as_i64().unwrap_or(0) >= 80).collect();
            if strong.len() == 1 {
                let m = strong[0];
                if self.reconcile_match(l, &[(m["kind"].as_str().unwrap().to_string(), m["id"].as_i64().unwrap())]).is_ok() { n += 1; }
            }
        }
        Ok(n)
    }

    /// Cash coding: create spend/receive transactions for many lines at once.
    pub fn cash_code(&self, rows: &[(i64, i64, Option<i64>, Option<String>, String)]) -> Result<i64> {
        self.atomic(|| {
            let mut n = 0;
            for (line, account, tax, contact, desc) in rows {
                self.reconcile_create(*line, BankTxInput {
                    contact_name: contact.clone(), line_amount_type: Some("INCLUSIVE".into()),
                    lines: vec![LineInput { description: desc.clone(), account_id: Some(*account), tax_rate_id: *tax, ..Default::default() }],
                    ..Default::default()
                })?;
                n += 1;
            }
            Ok(n)
        })
    }

    /// Bank reconciliation report as at a date.
    pub fn reconciliation_report(&self, account: i64, date: &str) -> Result<Value> {
        // Statement opening balance = ledger balance before the first imported statement line.
        let stmt_bal: i64 = self.conn.query_row("SELECT COALESCE(SUM(amount), 0) FROM statement_lines WHERE account_id = ?1 AND status <> 'DELETED' AND date <= ?2", params![account, date], |r| r.get::<_, i64>(0))?
            + self.conn.query_row("SELECT COALESCE(SUM(COALESCE(jl.fx_amount, jl.amount)), 0) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE jl.account_id = ?1 AND j.posted = 1
                AND j.date < (SELECT MIN(date) FROM statement_lines s WHERE s.account_id = ?1 AND s.status <> 'DELETED')", [account], |r| r.get::<_, i64>(0))?;
        let unrec_stmt = util::query_json(&self.conn, "SELECT id, date, payee, description, reference, amount FROM statement_lines WHERE account_id = ?1 AND status = 'UNRECONCILED' AND date <= ?2 ORDER BY date", params![account, date])?;
        let mut unrec_ledger = self.unreconciled_ledger_items(account)?;
        unrec_ledger.retain(|l| l["date"].as_str().unwrap_or("") <= date);
        let ledger_bal: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(COALESCE(jl.fx_amount, jl.amount)), 0) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE jl.account_id = ?1 AND j.posted = 1 AND j.date <= ?2", params![account, date], |r| r.get(0))?;
        let unrec_stmt_total: i64 = unrec_stmt.iter().map(|l| l["amount"].as_i64().unwrap_or(0)).sum();
        let unrec_ledger_total: i64 = unrec_ledger.iter().map(|l| l["amount"].as_i64().unwrap_or(0)).sum();
        let last_rec: Option<String> = self.conn.query_row("SELECT MAX(date) FROM statement_lines WHERE account_id = ?1 AND status = 'RECONCILED'", [account], |r| r.get(0)).optional()?.flatten();
        Ok(json!({
            "statement_balance": stmt_bal, "unreconciled_statement_lines": unrec_stmt, "unreconciled_statement_total": unrec_stmt_total,
            "unreconciled_ledger_items": unrec_ledger, "unreconciled_ledger_total": unrec_ledger_total,
            "ledger_balance": ledger_bal, "calculated_ledger_balance": stmt_bal - unrec_stmt_total + unrec_ledger_total,
            "difference": ledger_bal - (stmt_bal - unrec_stmt_total + unrec_ledger_total), "last_reconciled": last_rec,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_parse() {
        let t = "Date,Description,Amount\n01/02/2026,\"Coffee, beans\",-12.50\n02/02/2026,Deposit,100\n";
        let m = CsvMapping { has_header: true, date_col: 0, date_format: "DMY".into(), amount_col: Some(2), description_col: Some(1), ..Default::default() };
        let l = parse_csv(t, &m).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].date, "2026-02-01");
        assert_eq!(l[0].amount, -1250);
        assert_eq!(l[0].description, "Coffee, beans");
    }
    #[test]
    fn ofx_parse() {
        let t = "OFXHEADER:100\n<OFX><BANKTRANLIST><STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20260105120000<TRNAMT>-45.20<FITID>abc1<NAME>OFFICEWORKS<MEMO>Stationery</STMTTRN>\n<STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20260106<TRNAMT>1000.00<FITID>abc2<NAME>ACME PTY LTD</STMTTRN></BANKTRANLIST></OFX>";
        let l = parse_ofx(t).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0], StmtLine { date: "2026-01-05".into(), amount: -4520, payee: "OFFICEWORKS".into(), description: "Stationery".into(), reference: "".into(), txn_type: "DEBIT".into(), fitid: Some("abc1".into()), balance: None });
    }
    #[test]
    fn qif_parse() {
        let t = "!Type:Bank\nD03/01/2026\nT-20.00\nPCafe\nMLunch\n^\nD04/01/2026\nT50.00\nPRefund\n^\n";
        let l = parse_qif(t, "DMY").unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].date, "2026-01-03");
        assert_eq!(l[1].amount, 5000);
    }
    #[test]
    fn camt_parse() {
        let t = r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.02"><BkToCstmrStmt><Stmt><Ntry><Amt Ccy="EUR">250.00</Amt><CdtDbtInd>DBIT</CdtDbtInd><BookgDt><Dt>2026-03-04</Dt></BookgDt><AcctSvcrRef>REF1</AcctSvcrRef><NtryDtls><TxDtls><RltdPties><Cdtr><Nm>Landlord Ltd</Nm></Cdtr></RltdPties><RmtInf><Ustrd>March rent</Ustrd></RmtInf></TxDtls></NtryDtls></Ntry></Stmt></BkToCstmrStmt></Document>"#;
        let l = parse_camt053(t).unwrap();
        assert_eq!(l[0].amount, -25000);
        assert_eq!(l[0].payee, "Landlord Ltd");
        assert_eq!(l[0].date, "2026-03-04");
    }
    #[test]
    fn mt940_parse() {
        let t = ":20:STMT\n:25:123456\n:60F:C260101EUR1000,00\n:61:2601050105D45,20NTRFNONREF//X\n:86:/NAME/Office Supplies/REMI/Paper\n:61:260106C1000,00NTRFINV-1\n:86:Customer payment\n:62F:C260106EUR1954,80\n";
        let l = parse_mt940(t).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].amount, -4520);
        assert_eq!(l[0].payee, "Office Supplies");
        assert_eq!(l[1].amount, 100000);
        assert_eq!(l[1].date, "2026-01-06");
    }
    #[test]
    fn dates() {
        assert_eq!(parse_flex_date("12/31/2025", "MDY").as_deref(), Some("2025-12-31"));
        assert_eq!(parse_flex_date("5 Jan 2026", "DMY").as_deref(), Some("2026-01-05"));
        assert_eq!(parse_flex_date("2026-02-03", "DMY").as_deref(), Some("2026-02-03"));
        assert_eq!(parse_flex_date("3/2/26", "DMY").as_deref(), Some("2026-02-03"));
    }
}
