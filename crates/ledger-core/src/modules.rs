//! Feature modules dispatch: fixed assets, tax returns, expense claims, projects, payroll,
//! import/export, files inbox, workpapers. Each module's RPC methods are registered here.

use crate::{assets::{AssetInput, AssetTypeInput}, expenses::ReceiptInput, payroll::EmployeeInput, reports::ReportParams, util, Error, Ledger, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

fn i(p: &Value, k: &str) -> Result<i64> { p[k].as_i64().ok_or_else(|| Error::Validation(format!("missing '{k}'"))) }
fn s<'a>(p: &'a Value, k: &str) -> &'a str { p[k].as_str().unwrap_or("") }
fn de<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T> { Ok(serde_json::from_value(v.clone())?) }
fn ok() -> Value { json!({"ok": true}) }

pub fn dispatch(l: &mut Ledger, method: &str, p: &Value, need: &dyn Fn(&str) -> Result<()>) -> Result<Option<Value>> {
    let today = util::today();
    let r = match method {
        // ---- tax returns ----
        "taxReturns.list" => { need("reports:read")?; json!({"returns": l.list_tax_returns()?, "next": l.next_tax_period()?, "form": crate::taxreturns::default_form(&l.setting_or("country", "AU")?)}) }
        "taxReturns.create" => { need("accounting:write")?; json!({"id": l.create_tax_return(p["form"].as_str(), s(p, "from"), s(p, "to"), p["basis"].as_str())?}) }
        "taxReturns.get" => { need("reports:read")?; l.get_tax_return(i(p, "id")?)? }
        "taxReturns.file" => { need("accounting:write")?; l.file_tax_return(i(p, "id")?)?; ok() }
        "taxReturns.delete" => { need("accounting:write")?; l.delete_tax_return(i(p, "id")?)?; ok() }
        "taxReturns.mtd" => { need("reports:read")?; l.mtd_payload(i(p, "id")?)? }
        // ---- fixed assets ----
        "assets.list" => { need("assets:read")?; l.ensure_asset_types()?; json!({"assets": l.list_assets(p["status"].as_str().unwrap_or("ALL"))?, "types": l.list_asset_types()?, "runs": l.list_depreciation_runs()?}) }
        "assets.get" => { need("assets:read")?; l.get_asset(i(p, "id")?)? }
        "assets.save" => { need("assets:write")?; json!({"id": l.save_asset(p["id"].as_i64(), &de::<AssetInput>(&p["asset"])?)?}) }
        "assets.register" => { need("assets:write")?; l.register_asset(i(p, "id")?)?; ok() }
        "assets.delete" => { need("assets:write")?; l.conn.execute("DELETE FROM fixed_assets WHERE id = ?1 AND status = 'DRAFT'", [i(p, "id")?])?; ok() }
        "assets.saveType" => { need("assets:write")?; json!({"id": l.save_asset_type(p["id"].as_i64(), &de::<AssetTypeInput>(&p["type"])?)?}) }
        "assets.runDepreciation" => { need("assets:write")?; l.run_depreciation(p["date"].as_str().unwrap_or(&today))? }
        "assets.rollback" => { need("assets:write")?; l.rollback_depreciation()?; ok() }
        "assets.dispose" => { need("assets:write")?; json!({"journal_id": l.dispose_asset(i(p, "id")?, s(p, "date"), s(p, "sale_price"), p["proceeds_account_id"].as_i64(), i(p, "gain_loss_account_id")?)?}) }
        // ---- expense claims ----
        "receipts.list" => { need("expenses:read")?; l.sync_receipts()?; json!(l.list_receipts(p["status"].as_str().unwrap_or("ALL"))?) }
        "receipts.get" => { need("expenses:read")?; l.get_receipt(i(p, "id")?)? }
        "receipts.save" => { need("expenses:write")?; json!({"id": l.save_receipt(p["id"].as_i64(), &de::<ReceiptInput>(&p["receipt"])?)?}) }
        "receipts.setStatus" => {
            let st = s(p, "status");
            if st == "AUTHORISED" || st == "DECLINED" { need("purchases:write")?; } else { need("expenses:write")?; }
            l.set_receipt_status(i(p, "id")?, st)?; ok()
        }
        // ---- projects ----
        "projects.list" => { need("projects:read")?; json!({"projects": l.list_projects(p["status"].as_str().unwrap_or("ALL"))?, "timer": l.running_timer()?}) }
        "projects.get" => { need("projects:read")?; let mut v = l.get_project(i(p, "id")?)?; v["timer"] = l.running_timer()?; v }
        "projects.save" => { need("projects:write")?; json!({"id": l.save_project(p["id"].as_i64(), &p["project"])?}) }
        "projects.saveTask" => { need("projects:write")?; json!({"id": l.save_task(p["id"].as_i64(), i(p, "project_id")?, &p["task"])?}) }
        "projects.addTime" => { need("projects:write")?; json!({"id": l.add_time(i(p, "project_id")?, i(p, "task_id")?, s(p, "date"), p["minutes"].as_i64().unwrap_or(0), s(p, "description"))?}) }
        "projects.deleteTime" => { need("projects:write")?; l.delete_time(i(p, "id")?)?; ok() }
        "projects.startTimer" => { need("projects:write")?; json!({"id": l.start_timer(i(p, "project_id")?, i(p, "task_id")?, s(p, "description"))?}) }
        "projects.stopTimer" => { need("projects:write")?; l.stop_timer()? }
        "projects.addCost" => { need("projects:write")?; json!({"id": l.add_project_cost(i(p, "project_id")?, &p["cost"])?}) }
        "projects.invoice" => { need("sales:write")?; json!({"id": l.invoice_project(i(p, "id")?)?}) }
        // ---- payroll ----
        "payroll.settings" => { need("payroll:read")?; l.payroll_settings()? }
        "payroll.savePayItem" => { need("payroll:write")?; json!({"id": l.save_pay_item(p["id"].as_i64(), &p["item"])?}) }
        "payroll.saveCalendar" => { need("payroll:write")?; json!({"id": l.save_pay_calendar(p["id"].as_i64(), s(p, "name"), s(p, "frequency"), s(p, "start_date"))?}) }
        "payroll.saveLeaveType" => {
            need("payroll:write")?;
            let hrs = p["hours"].as_str().and_then(|h| crate::money::parse_fixed(h, 2)).unwrap_or(0);
            match p["id"].as_i64() { Some(id) => { l.conn.execute("UPDATE leave_types SET name = ?2, accrual_hours_per_year_e2 = ?3 WHERE id = ?1", rusqlite::params![id, s(p, "name"), hrs])?; }
                None => { l.conn.execute("INSERT INTO leave_types(name, accrual_hours_per_year_e2) VALUES (?1, ?2)", rusqlite::params![s(p, "name"), hrs])?; } }
            ok()
        }
        "payroll.employees" => { need("payroll:read")?; json!(l.list_employees(p["status"].as_str().unwrap_or("ALL"))?) }
        "payroll.employee" => { need("payroll:read")?; l.get_employee(i(p, "id")?)? }
        "payroll.saveEmployee" => { need("payroll:write")?; json!({"id": l.save_employee(p["id"].as_i64(), &de::<EmployeeInput>(&p["employee"])?)?}) }
        "payroll.payRuns" => { need("payroll:read")?; json!(l.list_pay_runs()?) }
        "payroll.nextPeriod" => { need("payroll:read")?; let (a, b) = l.next_pay_period(i(p, "calendar_id")?)?; json!({"from": a, "to": b}) }
        "payroll.createPayRun" => { need("payroll:write")?; json!({"id": l.create_pay_run(i(p, "calendar_id")?, p["payment_date"].as_str())?}) }
        "payroll.payRun" => { need("payroll:read")?; l.get_pay_run(i(p, "id")?)? }
        "payroll.updatePayslip" => { need("payroll:write")?; l.update_payslip(i(p, "id")?, p["lines"].as_array().map(|a| a.as_slice()).unwrap_or(&[]))?; ok() }
        "payroll.recalc" => { need("payroll:write")?; l.recalc_pay_run(i(p, "id")?)?; ok() }
        "payroll.post" => { need("payroll:write")?; json!({"journal_id": l.post_pay_run(i(p, "id")?, p["bank_account_id"].as_i64())?}) }
        "payroll.deletePayRun" => { need("payroll:write")?; l.delete_pay_run(i(p, "id")?)?; ok() }
        "payroll.filing" => { need("payroll:read")?; l.payroll_filing_payload(i(p, "id")?)? }
        "payroll.timesheets" => { need("payroll:read")?; json!(l.list_timesheets()?) }
        "payroll.saveTimesheet" => { need("payroll:write")?; json!({"id": l.save_timesheet(p["id"].as_i64(), &p["timesheet"])?}) }
        "payroll.timesheetStatus" => { need("payroll:write")?; l.set_timesheet_status(i(p, "id")?, s(p, "status"))?; ok() }
        "payroll.leave" => { need("payroll:read")?; l.list_leave()? }
        "payroll.saveLeave" => { need("payroll:write")?; json!({"id": l.save_leave_request(&p["request"])?}) }
        "payroll.leaveStatus" => { need("payroll:write")?; l.set_leave_status(i(p, "id")?, s(p, "status"))?; ok() }
        "payroll.emailPayslips" => {
            need("payroll:write")?;
            let r = l.get_pay_run(i(p, "id")?)?;
            let mut n = 0;
            for sl in r["payslips"].as_array().unwrap() {
                let to = sl["email"].as_str().unwrap_or("");
                if to.is_empty() { continue; }
                let long = |k: &str| r[k].as_str().map(crate::reports::fmt_long).unwrap_or_default();
                crate::email::queue_email(l, to, &format!("Your payslip for {} to {}", long("period_start"), long("period_end")),
                    &format!("Hi,\n\nYour payslip for {} to {} is attached. Net pay: {}, paid on {}.\n\nThanks,\n{}", long("period_start"), long("period_end"),
                        crate::money::fmt_money_grouped(sl["net"].as_i64().unwrap_or(0)), long("payment_date"), l.setting_or("org_name", "")?), "payslip", sl["id"].as_i64().unwrap_or(0))?;
                l.conn.execute("UPDATE payslips SET sent = 1 WHERE id = ?1", [sl["id"].as_i64()])?;
                n += 1;
            }
            json!({"queued": n})
        }
        // ---- import / export ----
        "import.run" => { need("settings:write")?; l.import_csv(s(p, "what"), s(p, "text"), p["dry_run"].as_bool().unwrap_or(true), p["date"].as_str())? }
        // ---- attachments / files inbox / workpapers / assurance ----
        "attachments.add" => { json!({"id": l.add_attachment(s(p, "entity"), p["entity_id"].as_i64(), s(p, "filename"), s(p, "mime"), s(p, "data"))?}) }
        "attachments.list" => json!(l.list_attachments(s(p, "entity"), i(p, "id")?)?),
        "attachments.delete" => { l.delete_attachment(i(p, "id")?)?; ok() }
        "inbox.list" => json!(l.list_inbox()?),
        "inbox.upload" => { need("purchases:write")?; json!({"id": l.inbox_upload(s(p, "filename"), s(p, "mime"), s(p, "data"))?}) }
        "inbox.toBill" => { need("purchases:write")?; json!({"id": l.inbox_to_bill(i(p, "id")?, &p["overrides"])?}) }
        "inbox.archive" => { l.archive_inbox(i(p, "id")?)?; ok() }
        "workpapers.list" => { need("accounting:read")?; json!(l.workpapers(p["period_end"].as_str().unwrap_or(&today))?) }
        "workpapers.save" => { need("accounting:write")?; l.save_workpaper(i(p, "account_id")?, s(p, "period_end"), s(p, "note"), p["reviewed"].as_bool().unwrap_or(false))?; ok() }
        "assurance.health" => { need("accounting:read")?; l.assurance_health()? }
        _ => return Ok(None),
    };
    Ok(Some(r))
}

pub fn notifications(l: &Ledger) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    let n: i64 = l.conn.query_row("SELECT COUNT(*) FROM receipts WHERE status = 'SUBMITTED'", [], |r| r.get(0))?;
    if n > 0 { out.push(json!({"kind": "expense_claims", "level": "info", "text": format!("{n} expense claim{} to review", if n == 1 { "" } else { "s" }), "link": "/expenses"})); }
    let due: i64 = l.conn.query_row("SELECT COUNT(*) FROM fixed_assets WHERE status = 'REGISTERED' AND method NOT IN ('NONE','FULL') AND COALESCE(last_dep_date, '0000') < date('now','localtime','start of month','-1 day')", [], |r| r.get(0))?;
    if due > 0 { out.push(json!({"kind": "depreciation", "level": "info", "text": format!("Depreciation is due for {due} asset{}", if due == 1 { "" } else { "s" }), "link": "/accounting/assets"})); }
    let inbox: i64 = l.conn.query_row("SELECT COUNT(*) FROM file_inbox WHERE status = 'NEW'", [], |r| r.get(0))?;
    if inbox > 0 { out.push(json!({"kind": "inbox", "level": "info", "text": format!("{inbox} file{} in your inbox", if inbox == 1 { "" } else { "s" }), "link": "/files"})); }
    let leave: i64 = l.conn.query_row("SELECT COUNT(*) FROM leave_requests WHERE status = 'PENDING'", [], |r| r.get(0))?;
    if leave > 0 { out.push(json!({"kind": "leave", "level": "info", "text": format!("{leave} leave request{} to approve", if leave == 1 { "" } else { "s" }), "link": "/payroll/leave"})); }
    Ok(out)
}

pub fn report(l: &Ledger, name: &str, p: &ReportParams) -> Result<Value> {
    let to = p.to.clone().or(p.date.clone()).unwrap_or_else(util::today);
    let from = p.from.clone().unwrap_or(l.fy_start(&to)?);
    match name {
        "fixed-asset-register" => l.asset_register_report(&to),
        "fixed-asset-reconciliation" => l.asset_reconciliation_report(&from, &to),
        "depreciation-schedule" => {
            let assets = l.list_assets("REGISTERED")?;
            let mut rows = Vec::new();
            let mut cols: Vec<String> = Vec::new();
            for a in assets {
                let sch = l.depreciation_schedule(a["id"].as_i64().unwrap(), 5)?;
                if cols.is_empty() { cols = sch.as_array().unwrap().iter().map(|y| format!("FY from {}", y["year_start"].as_str().unwrap_or(""))).collect(); }
                rows.push(json!({"label": a["name"], "values": sch.as_array().unwrap().iter().map(|y| y["depreciation"].clone()).collect::<Vec<_>>()}));
            }
            Ok(json!({"title": "Depreciation Schedule", "org": l.setting_or("org_name", "")?, "subtitle": "Projected book depreciation", "columns": cols, "sections": [{"title": "", "rows": rows}]}))
        }
        "project-profitability" => l.project_profitability_report(),
        "contractor-1099" => l.report_1099(p),
        "w2-summary" => l.report_w2(p),
        "stock-by-location" => l.stock_by_location_report(),
        "sales-tax-by-agency" => l.report_tax_by_agency(p),
        "payroll-activity" => l.payroll_activity_report(&from, &to),
        "leave-balances" => l.leave_balances_report(),
        "expense-claims" => {
            let rows: Vec<Value> = util::query_json(&l.conn, "SELECT c.name AS label, COUNT(*) AS n, SUM(r.total) AS total, SUM(CASE WHEN r.status = 'PAID' THEN r.total ELSE 0 END) AS paid FROM receipts r JOIN contacts c ON c.id = r.claimant_contact_id
                WHERE r.date >= ?1 AND r.date <= ?2 AND r.status NOT IN ('VOIDED','DECLINED') GROUP BY c.id ORDER BY c.name", rusqlite::params![from, to])?
                .into_iter().map(|r| json!({"label": r["label"], "values": [r["n"], r["total"], r["paid"], r["total"].as_i64().unwrap_or(0) - r["paid"].as_i64().unwrap_or(0)]})).collect();
            Ok(json!({"title": "Expense Claim Detail", "org": l.setting_or("org_name", "")?, "subtitle": format!("{} to {}", crate::reports::fmt_long(&from), crate::reports::fmt_long(&to)),
                "columns": ["Claims", "Total", "Reimbursed", "Outstanding"], "count_cols": [0], "sections": [{"title": "", "rows": rows}]}))
        }
        _ => Err(Error::NotFound(format!("report '{name}'"))),
    }
}

pub fn render_view(l: &Ledger, kind: &str, id: i64) -> Result<String> {
    match kind {
        "payslip" => l.payslip_html(id),
        _ => Err(Error::NotFound(format!("view '{kind}'"))),
    }
}

pub fn download(l: &Ledger, kind: &str, q: &HashMap<String, String>) -> Result<(String, String, Vec<u8>)> {
    let id = || q.get("id").and_then(|x| x.parse::<i64>().ok()).ok_or_else(|| Error::Validation("missing id".into()));
    match kind {
        "contacts.csv" | "accounts.csv" | "items.csv" | "invoices.csv" | "bills.csv" | "journals.csv" => {
            let what = kind.trim_end_matches(".csv");
            Ok(("text/csv; charset=utf-8".into(), kind.to_string(), l.export_csv(what)?.into_bytes()))
        }
        "archive" => Ok(("application/json".into(), format!("ledgercraft-archive-{}.json", util::today()), serde_json::to_vec_pretty(&l.export_archive()?)?)),
        "batch-file" => { let (name, body) = l.batch_file(id()?, q.get("format").map(String::as_str).unwrap_or("ABA"))?; Ok(("application/octet-stream".into(), name, body.into_bytes())) }
        "payrun-file" => { let (name, body) = l.payrun_file(id()?, q.get("format").map(String::as_str).unwrap_or("ABA"))?; Ok(("application/octet-stream".into(), name, body.into_bytes())) }
        "attachment" => { let (name, mime, data) = l.attachment(id()?)?; Ok((mime, name, data)) }
        "mtd" => Ok(("application/json".into(), format!("vat-return-{}.json", id()?), serde_json::to_vec_pretty(&l.mtd_payload(id()?)?)?)),
        "payroll-filing" => Ok(("application/json".into(), format!("payroll-filing-{}.json", id()?), serde_json::to_vec_pretty(&l.payroll_filing_payload(id()?)?)?)),
        _ => Err(Error::NotFound(format!("download '{kind}'"))),
    }
}

pub fn seed_demo_extras(l: &Ledger) -> Result<()> {
    l.ensure_asset_types()?;
    let t = util::parse_date(&util::today())?;
    let bought = util::fmt_date(util::add_months(t, -8));
    let types = l.list_asset_types()?;
    if let Some(ct) = types.iter().find(|x| x["name"] == "Computer equipment").and_then(|x| x["id"].as_i64()) {
        let a = l.save_asset(None, &AssetInput { name: "MacBook Pro 16\"".into(), asset_type_id: Some(ct), purchase_date: bought.clone(), purchase_price: "4200".into(), serial_number: "C02XK1ABMD6T".into(), ..Default::default() })?;
        l.register_asset(a)?;
        let a2 = l.save_asset(None, &AssetInput { name: "Studio display".into(), asset_type_id: Some(ct), purchase_date: bought, purchase_price: "2499".into(), ..Default::default() })?;
        l.register_asset(a2)?;
        let prev = util::fmt_date(chrono::NaiveDate::from_ymd_opt(chrono::Datelike::year(&t), chrono::Datelike::month(&t), 1).unwrap() - chrono::Duration::days(1));
        l.run_depreciation(&prev)?;
    }
    // Payroll: two employees on the fortnightly calendar
    l.ensure_payroll_setup()?;
    let cal: i64 = l.conn.query_row("SELECT id FROM pay_calendars WHERE frequency = 'FORTNIGHTLY' LIMIT 1", [], |r| r.get(0))?;
    let start = util::fmt_date(util::add_months(t, -6));
    l.save_employee(None, &EmployeeInput { first_name: "Sam".into(), last_name: "Rivera".into(), email: "sam@harbourside.example".into(), start_date: start.clone(), pay_calendar_id: Some(cal),
        pay_basis: "SALARY".into(), annual_salary: Some("78000".into()), employment_type: "FULLTIME".into(), hours_per_week: Some("38".into()), tax_id: "123 456 782".into(), bank_account: "062-001 1122 3344".into(), ..Default::default() })?;
    l.save_employee(None, &EmployeeInput { first_name: "Priya".into(), last_name: "Nair".into(), email: "priya@harbourside.example".into(), start_date: start, pay_calendar_id: Some(cal),
        pay_basis: "HOURLY".into(), hourly_rate: Some("42.50".into()), employment_type: "PARTTIME".into(), hours_per_week: Some("24".into()), bank_account: "033-002 5566 7788".into(), ..Default::default() })?;
    // Project with time
    let cust: Option<i64> = l.conn.query_row("SELECT id FROM contacts WHERE name = 'Pinecrest Realty'", [], |r| r.get(0)).ok();
    let pid = l.save_project(None, &json!({"name": "Pinecrest brand refresh", "contact_id": cust, "estimate": "9500", "deadline": util::fmt_date(t + chrono::Duration::days(30))}))?;
    let task = l.save_task(None, pid, &json!({"name": "Design", "charge_type": "TIME", "rate": "145"}))?;
    l.save_task(None, pid, &json!({"name": "Brand guidelines document", "charge_type": "FIXED", "rate": "1200"}))?;
    for (d, m, desc) in [(-6, 240, "Discovery workshop"), (-4, 300, "Logo concepts"), (-2, 180, "Revisions")] {
        l.add_time(pid, task, &util::fmt_date(t + chrono::Duration::days(d)), m, desc)?;
    }
    l.add_project_cost(pid, &json!({"date": util::today(), "description": "Stock photography licence", "amount": "180", "chargeable": true, "markup": "10"}))?;
    // Expense claim awaiting approval
    let travel = l.account_by_code("493")?;
    let rid = l.save_receipt(None, &ReceiptInput { supplier: "Sydney Trains".into(), date: util::fmt_date(t - chrono::Duration::days(3)), kind: "RECEIPT".into(), line_amount_type: Some("INCLUSIVE".into()),
        lines: vec![crate::documents::LineInput { description: "Client site visit".into(), amount: Some("24.60".into()), account_id: travel, ..Default::default() }], ..Default::default() })?;
    l.set_receipt_status(rid, "SUBMITTED")?;
    Ok(())
}
