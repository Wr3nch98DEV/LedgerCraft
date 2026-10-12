//! QuickBooks Desktop-style versions of the smaller forms: New Customer / New Vendor, New Item,
//! Transfer Funds Between Accounts and Make General Journal Entries. Same state and messages as
//! `forms.rs`.

use crate::app::{App, Message, Opt, Route};
use crate::forms::{Msg, State};
use crate::icons::icon;
use crate::theme::{self, t};
use crate::ui::{self, BOLD};
use iced::widget::{Space, button, checkbox, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Element, Fill, Length, Padding};

fn label<'a>(s: &str) -> Element<'a, Message> { text(s.to_uppercase()).size(11).font(BOLD).color(t().info).into() }

fn input<'a>(lbl: &str, val: &str, w: f32, f: fn(String) -> Msg) -> Element<'a, Message> {
    column![label(lbl), text_input("", val).on_input(move |s| Message::Form(f(s))).padding([5, 7]).size(13).style(theme::input)].spacing(3).width(Length::Fixed(w)).into()
}

fn buttons<'a>(s: &State, new: bool) -> Element<'a, Message> {
    let busy = s.busy;
    let mut r = row![ui::hspace()].spacing(8);
    if new { r = r.push(ui::btn("Save & New", if busy { None } else { Some(Message::Form(Msg::SaveNew)) })); }
    r.push(ui::btn_primary(if new { "Save & Close" } else { "OK" }, if busy { None } else { Some(Message::Form(Msg::Save)) }))
        .push(ui::btn(if new { "Clear" } else { "Cancel" }, Some(if new { Message::Refresh } else { Message::Back }))).into()
}

fn balance(app: &App, acc: &Option<Opt>) -> i64 {
    acc.as_ref().and_then(|a| app.lk.accounts.iter().find(|x| x["id"].as_i64() == Some(a.id))).and_then(|a| a["ytd_balance"].as_i64()).unwrap_or(0)
}

pub fn view<'a>(app: &'a App, s: &'a State) -> Element<'a, Message> {
    let body: Element<Message> = match &app.route {
        Route::NewContact(customer) => contact(app, s, *customer),
        Route::EditContact(_) => contact(app, s, s.customer),
        Route::NewProduct => item(app, s),
        Route::Transfer => transfer(app, s),
        _ => journal(app, s),
    };
    scrollable(container(body).padding(Padding { top: 14.0, left: 18.0, right: 18.0, bottom: 18.0 }).max_width(1100)).style(theme::scroll).height(Fill).into()
}

/// New Customer / New Vendor.
fn contact<'a>(app: &'a App, s: &'a State, customer: bool) -> Element<'a, Message> {
    let k = t();
    let who = if customer { "Customer Name" } else { "Vendor Name" };
    let tabs = column![
        text("Address Info").size(13).font(BOLD).color(k.text),
        text("Payment Settings").size(13).color(k.faint),
        text(if customer { "Sales Tax Settings" } else { "Tax Settings" }).size(13).color(k.faint),
        text("Additional Info").size(13).color(k.faint),
        text(if customer { "Job Info" } else { "Account Settings" }).size(13).color(k.faint),
    ].spacing(10).width(150);
    let mut fields = column![
        column![label("Full Name"), row![input("First", &s.first, 170.0, Msg::First), input("Last", &s.last, 200.0, Msg::Last)].spacing(12)].spacing(6),
        row![input("Main Phone", &s.phone, 200.0, Msg::Phone), input("Main Email", &s.email, 300.0, Msg::Email)].spacing(12),
        column![label(if customer { "Invoice/Bill To" } else { "Billed From" }),
            text_input("Street, city, state, ZIP", &s.address).on_input(|v| Message::Form(Msg::Address(v))).padding([5, 7]).size(13).style(theme::input).width(Length::Fixed(520.0))].spacing(3),
    ].spacing(12);
    if customer && !app.lk.price_levels.is_empty() {
        let mut opts = vec![Opt { id: 0, label: "(standard prices)".into() }];
        opts.extend(app.lk.price_levels.iter().map(|p| Opt { id: p["id"].as_i64().unwrap_or(0), label: p["name"].as_str().unwrap_or("").to_string() }));
        fields = fields.push(column![label("Price Level"), pick_list(opts, s.price_level.clone(), |o| Message::Form(Msg::PriceLevel(o))).placeholder("(standard prices)").padding([5, 7]).text_size(13).style(theme::pick).width(240)].spacing(3));
    }
    if !customer && app.country() == "US" {
        fields = fields.push(checkbox(s.track_1099).label("Vendor eligible for 1099").on_toggle(|v| Message::Form(Msg::Track1099(v))).text_size(13));
    }
    column![
        row![column![label(who), text_input("", &s.name).on_input(|v| Message::Form(Msg::Name(v))).padding([5, 7]).size(14).style(theme::input).width(Length::Fixed(360.0))].spacing(3),
            Space::new().width(30), column![label("Opening Balance"), container(text("Enter on the Opening Balance screen").size(12).color(k.faint)).padding([6, 0])].spacing(3)].align_y(Alignment::End),
        container(row![tabs, container(fields).padding(Padding { left: 18.0, ..Padding::ZERO })]).padding(16).width(Fill).style(theme::card),
        buttons(s, s.orig.is_none()),
    ].spacing(14).into()
}

/// New Item.
fn item<'a>(app: &'a App, s: &'a State) -> Element<'a, Message> {
    let k = t();
    let types = vec!["Service".to_string(), "Non-inventory Part".to_string()];
    let ty = if s.service { "Service" } else { "Non-inventory Part" };
    let income = app.lk.account_opts(|a| a["class"] == "REVENUE");
    let expense = app.lk.account_opts(|a| a["class"] == "EXPENSE" || a["type"] == "DIRECTCOSTS");
    let pick = |lbl: &str, opts: Vec<Opt>, sel: &Option<Opt>, f: fn(Opt) -> Msg, w: f32| -> Element<'a, Message> {
        column![label(lbl), pick_list(opts, sel.clone(), move |o| Message::Form(f(o))).padding([5, 7]).text_size(13).style(theme::pick).width(Length::Fixed(w))].spacing(3).into()
    };
    column![
        row![column![label("Type"), pick_list(types, Some(ty.to_string()), |v: String| Message::Form(Msg::Service(v == "Service"))).padding([5, 7]).text_size(13).style(theme::pick).width(220)].spacing(3),
            text(if s.service { "Use for services you charge for or purchase, like specialized labor, consulting hours, or professional fees." } else { "Use for goods you buy but don't track, like office supplies, or materials for a specific job." }).size(12).color(k.muted).width(Fill)].spacing(16).align_y(Alignment::End),
        container(column![
            row![input("Item Name/Number", &s.name, 300.0, Msg::Name), input("Sku", &s.code, 160.0, Msg::Code)].spacing(14),
            text("Sales Information").size(13).font(BOLD),
            row![input("Rate", &s.price, 140.0, Msg::Price), pick("Tax Code", app.lk.tax_opts(), &s.tax, Msg::Tax, 220.0), pick("Income Account", income, &s.sale_account, Msg::SaleAccount, 300.0)].spacing(14),
            text("Purchase Information").size(13).font(BOLD),
            row![input("Cost", &s.cost, 140.0, Msg::Cost), pick("Expense Account", expense, &s.purchase_account, Msg::PurchaseAccount, 300.0)].spacing(14),
        ].spacing(12)).padding(16).width(Fill).style(theme::card),
        buttons(s, true),
    ].spacing(14).into()
}

/// Banking ▸ Transfer Funds.
fn transfer<'a>(app: &'a App, s: &'a State) -> Element<'a, Message> {
    let k = t();
    let banks = app.lk.bank_opts();
    let side = |lbl: &str, sel: &Option<Opt>, f: fn(Opt) -> Msg| -> Element<'a, Message> {
        row![
            column![label(lbl), pick_list(banks.clone(), sel.clone(), move |o| Message::Form(f(o))).padding([5, 7]).text_size(13).style(theme::pick).width(320)].spacing(3),
            column![label("Account Balance"), container(text(ui::money(balance(app, sel))).size(13)).padding([5, 0])].spacing(3),
        ].spacing(20).align_y(Alignment::End).into()
    };
    column![
        text("Transfer Funds").size(26).font(BOLD),
        container(column![
            row![input("Date", &s.date, 130.0, Msg::Date), Space::new().width(20), input("Memo", &s.memo, 360.0, Msg::Memo)].align_y(Alignment::End),
            side("Transfer Funds From", &s.from, Msg::From),
            side("Transfer Funds To", &s.to, Msg::To),
            row![ui::hspace(), input("Transfer Amount $", &s.amount, 180.0, Msg::Amount)],
        ].spacing(14)).padding(16).width(Fill).style(theme::card),
        text("The transfer appears in both registers and reconciles on each side.").size(12).color(k.muted),
        buttons(s, true),
    ].spacing(14).into()
}

/// Company ▸ Make General Journal Entries.
fn journal<'a>(app: &'a App, s: &'a State) -> Element<'a, Message> {
    let k = t();
    let accounts = app.lk.account_opts(|_| true);
    let (dr, cr) = crate::forms::totals(s);
    let cols: [(&str, u16, bool); 5] = [("Account", 26, false), ("Debit", 11, true), ("Credit", 11, true), ("Memo", 28, false), ("", 3, false)];
    let head = container(row(cols.iter().map(|(h, w, r)| {
        let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
        container(if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
    })).spacing(6).padding([6, 8])).style(theme::header_row).width(Fill);
    let mut grid = column![head];
    for (n, l) in s.lines.iter().enumerate() {
        let cells: Vec<Element<Message>> = vec![
            pick_list(accounts.clone(), l.account.clone(), move |a| Message::Form(Msg::JAccount(n, a))).padding([4, 5]).text_size(13).style(theme::pick).width(Fill).into(),
            text_input("", &l.debit).on_input(move |v| Message::Form(Msg::JDebit(n, v))).padding([4, 5]).size(13).align_x(iced::alignment::Horizontal::Right).style(theme::cell_input).into(),
            text_input("", &l.credit).on_input(move |v| Message::Form(Msg::JCredit(n, v))).padding([4, 5]).size(13).align_x(iced::alignment::Horizontal::Right).style(theme::cell_input).into(),
            text_input("", &l.description).on_input(move |v| Message::Form(Msg::JDesc(n, v))).padding([4, 5]).size(13).style(theme::cell_input).into(),
            Space::new().into(),
        ];
        let r = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(6).align_y(Alignment::Center).padding([2, 8]);
        grid = grid.push(column![r, ui::hr()]);
    }
    let total = row![
        container(text("Totals").size(13).font(BOLD)).width(Length::FillPortion(26)),
        container(text(ui::money(dr)).size(13).font(BOLD).align_x(iced::alignment::Horizontal::Right).width(Fill)).width(Length::FillPortion(11)),
        container(text(ui::money(cr)).size(13).font(BOLD).align_x(iced::alignment::Horizontal::Right).width(Fill)).width(Length::FillPortion(11)),
        container(if dr == cr && dr != 0 { row![icon("check", 13.0, k.ok), text("In balance").size(12).color(k.ok)].spacing(5).into() } else if dr == 0 && cr == 0 { Element::from(Space::new()) }
            else { Element::from(text(format!("Out of balance by {}", ui::money((dr - cr).abs()))).size(12).color(k.bad)) }).width(Length::FillPortion(31)),
    ].spacing(6).padding([6, 8]);
    let ribbon = row![
        container(column![icon("refresh", 18.0, k.faint), text("Previous").size(11).color(k.faint)].align_x(Alignment::Center)).padding([4, 8]),
        container(column![icon("arrow", 18.0, k.faint), text("Next").size(11).color(k.faint)].align_x(Alignment::Center)).padding([4, 8]),
        button(column![icon("check", 18.0, k.info), text("Save").size(11)].align_x(Alignment::Center)).style(theme::ghost).padding([4, 8]).on_press(Message::Form(Msg::Save)),
        button(column![icon("reports", 18.0, k.info), text("Reports").size(11)].align_x(Alignment::Center)).style(theme::ghost).padding([4, 8]).on_press(Message::Nav(Route::Report("journal-report".into()))),
    ].spacing(4);
    column![
        container(ribbon).padding([4, 6]).width(Fill).style(theme::topbar),
        row![input("Date", &s.date, 130.0, Msg::Date), column![label("Entry No."), container(text("Auto").size(13).color(k.muted)).padding([5, 7])].spacing(3).width(110),
            checkbox(false).label("Adjusting Entry").text_size(13)].spacing(16).align_y(Alignment::End),
        container(column![grid, total]).style(theme::card).width(Fill).clip(true),
        row![ui::btn("Add lines", Some(Message::Form(Msg::AddLine))), Space::new().width(20), input("Memo", &s.memo, 420.0, Msg::Memo)].align_y(Alignment::End),
        buttons(s, true),
    ].spacing(12).into()
}
