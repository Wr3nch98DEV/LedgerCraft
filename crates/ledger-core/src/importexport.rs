//! CSV import (Xero / QuickBooks Online / MYOB / Wave / generic column names), CSV & JSON export,
//! and bank batch-payment files (ABA, BACS, NACHA, SEPA pain.001).
//! SQL is parameterised; the only dynamic identifier is a table name read from sqlite_master and
//! validated to [A-Za-z0-9_]. `.format()` calls are chrono date formatting.

use crate::{accounts::AccountInput, contacts::ContactInput, documents::{DocInput, LineInput}, error::invalid, items::ItemInput, money, statements::{parse_csv_rows, parse_flex_date}, util, Error, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

struct Csv { head: Vec<String>, rows: Vec<Vec<String>> }
impl Csv {
    fn parse(text: &str) -> Result<Csv> {
        let mut rows = parse_csv_rows(text);
        if rows.is_empty() { return invalid("the file is empty"); }
        let head = rows.remove(0).into_iter().map(|h| h.trim().trim_start_matches('*').to_lowercase().replace([' ', '_'], "")).collect();
        Ok(Csv { head, rows })
    }
    fn col(&self, aliases: &[&str]) -> Option<usize> {
        aliases.iter().find_map(|a| { let a = a.to_lowercase().replace([' ', '_'], ""); self.head.iter().position(|h| *h == a) })
    }
    fn get<'a>(&self, row: &'a [String], aliases: &[&str]) -> &'a str { self.col(aliases).and_then(|i| row.get(i)).map(|s| s.trim()).unwrap_or("") }
}

fn type_from_label(s: &str) -> Option<&'static str> {
    let t = s.to_lowercase();
    Some(match t.as_str() {
        "bank" => "BANK", "current asset" | "current" => "CURRENT", "fixed asset" | "fixed" => "FIXED", "non-current asset" | "noncurrent" => "NONCURRENT",
        "inventory" => "INVENTORY", "prepayment" => "PREPAYMENT", "current liability" | "currliab" => "CURRLIAB", "liability" => "LIABILITY",
        "non-current liability" | "termliab" | "long term liability" => "TERMLIAB", "equity" => "EQUITY", "revenue" | "income" => "REVENUE", "sales" => "SALES",
        "other income" | "otherincome" => "OTHERINCOME", "direct costs" | "directcosts" | "cost of sales" | "cost of goods sold" => "DIRECTCOSTS",
        "expense" | "expenses" => "EXPENSE", "overhead" | "overheads" => "OVERHEADS", "depreciation" | "depreciatn" => "DEPRECIATN",
        _ => return None,
    })
}

fn csv_cell(v: &str) -> String { if v.contains([',', '"', '\n']) { String::from("\"") + &v.replace('"', "\"\"") + "\"" } else { v.to_string() } }
fn pad(s: &str, n: usize) -> String {
    let t: String = s.chars().filter(|c| c.is_ascii() && !c.is_control()).collect::<String>().to_uppercase();
    let mut t: String = t.chars().take(n).collect();
    while t.len() < n { t.push(' '); }
    t
}
fn digits(s: &str) -> String { s.chars().filter(|c| c.is_ascii_digit()).collect() }
fn ddmmyy(d: chrono::NaiveDate) -> String { use chrono::Datelike; format!("{:02}{:02}{:02}", d.day(), d.month(), d.year() % 100) }
fn yymmdd(d: chrono::NaiveDate) -> String { use chrono::Datelike; format!("{:02}{:02}{:02}", d.year() % 100, d.month(), d.day()) }

impl Ledger {
    fn tax_by_name(&self, name: &str, sales: bool) -> Option<i64> {
        let n = name.trim();
        if n.is_empty() { return None; }
        self.conn.query_row("SELECT id FROM tax_rates WHERE name = ?1 COLLATE NOCASE", [n], |r| r.get(0)).ok().or_else(|| {
            let l = n.to_lowercase();
            let rt = if l.contains("free") || l.contains("exempt") || l.contains("zero") || l.contains("no ") || l == "none" || l.contains("bas excluded") { if sales { "EXEMPTOUTPUT" } else { "EXEMPTINPUT" } }
                else if l.contains("income") || l.contains("output") || l.contains("sales") { "OUTPUT" } else if l.contains("expense") || l.contains("input") || l.contains("purchase") { "INPUT" }
                else if sales { "OUTPUT" } else { "INPUT" };
            self.conn.query_row("SELECT id FROM tax_rates WHERE report_type = ?1 AND status = 'ACTIVE' ORDER BY rate_bp DESC LIMIT 1", [rt], |r| r.get(0)).ok()
        })
    }

    /// Import a CSV. With `dry_run`, everything runs in a transaction that is rolled back.
    pub fn import_csv(&self, what: &str, text: &str, dry_run: bool, date: Option<&str>) -> Result<Value> {
        let csv = Csv::parse(text)?;
        let mut created = 0; let mut skipped = 0;
        let mut messages: Vec<String> = Vec::new(); let mut errors: Vec<String> = Vec::new();
        let r = self.atomic(|| {
            match what {
                "contacts" => for (i, row) in csv.rows.iter().enumerate() {
                    let name = csv.get(row, &["ContactName", "Name", "Customer", "Supplier", "Display Name", "Company", "Vendor"]);
                    if name.is_empty() { skipped += 1; continue; }
                    let exists: i64 = self.conn.query_row("SELECT COUNT(*) FROM contacts WHERE name = ?1 COLLATE NOCASE", [name], |r| r.get(0))?;
                    if exists > 0 { skipped += 1; messages.push(String::from("Skip existing contact ") + name); continue; }
                    let addr = ["POAddressLine1", "POAddressLine2", "POCity", "PORegion", "POPostalCode", "POCountry"].iter().map(|k| csv.get(row, &[k])).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
                    let addr = if addr.is_empty() { csv.get(row, &["Address", "Billing Address", "Street"]).to_string() } else { addr };
                    match self.create_contact(&ContactInput { name: name.into(), first_name: csv.get(row, &["FirstName", "First Name"]).into(), last_name: csv.get(row, &["LastName", "Last Name"]).into(),
                        email: csv.get(row, &["EmailAddress", "Email"]).into(), phone: csv.get(row, &["PhoneNumber", "Phone"]).into(), mobile: csv.get(row, &["MobileNumber", "Mobile"]).into(),
                        website: csv.get(row, &["Website"]).into(), tax_number: csv.get(row, &["TaxNumber", "ABN", "Tax ID", "VAT Number"]).into(), account_number: csv.get(row, &["AccountNumber", "Account No"]).into(),
                        bank_account: csv.get(row, &["BankAccountNumber", "Bank Account"]).into(), postal_address: addr, ..Default::default() }) {
                        Ok(_) => created += 1, Err(e) => errors.push(format!("Row {}: {e}", i + 2)),
                    }
                },
                "accounts" => for (i, row) in csv.rows.iter().enumerate() {
                    let code = csv.get(row, &["Code", "AccountCode", "Account Number"]); let name = csv.get(row, &["Name", "AccountName", "Account"]);
                    if code.is_empty() || name.is_empty() { skipped += 1; continue; }
                    if self.account_by_code(code)?.is_some() { skipped += 1; messages.push(String::from("Skip existing account ") + code); continue; }
                    let Some(kind) = type_from_label(csv.get(row, &["Type", "AccountType", "Account Type"])) else { errors.push(format!("Row {}: unknown account type", i + 2)); continue };
                    let sales = matches!(kind, "REVENUE" | "SALES" | "OTHERINCOME");
                    match self.create_account(&AccountInput { code: code.into(), name: name.into(), kind: kind.into(), tax_rate_id: self.tax_by_name(csv.get(row, &["Tax Code", "TaxCode", "Tax Rate"]), sales),
                        description: csv.get(row, &["Description"]).into(), ..Default::default() }) {
                        Ok(_) => created += 1, Err(e) => errors.push(format!("Row {}: {e}", i + 2)),
                    }
                },
                "items" => for (i, row) in csv.rows.iter().enumerate() {
                    let code = csv.get(row, &["ItemCode", "Code", "SKU", "Product/Service", "Item"]);
                    if code.is_empty() { skipped += 1; continue; }
                    if self.conn.query_row("SELECT COUNT(*) FROM items WHERE code = ?1", [code], |r| r.get::<_, i64>(0))? > 0 { skipped += 1; continue; }
                    let acct = |k: &[&str]| self.account_by_code(csv.get(row, k)).ok().flatten();
                    match self.create_item(&ItemInput { code: code.into(), name: csv.get(row, &["ItemName", "Name", "Description"]).into(), sales_description: csv.get(row, &["SalesDescription", "Description"]).into(),
                        purchase_description: csv.get(row, &["PurchasesDescription", "Purchase Description"]).into(), sale_price: Some(csv.get(row, &["SalesUnitPrice", "Sales Price", "Price", "Rate"]).into()),
                        purchase_price: Some(csv.get(row, &["PurchasesUnitPrice", "Cost", "Purchase Cost"]).into()), sale_account_id: acct(&["SalesAccount"]), purchase_account_id: acct(&["PurchasesAccount"]),
                        sale_tax_rate_id: self.tax_by_name(csv.get(row, &["SalesTaxRate"]), true), purchase_tax_rate_id: self.tax_by_name(csv.get(row, &["PurchasesTaxRate"]), false), ..Default::default() }) {
                        Ok(_) => created += 1, Err(e) => errors.push(format!("Row {}: {e}", i + 2)),
                    }
                },
                "invoices" | "bills" => {
                    let sales = what == "invoices";
                    let mut groups: Vec<(String, Vec<&Vec<String>>)> = Vec::new();
                    for row in &csv.rows {
                        let num = csv.get(row, &["InvoiceNumber", "Invoice No", "Number", "Bill No"]).to_string();
                        if num.is_empty() { continue; }
                        match groups.iter_mut().find(|g| g.0 == num) { Some(g) => g.1.push(row), None => groups.push((num, vec![row])) }
                    }
                    for (num, rows) in groups {
                        let first = rows[0];
                        let kind = if sales { "ACCREC" } else { "ACCPAY" };
                        let dup: i64 = self.conn.query_row("SELECT COUNT(*) FROM documents WHERE type = ?1 AND number = ?2", params![kind, num], |r| r.get(0))?;
                        if dup > 0 { skipped += 1; messages.push(String::from("Skip existing ") + &num); continue; }
                        let Some(d) = parse_flex_date(csv.get(first, &["InvoiceDate", "Date", "Bill Date"]), "DMY") else { errors.push(num.clone() + ": invalid date"); continue };
                        let due = parse_flex_date(csv.get(first, &["DueDate", "Due Date"]), "DMY");
                        let lines: Vec<LineInput> = rows.iter().map(|r| LineInput {
                            description: csv.get(r, &["Description", "Memo"]).into(), quantity: Some(csv.get(r, &["Quantity", "Qty"]).to_string()).filter(|s| !s.is_empty()).or(Some("1".into())),
                            unit_price: Some(csv.get(r, &["UnitAmount", "Unit Price", "Rate", "Amount"]).into()), discount: Some(csv.get(r, &["Discount"]).into()),
                            account_id: self.account_by_code(csv.get(r, &["AccountCode", "Account"])).ok().flatten(), tax_rate_id: self.tax_by_name(csv.get(r, &["TaxType", "Tax Rate", "Tax Code"]), sales),
                            ..Default::default() }).collect();
                        let lat = match csv.get(first, &["LineAmountTypes", "Amounts Are"]).to_lowercase().as_str() { "inclusive" => "INCLUSIVE", "notax" | "no tax" => "NOTAX", _ => "EXCLUSIVE" };
                        match self.save_document(None, &DocInput { kind: kind.into(), contact_name: Some(csv.get(first, &["ContactName", "Customer", "Supplier", "Vendor"]).into()), number: Some(num.clone()), date: d, due_date: due,
                            reference: csv.get(first, &["Reference"]).into(), currency: Some(csv.get(first, &["Currency"]).to_string()).filter(|s| !s.is_empty()), line_amount_type: Some(lat.into()), lines,
                            status: Some("AUTHORISED".into()), ..Default::default() }) {
                            Ok(_) => { created += 1; messages.push(num.clone() + " approved"); } Err(e) => errors.push(format!("{num}: {e}")),
                        }
                    }
                }
                "journals" => {
                    let mut groups: Vec<(String, Vec<&Vec<String>>)> = Vec::new();
                    for row in &csv.rows {
                        let key = String::from(csv.get(row, &["Narration", "Journal No", "JournalNumber"])) + "|" + csv.get(row, &["Date"]);
                        match groups.iter_mut().find(|g| g.0 == key) { Some(g) => g.1.push(row), None => groups.push((key, vec![row])) }
                    }
                    for (key, rows) in groups {
                        let (narr, d) = key.split_once('|').unwrap();
                        let Some(d) = parse_flex_date(d, "DMY") else { errors.push(String::from(narr) + ": invalid date"); continue };
                        let lines: Vec<crate::advanced::MjLine> = rows.iter().map(|r| {
                            let amt = money::parse_fixed(csv.get(r, &["Amount"]), 2).unwrap_or(0);
                            let dr = money::parse_fixed(csv.get(r, &["Debit"]), 2).unwrap_or(0); let cr = money::parse_fixed(csv.get(r, &["Credit"]), 2).unwrap_or(0);
                            let v = if amt != 0 { amt } else { dr - cr };
                            crate::advanced::MjLine { description: csv.get(r, &["Description"]).into(), account_id: self.account_by_code(csv.get(r, &["AccountCode", "Account"])).ok().flatten(),
                                debit: if v > 0 { Some(money::fmt_money(v)) } else { None }, credit: if v < 0 { Some(money::fmt_money(-v)) } else { None }, ..Default::default() }
                        }).collect();
                        match self.save_manual_journal(None, &crate::advanced::ManualJournalInput { date: d, narration: narr.into(), line_amount_type: Some("NOTAX".into()), lines, status: Some("POSTED".into()), ..Default::default() }) {
                            Ok(_) => created += 1, Err(e) => errors.push(format!("{narr}: {e}")),
                        }
                    }
                }
                "trial-balance" => {
                    let date = date.ok_or_else(|| Error::Validation("choose the conversion date".into()))?;
                    let mut lines = Vec::new();
                    for row in &csv.rows {
                        let code = csv.get(row, &["AccountCode", "Code", "Account Code"]);
                        let acct = if !code.is_empty() { self.account_by_code(code)? } else {
                            let name = csv.get(row, &["Account", "AccountName", "Name"]);
                            let n = name.split(" (").next().unwrap_or(name);
                            self.conn.query_row("SELECT id FROM accounts WHERE name = ?1 COLLATE NOCASE", [n], |r| r.get(0)).ok()
                        };
                        let Some(a) = acct else { skipped += 1; continue };
                        lines.push(json!({"account_id": a, "debit": csv.get(row, &["Debit"]), "credit": csv.get(row, &["Credit"])}));
                        created += 1;
                    }
                    self.save_conversion_balances(date, &lines)?;
                }
                _ => return invalid("unknown import type"),
            }
            if dry_run { Err(Error::Validation("__dry_run__".into())) } else { Ok(()) }
        });
        match r { Ok(()) => {}, Err(Error::Validation(m)) if m == "__dry_run__" => {}, Err(e) => return Err(e) }
        Ok(json!({"created": created, "skipped": skipped, "messages": messages, "errors": errors, "dry_run": dry_run}))
    }

    pub fn export_csv(&self, what: &str) -> Result<String> {
        let (sql, cols): (&str, &[&str]) = match what {
            "contacts" => ("SELECT name, first_name, last_name, email, phone, mobile, website, tax_number, account_number, postal_address, is_customer, is_supplier, status FROM contacts WHERE merged_into IS NULL ORDER BY name",
                &["*ContactName", "FirstName", "LastName", "EmailAddress", "PhoneNumber", "MobileNumber", "Website", "TaxNumber", "AccountNumber", "POAddressLine1", "IsCustomer", "IsSupplier", "Status"]),
            "accounts" => ("SELECT a.code, a.name, a.type, t.name, a.description, a.status FROM accounts a LEFT JOIN tax_rates t ON t.id = a.tax_rate_id ORDER BY a.code", &["*Code", "*Name", "*Type", "*Tax Code", "Description", "Status"]),
            "items" => ("SELECT code, name, purchase_description, purchase_unit_e2 / 10000.0, sales_description, sale_unit_e2 / 10000.0, is_tracked, qty_on_hand_e4 / 10000.0 FROM items ORDER BY code",
                &["*ItemCode", "ItemName", "PurchasesDescription", "PurchasesUnitPrice", "SalesDescription", "SalesUnitPrice", "Tracked", "QuantityOnHand"]),
            "invoices" | "bills" => ("SELECT c.name, d.number, d.reference, d.date, d.due_date, d.currency, d.status, li.description, li.qty_e4 / 10000.0, li.unit_e2 / 10000.0, a.code, t.name, li.net_amount / 100.0, li.tax_amount / 100.0
                FROM documents d JOIN contacts c ON c.id = d.contact_id JOIN line_items li ON li.parent_kind = 'DOC' AND li.parent_id = d.id LEFT JOIN accounts a ON a.id = li.account_id LEFT JOIN tax_rates t ON t.id = li.tax_rate_id
                WHERE d.type = CASE ?1 WHEN 'invoices' THEN 'ACCREC' ELSE 'ACCPAY' END AND d.status <> 'DELETED' ORDER BY d.date, d.id, li.sort",
                &["*ContactName", "*InvoiceNumber", "Reference", "*InvoiceDate", "*DueDate", "Currency", "Status", "Description", "*Quantity", "*UnitAmount", "*AccountCode", "*TaxType", "LineAmount", "TaxAmount"]),
            "journals" => ("SELECT j.number, j.date, j.source_type, j.reference, a.code, a.name, jl.description, CASE WHEN jl.amount > 0 THEN jl.amount / 100.0 END, CASE WHEN jl.amount < 0 THEN -jl.amount / 100.0 END, t.name, jl.tax_amount / 100.0
                FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id JOIN accounts a ON a.id = jl.account_id LEFT JOIN tax_rates t ON t.id = jl.tax_rate_id WHERE j.posted = 1 ORDER BY j.date, j.number, jl.id",
                &["JournalNumber", "Date", "Source", "Reference", "AccountCode", "AccountName", "Description", "Debit", "Credit", "TaxRate", "TaxAmount"]),
            _ => return invalid("unknown export"),
        };
        let mut out = cols.join(",") + "\r\n";
        let mut st = self.conn.prepare(sql)?;
        let n = st.column_count();
        let mut rows = if sql.contains("?1") { st.query([what])? } else { st.query([])? };
        while let Some(r) = rows.next()? {
            let cells: Vec<String> = (0..n).map(|i| {
                let v = match r.get_ref(i).unwrap() { rusqlite::types::ValueRef::Null => String::new(), rusqlite::types::ValueRef::Integer(x) => x.to_string(),
                    rusqlite::types::ValueRef::Real(f) => { let s = format!("{f:.4}"); s.trim_end_matches('0').trim_end_matches('.').to_string() },
                    rusqlite::types::ValueRef::Text(t) => String::from_utf8_lossy(t).to_string(), _ => String::new() };
                csv_cell(&v)
            }).collect();
            out += &(cells.join(",") + "\r\n");
        }
        Ok(out)
    }

    /// Full-organisation JSON archive (every table except attachment bodies).
    pub fn export_archive(&self) -> Result<Value> {
        let tables: Vec<String> = { let mut st = self.conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'search_index%' ORDER BY name")?; st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()? };
        let mut m = serde_json::Map::new();
        for t in tables {
            if t == "attachments" { m.insert(t, json!(util::query_json(&self.conn, "SELECT id, entity, entity_id, filename, mime, size, created_at FROM attachments", [])?)); continue; }
            if !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { continue; }
            let sql = String::from("SELECT * FROM \"") + &t + "\"";
            m.insert(t, json!(util::query_json(&self.conn, &sql, [])?));
        }
        Ok(json!({"format": "LedgerCraft archive", "version": 1, "exported_at": chrono::Utc::now().to_rfc3339(), "organisation": self.setting_or("org_name", "")?, "tables": m}))
    }

    /// Bank batch payment file for a batch of supplier payments. Returns (filename, contents).
    pub fn batch_file(&self, id: i64, format: &str) -> Result<(String, String)> {
        let b = self.batch_payment_detail(id)?;
        self.bank_file(&b, format, &format!("batch-{id}"))
    }

    /// Bank payment file for a posted pay run: each employee's net pay to their bank account.
    pub fn payrun_file(&self, id: i64, format: &str) -> Result<(String, String)> {
        let r = util::query_one_json(&self.conn, "SELECT number, payment_date, status FROM pay_runs WHERE id = ?1", [id])?;
        if r.is_null() { return crate::error::invalid("pay run not found"); }
        let number = r["number"].as_str().unwrap_or("").to_string();
        // Debit the account the net wages were paid from (falls back to the first bank account).
        let debit: String = self.conn.query_row(
            "SELECT COALESCE((SELECT a.bank_account_number FROM bank_transactions b JOIN accounts a ON a.id = b.account_id WHERE b.reference = ?1 AND b.status <> 'DELETED' ORDER BY b.id DESC LIMIT 1),
                             (SELECT bank_account_number FROM accounts WHERE type = 'BANK' AND status = 'ACTIVE' ORDER BY code LIMIT 1), '')",
            [format!("Net wages {number}")], |r| r.get(0))?;
        let pays: Vec<Value> = util::query_json(&self.conn,
            "SELECT s.net AS amount, e.first_name || ' ' || e.last_name AS contact_name, e.bank_account FROM payslips s JOIN employees e ON e.id = s.employee_id
             WHERE s.pay_run_id = ?1 AND s.net > 0 ORDER BY e.last_name, e.first_name", [id])?
            .into_iter().map(|mut p| { p["number"] = Value::from(format!("WAGES {number}")); p }).collect();
        let b = serde_json::json!({"date": r["payment_date"], "bank_account_number": debit, "payments": pays});
        self.bank_file(&b, format, &format!("payrun-{number}"))
    }

    /// ABA / BACS / NACHA / SEPA file from `{date, bank_account_number, payments: [{amount, contact_name, bank_account, number}]}`.
    fn bank_file(&self, b: &Value, format: &str, stem: &str) -> Result<(String, String)> {
        let missing: Vec<String> = b["payments"].as_array().map(|ps| ps.iter()
            .filter(|p| digits(p["bank_account"].as_str().unwrap_or("")).is_empty() && p["bank_account"].as_str().unwrap_or("").trim().is_empty())
            .map(|p| p["contact_name"].as_str().unwrap_or("?").to_string()).collect()).unwrap_or_default();
        if !missing.is_empty() {
            return invalid(format!("Add bank account details for {} before creating a bank file.", missing.join(", ")));
        }
        let org = self.setting_or("legal_name", "")?.to_uppercase();
        let date = util::parse_date(b["date"].as_str().unwrap_or(""))?;
        let acct = digits(b["bank_account_number"].as_str().unwrap_or(""));
        let pays = b["payments"].as_array().cloned().unwrap_or_default();
        let total: i64 = pays.iter().map(|p| p["amount"].as_i64().unwrap_or(0)).sum();
        let split = |a: &str, n: usize| -> (String, String) { if a.len() > n { (a[..n].to_string(), a[n..].to_string()) } else { (String::new(), a.to_string()) } };
        Ok(match format {
            "ABA" => {
                let (bsb, an) = split(&acct, 6);
                let bsb_f = |b: &str| if b.len() == 6 { format!("{}-{}", &b[..3], &b[3..]) } else { "000-000".to_string() };
                let mut s = format!("0                 01{}       {}{:06}{}{}{}\r\n", pad("", 3), pad(&org, 26), 0, pad("PAYMENTS", 12), ddmmyy(date), pad("", 40));
                for p in &pays {
                    let (pb, pa) = split(&digits(p["bank_account"].as_str().unwrap_or("")), 6);
                    s += &format!("1{}{:>9} 50{:010}{}{}{}{:>9}{}{:08}\r\n", bsb_f(&pb), pa, p["amount"].as_i64().unwrap_or(0), pad(p["contact_name"].as_str().unwrap_or(""), 32),
                        pad(p["number"].as_str().unwrap_or(""), 18), bsb_f(&bsb), an, pad(&org, 16), 0);
                }
                s += &format!("7999-999{}{:010}{:010}{:010}{}{:06}{}\r\n", pad("", 12), total, 0, total, pad("", 24), pays.len(), pad("", 40));
                (format!("{stem}.aba"), s)
            }
            "BACS" => {
                let mut s = String::from("Sort code,Account number,Account name,Amount,Reference\r\n");
                for p in &pays { let (sc, an) = split(&digits(p["bank_account"].as_str().unwrap_or("")), 6);
                    s += &format!("{sc},{an},{},{},{}\r\n", csv_cell(p["contact_name"].as_str().unwrap_or("")), money::fmt_money(p["amount"].as_i64().unwrap_or(0)), csv_cell(p["number"].as_str().unwrap_or(""))); }
                (format!("{stem}-bacs.csv"), s)
            }
            "NACHA" => {
                let d = yymmdd(date);
                let mut s = format!("101 {:>9}{:>10}{d}0000A094101{}{}{}\r\n", "", "", pad("", 23), pad(&org, 23), pad("", 8));
                s += &format!("5220{}{}{:>10}PPDPAYMENTS   {d}{d}   1{:>8}{:07}\r\n", pad(&org, 16), pad("", 20), "", "", 1);
                for (i, p) in pays.iter().enumerate() { let (rt, an) = split(&digits(p["bank_account"].as_str().unwrap_or("")), 9);
                    s += &format!("622{:0>9}{}{:010}{}{}  0{:>15}\r\n", rt, pad(&an, 17), p["amount"].as_i64().unwrap_or(0), pad("", 15), pad(p["contact_name"].as_str().unwrap_or(""), 22), i + 1); }
                s += &format!("8220{:06}{:010}{:012}{:012}{}{}{:07}\r\n", pays.len(), 0, total, 0, pad("", 35), pad("", 8), 1);
                s += &format!("9000001000001{:08}{:010}{:012}{:012}{}\r\n", pays.len(), 0, total, 0, pad("", 39));
                (format!("{stem}.ach"), s)
            }
            _ => {
                let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                let mut tx = String::new();
                for (i, p) in pays.iter().enumerate() {
                    tx += &format!("<CdtTrfTxInf><PmtId><EndToEndId>{}</EndToEndId></PmtId><Amt><InstdAmt Ccy=\"EUR\">{}</InstdAmt></Amt><Cdtr><Nm>{}</Nm></Cdtr><CdtrAcct><Id><IBAN>{}</IBAN></Id></CdtrAcct><RmtInf><Ustrd>{}</Ustrd></RmtInf></CdtTrfTxInf>",
                        i + 1, money::fmt_money(p["amount"].as_i64().unwrap_or(0)), esc(p["contact_name"].as_str().unwrap_or("")), esc(&p["bank_account"].as_str().unwrap_or("").replace(' ', "")), esc(p["number"].as_str().unwrap_or("")));
                }
                let d = util::fmt_date(date);
                (format!("{stem}-sepa.xml"), format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:pain.001.001.03\"><CstmrCdtTrfInitn><GrpHdr><MsgId>LC-{stem}</MsgId><CreDtTm>{d}T00:00:00</CreDtTm><NbOfTxs>{}</NbOfTxs><CtrlSum>{}</CtrlSum><InitgPty><Nm>{}</Nm></InitgPty></GrpHdr><PmtInf><PmtInfId>LC-{stem}</PmtInfId><PmtMtd>TRF</PmtMtd><ReqdExctnDt>{d}</ReqdExctnDt><Dbtr><Nm>{}</Nm></Dbtr><DbtrAcct><Id><IBAN>{}</IBAN></Id></DbtrAcct><DbtrAgt><FinInstnId/></DbtrAgt>{tx}</PmtInf></CstmrCdtTrfInitn></Document>",
                    pays.len(), money::fmt_money(total), esc(&org), esc(&org), esc(b["bank_account_number"].as_str().unwrap_or(""))))
            }
        })
    }
}
