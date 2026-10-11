//! Home screen: QuickBooks-style "Business overview" or Xero-style dashboard, from the same data.

use crate::app::{App, Message, Route};
use crate::icons::icon;
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD, money};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Color, Element, Fill, Length};
use serde_json::{Value, json};

/// Dashboard data = engine dashboard + last-30-days P&L (for the expenses breakdown).
pub fn load(app: &mut App) -> iced::Task<Message> {
    let be = app.be.clone();
    let g = app.page_gen;
    iced::Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let mut d = be.call_now("dashboard", json!({}))?;
                let to = d["today"].as_str().unwrap_or("").to_string();
                let from = chrono::NaiveDate::parse_from_str(&to, "%Y-%m-%d").map(|x| (x - chrono::Duration::days(30)).format("%Y-%m-%d").to_string()).unwrap_or_default();
                if let Ok(pl) = be.call_now("reports.run", json!({"name": "profit-and-loss", "params": {"from": from, "to": to}})) { d["pl30"] = pl; }
                Ok(d)
            }).await.unwrap_or_else(|e| Err(e.to_string()))
        },
        move |r| Message::Loaded(g, r),
    )
}

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading(app) };
    match app.skin { Skin::QuickBooks => qb(app, d), Skin::Xero => xero(app, d) }
}

fn loading(app: &App) -> Element<'_, Message> {
    let body: Element<Message> = match &app.error { Some(e) => ui::card(column![ui::h2("Couldn't load"), ui::muted(e.clone())].spacing(6)), None => ui::muted("Loading…") };
    ui::page(ui::h1("Dashboard"), vec![], body)
}

fn i(v: &Value) -> i64 { v.as_i64().unwrap_or(0) }

/// Simple vertical bar chart from (label, [(value, colour)]) groups.
pub fn bars<'a>(data: Vec<(String, Vec<(i64, Color)>)>, height: f32) -> Element<'a, Message> {
    let max = data.iter().flat_map(|(_, v)| v.iter().map(|x| x.0.abs())).max().unwrap_or(0).max(1) as f32;
    let k = t();
    row(data.into_iter().map(|(label, vals)| {
        let bars = row(vals.into_iter().map(|(v, c)| {
            let h = (v.abs() as f32 / max * height).max(if v != 0 { 2.0 } else { 0.0 });
            column![Space::new().height(Fill), container(Space::new()).width(Fill).height(h).style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(c)), border: iced::Border { radius: iced::border::Radius { top_left: 3.0, top_right: 3.0, bottom_right: 0.0, bottom_left: 0.0 }, ..Default::default() }, ..Default::default() })]
                .width(Fill).height(height).into()
        })).spacing(3).width(Fill);
        column![bars, text(label).size(11).color(k.muted).width(Fill).align_x(iced::alignment::Horizontal::Center)].spacing(4).width(Fill).into()
    })).spacing(10).width(Fill).into()
}

/// Horizontal proportion bar (e.g. overdue vs not due).
fn split_bar<'a>(parts: Vec<(i64, Color)>) -> Element<'a, Message> {
    let total: i64 = parts.iter().map(|p| p.0.max(0)).sum();
    let k = t();
    if total == 0 {
        return container(Space::new()).width(Fill).height(10).style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.bg2)), border: iced::Border { radius: 5.0.into(), ..Default::default() }, ..Default::default() }).into();
    }
    row(parts.into_iter().filter(|p| p.0 > 0).map(|(v, c)| {
        container(Space::new()).width(Length::FillPortion(((v * 1000 / total).max(1)) as u16)).height(10)
            .style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(c)), ..Default::default() }).into()
    })).spacing(2).width(Fill).into()
}

fn stat<'a>(label: &str, amount: i64, color: Color) -> Element<'a, Message> {
    column![text(ui::money(amount)).size(20).font(BOLD).color(color), ui::muted(label.to_string())].spacing(2).into()
}

fn qb<'a>(app: &'a App, d: &'a Value) -> Element<'a, Message> {
    let k = t();
    let cur = d["base_currency"].as_str().unwrap_or("").to_string();
    let s = &d["sales"];
    let overdue = i(&s["overdue_total"]);
    let open = i(&s["awaiting_total"]) - overdue;

    // Invoices: unpaid split into overdue / not due yet.
    let invoices = ui::card(column![
        row![ui::h2("Invoices"), ui::hspace(), ui::muted("Unpaid")].align_y(Alignment::Center),
        row![stat("Overdue", overdue, k.warn), ui::hspace(), stat("Not due yet", open, k.muted)],
        split_bar(vec![(overdue, k.warn), (open, k.line2)]),
        row![ui::muted(format!("{} unpaid · {} draft", i(&s["awaiting_count"]), i(&s["draft_count"]))), ui::hspace(), ui::link("View invoices", Message::Nav(Route::Invoices))],
    ].spacing(12));

    // Expenses by category, last 30 days.
    let mut cats: Vec<(String, i64)> = vec![];
    for sec in d["pl30"]["sections"].as_array().cloned().unwrap_or_default() {
        let title = sec["title"].as_str().unwrap_or("");
        if title.contains("Expense") || title.contains("Cost of Sales") || title.contains("Direct Costs") {
            for r in sec["rows"].as_array().cloned().unwrap_or_default() { cats.push((r["label"].as_str().unwrap_or("").to_string(), i(&r["values"][0]))); }
        }
    }
    cats.sort_by(|a, b| b.1.cmp(&a.1));
    let total_exp: i64 = cats.iter().map(|c| c.1).sum();
    let palette = [k.accent, k.info, k.warn, k.bad, k.ok, k.muted];
    let top: Vec<(String, i64, Color)> = cats.iter().take(5).enumerate().map(|(n, c)| (c.0.clone(), c.1, palette[n])).chain(
        if cats.len() > 5 { Some(("Everything else".to_string(), cats.iter().skip(5).map(|c| c.1).sum(), palette[5])) } else { None }).collect();
    let legend = column(top.iter().map(|(l, v, c)| {
        let c = *c;
        row![container(Space::new()).width(10).height(10).style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(c)), border: iced::Border { radius: 2.0.into(), ..Default::default() }, ..Default::default() }),
            text(l.clone()).size(13).width(Fill), text(ui::money(*v)).size(13)].spacing(8).align_y(Alignment::Center).into()
    })).spacing(6);
    let expenses = ui::card(column![
        row![ui::h2("Expenses"), ui::hspace(), ui::muted("Last 30 days")].align_y(Alignment::Center),
        column![text(format!("{cur} {}", ui::money(total_exp))).size(20).font(BOLD), ui::muted("Total spending")].spacing(2),
        split_bar(top.iter().map(|x| (x.1, x.2)).collect()),
        legend,
    ].spacing(12));

    // Profit and loss this month.
    let pl = &d["pl_month"];
    let (inc, exp) = (i(&pl["income"]), i(&pl["expenses"]));
    let net = i(&pl["net"]);
    let maxv = inc.abs().max(exp.abs()).max(1);
    let hbar = |v: i64, c: Color| -> Element<'_, Message> {
        let w = ((v.abs() * 1000) / maxv).max(1) as u16;
        row![container(Space::new()).width(Length::FillPortion(w)).height(14).style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(c)), border: iced::Border { radius: 3.0.into(), ..Default::default() }, ..Default::default() }),
            Space::new().width(Length::FillPortion((1000 - w.min(1000)).max(1)))].into()
    };
    let profit = ui::card(column![
        row![ui::h2("Profit and loss"), ui::hspace(), ui::muted("This month")].align_y(Alignment::Center),
        column![text(format!("{cur} {}", ui::money(net))).size(20).font(BOLD).color(if net >= 0 { k.ok } else { k.bad }), ui::muted(if net >= 0 { "Net income" } else { "Net loss" })].spacing(2),
        row![text("Income").size(13).width(80), hbar(inc, k.ok), text(ui::money(inc)).size(13).width(100).align_x(iced::alignment::Horizontal::Right)].spacing(8).align_y(Alignment::Center),
        row![text("Expenses").size(13).width(80), hbar(exp, k.bad), text(ui::money(exp)).size(13).width(100).align_x(iced::alignment::Horizontal::Right)].spacing(8).align_y(Alignment::Center),
        row![ui::hspace(), ui::link("See profit and loss report", Message::Nav(Route::Report("profit-and-loss".into())))],
    ].spacing(12));

    // Bank accounts.
    let mut banks = column![row![ui::h2("Bank accounts"), ui::hspace(), ui::link("Go to banking", Message::Nav(Route::Banking(None)))].align_y(Alignment::Center)].spacing(10);
    for b in d["banks"].as_array().cloned().unwrap_or_default().into_iter().filter(|b| b["status"] == "ACTIVE") {
        let id = b["id"].as_i64().unwrap_or(0);
        let review = i(&b["unreconciled_count"]);
        banks = banks.push(ui::hr());
        banks = banks.push(button(column![
            row![text(b["name"].as_str().unwrap_or("").to_string()).size(14).font(BOLD), ui::hspace(),
                if review > 0 { ui::chip(&format!("{review} to review"), k.accent) } else { Space::new().into() }].align_y(Alignment::Center),
            row![ui::muted("Bank balance"), ui::hspace(), text(if b["last_statement_date"].is_null() { "—".to_string() } else { ui::money(i(&b["statement_balance"])) }).size(13)],
            row![ui::muted("In LedgerCraft"), ui::hspace(), text(ui::money(i(&b["ledger_balance_fx"]))).size(13)],
        ].spacing(4)).style(theme::row_btn).padding([6, 4]).width(Fill).on_press(Message::Nav(Route::Banking(Some(id)))));
    }

    // Sales (money in, last 6 months).
    let sales_chart = bars(d["cash_flow"].as_array().cloned().unwrap_or_default().iter().map(|m| (m["label"].as_str().unwrap_or("").to_string(), vec![(i(&m["in"]), k.accent)])).collect(), 130.0);
    let sales = ui::card(column![row![ui::h2("Sales"), ui::hspace(), ui::muted("Money in · last 6 months")].align_y(Alignment::Center), sales_chart].spacing(14));

    let actions: Vec<Element<Message>> = vec![
        ui::btn("Create invoice", Some(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None }))),
        ui::btn("Record expense", Some(Message::Nav(Route::Spend(None)))),
    ];
    let body = column![
        tasks(app, d),
        row![container(invoices).width(Fill), container(expenses).width(Fill)].spacing(16),
        row![container(profit).width(Fill), container(ui::card(banks)).width(Fill)].spacing(16),
        sales,
    ].spacing(16);
    ui::page(column![ui::h1("Business overview"), ui::muted(format!("Welcome back{}", first_name(app)))].spacing(2).into(), actions, body.into())
}

fn first_name(app: &App) -> String {
    let n = app.user_name();
    let f = n.split_whitespace().next().unwrap_or("");
    if f.is_empty() || f == "Owner" { String::new() } else { format!(", {f}") }
}

/// Getting-started checklist and alerts (both layouts).
fn tasks<'a>(_app: &'a App, d: &'a Value) -> Element<'a, Message> {
    let k = t();
    let gs = d["getting_started"].as_array().cloned().unwrap_or_default();
    let done = gs.iter().filter(|s| s["done"] == true).count();
    let mut col = column![].spacing(10);
    if !gs.is_empty() && done < gs.len() {
        let items = column(gs.iter().map(|s| {
            let ok = s["done"] == true;
            row![icon(if ok { "check" } else { "chevron" }, 14.0, if ok { k.ok } else { k.muted }), text(s["text"].as_str().unwrap_or("").to_string()).size(13).color(if ok { k.muted } else { k.text })].spacing(8).align_y(Alignment::Center).into()
        })).spacing(5);
        col = col.push(ui::card(column![row![ui::h2("Get set up"), ui::hspace(), ui::muted(format!("{done} of {} done", gs.len()))], items].spacing(10)));
    }
    let alerts: Vec<Element<Message>> = d["tasks"].as_array().cloned().unwrap_or_default().iter().map(|t| {
        let warn = t["level"] == "warn";
        container(text(t["text"].as_str().unwrap_or("").to_string()).size(13)).padding([6, 12]).style(if warn { theme::bad_soft } else { theme::accent_soft }).into()
    }).collect();
    if !alerts.is_empty() { col = col.push(row(alerts).spacing(8)); }
    col.into()
}

fn xero<'a>(app: &'a App, d: &'a Value) -> Element<'a, Message> {
    let k = t();
    let mut cards: Vec<Element<Message>> = vec![];
    for b in d["banks"].as_array().cloned().unwrap_or_default().into_iter().filter(|b| b["status"] == "ACTIVE") {
        let id = b["id"].as_i64().unwrap_or(0);
        let review = i(&b["unreconciled_count"]);
        cards.push(ui::card(column![
            text(b["name"].as_str().unwrap_or("").to_string()).size(15).font(BOLD),
            ui::muted(b["bank_account_number"].as_str().unwrap_or("").to_string()),
            row![column![ui::muted("Balance in LedgerCraft"), text(ui::money(i(&b["ledger_balance_fx"]))).size(20).font(BOLD)].spacing(2).width(Fill),
                column![ui::muted("Statement balance"), text(if b["last_statement_date"].is_null() { "—".into() } else { ui::money(i(&b["statement_balance"])) }).size(20).color(k.muted)].spacing(2).width(Fill)],
            if review > 0 { button(text(format!("Reconcile {review} items")).size(14).font(BOLD).width(Fill).align_x(iced::alignment::Horizontal::Center)).style(theme::primary).padding(9).width(Fill).on_press(Message::Nav(Route::Banking(Some(id)))).into() }
            else { ui::btn("Transactions", Some(Message::Nav(Route::Banking(Some(id))))) },
        ].spacing(12)));
    }
    let s = &d["sales"];
    let p = &d["purchases"];
    let owed = |title: &str, s: &Value, route: Route, color: Color| -> Element<'_, Message> {
        ui::card(column![
            row![text(title.to_string()).size(15).font(BOLD), ui::hspace(), ui::link("View", Message::Nav(route))],
            row![
                container(column![ui::muted(format!("{} draft", i(&s["draft_count"]))), text(money(i(&s["draft_total"]))).size(14).font(BOLD)].spacing(2)).padding(8).width(Fill).style(theme::soft),
                container(column![ui::muted(format!("{} awaiting", i(&s["awaiting_count"]))), text(money(i(&s["awaiting_total"]))).size(14).font(BOLD)].spacing(2)).padding(8).width(Fill).style(theme::soft),
                container(column![text(format!("{} overdue", i(&s["overdue_count"]))).size(13).color(k.bad), text(money(i(&s["overdue_total"]))).size(14).font(BOLD)].spacing(2)).padding(8).width(Fill).style(theme::bad_soft),
            ].spacing(8),
            bars(vec![("Overdue".into(), vec![(i(&s["overdue_total"]), k.bad)]), ("Awaiting".into(), vec![(i(&s["awaiting_total"]) - i(&s["overdue_total"]), color)]), ("Draft".into(), vec![(i(&s["draft_total"]), k.line2)])], 80.0),
        ].spacing(12))
    };
    cards.push(owed("Invoices owed to you", s, Route::Invoices, k.accent));
    cards.push(owed("Bills you need to pay", p, Route::Bills, k.info));
    cards.push(ui::card(column![
        row![text("Total cash in and out").size(15).font(BOLD), ui::hspace(), ui::muted("Last 6 months")],
        bars(d["cash_flow"].as_array().cloned().unwrap_or_default().iter().map(|m| (m["label"].as_str().unwrap_or("").to_string(), vec![(i(&m["in"]), k.ok), (i(&m["out"]), k.bad)])).collect(), 120.0),
    ].spacing(12)));
    let mut wl = column![text("Account watchlist").size(15).font(BOLD)].spacing(8);
    for w in d["watchlist"].as_array().cloned().unwrap_or_default() {
        let sign = if matches!(w["class"].as_str(), Some("REVENUE" | "LIABILITY" | "EQUITY")) { -1 } else { 1 };
        wl = wl.push(row![text(w["name"].as_str().unwrap_or("").to_string()).size(13).width(Fill), text(money(sign * i(&w["this_month"]))).size(13).width(100).align_x(iced::alignment::Horizontal::Right),
            text(money(sign * i(&w["ytd"]))).size(13).width(110).align_x(iced::alignment::Horizontal::Right)]);
    }
    cards.push(ui::card(wl));
    // Three-column grid.
    let mut grid = column![].spacing(16);
    let mut it = cards.into_iter().peekable();
    while it.peek().is_some() {
        let mut r = row![].spacing(16);
        for _ in 0..3 { if let Some(c) = it.next() { r = r.push(container(c).width(Fill)); } else { r = r.push(Space::new().width(Fill)); } }
        grid = grid.push(r);
    }
    let greeting = format!("Good day{}", first_name(app));
    ui::page(ui::h1(greeting), vec![], column![tasks(app, d), grid].spacing(16).into())
}
