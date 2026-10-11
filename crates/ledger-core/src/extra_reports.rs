//! Additional reports: inventory, bank reconciliation, contact transactions, invoice detail,
//! budget summary, and registry of all report names for the Reports centre.

use crate::{reports::{fmt_long, ReportParams}, util, Error, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

/// Catalogue shown in the Reports centre: (key, title, category, description)
pub const CATALOGUE: &[(&str, &str, &str, &str)] = &[
    ("balance-sheet", "Balance Sheet", "Financial", "Assets, liabilities and equity at a point in time"),
    ("profit-and-loss", "Profit and Loss", "Financial", "Income and expenses over a period, with comparisons and budgets"),
    ("cash-summary", "Cash Summary", "Financial", "Where cash came from and went"),
    ("executive-summary", "Executive Summary", "Financial", "Key metrics and ratios over three months"),
    ("budget-summary", "Budget Summary", "Financial", "Budgeted amounts by month"),
    ("budget-variance", "Budget Variance", "Financial", "Actuals vs budget"),
    ("trial-balance", "Trial Balance", "Accounting", "Balances of every account"),
    ("general-ledger", "General Ledger Summary", "Accounting", "Opening, movements and closing per account"),
    ("general-ledger-detail", "General Ledger Detail", "Accounting", "Every transaction by account"),
    ("account-transactions", "Account Transactions", "Accounting", "Transactions for one account with running balance"),
    ("journal-report", "Journal Report", "Accounting", "All posted journals"),
    ("aged-receivables", "Aged Receivables", "Receivables", "Who owes you, by age"),
    ("receivable-invoice-detail", "Receivable Invoice Detail", "Receivables", "Every sales invoice in a period"),
    ("sales-by-contact", "Sales by Contact", "Receivables", "Sales totals per customer"),
    ("sales-by-item", "Sales by Item", "Receivables", "Quantity and value sold per item"),
    ("aged-payables", "Aged Payables", "Payables", "Who you owe, by age"),
    ("payable-invoice-detail", "Payable Invoice Detail", "Payables", "Every bill in a period"),
    ("purchases-by-contact", "Purchases by Contact", "Payables", "Purchase totals per supplier"),
    ("purchases-by-item", "Purchases by Item", "Payables", "Quantity and value bought per item"),
    ("contact-transactions", "Contact Transactions", "Receivables", "All activity for contacts"),
    ("bank-summary", "Bank Summary", "Banking", "Opening, received, spent and closing per bank account"),
    ("bank-reconciliation", "Bank Reconciliation", "Banking", "Statement vs ledger with unreconciled items"),
    ("tax-reconciliation", "Tax Reconciliation", "Tax", "Tax collected and paid by rate, tied to the ledger"),
    ("inventory-summary", "Inventory Item Summary", "Inventory", "Stock on hand, average cost and value"),
    ("fixed-asset-register", "Fixed Asset Register", "Fixed assets", "Cost, depreciation and book value per asset"),
    ("fixed-asset-reconciliation", "Fixed Asset Reconciliation", "Fixed assets", "Movements in asset cost and depreciation"),
    ("depreciation-schedule", "Depreciation Schedule", "Fixed assets", "Projected depreciation per asset"),
    ("project-profitability", "Project Profitability", "Projects", "Income, costs and profit by project"),
    ("expense-claims", "Expense Claim Detail", "Payables", "Expense claims by employee"),
    ("payroll-activity", "Payroll Activity Summary", "Payroll", "Wages, tax, pension and net pay"),
    ("leave-balances", "Leave Balances", "Payroll", "Leave balances per employee"),
    ("fx-gains-losses", "Foreign Currency Gains and Losses", "Accounting", "Realised and unrealised currency gains"),
    ("income-by-contact", "Income by Contact", "Receivables", "Income per contact (cash and accrual)"),
];

pub fn inventory_report(l: &Ledger) -> Result<Value> {
    let v = l.inventory_valuation()?;
    let rows: Vec<Value> = v["items"].as_array().unwrap().iter().map(|i| json!({
        "label": format!("{} - {}", i["code"].as_str().unwrap_or(""), i["name"].as_str().unwrap_or("")), "item_id": i["id"],
        "values": [i["qty_on_hand_e4"], i["avg_cost"], i["total_cost"]], "flag": if i["low_stock"].as_i64() == Some(1) { "Low stock" } else { "" }})).collect();
    Ok(json!({"title": "Inventory Item Summary", "org": l.setting_or("org_name", "")?, "subtitle": format!("As at {} · ledger balance {}", fmt_long(&util::today()), crate::money::fmt_money(v["gl_balance"].as_i64().unwrap_or(0))),
        "columns": ["Quantity on hand", "Average cost", "Value"], "qty_cols": [0], "sections": [{"title": "", "rows": rows, "total": {"label": "Total", "values": [Value::Null, Value::Null, v["total"]]}}]}))
}

pub fn reconciliation_as_report(l: &Ledger, account: i64, r: &Value) -> Result<Value> {
    let name: String = l.conn.query_row("SELECT name FROM accounts WHERE id = ?1", [account], |r| r.get(0))?;
    let lines = |arr: &Value| -> Vec<Value> { arr.as_array().cloned().unwrap_or_default().iter().map(|x| json!({"label": format!("{} {} {}", x["date"].as_str().unwrap_or(""), x["payee"].as_str().or(x["contact"].as_str()).unwrap_or(""), x["description"].as_str().or(x["reference"].as_str()).unwrap_or("")), "values": [x["amount"]]})).collect() };
    Ok(json!({"title": "Bank Reconciliation", "org": l.setting_or("org_name", "")?, "subtitle": format!("{name} · last reconciled {}", r["last_reconciled"].as_str().unwrap_or("never")),
        "columns": ["Amount"], "sections": [
            {"title": "", "rows": [{"label": "Balance per bank statement", "values": [r["statement_balance"]]}]},
            {"title": "Less: unreconciled statement lines", "rows": lines(&r["unreconciled_statement_lines"]), "total": {"label": "Total", "values": [r["unreconciled_statement_total"]]}},
            {"title": "Plus: unreconciled transactions in the ledger", "rows": lines(&r["unreconciled_ledger_items"]), "total": {"label": "Total", "values": [r["unreconciled_ledger_total"]]}},
            {"title": "", "summary": true, "grand": true, "total": {"label": "Balance in LedgerCraft", "values": [r["ledger_balance"]]}},
            {"title": "", "summary": true, "total": {"label": "Unexplained difference", "values": [r["difference"]]}},
        ]}))
}

pub fn contact_transactions(l: &Ledger, p: &ReportParams) -> Result<Value> {
    let to = p.to.clone().unwrap_or_else(util::today);
    let from = p.from.clone().unwrap_or(l.fy_start(&to)?);
    let rows = util::query_json(&l.conn,
        "SELECT c.name AS contact, c.id AS contact_id, d.id, d.type, d.number, d.date, d.reference, d.base_total, d.status FROM documents d JOIN contacts c ON c.id = d.contact_id
         WHERE d.status IN ('AUTHORISED','PAID') AND d.date >= ?1 AND d.date <= ?2 AND (?3 IS NULL OR d.contact_id = ?3) AND d.type IN ('ACCREC','ACCPAY','ACCRECCREDIT','ACCPAYCREDIT')
         ORDER BY c.name, d.date", params![from, to, p.contact_id])?;
    let mut sections: Vec<Value> = Vec::new();
    let mut cur = String::new();
    let mut rows_acc: Vec<Value> = Vec::new();
    let mut tot = (0i64, 0i64);
    let flush = |sections: &mut Vec<Value>, name: &str, rows: &mut Vec<Value>, tot: &mut (i64, i64)| {
        if !rows.is_empty() { sections.push(json!({"title": name, "rows": std::mem::take(rows), "total": {"label": "Total", "values": [tot.0, tot.1]}})); }
        *tot = (0, 0);
    };
    for r in rows {
        let name = r["contact"].as_str().unwrap_or("").to_string();
        if name != cur { flush(&mut sections, &cur, &mut rows_acc, &mut tot); cur = name; }
        let amt = r["base_total"].as_i64().unwrap_or(0);
        let ty = r["type"].as_str().unwrap_or("");
        let (inc, exp) = match ty { "ACCREC" => (amt, 0), "ACCRECCREDIT" => (-amt, 0), "ACCPAY" => (0, amt), _ => (0, -amt) };
        tot.0 += inc; tot.1 += exp;
        rows_acc.push(json!({"label": format!("{} {} {}", r["date"].as_str().unwrap_or(""), crate::reports::source_label(ty), r["number"].as_str().unwrap_or("")), "document_id": r["id"], "values": [inc, exp]}));
    }
    flush(&mut sections, &cur, &mut rows_acc, &mut tot);
    Ok(json!({"title": "Contact Transactions", "org": l.setting_or("org_name", "")?, "subtitle": format!("{} to {}", fmt_long(&from), fmt_long(&to)), "columns": ["Sales", "Purchases"], "sections": sections}))
}

pub fn invoice_detail(l: &Ledger, sales: bool, p: &ReportParams) -> Result<Value> {
    let to = p.to.clone().unwrap_or_else(util::today);
    let from = p.from.clone().unwrap_or(l.fy_start(&to)?);
    let rows = util::query_json(&l.conn,
        "SELECT d.id, d.number, d.date, d.due_date, d.reference, c.name AS contact, d.status, d.currency, d.subtotal, d.total_tax, d.total, d.total - d.amount_paid - d.amount_credited AS due
         FROM documents d JOIN contacts c ON c.id = d.contact_id WHERE d.type = ?1 AND d.status IN ('AUTHORISED','PAID') AND d.date >= ?2 AND d.date <= ?3 ORDER BY d.date",
        params![if sales { "ACCREC" } else { "ACCPAY" }, from, to])?;
    let mut t = (0i64, 0i64, 0i64, 0i64);
    let rows: Vec<Value> = rows.into_iter().map(|r| {
        t.0 += r["subtotal"].as_i64().unwrap_or(0); t.1 += r["total_tax"].as_i64().unwrap_or(0); t.2 += r["total"].as_i64().unwrap_or(0); t.3 += r["due"].as_i64().unwrap_or(0);
        json!({"label": format!("{} · {} · {}", r["date"].as_str().unwrap_or(""), r["number"].as_str().unwrap_or(""), r["contact"].as_str().unwrap_or("")), "document_id": r["id"],
            "values": [r["subtotal"], r["total_tax"], r["total"], r["due"]]})
    }).collect();
    Ok(json!({"title": if sales { "Receivable Invoice Detail" } else { "Payable Invoice Detail" }, "org": l.setting_or("org_name", "")?,
        "subtitle": format!("{} to {}", fmt_long(&from), fmt_long(&to)), "columns": ["Subtotal", "Tax", "Total", "Balance due"],
        "sections": [{"title": "", "rows": rows, "total": {"label": "Total", "values": [t.0, t.1, t.2, t.3]}}]}))
}

pub fn run_extra(l: &Ledger, name: &str, p: &ReportParams) -> Result<Value> {
    match name {
        "budget-variance" => {
            let b = p.budget_id.or_else(|| l.conn.query_row("SELECT id FROM budgets ORDER BY id LIMIT 1", [], |r| r.get(0)).ok())
                .ok_or_else(|| Error::Validation("create a budget first (Accounting → Budget manager)".into()))?;
            let mut r = l.profit_and_loss(&ReportParams { budget_id: Some(b), ..p.clone() })?;
            r["title"] = json!("Budget Variance");
            Ok(r)
        }
        "budget-summary" => {
            let b = p.budget_id.or_else(|| l.conn.query_row("SELECT id FROM budgets ORDER BY id LIMIT 1", [], |r| r.get(0)).ok())
                .ok_or_else(|| Error::Validation("create a budget first (Accounting → Budget manager)".into()))?;
            let to = p.to.clone().unwrap_or_else(util::today);
            let from = p.from.clone().unwrap_or(l.fy_start(&to)?);
            let periods = l.periods(&from, &l.fy_end(&from)?, 0, "MONTH", true)?;
            let rows = util::query_json(&l.conn, "SELECT DISTINCT a.id, a.code, a.name, a.class FROM budget_lines bl JOIN accounts a ON a.id = bl.account_id WHERE bl.budget_id = ?1 ORDER BY a.code", [b])?;
            let mut out = Vec::new();
            for a in rows {
                let sign = if a["class"] == "REVENUE" { -1 } else { 1 };
                let vals: Vec<i64> = periods.iter().map(|pr| l.conn.query_row("SELECT COALESCE(SUM(amount),0) FROM budget_lines WHERE budget_id = ?1 AND account_id = ?2 AND period = substr(?3,1,7)",
                    params![b, a["id"].as_i64().unwrap(), pr.from], |r| r.get::<_, i64>(0)).unwrap_or(0) * sign).collect();
                out.push(json!({"label": a["name"], "account_id": a["id"], "values": vals}));
            }
            Ok(json!({"title": "Budget Summary", "org": l.setting_or("org_name", "")?, "subtitle": format!("{} to {}", fmt_long(&from), fmt_long(&l.fy_end(&from)?)),
                "columns": periods.iter().map(|p| p.label.clone()).collect::<Vec<_>>(), "sections": [{"title": "", "rows": out}]}))
        }
        "fx-gains-losses" => {
            let to = p.to.clone().unwrap_or_else(util::today);
            let from = p.from.clone().unwrap_or(l.fy_start(&to)?);
            let rows = util::query_json(&l.conn,
                "SELECT a.name, a.system_account, COALESCE(-SUM(jl.amount), 0) AS amount FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id JOIN accounts a ON a.id = jl.account_id
                 WHERE j.posted = 1 AND j.date >= ?1 AND j.date <= ?2 AND a.system_account IN ('FXGAIN','UNREALISEDFX','BANKREVAL') GROUP BY a.id", params![from, to])?;
            let total: i64 = rows.iter().map(|r| r["amount"].as_i64().unwrap_or(0)).sum();
            Ok(json!({"title": "Foreign Currency Gains and Losses", "org": l.setting_or("org_name", "")?, "subtitle": format!("{} to {} · positive = gain", fmt_long(&from), fmt_long(&to)),
                "columns": ["Gain / (loss)"], "sections": [{"title": "", "rows": rows.iter().map(|r| json!({"label": r["name"], "values": [r["amount"]]})).collect::<Vec<_>>(), "total": {"label": "Net gain / (loss)", "values": [total]}}]}))
        }
        "income-by-contact" => l.sales_by_contact(true, p),
        _ => crate::modules::report(l, name, p),
    }
}

/// Run any report by its catalogue key.
pub fn run_any(l: &Ledger, name: &str, p: &ReportParams) -> Result<Value> {
    match name {
        "profit-and-loss" => l.profit_and_loss(p),
        "balance-sheet" => l.balance_sheet(p),
        "trial-balance" => l.trial_balance(p),
        "account-transactions" => l.account_transactions(p),
        "general-ledger" => l.general_ledger_summary(p),
        "general-ledger-detail" => l.account_transactions(&ReportParams { account_id: None, ..p.clone() }),
        "journal-report" => l.journal_report(p),
        "aged-receivables" => l.aged(true, p),
        "aged-payables" => l.aged(false, p),
        "cash-summary" => l.cash_summary(p),
        "bank-summary" => l.bank_summary(p),
        "sales-by-contact" => l.sales_by_contact(true, p),
        "purchases-by-contact" => l.sales_by_contact(false, p),
        "sales-by-item" => l.sales_by_item(true, p),
        "purchases-by-item" => l.sales_by_item(false, p),
        "tax-reconciliation" => l.tax_report(p),
        "executive-summary" => l.executive_summary(p),
        "inventory-summary" => inventory_report(l),
        "bank-reconciliation" => {
            let acct = p.account_id.ok_or_else(|| Error::Validation("choose a bank account".into()))?;
            let r = l.reconciliation_report(acct, p.date.as_deref().or(p.to.as_deref()).unwrap_or(&util::today()))?;
            reconciliation_as_report(l, acct, &r)
        }
        "contact-transactions" => contact_transactions(l, p),
        "receivable-invoice-detail" => invoice_detail(l, true, p),
        "payable-invoice-detail" => invoice_detail(l, false, p),
        _ => run_extra(l, name, p),
    }
}
