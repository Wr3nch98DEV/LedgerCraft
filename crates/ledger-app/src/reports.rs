//! Report centre (grouped and named per layout) and a generic report viewer for every engine report.

use crate::app::{App, Message, Opt, Route};
use crate::icons::icon;
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD};
use iced::widget::{Space, button, checkbox, column, container, pick_list, row, text};
use iced::{Alignment, Element, Fill, Length, Task};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Period { #[default] ThisMonth, LastMonth, ThisQuarter, ThisYearToDate, LastYear, Last12Months, Custom }
impl Period {
    const ALL: [Period; 7] = [Period::ThisMonth, Period::LastMonth, Period::ThisQuarter, Period::ThisYearToDate, Period::LastYear, Period::Last12Months, Period::Custom];
    /// (from, to) as ISO dates. Financial year follows the calendar here; the engine applies the org's year for balance sheets.
    fn range(self) -> (String, String) {
        use chrono::Datelike;
        let today = chrono::Local::now().date_naive();
        let first = |y: i32, m: u32| chrono::NaiveDate::from_ymd_opt(y, m, 1).unwrap();
        let last = |y: i32, m: u32| { let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) }; first(ny, nm) - chrono::Duration::days(1) };
        let (y, m) = (today.year(), today.month());
        let (f, t) = match self {
            Period::ThisMonth | Period::Custom => (first(y, m), last(y, m)),
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
        f.write_str(match self { Period::ThisMonth => "This month", Period::LastMonth => "Last month", Period::ThisQuarter => "This quarter", Period::ThisYearToDate => "This year to date", Period::LastYear => "Last year", Period::Last12Months => "Last 12 months", Period::Custom => "Custom dates" })
    }
}

/// How amounts are spread across columns ("Display columns by" / comparisons).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Columns { #[default] Total, Months, PreviousPeriod, PreviousYear }
impl Columns {
    const ALL: [Columns; 4] = [Columns::Total, Columns::Months, Columns::PreviousPeriod, Columns::PreviousYear];
}
impl std::fmt::Display for Columns {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self { Columns::Total => "Total only", Columns::Months => "Months", Columns::PreviousPeriod => "Compare: previous period", Columns::PreviousYear => "Compare: previous year" })
    }
}

/// Negative number style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Negative { #[default] Minus, Parentheses }
impl std::fmt::Display for Negative {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(match self { Negative::Minus => "-100", Negative::Parentheses => "(100)" }) }
}

#[derive(Default)]
pub struct State {
    pub period: Period,
    pub cash: bool,
    /// Customize panel open.
    pub customize: bool,
    pub from: String,
    pub to: String,
    pub columns: Columns,
    pub negative: Negative,
    pub show_zero: bool,
    pub contact: Option<Opt>,
    /// Desktop "Hide Header".
    pub hide_header: bool,
    /// QuickZoom: the account shown on the Account QuickReport.
    pub account: Option<i64>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Period(Period), Basis(bool), Export(bool), Exported(Result<String, String>),
    Customize, From(String), To(String), Columns(Columns), Negative(Negative), ShowZero(bool), Contact(Option<Opt>), Run, HideHeader, Zoom(i64),
}

/// The report's dates: the chosen period, or the custom dates when they parse.
fn dates(app: &App) -> (String, String) {
    let (f, t) = app.report.period.range();
    if app.report.period != Period::Custom { return (f, t); }
    let st = app.date_style();
    (ui::to_iso(&app.report.from, st).unwrap_or(f), ui::to_iso(&app.report.to, st).unwrap_or(t))
}

/// Engine parameters for the current settings.
pub fn params(app: &App) -> Value {
    let (from, to) = dates(app);
    let r = &app.report;
    let mut p = json!({"from": from, "to": to, "date": to, "basis": if r.cash { "CASH" } else { "ACCRUAL" }, "show_zero": r.show_zero});
    match r.columns {
        Columns::Total => {}
        Columns::Months => p["monthly"] = json!(true),
        Columns::PreviousYear => { p["compare"] = json!(1); p["period"] = json!("YEAR"); }
        Columns::PreviousPeriod => {
            // Step back by the length of the range in whole months (a quarter -> previous quarter).
            let months = |s: &str| s.get(..4).and_then(|y| y.parse::<i64>().ok()).unwrap_or(0) * 12 + s.get(5..7).and_then(|m| m.parse::<i64>().ok()).unwrap_or(0);
            let span = months(&to) - months(&from) + 1;
            p["compare"] = json!(1);
            p["period"] = json!(match span { 3 => "QUARTER", 12 => "YEAR", _ => "MONTH" });
        }
    }
    if let Some(c) = &r.contact { p["contact_id"] = json!(c.id); }
    // Account QuickReport: the account picked by QuickZoom.
    if matches!(&app.route, Route::Report(k) if k == "account-transactions") { if let Some(a) = r.account { p["account_id"] = json!(a); } }
    p
}

pub fn load(app: &mut App, key: &str) -> Task<Message> {
    if app.report.from.is_empty() {
        let (f, t) = app.report.period.range();
        app.report.from = app.d(&f);
        app.report.to = app.d(&t);
    }
    let g = app.page_gen;
    let params = params(app);
    app.be.call("reports.run", json!({"name": key, "params": params}), move |r| Message::Loaded(g, r))
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    match m {
        Msg::Period(p) => {
            app.report.period = p;
            if p == Period::Custom { app.report.customize = true; return Task::none(); }
            let (f, t) = p.range();
            app.report.from = app.d(&f);
            app.report.to = app.d(&t);
        }
        Msg::Basis(c) => app.report.cash = c,
        Msg::Customize => { app.report.customize = !app.report.customize; return Task::none(); }
        Msg::Zoom(id) => { app.report.account = Some(id); return app.go(Route::Report("account-transactions".into())); }
        Msg::HideHeader => { app.report.hide_header = !app.report.hide_header; return Task::none(); }
        Msg::From(v) => { app.report.from = v; app.report.period = Period::Custom; return Task::none(); }
        Msg::To(v) => { app.report.to = v; app.report.period = Period::Custom; return Task::none(); }
        Msg::Columns(c) => app.report.columns = c,
        Msg::Negative(n) => { app.report.negative = n; return Task::none(); }
        Msg::ShowZero(v) => app.report.show_zero = v,
        Msg::Contact(c) => app.report.contact = c,
        Msg::Run => {
            if app.report.period == Period::Custom {
                let st = app.date_style();
                match (ui::to_iso(&app.report.from, st), ui::to_iso(&app.report.to, st)) {
                    (Some(f), Some(t)) if f <= t => {}
                    (Some(_), Some(_)) => return app.toast("The start date is after the end date", true),
                    _ => return app.toast("Check the report dates", true),
                }
            }
        }
        Msg::Export(pdf) => {
            let (Some(rep), Route::Report(key)) = (app.data.clone(), app.route.clone()) else { return Task::none() };
            let skin = app.skin;
            let title = name(skin, &key, rep["title"].as_str().unwrap_or("Report"));
            let letter = matches!(app.country().as_str(), "US" | "CA");
            return Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let relabel = |s: &str| qb_label(skin, s);
                    let safe: String = title.chars().map(|c| if c.is_alphanumeric() || c == ' ' { c } else { '-' }).collect();
                    let path = crate::pdf::output_dir().join(format!("{} {}.{}", safe.trim(), chrono::Local::now().format("%Y-%m-%d"), if pdf { "pdf" } else { "csv" }));
                    let written = if pdf { std::fs::write(&path, crate::pdf::report(&rep, &title, letter, &relabel)) } else { std::fs::write(&path, crate::pdf::report_csv(&rep, &relabel)) };
                    written.map_err(|e| format!("Couldn't save: {e}"))?;
                    crate::pdf::open_file(&path);
                    Ok(path.display().to_string())
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, |r| Message::Report(Msg::Exported(r)));
        }
        Msg::Exported(r) => return match r { Ok(p) => app.toast(format!("Saved {p}"), false), Err(e) => app.toast(e, true) },
    }
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
        "purchases-by-contact" | "purchases-by-item" | "expense-claims" | "contractor-1099" => "Expenses and vendors",
        "tax-reconciliation" | "sales-tax-by-agency" => "Sales tax",
        "payroll-activity" | "leave-balances" => "Employees",
        "inventory-summary" | "fixed-asset-register" | "fixed-asset-reconciliation" | "depreciation-schedule" | "project-profitability" => "Inventory, assets and projects",
        _ => "For my accountant",
    }
}

const FAVOURITES: [&str; 5] = ["profit-and-loss", "balance-sheet", "cash-flow-statement", "aged-receivables", "aged-payables"];

pub fn centre(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return ui::page(ui::h1("Reports"), vec![], ui::muted("Loading…")) };
    let k = t();
    // 1099s are US-only.
    let us = app.country() == "US";
    let cat: Vec<Value> = d.as_array().cloned().unwrap_or_default().into_iter().filter(|r| us || !matches!(r["key"].as_str(), Some("contractor-1099" | "sales-tax-by-agency"))).collect();
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
pub fn qb_label(skin: Skin, s: &str) -> String {
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

fn cell_value(v: &Value, qty: bool, neg: Negative) -> String {
    match v.as_i64() {
        None => String::new(),
        Some(x) if qty => { let s = format!("{:.2}", x as f64 / 10000.0); s.trim_end_matches('0').trim_end_matches('.').to_string() }
        Some(x) if x < 0 && neg == Negative::Parentheses => format!("({})", ui::money(-x)),
        Some(x) => ui::money(x),
    }
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
        column![Space::new().height(18), row![
            ui::btn(if app.skin == Skin::QuickBooks { "Customize" } else { "Report settings" }, Some(Message::Report(Msg::Customize))),
            ui::btn("Export to PDF", app.data.as_ref().map(|_| Message::Report(Msg::Export(true)))),
            ui::btn(if app.skin == Skin::QuickBooks { "Export to Excel (CSV)" } else { "Export CSV" }, app.data.as_ref().map(|_| Message::Report(Msg::Export(false)))),
            ui::btn("Back to report list", Some(Message::Nav(Route::Reports))),
        ].spacing(6)],
    ].spacing(14).align_y(Alignment::End);
    let controls: Element<Message> = if app.desktop() { desk_toolbar(app, key) } else { controls.into() };
    let controls: Element<Message> = if app.report.customize { column![controls, customize(app, key)].spacing(12).into() } else { controls };
    let Some(d) = &app.data else {
        let body: Element<Message> = match &app.error { Some(e) => ui::card(column![ui::h2("Couldn't run this report"), ui::muted(e.clone())].spacing(6)), None => ui::muted("Running report…") };
        return ui::page(ui::h1(name(app.skin, key, "Report")), vec![], column![controls, body].spacing(16).into());
    };
    let title = name(app.skin, key, d["title"].as_str().unwrap_or("Report"));
    let columns: Vec<String> = d["columns"].as_array().cloned().unwrap_or_default().iter().map(|c| c.as_str().unwrap_or("").to_string()).collect();
    let neg = app.report.negative;
    let qty_cols: Vec<usize> = d["qty_cols"].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x.as_u64().map(|x| x as usize)).collect();
    let ncol = columns.len().max(1);
    let label_w: u16 = if ncol > 4 { 30 } else { 40 };
    let val_row = |label: Element<'static, Message>, values: &Value, bold: bool, msg: Option<Message>| -> Element<'static, Message> {
        let mut r = row![container(label).width(Length::FillPortion(label_w))].spacing(10).align_y(Alignment::Center);
        for c in 0..ncol {
            let s = cell_value(&values[c], qty_cols.contains(&c), neg);
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
            // QuickZoom: a document row opens it; an account row opens that account's transactions.
            let msg = r["document_id"].as_i64().map(|id| Message::Nav(Route::Doc { kind: "ACCREC".into(), id: Some(id) }))
                .or_else(|| r["account_id"].as_i64().filter(|_| key != "account-transactions").map(|a| Message::Report(Msg::Zoom(a))));
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
    let header: Element<Message> = if app.report.hide_header { Space::new().into() } else { header.into() };
    let report = container(column![header, ui::gap(8.0), body].spacing(4)).padding([20, 0]).style(theme::card).width(Fill);
    if app.desktop() {
        return column![container(controls).padding([8, 12]), iced::widget::scrollable(container(report).padding(iced::Padding { top: 0.0, left: 12.0, right: 12.0, bottom: 16.0 })).style(theme::scroll).height(Fill)].into();
    }
    ui::page(ui::h1(title), vec![], column![controls, report].spacing(16).into())
}

/// QuickBooks Desktop report window bar: buttons, then Dates / From / To / Columns.
fn desk_toolbar<'a>(app: &'a App, key: &str) -> Element<'a, Message> {
    let k = t();
    let has = app.data.is_some();
    let b = |l: &'static str, m: Option<Message>| ui::btn(l, m);
    let buttons = row![
        b("Customize Report", Some(Message::Report(Msg::Customize))), b("Comment on Report", None), b("Memorize", None),
        b("Print", if has { Some(Message::Report(Msg::Export(true))) } else { None }), b("E-mail", None),
        b("Excel", if has { Some(Message::Report(Msg::Export(false))) } else { None }),
        b(if app.report.hide_header { "Show Header" } else { "Hide Header" }, Some(Message::Report(Msg::HideHeader))),
        b("Refresh", Some(Message::Report(Msg::Run))),
    ].spacing(6);
    let lbl = |s: &'static str| text(s).size(12).color(k.info);
    let mut dates = row![
        lbl("Dates"), pick_list(&Period::ALL[..], Some(app.report.period), |p| Message::Report(Msg::Period(p))).padding([4, 7]).text_size(13).style(theme::pick).width(190),
        lbl("From"), iced::widget::text_input("", &app.report.from).on_input(|v| Message::Report(Msg::From(v))).on_submit(Message::Report(Msg::Run)).padding([4, 7]).size(13).style(theme::input).width(110),
        lbl("To"), iced::widget::text_input("", &app.report.to).on_input(|v| Message::Report(Msg::To(v))).on_submit(Message::Report(Msg::Run)).padding([4, 7]).size(13).style(theme::input).width(110),
    ].spacing(8).align_y(Alignment::Center);
    if by_period(key) {
        dates = dates.push(lbl("Columns")).push(pick_list(&Columns::ALL[..], Some(app.report.columns), |c| Message::Report(Msg::Columns(c))).padding([4, 7]).text_size(13).style(theme::pick).width(210));
    }
    dates = dates.push(lbl("Basis")).push(
        button(text(if app.report.cash { "Cash" } else { "Accrual" }).size(13)).style(theme::secondary).padding([4, 10]).on_press(Message::Report(Msg::Basis(!app.report.cash))));
    column![buttons, dates].spacing(8).into()
}

/// Reports that can be filtered to one customer or vendor.
fn by_contact(key: &str) -> bool {
    matches!(key, "aged-receivables" | "aged-payables" | "contact-transactions" | "general-ledger-detail" | "account-transactions")
}
/// Reports that can show months or comparison columns.
fn by_period(key: &str) -> bool { matches!(key, "profit-and-loss" | "balance-sheet" | "budget-variance") }

/// The Customize panel (QuickBooks) / report settings (Xero).
fn customize<'a>(app: &'a App, key: &str) -> Element<'a, Message> {
    let qb = app.skin == Skin::QuickBooks;
    let mut general = row![
        ui::field("From", &app.report.from, |v| Message::Report(Msg::From(v))),
        ui::field("To", &app.report.to, |v| Message::Report(Msg::To(v))),
        column![ui::label("Negative numbers"), pick_list(&[Negative::Minus, Negative::Parentheses][..], Some(app.report.negative), |n| Message::Report(Msg::Negative(n))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill),
    ].spacing(14);
    if by_period(key) {
        general = general.push(column![ui::label(if qb { "Display columns by" } else { "Columns" }), pick_list(&Columns::ALL[..], Some(app.report.columns), |c| Message::Report(Msg::Columns(c))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill));
    }
    let mut extra = row![checkbox(app.report.show_zero).label(if qb { "Show rows with zero amounts" } else { "Show accounts with no transactions" }).on_toggle(|v| Message::Report(Msg::ShowZero(v))).text_size(14)].spacing(14).align_y(Alignment::Center);
    if by_contact(key) {
        let payables = key.contains("payable");
        let all = if payables { if qb { "All vendors" } else { "All suppliers" } } else if qb { "All customers" } else { "All contacts" };
        let mut opts = vec![Opt { id: 0, label: all.into() }];
        opts.extend(app.lk.contact_opts(!payables));
        let sel = app.report.contact.clone().unwrap_or_else(|| opts[0].clone());
        let who = if payables { if qb { "Vendor" } else { "Supplier" } } else if qb { "Customer" } else { "Contact" };
        extra = extra.push(Space::new().width(20)).push(text(who).size(13).color(t().muted))
            .push(pick_list(opts, Some(sel), |o: Opt| Message::Report(Msg::Contact(if o.id == 0 { None } else { Some(o) }))).padding([7, 10]).text_size(14).style(theme::pick).width(260));
    }
    ui::card(column![
        text(if qb { "Customize report" } else { "Report settings" }).size(15).font(BOLD),
        general,
        extra,
        row![ui::hspace(), ui::btn("Close", Some(Message::Report(Msg::Customize))), ui::btn_primary(if qb { "Run report" } else { "Update" }, Some(Message::Report(Msg::Run)))].spacing(8),
    ].spacing(12))
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
            if p == Period::Custom { continue; }
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
