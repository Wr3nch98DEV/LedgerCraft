//! Bank feeds: QuickBooks-style "For review / Categorized" and Xero-style "Reconcile" over the
//! same statement lines, suggestions, rules and matching engine.

use crate::app::{App, Message, Opt, Route};
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD};
use iced::widget::{Space, button, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Fill, Length, Task};
use serde_json::{Value, json};

#[derive(Default)]
pub struct State {
    pub account: Option<i64>,
    pub tab: Tab,
    pub expanded: Option<i64>,
    pub cat_account: Option<Opt>,
    pub cat_tax: Option<Opt>,
    pub payee: String,
    pub busy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab { #[default] Review, Categorized }

#[derive(Debug, Clone)]
pub enum Msg {
    Account(i64),
    Tab(Tab),
    Expand(i64),
    CatAccount(Opt),
    CatTax(Opt),
    Payee(String),
    /// Match a statement line to an open invoice/bill (records the payment).
    MatchDoc(i64, i64, i64),
    /// Match a statement line to an existing transaction.
    MatchTx(i64, String, i64),
    /// Create a spend/receive transaction from the line with the chosen category.
    Add(i64),
    /// Apply the matching bank rule.
    ApplyRule(i64),
    Undo(i64),
    AcceptAll,
    Done(Result<Value, String>, &'static str),
}

fn i(v: &Value) -> i64 { v.as_i64().unwrap_or(0) }

/// Loads bank accounts, the lines for the chosen account and a suggestion for each line.
pub fn load(app: &mut App, account: Option<i64>) -> Task<Message> {
    let be = app.be.clone();
    let g = app.page_gen;
    let tab = app.bank.tab;
    let acc = account.or(app.bank.account);
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let banks = be.call_now("accounts.bank", json!({}))?;
                let active: Vec<Value> = banks.as_array().cloned().unwrap_or_default().into_iter().filter(|b| b["status"] == "ACTIVE").collect();
                let acc = acc.filter(|a| active.iter().any(|b| b["id"] == *a)).or_else(|| active.first().and_then(|b| b["id"].as_i64()));
                let Some(acc) = acc else { return Ok(json!({"banks": active, "lines": [], "account": null})) };
                let status = if tab == Tab::Review { "UNRECONCILED" } else { "RECONCILED" };
                let mut lines = be.call_now("statements.lines", json!({"account_id": acc, "status": status, "limit": 300}))?;
                if tab == Tab::Review {
                    if let Some(arr) = lines.as_array_mut() {
                        for l in arr.iter_mut().take(80) {
                            if let Ok(s) = be.call_now("rec.suggestions", json!({"id": l["id"]})) { l["suggest"] = s; }
                        }
                    }
                }
                Ok(json!({"banks": active, "lines": lines, "account": acc}))
            }).await.unwrap_or_else(|e| Err(e.to_string()))
        },
        move |r| Message::Loaded(g, r),
    )
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    let acc = app.data.as_ref().and_then(|d| d["account"].as_i64());
    match m {
        Msg::Account(id) => { app.bank.account = Some(id); app.bank.expanded = None; app.route = Route::Banking(Some(id)); app.load() }
        Msg::Tab(tb) => { app.bank.tab = tb; app.bank.expanded = None; app.load() }
        Msg::Expand(id) => {
            if app.bank.expanded == Some(id) { app.bank.expanded = None; return Task::none(); }
            app.bank.expanded = Some(id);
            // Pre-fill the category from the bank rule, if one matches.
            let line = line_by_id(app, id);
            let rule_line = line.as_ref().and_then(|l| l["suggest"]["rule"]["rule"]["percent_lines"].get(0).cloned());
            app.bank.cat_account = rule_line.as_ref().and_then(|r| r["account_id"].as_i64()).and_then(|a| app.lk.account_opts(|_| true).into_iter().find(|o| o.id == a));
            app.bank.cat_tax = rule_line.as_ref().and_then(|r| r["tax_rate_id"].as_i64()).and_then(|tid| app.lk.tax_opts().into_iter().find(|o| o.id == tid));
            app.bank.payee = line.map(|l| title_case(l["payee"].as_str().unwrap_or(""))).unwrap_or_default();
            Task::none()
        }
        Msg::CatAccount(a) => {
            let tid = app.lk.accounts.iter().find(|x| x["id"] == a.id).and_then(|x| x["tax_rate_id"].as_i64());
            if let Some(t) = tid.and_then(|tid| app.lk.tax_opts().into_iter().find(|o| o.id == tid)) { app.bank.cat_tax = Some(t); }
            app.bank.cat_account = Some(a); Task::none()
        }
        Msg::CatTax(tx) => { app.bank.cat_tax = Some(tx); Task::none() }
        Msg::Payee(p) => { app.bank.payee = p; Task::none() }
        m @ (Msg::MatchDoc(..) | Msg::MatchTx(..)) => {
            app.bank.busy = true;
            let (method, params) = match_command(&m).unwrap();
            app.be.call(method, params, |r| Message::Bank(Msg::Done(r, "Matched")))
        }
        Msg::Add(line) => {
            let Some(a) = app.bank.cat_account.clone() else { return app.toast("Choose a category first", true) };
            let l = line_by_id(app, line).unwrap_or(json!({"id": line}));
            app.bank.busy = true;
            let (method, params) = add_command(&l, a.id, app.bank.cat_tax.as_ref().map(|t| t.id), &app.bank.payee);
            app.be.call(method, params, |r| Message::Bank(Msg::Done(r, "Added")))
        }
        Msg::ApplyRule(line) => {
            // Same as Add, with the rule's category (already pre-filled on expand).
            app.bank.expanded = None;
            let _ = update(app, Msg::Expand(line));
            update(app, Msg::Add(line))
        }
        Msg::Undo(line) => app.be.call("rec.unreconcile", json!({"id": line}), |r| Message::Bank(Msg::Done(r, "Moved back to review"))),
        Msg::AcceptAll => {
            let Some(acc) = acc else { return Task::none() };
            app.be.call("rec.auto", json!({"account_id": acc}), |r| Message::Bank(Msg::Done(r, "auto")))
        }
        Msg::Done(r, what) => {
            app.bank.busy = false;
            match r {
                Ok(v) => {
                    app.bank.expanded = None;
                    let msg = if what == "auto" { format!("{} transaction(s) matched automatically", i(&v["reconciled"])) } else { what.to_string() };
                    let t1 = app.toast(msg, false);
                    Task::batch([t1, app.load()])
                }
                Err(e) => app.toast(e, true),
            }
        }
    }
}

/// The one-click action for a line (match an invoice/bill or transaction, or apply a bank rule).
pub fn primary_action(l: &Value) -> Option<Msg> {
    let id = i(&l["id"]);
    let amount = i(&l["amount"]);
    let s = &l["suggest"];
    if let Some(doc) = s["documents"].as_array().and_then(|a| a.first()).filter(|d| i(&d["amount_due"]) == amount.abs() || i(&d["score"]) >= 100) {
        return Some(Msg::MatchDoc(id, i(&doc["id"]), amount.abs().min(i(&doc["amount_due"]))));
    }
    if let Some(m) = s["matches"].as_array().and_then(|a| a.first()).filter(|m| i(&m["amount"]) == amount) {
        return Some(Msg::MatchTx(id, m["kind"].as_str().unwrap_or("").into(), i(&m["id"])));
    }
    if !s["rule"].is_null() { return Some(Msg::ApplyRule(id)); }
    None
}

/// Engine command for a match action.
pub fn match_command(m: &Msg) -> Option<(&'static str, Value)> {
    match m {
        Msg::MatchDoc(line, doc, amount) => Some(("rec.pay", json!({"id": line, "documents": [{"id": doc, "amount": ui::plain(*amount)}], "targets": []}))),
        Msg::MatchTx(line, kind, id) => Some(("rec.match", json!({"id": line, "targets": [{"kind": kind, "id": id}]}))),
        _ => None,
    }
}

/// Engine command to create a transaction from a line with a category.
pub fn add_command(l: &Value, account: i64, tax: Option<i64>, payee: &str) -> (&'static str, Value) {
    let mut tx = json!({"type": "", "line_amount_type": "INCLUSIVE", "reference": l["reference"],
        "lines": [{"description": l["description"], "account_id": account, "tax_rate_id": tax}]});
    if !payee.trim().is_empty() { tx["contact_name"] = json!(payee.trim()); }
    ("rec.create", json!({"id": l["id"], "tx": tx}))
}

fn line_by_id(app: &App, id: i64) -> Option<Value> {
    app.data.as_ref()?["lines"].as_array()?.iter().find(|l| l["id"] == id).cloned()
}

fn title_case(s: &str) -> String {
    s.split_whitespace().map(|w| { let mut c = w.chars(); match c.next() { Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(), None => String::new() } }).collect::<Vec<_>>().join(" ")
}

/// What the line will be matched or categorised to, in words.
fn suggestion_text(app: &App, l: &Value) -> (String, bool) {
    let s = &l["suggest"];
    if let Some(d) = s["documents"].as_array().and_then(|a| a.first()) {
        if i(&d["amount_due"]) == i(&l["amount"]).abs() || i(&d["score"]) >= 100 {
            let kind = if d["type"] == "ACCREC" { "Invoice" } else { "Bill" };
            return (format!("{kind} {} · {}", d["number"].as_str().unwrap_or(""), d["contact"].as_str().unwrap_or("")), true);
        }
    }
    if let Some(m) = s["matches"].as_array().and_then(|a| a.first()) {
        if i(&m["amount"]) == i(&l["amount"]) {
            return (format!("{} · {}", m["description"].as_str().unwrap_or(""), app.d(m["date"].as_str().unwrap_or(""))), true);
        }
    }
    if let Some(a) = s["rule"]["rule"]["percent_lines"].get(0).and_then(|r| r["account_id"].as_i64()) {
        return (format!("{} (rule: {})", app.lk.account_name(Some(a)), s["rule"]["rule"]["name"].as_str().unwrap_or("")), false);
    }
    ("Uncategorized".to_string(), false)
}

pub fn view(app: &App) -> Element<'_, Message> {
    let qb = app.skin == Skin::QuickBooks;
    let title = if qb { "Bank transactions" } else { "Bank accounts" };
    let Some(d) = &app.data else {
        let body: Element<Message> = match &app.error { Some(e) => ui::card(column![ui::h2("Couldn't load"), ui::muted(e.clone())].spacing(6)), None => ui::muted("Loading…") };
        return ui::page(ui::h1(title), vec![], body);
    };
    let k = t();
    let acc = d["account"].as_i64();
    // ---- account tiles ----
    let tiles = row(d["banks"].as_array().cloned().unwrap_or_default().into_iter().map(|b| {
        let id = i(&b["id"]);
        let active = Some(id) == acc;
        let review = i(&b["unreconciled_count"]);
        let content = column![
            text(b["name"].as_str().unwrap_or("").to_string()).size(14).font(BOLD),
            row![ui::muted("Bank balance"), ui::hspace(), text(if b["last_statement_date"].is_null() { "—".into() } else { ui::money(i(&b["statement_balance"])) }).size(13)],
            row![ui::muted("In LedgerCraft"), ui::hspace(), text(ui::money(i(&b["ledger_balance_fx"]))).size(13)],
            if review > 0 { ui::chip(&format!("{review} {}", if qb { "for review" } else { "to reconcile" }), k.accent) } else { ui::chip("Up to date", k.ok) },
        ].spacing(5);
        button(content).padding(14).width(260).on_press(Message::Bank(Msg::Account(id)))
            .style(move |_, s| {
                let hover = matches!(s, button::Status::Hovered);
                button::Style { background: Some(iced::Background::Color(if hover { k.hover } else { k.panel })), text_color: k.text,
                    border: iced::Border { color: if active { k.accent } else { k.line }, width: if active { 2.0 } else { 1.0 }, radius: 8.0.into() }, shadow: Default::default(), snap: true }
            }).into()
    })).spacing(12);

    // ---- tabs ----
    let tab = |label: String, tb: Tab| -> Element<'_, Message> {
        let active = app.bank.tab == tb;
        column![
            button(text(label).size(14).font(if active { BOLD } else { iced::Font::DEFAULT })).style(theme::tab(active)).padding([8, 12]).on_press(Message::Bank(Msg::Tab(tb))),
            container(Space::new()).width(Fill).height(2).style(move |_: &iced::Theme| container::Style { background: if active { Some(iced::Background::Color(k.accent)) } else { None }, ..Default::default() }),
        ].width(iced::Shrink).into()
    };
    let lines: &[Value] = d["lines"].as_array().map(|v| v.as_slice()).unwrap_or(&[]);
    let tabs = row![
        tab(if qb { format!("For review ({})", if app.bank.tab == Tab::Review { lines.len() } else { 0 }) } else { "Reconcile".into() }, Tab::Review),
        tab(if qb { "Categorized".into() } else { "Reconciled".into() }, Tab::Categorized),
        ui::hspace(),
        if app.bank.tab == Tab::Review && !lines.is_empty() { ui::btn(if qb { "Accept all matches" } else { "Auto-reconcile exact matches" }, Some(Message::Bank(Msg::AcceptAll))) } else { Space::new().into() },
    ].align_y(Alignment::Center).spacing(4);

    // ---- table ----
    let review = app.bank.tab == Tab::Review;
    let cols: Vec<(&str, u16, bool)> = if review {
        vec![("Date", 10, false), ("Description", 30, false), ("Spent", 11, true), ("Received", 11, true), (if qb { "Assign to" } else { "Match / create" }, 28, false), ("Action", 10, false)]
    } else {
        vec![("Date", 10, false), ("Description", 40, false), ("Spent", 12, true), ("Received", 12, true), ("Action", 12, false)]
    };
    let head = container(row(cols.iter().map(|(h, w, r)| {
        let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
        container(if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
    })).spacing(10).padding([9, 14])).style(theme::header_row).width(Fill);
    let mut table = column![head];
    if lines.is_empty() {
        table = table.push(container(ui::muted(if review { "Nothing to review — you're all caught up. Import a statement (web version for now) to get new lines." } else { "Nothing categorized yet." })).padding(28).center_x(Fill));
    }
    let busy = app.bank.busy;
    for l in lines {
        let id = i(&l["id"]);
        let amount = i(&l["amount"]);
        let desc = column![text(title_case(l["payee"].as_str().unwrap_or(""))).size(14), ui::muted(l["description"].as_str().unwrap_or("").to_string())].spacing(1);
        let spent = if amount < 0 { ui::cell_money(-amount) } else { ui::cell("") };
        let recv = if amount > 0 { ui::cell_money(amount) } else { ui::cell("") };
        let cells: Vec<Element<Message>> = if review {
            let (sugg, is_match) = suggestion_text(app, l);
            let s = &l["suggest"];
            let _ = (s, is_match);
            let action: Element<Message> = if let Some(a) = primary_action(l) {
                let label = if qb { if matches!(a, Msg::ApplyRule(_)) { "Add" } else { "Match" } } else { "OK" };
                ui::btn_primary(label, if busy { None } else { Some(Message::Bank(a)) })
            } else {
                ui::btn(if app.bank.expanded == Some(id) { "Close" } else { if qb { "Categorize" } else { "Create" } }, Some(Message::Bank(Msg::Expand(id))))
            };
            vec![ui::cell(app.d(l["date"].as_str().unwrap_or(""))), desc.into(), spent, recv,
                 text(sugg).size(13).color(if is_match { k.ok } else { k.muted }).into(), action]
        } else {
            vec![ui::cell(app.d(l["date"].as_str().unwrap_or(""))), desc.into(), spent, recv, ui::btn("Undo", Some(Message::Bank(Msg::Undo(id))))]
        };
        let r = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(10).align_y(Alignment::Center).padding([8, 14]);
        let r = button(r).style(theme::row_btn).padding(0).width(Fill).on_press_maybe(if review { Some(Message::Bank(Msg::Expand(id))) } else { None });
        table = table.push(r);
        if review && app.bank.expanded == Some(id) { table = table.push(detail(app, l)); }
        table = table.push(ui::hr());
    }
    let body = column![tiles, tabs, container(table).style(theme::card).width(Fill).clip(true)].spacing(16);
    let actions: Vec<Element<Message>> = vec![ui::btn(if qb { "Record expense" } else { "Spend money" }, Some(Message::Nav(Route::Spend(acc))))];
    ui::page(ui::h1(title), actions, body.into())
}

/// Expanded row: categorise (create a transaction) or pick a different match.
fn detail<'a>(app: &'a App, l: &'a Value) -> Element<'a, Message> {
    let k = t();
    let id = i(&l["id"]);
    let qb = app.skin == Skin::QuickBooks;
    let amount = i(&l["amount"]);
    let accounts = app.lk.account_opts(|a| a["type"] != "BANK");
    let taxes = app.lk.tax_opts();
    let cat = row![
        column![ui::label(if amount < 0 { "Payee" } else { "Received from" }), text_input("", &app.bank.payee).on_input(|p| Message::Bank(Msg::Payee(p))).padding([7, 10]).size(14).style(theme::input)].spacing(5).width(Fill),
        column![ui::label(if qb { "Category" } else { "Account" }), pick_list(accounts, app.bank.cat_account.clone(), |a| Message::Bank(Msg::CatAccount(a))).placeholder("Choose a category").padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill),
        column![ui::label("Tax"), pick_list(taxes, app.bank.cat_tax.clone(), |t| Message::Bank(Msg::CatTax(t))).placeholder("Tax").padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill),
        column![Space::new().height(18), ui::btn_primary(if qb { "Add" } else { "Create & reconcile" }, if app.bank.busy { None } else { Some(Message::Bank(Msg::Add(id))) })].width(iced::Shrink),
    ].spacing(12).align_y(Alignment::End);
    // Other possible matches.
    let s = &l["suggest"];
    let mut matches = column![].spacing(6);
    for d in s["documents"].as_array().cloned().unwrap_or_default().into_iter().take(4) {
        let amt = amount.abs().min(i(&d["amount_due"]));
        matches = matches.push(row![
            text(format!("{} {} · {} · due {}", if d["type"] == "ACCREC" { "Invoice" } else { "Bill" }, d["number"].as_str().unwrap_or(""), d["contact"].as_str().unwrap_or(""), app.d(d["due_date"].as_str().unwrap_or("")))).size(13).width(Fill),
            text(ui::money(i(&d["amount_due"]))).size(13).width(110),
            ui::btn("Match", Some(Message::Bank(Msg::MatchDoc(id, i(&d["id"]), amt)))),
        ].spacing(10).align_y(Alignment::Center));
    }
    for m in s["matches"].as_array().cloned().unwrap_or_default().into_iter().take(4) {
        matches = matches.push(row![
            text(format!("{} · {} · {}", m["description"].as_str().unwrap_or(""), m["contact"].as_str().unwrap_or(""), app.d(m["date"].as_str().unwrap_or("")))).size(13).width(Fill),
            text(ui::money(i(&m["amount"]))).size(13).width(110),
            ui::btn("Match", Some(Message::Bank(Msg::MatchTx(id, m["kind"].as_str().unwrap_or("").into(), i(&m["id"]))))),
        ].spacing(10).align_y(Alignment::Center));
    }
    container(column![
        text(if qb { "Categorize" } else { "Create a transaction" }).size(13).font(BOLD).color(k.muted), cat,
        text(if qb { "Or find a match" } else { "Or match an existing transaction" }).size(13).font(BOLD).color(k.muted), matches,
    ].spacing(10)).padding([14, 16]).style(theme::soft).width(Fill).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every one-click action the For review screen offers on the demo company is accepted by
    /// the engine, and categorising the rest clears the list.
    #[test]
    fn review_actions_reconcile_demo_bank_feed() {
        let dir = std::env::temp_dir().join(format!("lc-bank-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let be = crate::backend::Backend::new(dir.clone());
        be.call_now("orgs.create", json!({"org_name": "Demo", "country": "AU", "demo": true, "owner_name": "Alex"})).unwrap();
        let bank = be.call_now("accounts.bank", json!({})).unwrap()[0]["id"].as_i64().unwrap();
        let expense = be.call_now("accounts.list", json!({})).unwrap().as_array().unwrap().iter().find(|a| a["code"] == "429").unwrap()["id"].as_i64().unwrap();
        let lines = be.call_now("statements.lines", json!({"account_id": bank})).unwrap();
        assert!(!lines.as_array().unwrap().is_empty());
        let mut one_click = 0;
        for l in lines.as_array().unwrap() {
            let mut l = l.clone();
            l["suggest"] = be.call_now("rec.suggestions", json!({"id": l["id"]})).unwrap();
            match primary_action(&l) {
                Some(m @ (Msg::MatchDoc(..) | Msg::MatchTx(..))) => { let (method, p) = match_command(&m).unwrap(); be.call_now(method, p).unwrap(); one_click += 1; }
                Some(Msg::ApplyRule(_)) => {
                    let r = &l["suggest"]["rule"]["rule"]["percent_lines"][0];
                    let (method, p) = add_command(&l, r["account_id"].as_i64().unwrap(), r["tax_rate_id"].as_i64(), "Rule payee");
                    be.call_now(method, p).unwrap(); one_click += 1;
                }
                _ => { let (method, p) = add_command(&l, expense, None, "Someone"); be.call_now(method, p).unwrap(); }
            }
        }
        assert!(one_click >= 2, "demo feed should offer one-click matches");
        assert_eq!(be.call_now("statements.lines", json!({"account_id": bank})).unwrap().as_array().unwrap().len(), 0);
        let rec = be.call_now("rec.report", json!({"account_id": bank, "date": "2099-12-31"})).unwrap();
        assert_eq!(rec["difference"], 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
