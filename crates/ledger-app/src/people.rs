//! Payroll (employees, pay runs, run payroll) and Taxes (GST/VAT/sales tax returns).

use crate::app::{App, Message, Opt, Route};
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD, col, coln};
use iced::widget::{Space, column, container, pick_list, row, text};
use iced::{Alignment, Element, Fill, Task};
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub enum Msg {
    RunPayroll,
    Created(Result<Value, String>, &'static str),
    Bank(Opt),
    Submit(i64),
    DeleteRun(i64),
    PrepareReturn,
    FileReturn(i64),
    InvoiceProject(i64),
}

#[derive(Default)]
pub struct State { pub bank: Option<Opt>, pub busy: bool }

fn i(v: &Value) -> i64 { v.as_i64().unwrap_or(0) }

pub fn load(app: &mut App) -> Task<Message> {
    let be = app.be.clone();
    let g = app.page_gen;
    let route = app.route.clone();
    if app.people.bank.is_none() { app.people.bank = app.lk.bank_opts().into_iter().next(); }
    Task::perform(async move {
        tokio::task::spawn_blocking(move || -> Result<Value, String> {
            Ok(match route {
                Route::Payroll => json!({"employees": be.call_now("payroll.employees", json!({}))?, "runs": be.call_now("payroll.payRuns", json!({}))?, "settings": be.call_now("payroll.settings", json!({}))?}),
                Route::PayRun(id) => be.call_now("payroll.payRun", json!({"id": id}))?,
                Route::Taxes => be.call_now("taxReturns.list", json!({}))?,
                Route::TaxReturn(id) => be.call_now("taxReturns.get", json!({"id": id}))?,
                Route::Projects => be.call_now("projects.list", json!({}))?,
                Route::Mileage => be.call_now("receipts.list", json!({}))?,
                _ => json!({}),
            })
        }).await.unwrap_or_else(|e| Err(e.to_string()))
    }, move |r| Message::Loaded(g, r))
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    match m {
        Msg::RunPayroll => {
            let cal = app.data.as_ref().and_then(|d| d["settings"]["calendars"][0]["id"].as_i64());
            let Some(cal) = cal else { return app.toast("Set up a pay calendar first (web version → Payroll settings)", true) };
            app.people.busy = true;
            app.be.call("payroll.createPayRun", json!({"calendar_id": cal}), |r| Message::People(Msg::Created(r, "run")))
        }
        Msg::Created(r, what) => {
            app.people.busy = false;
            match r {
                Ok(v) => {
                    let next = match what {
                        "run" => v["id"].as_i64().map(Route::PayRun).unwrap_or(Route::Payroll),
                        "return" => v["id"].as_i64().map(Route::TaxReturn).unwrap_or(Route::Taxes),
                        "project_invoice" => v["id"].as_i64().map(|id| Route::Doc { kind: "ACCREC".into(), id: Some(id) }).unwrap_or(Route::Projects),
                        "deleted" => Route::Payroll,
                        "filed" => app.route.clone(),
                        _ => app.route.clone(),
                    };
                    let msg = match what { "project_invoice" => "Draft invoice created from the project's unbilled work", "run" => "Pay run created — check it, then submit", "posted" => "Payroll submitted and posted", "deleted" => "Pay run deleted", "return" => "Return prepared", "filed" => "Marked as filed", _ => "Done" };
                    let t1 = app.toast(msg, false);
                    if next == app.route { return Task::batch([t1, app.load()]); }
                    Task::batch([t1, app.go(next)])
                }
                Err(e) => app.toast(e, true),
            }
        }
        Msg::Bank(b) => { app.people.bank = Some(b); Task::none() }
        Msg::Submit(id) => {
            app.people.busy = true;
            app.be.call("payroll.post", json!({"id": id, "bank_account_id": app.people.bank.as_ref().map(|b| b.id)}), |r| Message::People(Msg::Created(r, "posted")))
        }
        Msg::DeleteRun(id) => app.be.call("payroll.deletePayRun", json!({"id": id}), |r| Message::People(Msg::Created(r, "deleted"))),
        Msg::PrepareReturn => {
            let next = app.data.as_ref().map(|d| (d["form"].clone(), d["next"][0].clone(), d["next"][1].clone()));
            let Some((form, from, to)) = next else { return Task::none() };
            app.be.call("taxReturns.create", json!({"form": form, "from": from, "to": to}), |r| Message::People(Msg::Created(r, "return")))
        }
        Msg::FileReturn(id) => app.be.call("taxReturns.file", json!({"id": id}), |r| Message::People(Msg::Created(r, "filed"))),
        Msg::InvoiceProject(id) => app.be.call("projects.invoice", json!({"id": id}), |r| Message::People(Msg::Created(r, "project_invoice"))),
    }
}

fn loading<'a>(app: &'a App, title: &str) -> Option<Element<'a, Message>> {
    if app.data.is_some() { return None; }
    let body: Element<Message> = match &app.error { Some(e) => ui::card(column![ui::h2("Couldn't load"), ui::muted(e.clone())].spacing(6)), None => ui::muted("Loading…") };
    Some(ui::page(ui::h1(title.to_string()), vec![], body))
}

pub fn payroll(app: &App) -> Element<'_, Message> {
    if let Some(e) = loading(app, "Payroll") { return e; }
    let d = app.data.as_ref().unwrap();
    let k = t();
    let rate = |e: &Value| if e["pay_basis"] == "HOURLY" { format!("{}/hr", ui::money(i(&e["hourly_rate_e2"]) / 100)) } else { format!("{}/yr", ui::money(i(&e["annual_salary"]))) };
    let emps = d["employees"].as_array().cloned().unwrap_or_default().iter().map(|e| (vec![
        ui::cell(format!("{} {}", e["first_name"].as_str().unwrap_or(""), e["last_name"].as_str().unwrap_or(""))),
        ui::cell_muted(if e["pay_basis"] == "HOURLY" { "Hourly" } else { "Salary" }), ui::cell(rate(e)), ui::cell_muted(e["calendar_name"].as_str().unwrap_or("").to_string()),
        ui::chip(if e["status"] == "ACTIVE" { "Active" } else { "Inactive" }, if e["status"] == "ACTIVE" { k.ok } else { k.muted }),
    ], None)).collect();
    let runs = d["runs"].as_array().cloned().unwrap_or_default().iter().map(|r| (vec![
        ui::cell(r["number"].as_str().unwrap_or("").to_string()), ui::cell(format!("{} – {}", app.d(r["period_start"].as_str().unwrap_or("")), app.d(r["period_end"].as_str().unwrap_or("")))),
        ui::cell_muted(app.d(r["payment_date"].as_str().unwrap_or(""))), ui::cell_money(i(&r["wages"])), ui::cell_money(i(&r["net_pay"])),
        ui::chip(if r["status"] == "POSTED" { "Paid" } else { "Draft" }, if r["status"] == "POSTED" { k.ok } else { k.warn }),
    ], r["id"].as_i64().map(|id| Message::Nav(Route::PayRun(id))))).collect();
    let disclaimer = d["settings"]["disclaimer"].as_str().unwrap_or("Payroll tax tables are approximations — verify before paying anyone.").to_string();
    let body = column![
        container(text(disclaimer).size(13)).padding([8, 12]).style(theme::accent_soft).width(Fill),
        ui::h2("Pay runs"),
        ui::table(&[col("Pay run", 10), col("Pay period", 24), col("Pay date", 12), coln("Gross pay", 14), coln("Net pay", 14), col("Status", 10)], runs, "No pay runs yet."),
        ui::h2("Employees"),
        ui::table(&[col("Name", 26), col("Pay type", 12), col("Rate", 16), col("Pay schedule", 16), col("Status", 10)], emps, "No employees yet."),
    ].spacing(14);
    ui::page(ui::h1("Payroll"), vec![ui::btn_primary("Run payroll", if app.people.busy { None } else { Some(Message::People(Msg::RunPayroll)) })], body.into())
}

pub fn pay_run(app: &App) -> Element<'_, Message> {
    if let Some(e) = loading(app, "Pay run") { return e; }
    let r = app.data.as_ref().unwrap();
    let k = t();
    let id = i(&r["id"]);
    let draft = r["status"] == "DRAFT";
    let ret = if app.skin == Skin::QuickBooks && app.country() == "US" { "Employer taxes / 401(k)" } else { "Retirement" };
    let rows = r["payslips"].as_array().cloned().unwrap_or_default().iter().map(|s| (vec![
        ui::cell(s["employee"].as_str().unwrap_or("").to_string()), ui::cell_money(i(&s["gross"])), ui::cell_money(i(&s["tax"])), ui::cell_money(i(&s["deductions"])),
        ui::cell_money(i(&s["super"])), ui::cell_money(i(&s["net"])),
    ], None)).collect();
    let totals = row![
        column![ui::muted("Gross pay"), text(ui::money(i(&r["wages"]))).size(20).font(BOLD)].spacing(2).width(Fill),
        column![ui::muted("Tax withheld"), text(ui::money(i(&r["tax"]))).size(20).font(BOLD)].spacing(2).width(Fill),
        column![ui::muted(ret), text(ui::money(i(&r["super"]))).size(20).font(BOLD)].spacing(2).width(Fill),
        column![ui::muted("Net pay"), text(ui::money(i(&r["net_pay"]))).size(20).font(BOLD).color(k.accent)].spacing(2).width(Fill),
    ];
    let actions: Element<Message> = if draft {
        container(row![
            ui::btn_danger("Delete", Some(Message::People(Msg::DeleteRun(id)))), ui::hspace(),
            column![ui::label("Pay employees from"), pick_list(app.lk.bank_opts(), app.people.bank.clone(), |b| Message::People(Msg::Bank(b))).padding([7, 10]).text_size(14).style(theme::pick).width(240)].spacing(5),
            column![Space::new().height(18), ui::btn_primary("Submit payroll", if app.people.busy { None } else { Some(Message::People(Msg::Submit(id))) })],
        ].spacing(12).align_y(Alignment::End)).padding([12, 16]).style(theme::topbar).width(Fill).into()
    } else { ui::muted("Posted to the books. Payslips, emailing and bank files are in the web version for now.") };
    let title = format!("{} · {} – {}", r["number"].as_str().unwrap_or(""), app.d(r["period_start"].as_str().unwrap_or("")), app.d(r["period_end"].as_str().unwrap_or("")));
    ui::page(column![ui::link("Payroll", Message::Nav(Route::Payroll)), ui::h1(title)].spacing(4).into(), vec![ui::chip(if draft { "Draft" } else { "Paid" }, if draft { k.warn } else { k.ok })], column![
        ui::card(column![totals, ui::muted(format!("Pay date {}", app.d(r["payment_date"].as_str().unwrap_or(""))))].spacing(10)),
        ui::table(&[col("Employee", 26), coln("Gross", 14), coln("Tax", 14), coln("Deductions", 14), coln(if ret == "Retirement" { "Retirement" } else { "Employer" }, 14), coln("Net pay", 14)], rows, "No employees in this pay run."),
        actions,
    ].spacing(14).into())
}

pub fn taxes(app: &App) -> Element<'_, Message> {
    let title = if app.skin == Skin::QuickBooks { if app.country() == "US" { "Sales tax" } else { "Taxes" } } else { "Tax returns" };
    if let Some(e) = loading(app, title) { return e; }
    let d = app.data.as_ref().unwrap();
    let k = t();
    let rows = d["returns"].as_array().cloned().unwrap_or_default().iter().map(|r| {
        let boxes = r["data"]["boxes"].as_array().cloned().unwrap_or_default();
        let net = boxes.iter().find(|b| b["total"] == true).map(|b| i(&b["amount"])).unwrap_or(0);
        (vec![
            ui::cell(r["data"]["title"].as_str().unwrap_or(r["jurisdiction"].as_str().unwrap_or("")).to_string()),
            ui::cell(format!("{} – {}", app.d(r["period_start"].as_str().unwrap_or("")), app.d(r["period_end"].as_str().unwrap_or("")))),
            ui::cell_money(net),
            ui::chip(if r["status"] == "FILED" { "Filed" } else { "Draft" }, if r["status"] == "FILED" { k.ok } else { k.warn }),
        ], r["id"].as_i64().map(|id| Message::Nav(Route::TaxReturn(id))))
    }).collect();
    let next = format!("Next period: {} – {}", app.d(d["next"][0].as_str().unwrap_or("")), app.d(d["next"][1].as_str().unwrap_or("")));
    ui::page(ui::h1(title), vec![ui::btn("Liability report", Some(Message::Nav(Route::Report("tax-reconciliation".into())))), ui::btn_primary("Prepare return", Some(Message::People(Msg::PrepareReturn)))], column![
        ui::muted(next),
        ui::table(&[col("Return", 30), col("Period", 30), coln("Amount", 16), col("Status", 12)], rows, "No returns yet — prepare your first one."),
        ui::muted("LedgerCraft calculates the figures; lodging/filing with the tax office is done by you (direct e-filing needs government developer registration)."),
    ].spacing(14).into())
}

pub fn tax_return(app: &App) -> Element<'_, Message> {
    if let Some(e) = loading(app, "Tax return") { return e; }
    let r = app.data.as_ref().unwrap();
    let k = t();
    let id = i(&r["id"]);
    let data = &r["data"];
    let mut boxes = column![].spacing(0);
    for b in data["boxes"].as_array().cloned().unwrap_or_default() {
        let total = b["total"] == true;
        boxes = boxes.push(row![
            text(b["box"].as_str().unwrap_or("").to_string()).size(14).font(BOLD).color(k.accent).width(60),
            text(b["label"].as_str().unwrap_or("").to_string()).size(14).font(if total { BOLD } else { iced::Font::DEFAULT }).width(Fill),
            text(ui::money(i(&b["amount"]))).size(if total { 17 } else { 14 }).font(if total { BOLD } else { iced::Font::DEFAULT }),
        ].spacing(12).padding([9, 14]).align_y(Alignment::Center));
        boxes = boxes.push(ui::hr());
    }
    let filed = r["status"] == "FILED";
    let title = format!("{} · {} – {}", data["title"].as_str().unwrap_or("Return"), app.d(r["period_start"].as_str().unwrap_or("")), app.d(r["period_end"].as_str().unwrap_or("")));
    let actions = if filed { vec![ui::chip("Filed", k.ok)] } else { vec![ui::btn_primary("Mark as filed", Some(Message::People(Msg::FileReturn(id))))] };
    ui::page(column![ui::link("Returns", Message::Nav(Route::Taxes)), ui::h1(title)].spacing(4).into(), actions, column![
        container(boxes).style(theme::card).width(Fill).clip(true),
        ui::muted(if filed { "Filed returns lock the period so the figures can't change." } else { "Filing locks this period. Lodge the figures with your tax office yourself." }),
    ].spacing(14).into())
}

pub fn projects(app: &App) -> Element<'_, Message> {
    if let Some(e) = loading(app, "Projects") { return e; }
    let d = app.data.as_ref().unwrap();
    let k = t();
    let rows = d["projects"].as_array().cloned().unwrap_or_default().iter().map(|p| {
        let s = &p["summary"];
        let id = i(&p["id"]);
        let to_invoice = i(&s["to_invoice"]);
        let status = match p["status"].as_str().unwrap_or("") { "INPROGRESS" => ("In progress", k.info), "CLOSED" => ("Completed", k.ok), _ => ("Not started", k.muted) };
        (vec![
            ui::cell(p["name"].as_str().unwrap_or("").to_string()), ui::cell_muted(p["contact_name"].as_str().unwrap_or("").to_string()), ui::chip(status.0, status.1),
            ui::num(format!("{:.1} h", i(&s["minutes"]) as f64 / 60.0)), ui::cell_money(i(&s["costs"])), ui::cell_money(to_invoice),
            ui::num(ui::money(i(&s["profit"]))),
            if to_invoice > 0 { ui::link("Invoice", Message::People(Msg::InvoiceProject(id))) } else { ui::cell("") },
        ], None)
    }).collect();
    ui::page(ui::h1("Projects"), vec![], column![
        ui::table(&[col("Project", 22), col("Customer", 18), col("Status", 12), coln("Time", 9), coln("Costs", 11), coln("Unbilled", 11), coln("Profit", 11), col("", 8)], rows, "No projects yet."),
        ui::muted("Time tracking, tasks and expenses for projects are entered in the web version for now; invoicing unbilled work works here."),
    ].spacing(14).into())
}

pub fn mileage(app: &App) -> Element<'_, Message> {
    if let Some(e) = loading(app, "Mileage") { return e; }
    let d = app.data.as_ref().unwrap();
    let k = t();
    let all = d.as_array().cloned().unwrap_or_default();
    let trips: Vec<&Value> = all.iter().filter(|r| r["kind"] == "MILEAGE").collect();
    let unit = if app.country() == "US" { "mi" } else { "km" };
    let rows = trips.iter().map(|r| (vec![
        ui::cell(app.d(r["date"].as_str().unwrap_or(""))), ui::cell(r["claimant"].as_str().unwrap_or("").to_string()), ui::cell_muted(r["reference"].as_str().unwrap_or("").to_string()),
        ui::num(format!("{:.1} {unit}", i(&r["distance_e4"]) as f64 / 10000.0)), ui::cell_money(i(&r["total"])),
        ui::chip(match r["status"].as_str().unwrap_or("") { "PAID" => "Paid", "APPROVED" | "AUTHORISED" => "Approved", "DECLINED" => "Declined", "SUBMITTED" => "Submitted", _ => "Draft" }, k.info),
    ], None)).collect();
    let receipts = all.len() - trips.len();
    ui::page(ui::h1("Mileage"), vec![], column![
        ui::table(&[col("Date", 12), col("Driver", 20), col("Purpose", 26), coln("Distance", 12), coln("Amount", 12), col("Status", 12)], rows, "No trips recorded yet."),
        ui::muted(format!("{receipts} receipt claim(s) also on file. Recording new trips and claims is in the web version for now (Business → Expense claims).")),
    ].spacing(14).into())
}
