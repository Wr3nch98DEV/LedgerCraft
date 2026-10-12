//! Reconcile (QuickBooks-style tick-off): enter the statement's ending date and balance, tick the
//! transactions that cleared, and finish when the difference is zero. Uses the same reconciled
//! flags as statement-line matching, so the Bank transactions screen agrees with it.

use crate::app::{App, Message, Opt};
use crate::backend::Reply;
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD};
use iced::widget::{Space, button, checkbox, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Element, Fill, Length, Task};
use serde_json::{Value, json};

#[derive(Default)]
pub struct State {
    pub account: Option<Opt>,
    pub ending: String,
    pub date: String,
    /// The open reconciliation (tick-off screen) once started.
    pub session: Option<Value>,
    pub filter: Filter,
    pub busy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter { #[default] All, Payments, Deposits }

#[derive(Debug, Clone)]
pub enum Msg {
    Account(Opt),
    Ending(String),
    Date(String),
    Start,
    Session(Reply),
    Toggle(String, i64, bool),
    TickAll(bool),
    Filter(Filter),
    Later,
    Finish,
    Finished(Reply),
    Undo,
    Undone(Reply),
}

fn i(v: &Value) -> i64 { v.as_i64().unwrap_or(0) }

/// Loads the start screen for the chosen account: beginning balance and past reconciliations.
pub fn load(app: &mut App) -> Task<Message> {
    if app.rec.account.is_none() { app.rec.account = crate::payments::bank_only(app).into_iter().next(); }
    if app.rec.date.is_empty() { app.rec.date = app.d(&ui::today_iso()); }
    let Some(acc) = app.rec.account.as_ref().map(|a| a.id) else { app.data = Some(json!({"history": []})); return Task::none() };
    let be = app.be.clone();
    let g = app.page_gen;
    Task::perform(async move {
        tokio::task::spawn_blocking(move || {
            let begin = be.call_now("tickoff.beginning", json!({"account_id": acc}))?;
            let history = be.call_now("tickoff.history", json!({"account_id": acc}))?;
            Ok(json!({"account": acc, "beginning_balance": begin["beginning_balance"], "history": history}))
        }).await.unwrap_or_else(|e| Err(e.to_string()))
    }, move |r| Message::Loaded(g, r))
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    match m {
        Msg::Account(a) => { app.rec.account = Some(a); app.rec.ending.clear(); app.rec.session = None; app.load() }
        Msg::Ending(v) => { app.rec.ending = v; Task::none() }
        Msg::Date(v) => { app.rec.date = v; Task::none() }
        Msg::Start => {
            let Some(acc) = app.rec.account.as_ref().map(|a| a.id) else { return app.toast("Choose an account", true) };
            let Some(date) = ui::to_iso(&app.rec.date, app.date_style()) else { return app.toast("Check the ending date", true) };
            let Some(ending) = ui::parse_money(&app.rec.ending).filter(|_| !app.rec.ending.trim().is_empty()) else { return app.toast("Enter the ending balance from your statement", true) };
            app.rec.busy = true;
            app.be.call("tickoff.start", json!({"account_id": acc, "statement_date": date, "ending_balance": ending}), |r| Message::Rec(Msg::Session(r)))
        }
        Msg::Session(r) => {
            app.rec.busy = false;
            match r { Ok(v) => { app.rec.session = Some(v); Task::none() } Err(e) => app.toast(e, true) }
        }
        Msg::Toggle(kind, id, on) => {
            let Some(s) = app.rec.session.as_mut() else { return Task::none() };
            set_cleared(s, |it| it["kind"] == kind.as_str() && i(&it["id"]) == id, on);
            let rid = i(&s["id"]);
            app.be.call("tickoff.toggle", json!({"id": rid, "kind": kind, "target_id": id, "on": on}), |r| Message::Rec(Msg::Session(r)))
        }
        Msg::TickAll(on) => {
            let filter = app.rec.filter;
            let Some(s) = app.rec.session.as_mut() else { return Task::none() };
            let rid = i(&s["id"]);
            let targets: Vec<(String, i64)> = s["items"].as_array().cloned().unwrap_or_default().iter()
                .filter(|it| shown(filter, it) && (it["cleared"] == true) != on)
                .map(|it| (it["kind"].as_str().unwrap_or("").to_string(), i(&it["id"]))).collect();
            set_cleared(s, |it| shown(filter, it), on);
            let be = app.be.clone();
            Task::perform(async move {
                tokio::task::spawn_blocking(move || {
                    for (k, t) in &targets { be.call_now("tickoff.toggle", json!({"id": rid, "kind": k, "target_id": t, "on": on}))?; }
                    be.call_now("tickoff.session", json!({"id": rid}))
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, |r| Message::Rec(Msg::Session(r)))
        }
        Msg::Filter(f) => { app.rec.filter = f; Task::none() }
        Msg::Later => { app.rec.session = None; let t1 = app.toast("Saved — pick up where you left off any time", false); Task::batch([t1, app.load()]) }
        Msg::Finish => {
            let Some(s) = &app.rec.session else { return Task::none() };
            if i(&s["difference"]) != 0 { return app.toast("The difference must be 0.00 before you can finish", true); }
            app.rec.busy = true;
            app.be.call("tickoff.finish", json!({"id": s["id"]}), |r| Message::Rec(Msg::Finished(r)))
        }
        Msg::Finished(r) => {
            app.rec.busy = false;
            match r {
                Ok(_) => { app.rec.session = None; app.rec.ending.clear(); let t1 = app.toast("Reconciliation complete", false); Task::batch([t1, app.load()]) }
                Err(e) => app.toast(e, true),
            }
        }
        Msg::Undo => {
            let Some(acc) = app.rec.account.as_ref().map(|a| a.id) else { return Task::none() };
            app.be.call("tickoff.undo", json!({"account_id": acc}), |r| Message::Rec(Msg::Undone(r)))
        }
        Msg::Undone(r) => match r {
            Ok(_) => { let t1 = app.toast("Last reconciliation undone", false); Task::batch([t1, app.load()]) }
            Err(e) => app.toast(e, true),
        },
    }
}

fn shown(f: Filter, it: &Value) -> bool {
    match f { Filter::All => true, Filter::Payments => i(&it["amount"]) < 0, Filter::Deposits => i(&it["amount"]) > 0 }
}

/// Tick/untick matching items locally and recompute the totals (the engine's reply follows).
fn set_cleared(s: &mut Value, pick: impl Fn(&Value) -> bool, on: bool) {
    let mut cleared = 0;
    if let Some(items) = s["items"].as_array_mut() {
        for it in items.iter_mut() {
            if pick(it) { it["cleared"] = json!(on); }
            if it["cleared"] == true { cleared += i(&it["amount"]); }
        }
    }
    let begin = i(&s["beginning_balance"]);
    s["cleared_total"] = json!(cleared);
    s["cleared_balance"] = json!(begin + cleared);
    s["difference"] = json!(i(&s["ending_balance"]) - begin - cleared);
}

pub fn view(app: &App) -> Element<'_, Message> {
    match (&app.rec.session, app.desktop()) {
        (Some(s), true) => desk_session(app, s),
        (None, true) => desk_start(app),
        (Some(s), false) => session(app, s),
        (None, false) => start(app),
    }
}

fn start(app: &App) -> Element<'_, Message> {
    let qb = app.skin == Skin::QuickBooks;
    let banks = crate::payments::bank_only(app);
    let d = app.data.clone().unwrap_or(json!({}));
    let history = d["history"].as_array().cloned().unwrap_or_default();
    let open = history.iter().find(|h| h["status"] == "IN_PROGRESS");
    let begin = i(&d["beginning_balance"]);
    let form = ui::card(column![
        ui::h2("Which account do you want to reconcile?"),
        row![
            column![ui::label("Account"), pick_list(banks, app.rec.account.clone(), |a| Message::Rec(Msg::Account(a))).placeholder("Choose an account").padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Length::FillPortion(2)),
            Space::new().width(Length::FillPortion(1)),
        ].spacing(14),
        ui::gap(4.0),
        ui::h2("Add the following information"),
        ui::muted("Enter the ending balance and ending date from your bank or credit card statement."),
        row![
            column![ui::label("Beginning balance"), text(ui::money(begin)).size(16).font(BOLD)].spacing(9).width(Fill),
            ui::field_ph("Ending balance*", "0.00", &app.rec.ending, |v| Message::Rec(Msg::Ending(v))),
            ui::field("Ending date*", &app.rec.date, |v| Message::Rec(Msg::Date(v))),
        ].spacing(14),
        if let Some(o) = open {
            Element::from(container(text(format!("A reconciliation to {} is in progress — starting again resumes it with the figures above.", app.d(o["statement_date"].as_str().unwrap_or("")))).size(13)).padding([8, 12]).style(theme::accent_soft).width(Fill))
        } else { Space::new().into() },
        row![ui::hspace(), ui::btn_primary(if open.is_some() { "Resume reconciling" } else { "Start reconciling" }, if app.rec.busy || app.rec.account.is_none() { None } else { Some(Message::Rec(Msg::Start)) })],
    ].spacing(12));
    let finished: Vec<&Value> = history.iter().filter(|h| h["status"] == "FINISHED").collect();
    let rows: Vec<(Vec<Element<Message>>, Option<Message>)> = finished.iter().map(|h| (vec![
        ui::cell(app.d(h["statement_date"].as_str().unwrap_or(""))),
        ui::cell_muted(app.d(h["finished_at"].as_str().unwrap_or(""))),
        ui::cell_money(i(&h["beginning_balance"])),
        ui::cell_money(i(&h["ending_balance"])),
        ui::cell_money(i(&h["cleared_total"])),
    ], None)).collect();
    let hist = column![
        row![ui::h2("History by account"), ui::hspace(), if finished.is_empty() { Space::new().into() } else { ui::btn("Undo last reconciliation", Some(Message::Rec(Msg::Undo))) }].align_y(Alignment::Center),
        ui::table(&[ui::col("Statement ending date", 18), ui::col("Reconciled on", 16), ui::coln("Beginning balance", 14), ui::coln("Ending balance", 14), ui::coln("Cleared", 14)], rows,
            "No reconciliations yet for this account."),
    ].spacing(10);
    ui::page(ui::h1(if qb { "Reconcile" } else { "Reconcile account" }), vec![], column![form, hist].spacing(18).into())
}

fn session<'a>(app: &'a App, s: &'a Value) -> Element<'a, Message> {
    let k = t();
    let acc = app.rec.account.as_ref().map(|a| a.label.clone()).unwrap_or_default();
    let diff = i(&s["difference"]);
    let items: Vec<&Value> = s["items"].as_array().map(|v| v.iter().filter(|it| shown(app.rec.filter, it)).collect()).unwrap_or_default();
    let (pay_n, dep_n) = s["items"].as_array().map(|v| v.iter().filter(|it| it["cleared"] == true).fold((0, 0), |(p, d), it| if i(&it["amount"]) < 0 { (p + 1, d) } else { (p, d + 1) })).unwrap_or((0, 0));
    let (pay_t, dep_t) = s["items"].as_array().map(|v| v.iter().filter(|it| it["cleared"] == true).fold((0, 0), |(p, d), it| { let a = i(&it["amount"]); if a < 0 { (p - a, d) } else { (p, d + a) } })).unwrap_or((0, 0));
    let figure = |label: String, v: String, big: bool| -> Element<'a, Message> {
        column![text(v).size(if big { 26 } else { 20 }).font(BOLD), text(label).size(12).color(k.muted)].spacing(2).align_x(Alignment::Center).into()
    };
    let op = |sym: &'static str| -> Element<'a, Message> { text(sym).size(20).color(k.muted).into() };
    let summary = ui::card(row![
        figure("STATEMENT ENDING BALANCE".into(), ui::money(i(&s["ending_balance"])), false),
        op("−"),
        figure("CLEARED BALANCE".into(), ui::money(i(&s["cleared_balance"])), false),
        Space::new().width(30),
        column![
            row![ui::muted("Beginning balance"), ui::hspace(), text(ui::money(i(&s["beginning_balance"]))).size(13)].spacing(10),
            row![ui::muted(format!("{pay_n} payments")), ui::hspace(), text(ui::money(-pay_t)).size(13)].spacing(10),
            row![ui::muted(format!("{dep_n} deposits")), ui::hspace(), text(ui::money(dep_t)).size(13)].spacing(10),
        ].spacing(4).width(260),
        ui::hspace(),
        column![
            text(ui::money(diff)).size(30).font(BOLD).color(if diff == 0 { k.ok } else { k.text }),
            row![if diff == 0 { crate::icons::icon("check", 13.0, k.ok) } else { Space::new().into() }, text("DIFFERENCE").size(12).color(k.muted)].spacing(5).align_y(Alignment::Center),
        ].spacing(2).align_x(Alignment::End),
    ].spacing(18).align_y(Alignment::Center));

    let tab = |label: &'static str, f: Filter| -> Element<'a, Message> {
        let active = app.rec.filter == f;
        column![
            button(text(label).size(14).font(if active { BOLD } else { iced::Font::DEFAULT })).style(theme::tab(active)).padding([8, 12]).on_press(Message::Rec(Msg::Filter(f))),
            container(Space::new()).width(Fill).height(2).style(move |_: &iced::Theme| container::Style { background: if active { Some(iced::Background::Color(k.accent)) } else { None }, ..Default::default() }),
        ].width(iced::Shrink).into()
    };
    let all_on = !items.is_empty() && items.iter().all(|it| it["cleared"] == true);
    let cols: Vec<(&str, u16, bool)> = vec![("Date", 12, false), ("Type", 16, false), ("Ref no.", 10, false), ("Payee", 20, false), ("Memo", 20, false), ("Payment", 12, true), ("Deposit", 12, true), ("", 6, false)];
    let head = container(row(cols.iter().enumerate().map(|(n, (h, w, r))| {
        let cell: Element<Message> = if n == cols.len() - 1 { checkbox(all_on).on_toggle(|v| Message::Rec(Msg::TickAll(v))).into() } else {
            let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
            (if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).into()
        };
        container(cell).width(Length::FillPortion(*w)).into()
    })).spacing(10).padding([8, 12]).align_y(Alignment::Center)).style(theme::header_row).width(Fill);
    let mut table = column![head];
    if items.is_empty() { table = table.push(container(ui::muted("No transactions to reconcile up to this date.")).padding(24).center_x(Fill)); }
    for it in items {
        let amount = i(&it["amount"]);
        let on = it["cleared"] == true;
        let kind = it["kind"].as_str().unwrap_or("").to_string();
        let id = i(&it["id"]);
        let msg = Message::Rec(Msg::Toggle(kind.clone(), id, !on));
        let cells: Vec<Element<Message>> = vec![
            ui::cell(app.d(it["date"].as_str().unwrap_or(""))),
            ui::cell_muted(type_label(app.skin, &kind, it["description"].as_str().unwrap_or(""), amount)),
            ui::cell_muted(it["reference"].as_str().unwrap_or("").to_string()),
            ui::cell(it["contact"].as_str().unwrap_or("").to_string()),
            ui::cell_muted(it["description"].as_str().unwrap_or("").to_string()),
            if amount < 0 { ui::cell_money(-amount) } else { Space::new().into() },
            if amount > 0 { ui::cell_money(amount) } else { Space::new().into() },
            checkbox(on).on_toggle(move |v| Message::Rec(Msg::Toggle(kind.clone(), id, v))).into(),
        ];
        let r = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(10).align_y(Alignment::Center).padding([7, 12]);
        table = table.push(column![button(r).style(theme::row_btn).padding(0).width(Fill).on_press(msg), ui::hr()]);
    }
    let footer = container(row![
        ui::btn("Save for later", Some(Message::Rec(Msg::Later))),
        ui::hspace(),
        if diff != 0 { ui::muted(format!("Tick transactions until the difference is 0.00 (now {})", ui::money(diff))) } else { ui::muted("Balanced — you can finish now.") },
        ui::btn_primary("Finish now", if diff == 0 && !app.rec.busy { Some(Message::Rec(Msg::Finish)) } else { None }),
    ].spacing(12).align_y(Alignment::Center)).padding([12, 16]).style(theme::topbar).width(Fill);
    ui::page(
        column![ui::h1(format!("Reconcile {acc}")), ui::muted(format!("Statement ending date {}", app.d(s["statement_date"].as_str().unwrap_or(""))))].spacing(4).into(),
        vec![],
        column![
            summary,
            row![tab("Payments", Filter::Payments), tab("Deposits", Filter::Deposits), tab("All", Filter::All)].spacing(4),
            container(table).style(theme::card).width(Fill).clip(true),
            footer,
        ].spacing(14).into(),
    )
}

fn dlabel<'a>(s: &str) -> Element<'a, Message> { text(s.to_uppercase()).size(11).font(BOLD).color(t().info).into() }

/// QuickBooks Desktop "Begin Reconciliation".
fn desk_start(app: &App) -> Element<'_, Message> {
    let k = t();
    let banks = crate::payments::bank_only(app);
    let d = app.data.clone().unwrap_or(json!({}));
    let history = d["history"].as_array().cloned().unwrap_or_default();
    let last = history.iter().find(|h| h["status"] == "FINISHED").and_then(|h| h["statement_date"].as_str()).map(|x| app.d(x)).unwrap_or_else(|| "—".into());
    let form = container(column![
        text("Begin Reconciliation").size(20).font(BOLD),
        text("Select an account to reconcile, and then enter the ending balance from your account statement.").size(13).color(k.muted),
        row![dlabel("Account"), pick_list(banks, app.rec.account.clone(), |a| Message::Rec(Msg::Account(a))).padding([5, 7]).text_size(13).style(theme::pick).width(300),
            text(format!("last reconciled on {last}")).size(12).color(k.muted)].spacing(12).align_y(Alignment::Center),
        row![container(dlabel("Statement Date")).width(150), text_input("", &app.rec.date).on_input(|v| Message::Rec(Msg::Date(v))).padding([5, 7]).size(13).style(theme::input).width(140)].spacing(12).align_y(Alignment::Center),
        row![container(dlabel("Beginning Balance")).width(150), text(ui::money(i(&d["beginning_balance"]))).size(14)].spacing(12).align_y(Alignment::Center),
        row![container(dlabel("Ending Balance")).width(150), text_input("0.00", &app.rec.ending).on_input(|v| Message::Rec(Msg::Ending(v))).padding([5, 7]).size(13).style(theme::input).width(140)].spacing(12).align_y(Alignment::Center),
        row![
            if history.iter().any(|h| h["status"] == "FINISHED") { ui::btn("Undo Last Reconciliation", Some(Message::Rec(Msg::Undo))) } else { Space::new().into() },
            ui::hspace(),
            ui::btn_primary("Continue", if app.rec.busy || app.rec.account.is_none() { None } else { Some(Message::Rec(Msg::Start)) }),
            ui::btn("Cancel", Some(Message::Back)),
        ].spacing(8),
    ].spacing(14)).padding(20).width(640).style(theme::card);
    container(form).padding(24).into()
}

/// QuickBooks Desktop "Reconcile - Account": payments and deposits side by side.
fn desk_session<'a>(app: &'a App, s: &'a Value) -> Element<'a, Message> {
    let k = t();
    let acc = app.rec.account.as_ref().map(|a| a.label.clone()).unwrap_or_default();
    let diff = i(&s["difference"]);
    let items: Vec<&Value> = s["items"].as_array().map(|v| v.iter().collect()).unwrap_or_default();
    let side = |deposits: bool| -> Element<'a, Message> {
        let mut list = column![row![
            text("✓").size(11).font(BOLD).color(k.muted).width(22), text("DATE").size(11).font(BOLD).color(k.muted).width(Length::FillPortion(10)),
            text(if deposits { "CHK #" } else { "CHK #" }).size(11).font(BOLD).color(k.muted).width(Length::FillPortion(8)),
            text("PAYEE").size(11).font(BOLD).color(k.muted).width(Length::FillPortion(18)),
            text("AMOUNT").size(11).font(BOLD).color(k.muted).align_x(iced::alignment::Horizontal::Right).width(Length::FillPortion(10)),
        ].spacing(6).padding([6, 8])].spacing(0);
        let mut count = 0; let mut sum = 0;
        for it in items.iter().filter(|it| (i(&it["amount"]) > 0) == deposits) {
            let on = it["cleared"] == true;
            if on { count += 1; sum += i(&it["amount"]).abs(); }
            let kind = it["kind"].as_str().unwrap_or("").to_string();
            let id = i(&it["id"]);
            let r = row![
                text(if on { "✓" } else { "" }).size(13).color(k.ok).width(22),
                text(app.d(it["date"].as_str().unwrap_or(""))).size(13).width(Length::FillPortion(10)),
                text(it["reference"].as_str().unwrap_or("").to_string()).size(13).color(k.muted).width(Length::FillPortion(8)),
                text(it["contact"].as_str().unwrap_or("").to_string()).size(13).width(Length::FillPortion(18)),
                text(ui::money(i(&it["amount"]).abs())).size(13).align_x(iced::alignment::Horizontal::Right).width(Length::FillPortion(10)),
            ].spacing(6).padding([3, 8]).align_y(Alignment::Center);
            list = list.push(button(r).padding(0).width(Fill).on_press(Message::Rec(Msg::Toggle(kind, id, !on)))
                .style(move |_, st| button::Style { background: if on { Some(iced::Background::Color(k.accent_soft)) } else if matches!(st, button::Status::Hovered) { Some(iced::Background::Color(k.hover)) } else { None },
                    text_color: k.text, border: iced::Border::default(), shadow: Default::default(), snap: true }));
        }
        column![
            text(if deposits { "Deposits and Other Credits" } else { "Checks and Payments" }).size(14).font(BOLD),
            container(scrollable(list).style(theme::scroll).height(Fill)).style(theme::card).width(Fill).height(Length::Fixed(380.0)).clip(true),
            text(format!("{count} {} marked cleared: {}", if deposits { "deposits and other credits" } else { "checks and payments" }, ui::money(sum))).size(12).color(k.muted),
        ].spacing(6).width(Fill).into()
    };
    let figures = column![
        row![text("Beginning Balance").size(13).width(170), text(ui::money(i(&s["beginning_balance"]))).size(13)],
        row![text("Cleared Balance").size(13).width(170), text(ui::money(i(&s["cleared_balance"]))).size(13).font(BOLD)],
    ].spacing(4);
    let right = column![
        row![text("Ending Balance").size(13).width(140), text(ui::money(i(&s["ending_balance"]))).size(13)],
        row![text("Cleared Balance").size(13).width(140), text(ui::money(i(&s["cleared_balance"]))).size(13)],
        row![text("Difference").size(14).font(BOLD).width(140), text(ui::money(diff)).size(14).font(BOLD).color(if diff == 0 { k.ok } else { k.bad })],
    ].spacing(4);
    column![
        row![text(format!("Reconcile - {acc}")).size(20).font(BOLD), ui::hspace(), text(format!("For period: {}", app.d(s["statement_date"].as_str().unwrap_or("")))).size(13).color(k.muted)].align_y(Alignment::Center),
        row![side(false), side(true)].spacing(14),
        row![ui::btn("Mark All", Some(Message::Rec(Msg::TickAll(true)))), ui::btn("Unmark All", Some(Message::Rec(Msg::TickAll(false)))), ui::hspace(), figures, Space::new().width(40), right].spacing(8).align_y(Alignment::Start),
        row![ui::hspace(), ui::btn("Leave", Some(Message::Rec(Msg::Later))), ui::btn_primary("Reconcile Now", if diff == 0 && !app.rec.busy { Some(Message::Rec(Msg::Finish)) } else { None })].spacing(8),
    ].spacing(12).padding(14).into()
}

/// The transaction type as the layout calls it.
pub fn type_label(skin: Skin, kind: &str, desc: &str, amount: i64) -> String {
    let qb = skin == Skin::QuickBooks;
    match kind {
        "PAYMENT" | "BATCH" => if amount > 0 { "Payment".into() } else if qb { "Bill Payment".into() } else { "Payment".into() },
        "DEPOSIT" => "Deposit".into(),
        "TRANSFER_FROM" | "TRANSFER_TO" => "Transfer".into(),
        _ if desc.starts_with("Check") => "Check".into(),
        _ => if amount > 0 { if qb { "Deposit".into() } else { "Receive money".into() } } else if qb { "Expense".into() } else { "Spend money".into() },
    }
}
