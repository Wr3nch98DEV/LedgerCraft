use crate::{error::invalid, util, Error, Ledger, Result};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

pub const ACCOUNT_TYPES: &[(&str, &str, &str)] = &[
    ("BANK", "ASSET", "Bank"),
    ("CURRENT", "ASSET", "Current Asset"),
    ("FIXED", "ASSET", "Fixed Asset"),
    ("NONCURRENT", "ASSET", "Non-current Asset"),
    ("INVENTORY", "ASSET", "Inventory"),
    ("PREPAYMENT", "ASSET", "Prepayment"),
    ("CURRLIAB", "LIABILITY", "Current Liability"),
    ("LIABILITY", "LIABILITY", "Liability"),
    ("TERMLIAB", "LIABILITY", "Non-current Liability"),
    ("EQUITY", "EQUITY", "Equity"),
    ("REVENUE", "REVENUE", "Revenue"),
    ("SALES", "REVENUE", "Sales"),
    ("OTHERINCOME", "REVENUE", "Other Income"),
    ("DIRECTCOSTS", "EXPENSE", "Direct Costs"),
    ("EXPENSE", "EXPENSE", "Expense"),
    ("OVERHEADS", "EXPENSE", "Overhead"),
    ("DEPRECIATN", "EXPENSE", "Depreciation"),
];

pub fn class_of(t: &str) -> Result<&'static str> {
    ACCOUNT_TYPES.iter().find(|(ty, _, _)| *ty == t).map(|(_, c, _)| *c).ok_or_else(|| Error::Validation(format!("unknown account type {t}")))
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AccountInput {
    pub code: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub tax_rate_id: Option<i64>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub enable_payments: bool,
    #[serde(default)]
    pub show_in_expense_claims: bool,
    #[serde(default)]
    pub dashboard_watch: bool,
    #[serde(default)]
    pub bank_type: Option<String>,
    #[serde(default)]
    pub bank_account_number: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
    /// QuickBooks detail type; `None` on update keeps the current one.
    #[serde(default)]
    pub detail_type: Option<String>,
}

impl Ledger {
    pub fn sys_account(&self, key: &str) -> Result<i64> {
        self.conn.query_row("SELECT id FROM accounts WHERE system_account = ?1", [key], |r| r.get(0)).optional()?
            .ok_or_else(|| Error::NotFound(format!("system account {key}")))
    }

    pub fn account_by_code(&self, code: &str) -> Result<Option<i64>> {
        Ok(self.conn.query_row("SELECT id FROM accounts WHERE code = ?1", [code], |r| r.get(0)).optional()?)
    }

    pub fn account_class(&self, id: i64) -> Result<String> {
        Ok(self.conn.query_row("SELECT class FROM accounts WHERE id = ?1", [id], |r| r.get(0))?)
    }

    /// Chart of accounts with YTD balances (debit positive).
    pub fn list_accounts(&self) -> Result<Vec<Value>> {
        let fy = self.fy_start(&util::today())?;
        util::query_json(&self.conn,
            "SELECT a.*, t.name AS tax_rate_name,
               COALESCE((SELECT SUM(jl.amount) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id
                  WHERE jl.account_id = a.id AND j.posted = 1 AND (a.class IN ('ASSET','LIABILITY','EQUITY') OR j.date >= ?1)), 0) AS ytd_balance,
               (SELECT COUNT(*) FROM journal_lines jl WHERE jl.account_id = a.id) AS line_count
             FROM accounts a LEFT JOIN tax_rates t ON t.id = a.tax_rate_id
             ORDER BY CASE a.class WHEN 'ASSET' THEN 1 WHEN 'LIABILITY' THEN 2 WHEN 'EQUITY' THEN 3 WHEN 'REVENUE' THEN 4 ELSE 5 END, a.code", [fy])
    }

    pub fn get_account(&self, id: i64) -> Result<Value> {
        util::query_one_json(&self.conn, "SELECT * FROM accounts WHERE id = ?1", [id])
    }

    pub fn create_account(&self, a: &AccountInput) -> Result<i64> {
        if a.name.trim().is_empty() { return invalid("account name is required"); }
        if a.code.len() > 10 { return invalid("account code must be 10 characters or fewer"); }
        let class = class_of(&a.kind)?;
        // Account numbers are optional in QuickBooks: pick the next free one in the type's range.
        let code = if a.code.trim().is_empty() { self.next_account_code(&a.kind)? } else { a.code.trim().to_string() };
        if self.account_by_code(&code)?.is_some() { return invalid(format!("account code {code} is already used")); }
        let is_bank = a.kind == "BANK";
        self.conn.execute(
            "INSERT INTO accounts(code, name, type, class, tax_rate_id, description, enable_payments, show_in_expense_claims, dashboard_watch, bank_type, bank_account_number, currency, detail_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![code, a.name.trim(), a.kind, class, a.tax_rate_id, a.description, (a.enable_payments || is_bank) as i64,
                a.show_in_expense_claims as i64, (a.dashboard_watch || is_bank) as i64,
                if is_bank { Some(a.bank_type.clone().unwrap_or_else(|| "BANK".into())) } else { None },
                a.bank_account_number, if is_bank { Some(a.currency.clone().unwrap_or(self.base_currency()?)) } else { None }, a.detail_type.clone().unwrap_or_default().trim()],
        )?;
        let id = self.conn.last_insert_rowid();
        self.audit("CREATE", "account", Some(id), None, Some(json!({"code": code, "name": a.name, "type": a.kind})))?;
        self.index_search("account", id, &format!("{} {}", code, a.name), &a.description)?;
        Ok(id)
    }

    /// Next free numeric code after the highest one used by accounts of this type
    /// (or of its class), e.g. 404 after 403 for an expense.
    pub fn next_account_code(&self, kind: &str) -> Result<String> {
        let class = class_of(kind)?;
        let codes: Vec<(String, String)> = {
            let mut st = self.conn.prepare("SELECT code, type FROM accounts WHERE class = ?1")?;
            st.query_map([class], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?
        };
        let max = |same_type: bool| codes.iter().filter(|(_, t)| !same_type || t == kind).filter_map(|(c, _)| c.parse::<i64>().ok()).max();
        let base = max(true).or_else(|| max(false)).unwrap_or(match class { "ASSET" => 1000, "LIABILITY" => 2000, "EQUITY" => 3000, "REVENUE" => 4000, _ => 6000 });
        let mut n = base + 1;
        while self.account_by_code(&n.to_string())?.is_some() { n += 1; }
        Ok(n.to_string())
    }

    pub fn update_account(&self, id: i64, a: &AccountInput) -> Result<()> {
        let before = self.get_account(id)?;
        let class = class_of(&a.kind)?;
        if before["class"].as_str() != Some(class) && before["system_account"].is_string() {
            return invalid("cannot change the class of a system account");
        }
        if let Some(other) = self.account_by_code(a.code.trim())? { if other != id { return invalid("account code already used"); } }
        if before["type"] == "BANK" && a.kind != "BANK" { return invalid("bank accounts cannot change type"); }
        self.conn.execute(
            "UPDATE accounts SET code = ?2, name = ?3, type = ?4, class = ?5, tax_rate_id = ?6, description = ?7, enable_payments = ?8,
               show_in_expense_claims = ?9, dashboard_watch = ?10, bank_account_number = COALESCE(?11, bank_account_number), detail_type = COALESCE(?12, detail_type) WHERE id = ?1",
            params![id, a.code.trim(), a.name.trim(), a.kind, class, a.tax_rate_id, a.description, a.enable_payments as i64,
                a.show_in_expense_claims as i64, a.dashboard_watch as i64, a.bank_account_number, a.detail_type.as_deref().map(str::trim)],
        )?;
        self.audit("UPDATE", "account", Some(id), Some(before), Some(json!({"code": a.code, "name": a.name, "type": a.kind})))?;
        self.index_search("account", id, &format!("{} {}", a.code, a.name), &a.description)?;
        Ok(())
    }

    pub fn set_account_status(&self, id: i64, status: &str) -> Result<()> {
        let sys: Option<String> = self.conn.query_row("SELECT system_account FROM accounts WHERE id = ?1", [id], |r| r.get(0))?;
        if sys.is_some() && status == "ARCHIVED" { return invalid("system accounts cannot be archived"); }
        self.conn.execute("UPDATE accounts SET status = ?2 WHERE id = ?1", params![id, status])?;
        self.audit(status, "account", Some(id), None, None)?;
        Ok(())
    }

    pub fn delete_account(&self, id: i64) -> Result<()> {
        let used: i64 = self.conn.query_row(
            "SELECT (SELECT COUNT(*) FROM journal_lines WHERE account_id = ?1) + (SELECT COUNT(*) FROM line_items WHERE account_id = ?1)", [id], |r| r.get(0))?;
        if used > 0 { return invalid("this account has transactions; archive it instead"); }
        let sys: Option<String> = self.conn.query_row("SELECT system_account FROM accounts WHERE id = ?1", [id], |r| r.get(0))?;
        if sys.is_some() { return invalid("system accounts cannot be deleted"); }
        self.conn.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
        self.audit("DELETE", "account", Some(id), None, None)?;
        Ok(())
    }

    /// Bulk change the default tax rate of accounts.
    pub fn bulk_set_account_tax(&self, ids: &[i64], tax_rate_id: Option<i64>) -> Result<()> {
        self.atomic(|| {
            for id in ids { self.conn.execute("UPDATE accounts SET tax_rate_id = ?2 WHERE id = ?1", params![id, tax_rate_id])?; }
            Ok(())
        })
    }

    pub fn bank_accounts(&self) -> Result<Vec<Value>> {
        let mut rows = util::query_json(&self.conn,
            "SELECT a.id, a.code, a.name, a.bank_type, a.bank_account_number, a.currency, a.status,
               COALESCE((SELECT SUM(jl.amount) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE jl.account_id = a.id AND j.posted = 1), 0) AS ledger_balance,
               COALESCE((SELECT SUM(COALESCE(jl.fx_amount, jl.amount)) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE jl.account_id = a.id AND j.posted = 1), 0) AS ledger_balance_fx,
               COALESCE((SELECT SUM(amount) FROM statement_lines s WHERE s.account_id = a.id AND s.status NOT IN ('DELETED','EXCLUDED')), 0) AS statement_balance,
               (SELECT COUNT(*) FROM statement_lines s WHERE s.account_id = a.id AND s.status = 'UNRECONCILED') AS unreconciled_count,
               (SELECT MAX(date) FROM statement_lines s WHERE s.account_id = a.id) AS last_statement_date
             FROM accounts a WHERE a.type = 'BANK' ORDER BY a.status, a.code", [])?;
        for r in rows.iter_mut() {
            let id = r["id"].as_i64().unwrap_or(0);
            let opening = self.statement_opening_balance(id)?;
            r["statement_balance"] = Value::from(r["statement_balance"].as_i64().unwrap_or(0) + opening);
        }
        Ok(rows)
    }
}
