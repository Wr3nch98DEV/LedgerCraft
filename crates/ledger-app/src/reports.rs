//! Report centre (grouped and named per layout) and a generic report viewer for every engine report.

use crate::app::{App, Message, Route};
use crate::icons::icon;
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD};
use iced::widget::{Space, button, column, container, pick_list, row, text};
use iced::{Alignment, Element, Fill, Length, Task};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Period { #[default] ThisMonth, LastMonth, ThisQuarter, ThisYearToDate, LastYear, Last12Months }
impl Period {
    const ALL: [Period; 6] = [Period::ThisMonth, Period::LastMonth, Period::ThisQuarter, Period::ThisYearToDate, Period::LastYear, Period::Last12Months];
    /// (from, to) as ISO dates. Financial year follows the calendar here; the engine applies the org's year for balance sheets.
    fn range(self) -> (String, String) {
        use chrono::Datelike;
        let today = chrono::Local::now().date_naive();
        let first = |y: i32, m: u32| chrono::NaiveDate::from_ymd_opt(y, m, 1).unwrap();
        let last = |y: i32, m: u32| { let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) }; first(ny, nm) - chrono::Duration::days(1) };
        let (y, m) = (today.year(), today.month());
        let (f, t) = match self {
            Period::ThisMonth => (first(y, m), last(y, m)),
            Period::LastMonth => { let (py, pm) = if m == 1 { (y - 1, 12) } else { (y, m - 1) }; (first(py, pm), last(py, pm)) }
            Period::ThisQuarter => { let qm = (m - 1) / 3 * 3 + 1; (first(y, qm), last(y, qm + 2)) }
            Period::ThisYearToDate => (first(y, 1), today),
            Period::LastYear => (first(y - 1, 1), last(y - 1, 12)),
            Period::Last12Months => (first(y - 1, m), last(y, m)),
        };
        (f.format("%Y-%m-%d").to_string(), t.format("%Y-%m-%d").to_string())
    }
}
impl std::fmt::Display for Period {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self { Period::ThisMonth => "This month", Period::LastMonth => "Last month", Period::ThisQuarter => "This quarter", Period::ThisYearToDate => "This year to date", Period::LastYear => "Last year", Period::Last12Months => "Last 12 months" })
    }
}

#[derive(Default)]
pub struct State { pub period: Period, pub cash: bool }

#[derive(Debug, Clone)]
pub enum Msg { Period(Period), Basis(bool) }

pub fn load(app: &mut App, key: &str) -> Task<Message> {
    let (from, to) = app.report.period.range();
    let g = app.page_gen;
    let params = json!({"from": from, "to": to, "date": to, "basis": if app.report.cash { "CASH" } else { "ACCRUAL" }});
    app.be.call("reports.run", json!({"name": key, "params": params}), move |r| Message::Loaded(g, r))
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    match m { Msg::Period(p) => app.report.period = p, Msg::Basis(c) => app.report.cash = c }
    app.load()
}

/// The report's name in each product.
pub fn name(skin: Skin, key: &str, fallback: &str) -> String {
    if skin == Skin::Xero { return fallback.to_string(); }
    match key {
        "aged-receivables" => "A/R Aging Summary", "aged-payables" => "A/P Aging Summary", "receivable-invoice-detail" => "Invoice List",
        "sales-by-contact" => "Sales by Customer Summary", "sales-by-item" => "Sales by Product/Service Summary", "income-by-contact" => "Income by Customer Summary",
        "payable-invoice-detail" => "Bill List", "purchases-by-contact" => "Expenses by Vendor Summary", "purchases-by-item" => "Purchases by Product/Service Detail",
        "general-ledger" => "General Ledger", "general-ledger-detail" => "Transaction Detail by Account", "journal-report" => "Journal", "account-transactions" => "Account QuickReport",
        "bank-reconciliation" => "Reconciliation Report", "tax-reconciliation" => "Sales Tax Liability Report", "executive-summary" => "Business Snapshot",
        "budget-summary" => "Budget Overview", "budget-variance" => "Budget vs. Actuals", "payroll-activity" => "Payroll Summary", "leave-balances" => "Time Off Balances",
        "inventory-summary" => "Inventory Valuation Summary", "project-profitability" => "Project Profitability Summary", "contact-transactions" => "Transaction List by Customer",
        "fx-gains-losses" => "Realized and Unrealized Gains and Losses", "expense-claims" => "Expense Claims", "cash-summary" => "Cash Summary",
        _ => fallback,
    }.to_string()
}

/// QuickBooks report centre groups.
fn qb_group(key: &str) -> &'static str {
    match key {
        "balance-sheet" | "profit-and-loss" | "cash-flow-statement" | "executive-summary" | "budget-summary" | "budget-variance" | "cash-summary" => "Business overview",
        "aged-receivables" | "receivable-invoice-detail" | "contact-transactions" => "Who owes you",
        "sales-by-contact" | "sales-by-item" | "income-by-contact" => "Sales and customers",
        "aged-payables" | "payable-invoice-detail" => "What you owe",
        "purchases-by-contact" | "purchases-by-item" | "expense-claims" => "Expenses and vendors",
        "tax-reconciliation" => "Sales tax",
        "payroll-activity" | "leave-balances" => "Employees",
        "inventory-summary" | "fixed-asset-register" | "fixed-asset-reconciliation" | "depreciation-schedule" | "project-profitability" => "Inventory, assets and projects",
        _ => "For my accountant",
    }
}

const FAVOURITES: [&str; 5] = ["profit-and-loss", "balance-sheet", "cash-flow-statement", "aged-receivables", "aged-payables"];

pub fn centre(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return ui::page(ui::h1("Reports"), vec![], ui::muted("Loading…")) };
    let k = t();
    let cat: Vec<Value> = d.as_array().cloned().unwrap_or_default();
    let link_row = |key: String, title: String, desc: String| -> Element<'_, Message> {
        button(row![icon("reports", 16.0, k.accent), column![text(title).size(14).color(k.accent), ui::muted(desc)].spacing(1)].spacing(10).align_y(Alignment::Center))
            .style(theme::row_btn).padding([8, 10]).width(Fill).on_press(Message::Nav(Route::Report(key))).into()
    };
    let groups: Vec<(String, Vec<Value>)> = match app.skin {
        Skin::QuickBooks => {
            let order = ["Favourites", "Business overview", "Who owes you", "Sales and customers", "What you owe", "Expenses and vendors", "Sales tax", "Employees", "For my accountant", "Inventory, assets and projects"];
            order.iter().map(|g| (g.to_string(), cat.iter().filter(|r| if *g == "Favourites" { FAVOURITES.contains(&r["key"].as_str().unwrap_or("")) } else { qb_group(r["key"].as_str().unwrap_or("")) == *g }).cloned().collect())).collect()
        }
        Skin::Xero => {
            let mut seen: Vec<String> = vec![];
            for r in &cat { let c = r["category"].as_str().unwrap_or("").to_string(); if !seen.contains(&c) { seen.push(c); } }
            seen.into_iter().map(|g| { let items = cat.iter().filter(|r| r["category"] == g.as_str()).cloned().collect(); (g, items) }).collect()
        }
    };
    let mut col = column![].spacing(16);
    for (g, items) in groups.into_iter().filter(|(_, v)| !v.is_empty()) {
        let mut list = column![].spacing(0);
        for r in items {
            let key = r["key"].as_str().unwrap_or("").to_string();
            list = list.push(link_row(key.clone(), name(app.skin, &key, r["title"].as_str().unwrap_or("")), r["description"].as_str().unwrap_or("").to_string()));
        }
        col = col.push(ui::card(column![ui::h2(g), list].spacing(8)));
    }
    ui::page(ui::h1("Reports"), vec![], col.into())
}

/// QuickBooks wording for the engine's report section names.
fn qb_label(skin: Skin, s: &str) -> String {
    if skin == Skin::Xero { return s.to_string(); }
    match s {
        "Trading Income" | "Revenue" => "Income".into(), "Total Trading Income" | "Total Revenue" => "Total Income".into(),
        "Operating Expenses" => "Expenses".into(), "Total Operating Expenses" => "Total Expenses".into(),
        "Net Profit" => "Net Income".into(), "Gross Profit" => "Gross Profit".into(),
        "Cost of Sales" => "Cost of Goods Sold".into(), "Total Cost of Sales" => "Total Cost of Goods Sold".into(),
        "Net Assets" => "Net Assets".into(), "Current Year Earnings" => "Net Income".into(),
        _ => s.to_string(),
    }
}

fn cell_value(v: &Value, qty: bool) -> String {
    match v.as_i64() { None => String::new(), Some(x) if qty => { let s = format!("{:.2}", x as f64 / 10000.0); s.trim_end_matches('0').trim_end_matches('.').to_string() } Some(x) => ui::money(x) }
}

pub fn view(app: &App) -> Element<'_, Message> {
    let Route::Report(key) = &app.route else { return Space::new().into() };
    let k = t();
    let controls = row![
        column![ui::label("Report period"), pick_list(&Period::ALL[..], Some(app.report.period), |p| Message::Report(Msg::Period(p))).padding([7, 10]).text_size(14).style(theme::pick).width(220)].spacing(5),
        column![ui::label("Accounting method"), row![
            button(text("Accrual").size(14)).style(if !app.report.cash { theme::primary } else { theme::secondary }).padding([7, 14]).on_press(Message::Report(Msg::Basis(false))),
            button(text("Cash").size(14)).style(if app.report.cash { theme::primary } else { theme::secondary }).padding([7, 14]).on_press(Message::Report(Msg::Basis(true))),
        ].spacing(4)].spacing(5),
        ui::hspace(),
        column![Space::new().height(18), ui::btn("Back to report list", Some(Message::Nav(Route::Reports)))],
    ].spacing(14).align_y(Alignment::End);
    let Some(d) = &app.data else {
        let body: Element<Message> = match &app.error { Some(e) => ui::card(column![ui::h2("Couldn't run this report"), ui::muted(e.clone())].spacing(6)), None => ui::muted("Running report…") };
        return ui::page(ui::h1(name(app.skin, key, "Report")), vec![], column![controls, body].spacing(16).into());
    };
    let title = name(app.skin, key, d["title"].as_str().unwrap_or("Report"));
    let columns: Vec<String> = d["columns"].as_array().cloned().unwrap_or_default().iter().map(|c| c.as_str().unwrap_or("").to_string()).collect();
    let qty_cols: Vec<usize> = d["qty_cols"].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x.as_u64().map(|x| x as usize)).collect();
    let ncol = columns.len().max(1);
    let label_w: u16 = if ncol > 4 { 30 } else { 40 };
    let val_row = |label: Element<'static, Message>, values: &Value, bold: bool, msg: Option<Message>| -> Element<'static, Message> {
        let mut r = row![container(label).width(Length::FillPortion(label_w))].spacing(10).align_y(Alignment::Center);
        for c in 0..ncol {
            let s = cell_value(&values[c], qty_cols.contains(&c));
            r = r.push(container(text(s).size(14).font(if bold { BOLD } else { iced::Font::DEFAULT }).width(Fill).align_x(iced::alignment::Horizontal::Right)).width(Length::FillPortion(12)));
        }
        button(r.padding([6, 14])).style(theme::row_btn).padding(0).width(Fill).on_press_maybe(msg).into()
    };
    let head = {
        let mut r = row![container(Space::new()).width(Length::FillPortion(label_w))].spacing(10);
        for c in &columns { r = r.push(container(text(c.to_uppercase()).size(11).font(BOLD).color(k.muted).width(Fill).align_x(iced::alignment::Horizontal::Right)).width(Length::FillPortion(12))); }
        container(r.padding([9, 14])).style(theme::header_row).width(Fill)
    };
    let mut body = column![head].width(Fill);
    for s in d["sections"].as_array().cloned().unwrap_or_default() {
        if let Some(tt) = s["title"].as_str().filter(|x| !x.is_empty()) {
            body = body.push(container(text(qb_label(app.skin, tt)).size(14).font(BOLD)).padding(iced::Padding { top: 12.0, bottom: 4.0, left: 14.0, right: 14.0 }));
        }
        for r in s["rows"].as_array().cloned().unwrap_or_default() {
            let date = r["date"].as_str().map(|x| format!("{}  ", app.d(x))).unwrap_or_default();
            let lbl: Element<'static, Message> = text(format!("{date}{}", qb_label(app.skin, r["label"].as_str().unwrap_or("")))).size(14).into();
            let msg = r["document_id"].as_i64().map(|id| Message::Nav(Route::Doc { kind: "ACCREC".into(), id: Some(id) }));
            body = body.push(val_row(lbl, &r["values"], false, msg));
            for c in r["children"].as_array().cloned().unwrap_or_default() {
                let date = c["date"].as_str().map(|x| format!("{}  ", app.d(x))).unwrap_or_default();
                let lbl: Element<'static, Message> = container(text(format!("{date}{}", c["label"].as_str().unwrap_or(""))).size(13).color(k.muted)).padding(iced::Padding { left: 18.0, ..iced::Padding::ZERO }).into();
                let msg = c["document_id"].as_i64().map(|id| Message::Nav(Route::Doc { kind: "ACCREC".into(), id: Some(id) }));
                body = body.push(val_row(lbl, &c["values"], false, msg));
            }
        }
        if !s["total"].is_null() {
            body = body.push(ui::hr());
            let lbl: Element<'static, Message> = text(qb_label(app.skin, s["total"]["label"].as_str().unwrap_or("Total"))).size(14).font(BOLD).into();
            body = body.push(val_row(lbl, &s["total"]["values"], true, None));
            body = body.push(ui::gap(4.0));
        }
    }
    let header = column![
        text(d["org"].as_str().unwrap_or("").to_string()).size(13).color(k.muted),
        text(title.clone()).size(20).font(BOLD),
        ui::muted(format!("{}{}", d["subtitle"].as_str().unwrap_or(""), d["basis"].as_str().map(|b| format!(" · {b} basis")).unwrap_or_default())),
    ].spacing(2).align_x(Alignment::Center).width(Fill);
    let report = container(column![header, ui::gap(8.0), body].spacing(4)).padding([20, 0]).style(theme::card).width(Fill);
    ui::page(ui::h1(title), vec![], column![controls, report].spacing(16).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Every report in the engine's catalogue runs with the parameters the viewer sends.
    #[test]
    fn every_report_runs_from_the_viewer() {
        let dir = std::env::temp_dir().join(format!("lc-rep-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let be = crate::backend::Backend::new(dir.clone());
        be.call_now("orgs.create", json!({"org_name": "Demo", "country": "US", "demo": true, "owner_name": "Alex"})).unwrap();
        let cat = be.call_now("reports.catalogue", json!({})).unwrap();
        for p in Period::ALL {
            let (from, to) = p.range();
            assert!(from <= to, "{p}: {from} > {to}");
            for r in cat.as_array().unwrap() {
                let key = r["key"].as_str().unwrap();
                if key == "bank-reconciliation" { continue; } // needs an account
                let out = be.call_now("reports.run", json!({"name": key, "params": {"from": from, "to": to, "date": to, "basis": "CASH"}}));
                assert!(out.is_ok(), "{key} ({p}): {:?}", out.err());
                assert!(!name(Skin::QuickBooks, key, "x").is_empty());
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
