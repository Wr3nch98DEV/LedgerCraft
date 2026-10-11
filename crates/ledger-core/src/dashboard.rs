//! Dashboard widgets, notifications and short-term cash flow projection.
//! All SQL is parameterised; `.format()` calls below are chrono date formatting for labels.

use crate::{money, util, Ledger, Result};
use chrono::Datelike;
use rusqlite::params;
use serde_json::{json, Value};

const MON: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
fn day_mon(d: chrono::NaiveDate) -> String { d.day().to_string() + " " + MON[d.month0() as usize] }

/// "Getting started" checklist for a new organisation (like Xero's setup guide). Each step is done
/// when the underlying data exists, so the list completes itself as the business is set up.
fn getting_started(l: &Ledger) -> Result<Value> {
    let q = |sql: &str| -> Result<bool> { Ok(l.conn.query_row(sql, [], |r| r.get::<_, i64>(0))? > 0) };
    let steps = vec![
        ("details", "Add your business address and contact details", "/settings/organisation", !l.setting_or("address", "")?.trim().is_empty()),
        ("branding", "Add your logo to invoices and quotes", "/settings/branding", q("SELECT COUNT(*) FROM branding_themes WHERE logo_attachment_id IS NOT NULL")?),
        ("bank", "Add your bank account number", "/accounting/chart", q("SELECT COUNT(*) FROM accounts WHERE type = 'BANK' AND TRIM(COALESCE(bank_account_number,'')) <> ''")?),
        ("contacts", "Add or import your customers and suppliers", "/contacts", q("SELECT COUNT(*) FROM contacts WHERE is_customer = 1 OR is_supplier = 1")?),
        ("conversion", "Enter opening balances from your old system", "/accounting/conversion", q("SELECT COUNT(*) FROM journals WHERE source_type = 'CONVERSION'")?),
        ("invoice", "Create your first invoice", "/sales/invoices/new", q("SELECT COUNT(*) FROM documents WHERE type = 'ACCREC'")?),
        ("statement", "Import a bank statement and reconcile", "/banking", q("SELECT COUNT(*) FROM statement_lines")?),
    ];
    Ok(json!(steps.into_iter().map(|(key, text, link, done)| json!({"key": key, "text": text, "link": link, "done": done})).collect::<Vec<_>>()))
}

pub fn dashboard(l: &Ledger) -> Result<Value> {
    let today = util::today();
    let banks = l.bank_accounts()?;
    let summary = |kind: &str| -> Result<Value> {
        util::query_one_json(&l.conn,
            "SELECT COUNT(CASE WHEN status = 'DRAFT' THEN 1 END) AS draft_count, COALESCE(SUM(CASE WHEN status = 'DRAFT' THEN base_total END), 0) AS draft_total,
               COUNT(CASE WHEN status = 'SUBMITTED' THEN 1 END) AS submitted_count, COALESCE(SUM(CASE WHEN status = 'SUBMITTED' THEN base_total END), 0) AS submitted_total,
               COUNT(CASE WHEN status = 'AUTHORISED' THEN 1 END) AS awaiting_count,
               COALESCE(SUM(CASE WHEN status = 'AUTHORISED' THEN base_total * (total - amount_paid - amount_credited) / MAX(total, 1) END), 0) AS awaiting_total,
               COUNT(CASE WHEN status = 'AUTHORISED' AND due_date < ?2 THEN 1 END) AS overdue_count,
               COALESCE(SUM(CASE WHEN status = 'AUTHORISED' AND due_date < ?2 THEN base_total * (total - amount_paid - amount_credited) / MAX(total, 1) END), 0) AS overdue_total
             FROM documents WHERE type = ?1", params![kind, today])
    };
    let due_sql = "SELECT COALESCE(SUM(base_total * (total - amount_paid - amount_credited) / MAX(total,1)),0) FROM documents WHERE type = ?1 AND status = 'AUTHORISED' AND due_date >= ?2 AND due_date <= ?3";
    let buckets = |kind: &str| -> Result<Vec<Value>> {
        let t = util::parse_date(&today)?;
        let week_start = t - chrono::Duration::days(t.weekday().num_days_from_monday() as i64);
        let mut out = Vec::new();
        let older: i64 = l.conn.query_row(due_sql, params![kind, "0000-01-01", util::fmt_date(week_start - chrono::Duration::days(1))], |r| r.get(0))?;
        out.push(json!({"label": "Older", "amount": older}));
        for w in 0..4 {
            let s = week_start + chrono::Duration::days(7 * w);
            let e = s + chrono::Duration::days(6);
            let amt: i64 = l.conn.query_row(due_sql, params![kind, util::fmt_date(s), util::fmt_date(e)], |r| r.get(0))?;
            out.push(json!({"label": if w == 0 { "This week".to_string() } else { day_mon(s) }, "amount": amt}));
        }
        let future: i64 = l.conn.query_row(due_sql, params![kind, util::fmt_date(week_start + chrono::Duration::days(28)), "9999-12-31"], |r| r.get(0))?;
        out.push(json!({"label": "Future", "amount": future}));
        Ok(out)
    };
    let t = util::parse_date(&today)?;
    let mut cash = Vec::new();
    for i in (0..6).rev() {
        let m = util::add_months(chrono::NaiveDate::from_ymd_opt(t.year(), t.month(), 1).unwrap(), -i);
        let e = util::last_day_of_month(m.year(), m.month());
        let (cin, cout): (i64, i64) = l.conn.query_row(
            "SELECT COALESCE(SUM(CASE WHEN jl.amount > 0 THEN jl.amount END), 0), COALESCE(-SUM(CASE WHEN jl.amount < 0 THEN jl.amount END), 0)
             FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id JOIN accounts a ON a.id = jl.account_id
             WHERE a.type = 'BANK' AND j.posted = 1 AND j.source_type <> 'TRANSFER' AND j.date >= ?1 AND j.date <= ?2",
            params![util::fmt_date(m), util::fmt_date(e)], |r| Ok((r.get(0)?, r.get(1)?)))?;
        cash.push(json!({"label": MON[m.month0() as usize], "in": cin, "out": cout}));
    }
    let fy = l.fy_start(&today)?;
    let month_start = util::fmt_date(chrono::NaiveDate::from_ymd_opt(t.year(), t.month(), 1).unwrap());
    let watch = util::query_json(&l.conn,
        "SELECT a.id, a.code, a.name, a.class,
           COALESCE((SELECT SUM(jl.amount) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE jl.account_id = a.id AND j.posted = 1 AND j.date >= ?1 AND j.date <= ?3), 0) AS this_month,
           COALESCE((SELECT SUM(jl.amount) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE jl.account_id = a.id AND j.posted = 1 AND j.date >= ?2 AND j.date <= ?3), 0) AS ytd
         FROM accounts a WHERE a.dashboard_watch = 1 AND a.type <> 'BANK' ORDER BY a.code", params![month_start, fy, today])?;
    let pl = l.profit_and_loss(&crate::reports::ReportParams { from: Some(month_start.clone()), to: Some(today.clone()), ..Default::default() })?;
    let total = |label: &str| -> i64 {
        pl["sections"].as_array().unwrap().iter().find(|s| s["total"]["label"] == label).and_then(|s| s["total"]["values"][0].as_i64()).unwrap_or(0)
    };
    let total_cash: i64 = banks.iter().filter(|b| b["status"] == "ACTIVE").map(|b| b["ledger_balance"].as_i64().unwrap_or(0)).sum();
    Ok(json!({
        "org": l.setting_or("org_name", "")?, "today": today, "base_currency": l.base_currency()?,
        "banks": banks, "total_cash": total_cash,
        "sales": summary("ACCREC")?, "purchases": summary("ACCPAY")?,
        "sales_buckets": buckets("ACCREC")?, "purchase_buckets": buckets("ACCPAY")?,
        "cash_flow": cash, "watchlist": watch,
        "pl_month": {"income": total("Total Trading Income") + total("Total Other Income"), "expenses": total("Total Operating Expenses") + total("Total Cost of Sales"), "net": total("Net Profit")},
        "tasks": notifications(l)?,
        "getting_started": getting_started(l)?,
    }))
}

fn plural(n: i64) -> &'static str { if n == 1 { "" } else { "s" } }

/// Notification centre: things needing attention.
pub fn notifications(l: &Ledger) -> Result<Vec<Value>> {
    let today = util::today();
    let mut out = Vec::new();
    let due_sql = "SELECT COUNT(*), COALESCE(SUM(total - amount_paid - amount_credited), 0) FROM documents WHERE type = ?1 AND status = 'AUTHORISED' AND due_date < ?2";
    let (n, amt): (i64, i64) = l.conn.query_row(due_sql, params!["ACCREC", today], |r| Ok((r.get(0)?, r.get(1)?)))?;
    if n > 0 { out.push(json!({"kind": "overdue_invoices", "level": "warn", "text": n.to_string() + " overdue invoice" + plural(n) + " (" + &money::fmt_money_grouped(amt) + ")", "link": "/sales/invoices?tab=overdue"})); }
    let (n, amt): (i64, i64) = l.conn.query_row(due_sql, params!["ACCPAY", today], |r| Ok((r.get(0)?, r.get(1)?)))?;
    if n > 0 { out.push(json!({"kind": "overdue_bills", "level": "warn", "text": n.to_string() + " overdue bill" + plural(n) + " (" + &money::fmt_money_grouped(amt) + ")", "link": "/purchases/bills?tab=overdue"})); }
    let n: i64 = l.conn.query_row("SELECT COUNT(*) FROM statement_lines WHERE status = 'UNRECONCILED'", [], |r| r.get(0))?;
    if n > 0 { out.push(json!({"kind": "reconcile", "level": "info", "text": n.to_string() + " statement line" + plural(n) + " to reconcile", "link": "/banking"})); }
    let n: i64 = l.conn.query_row("SELECT COUNT(*) FROM documents WHERE status = 'SUBMITTED'", [], |r| r.get(0))?;
    if n > 0 { out.push(json!({"kind": "approvals", "level": "info", "text": n.to_string() + " item" + plural(n) + " awaiting approval", "link": "/sales/invoices?tab=submitted"})); }
    let n: i64 = l.conn.query_row("SELECT COUNT(*) FROM items WHERE is_tracked = 1 AND reorder_level_e4 IS NOT NULL AND qty_on_hand_e4 <= reorder_level_e4 AND status = 'ACTIVE'", [], |r| r.get(0))?;
    if n > 0 { out.push(json!({"kind": "low_stock", "level": "warn", "text": n.to_string() + " item" + plural(n) + " at or below reorder level", "link": "/products"})); }
    let n: i64 = l.conn.query_row("SELECT COUNT(*) FROM repeating_templates WHERE status = 'ACTIVE' AND next_date <= ?1", [&today], |r| r.get(0))?;
    if n > 0 { out.push(json!({"kind": "repeating", "level": "info", "text": n.to_string() + " repeating transaction" + plural(n) + " due to run", "link": "/sales/repeating"})); }
    for extra in crate::ext::extra_notifications(l)? { out.push(extra); }
    Ok(out)
}

/// Short-term cash flow projection: opening cash + scheduled receipts/payments (+ what-if adjustments).
pub fn cashflow_forecast(l: &Ledger, days: i64, adjustments: Vec<Value>) -> Result<Value> {
    let today = util::today();
    let end = util::add_days(&today, days)?;
    let opening: i64 = l.bank_accounts()?.iter().filter(|b| b["status"] == "ACTIVE").map(|b| b["ledger_balance"].as_i64().unwrap_or(0)).sum();
    let docs = util::query_json(&l.conn,
        "SELECT d.id, d.type, d.number, c.name AS contact, COALESCE(d.planned_payment_date, d.expected_payment_date, d.due_date, d.date) AS date,
           d.base_total * (d.total - d.amount_paid - d.amount_credited) / MAX(d.total, 1) AS amount
         FROM documents d JOIN contacts c ON c.id = d.contact_id
         WHERE d.status = 'AUTHORISED' AND d.type IN ('ACCREC','ACCPAY') AND COALESCE(d.planned_payment_date, d.expected_payment_date, d.due_date, d.date) <= ?1
         ORDER BY date", [&end])?;
    let reps = util::query_json(&l.conn, "SELECT id, kind, template, period_unit, period_count, next_date, end_date FROM repeating_templates WHERE status = 'ACTIVE' AND kind IN ('ACCREC','ACCPAY') AND next_date <= ?1", [&end])?;
    let mut events: Vec<(String, i64, String)> = Vec::new();
    for d in &docs {
        let date = d["date"].as_str().unwrap_or(&today).max(today.as_str()).to_string();
        let a = d["amount"].as_i64().unwrap_or(0);
        events.push((date, if d["type"] == "ACCREC" { a } else { -a }, String::from(d["number"].as_str().unwrap_or("")) + " " + d["contact"].as_str().unwrap_or("")));
    }
    for r in &reps {
        let t: Value = serde_json::from_str(r["template"].as_str().unwrap_or("{}")).unwrap_or(json!({}));
        let total = crate::ext::estimate_template_total(l, &t).unwrap_or(0);
        let mut date = r["next_date"].as_str().unwrap_or(&today).to_string();
        let mut guard = 0;
        while date.as_str() <= end.as_str() && guard < 60 {
            if r["end_date"].as_str().map(|e| date.as_str() > e).unwrap_or(false) { break; }
            events.push((date.clone().max(today.clone()), if r["kind"] == "ACCREC" { total } else { -total }, "Repeating transaction".into()));
            date = crate::ext::advance_date(&date, r["period_unit"].as_str().unwrap_or("MONTHLY"), r["period_count"].as_i64().unwrap_or(1))?;
            guard += 1;
        }
    }
    for a in &adjustments {
        let amt = a["amount"].as_str().and_then(|s| money::parse_fixed(s, 2)).or(a["amount"].as_i64()).unwrap_or(0);
        events.push((a["date"].as_str().unwrap_or(&today).to_string(), amt, a["label"].as_str().unwrap_or("Adjustment").to_string()));
    }
    events.sort();
    let mut series = Vec::new();
    let mut bal = opening;
    let mut d = util::parse_date(&today)?;
    let e = util::parse_date(&end)?;
    let (mut tin, mut tout) = (0i64, 0i64);
    while d <= e {
        let ds = util::fmt_date(d);
        let (mut cin, mut cout) = (0, 0);
        for (_, amt, _) in events.iter().filter(|x| x.0 == ds) { if *amt >= 0 { cin += amt } else { cout -= amt } }
        bal += cin - cout; tin += cin; tout += cout;
        series.push(json!({"date": ds, "in": cin, "out": cout, "balance": bal}));
        d += chrono::Duration::days(1);
    }
    Ok(json!({"opening": opening, "closing": bal, "total_in": tin, "total_out": tout, "series": series,
        "events": events.into_iter().map(|(d, a, l)| json!({"date": d, "amount": a, "label": l})).collect::<Vec<_>>()}))
}
