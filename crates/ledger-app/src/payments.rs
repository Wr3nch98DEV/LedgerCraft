//! Money in and out: Receive payment (one payment across several invoices), Pay bills (a batch,
//! which reconciles as a single bank line) and Expense / Spend money.

use crate::app::{App, Message, Opt, Route};
use crate::icons::icon;
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD};
use iced::widget::{Space, button, checkbox, column, combo_box, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Fill, Length, Task};
use serde_json::{Value, json};

/// An open invoice or bill offered for payment.
#[derive(Debug, Clone)]
pub struct OpenDoc { pub id: i64, pub number: String, pub contact: String, pub date: String, pub due_date: String, pub total: i64, pub due: i64, pub pick: bool, pub amount: String }

#[derive(Debug, Clone, Default)]
pub struct SpendLine { pub account: Option<Opt>, pub description: String, pub amount: String, pub tax: Option<Opt> }

#[derive(Clone, Copy, PartialEq)]
pub enum Mode { Receive, PayBills, Spend, Deposit }

pub struct State {
    pub mode: Mode,
    pub contact: Option<Opt>,
    pub contact_text: String,
    pub contacts: combo_box::State<Opt>,
    pub account: Option<Opt>,
    pub date: String,
    pub reference: String,
    pub docs: Vec<OpenDoc>,
    pub lines: Vec<SpendLine>,
    pub lat: String,
    pub busy: bool,
    /// A check (QuickBooks) rather than a plain expense.
    pub check: bool,
    pub check_no: String,
    /// Desktop "Save & New": start a fresh form after saving.
    pub new_after: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Docs(Result<Value, String>),
    Contact(Opt),
    ContactText(String),
    Account(Opt),
    Date(String),
    Reference(String),
    Pick(usize, bool),
    Amount(usize, String),
    LineAccount(usize, Opt),
    LineDesc(usize, String),
    LineAmount(usize, String),
    LineTax(usize, Opt),
    AddLine,
    RemoveLine(usize),
    Save,
    Saved(Result<Value, String>),
    CheckNo(String),
    NextCheck(Result<Value, String>),
    Undeposited(Result<Value, String>),
    /// Desktop: save, then open a fresh form.
    SaveNew,
}

fn i(v: &Value) -> i64 { v.as_i64().unwrap_or(0) }

fn base(app: &App, mode: Mode, customers: bool) -> State {
    State {
        mode, contact: None, contact_text: String::new(), contacts: combo_box::State::new(app.lk.contact_opts(customers)),
        account: app.lk.bank_opts().into_iter().next(), date: ui::date(&ui::today_iso(), app.date_style()), reference: String::new(),
        docs: vec![], lines: vec![], lat: "INCLUSIVE".into(), busy: false, check: false, check_no: String::new(), new_after: false,
    }
}

pub fn open_receive(app: &mut App, contact: Option<i64>) -> Task<Message> {
    let mut s = base(app, Mode::Receive, true);
    if let Some(cid) = contact { s.contact = app.lk.contact_opts(true).into_iter().find(|o| o.id == cid); s.contacts = combo_box::State::with_selection(app.lk.contact_opts(true), s.contact.as_ref()); }
    app.pay = Some(s);
    app.data = Some(json!({}));
    match contact { Some(cid) => load_docs(app, "ACCREC", Some(cid)), None => Task::none() }
}

pub fn open_pay_bills(app: &mut App) -> Task<Message> {
    app.pay = Some(base(app, Mode::PayBills, false));
    app.data = Some(json!({}));
    load_docs(app, "ACCPAY", None)
}

pub fn open_spend(app: &mut App, account: Option<i64>, check: bool) -> Task<Message> {
    let mut s = base(app, Mode::Spend, false);
    if let Some(a) = account { s.account = app.lk.bank_opts().into_iter().find(|o| o.id == a); }
    s.lines = vec![SpendLine::default(), SpendLine::default()];
    s.check = check;
    let acc = s.account.as_ref().map(|a| a.id);
    app.pay = Some(s);
    app.data = Some(json!({}));
    match (check, acc) { (true, Some(a)) => app.be.call("banktx.nextCheck", json!({"account_id": a}), |r| Message::Pay(Msg::NextCheck(r))), _ => Task::none() }
}

/// Bank deposit: pick payments from Undeposited Funds and deposit them as one entry.
pub fn open_deposit(app: &mut App) -> Task<Message> {
    let mut s = base(app, Mode::Deposit, true);
    let banks = bank_only(app);
    s.account = banks.into_iter().next();
    app.pay = Some(s);
    app.data = Some(json!({}));
    app.be.call("deposits.undeposited", json!({}), |r| Message::Pay(Msg::Undeposited(r)))
}

/// Real bank accounts only (not Undeposited Funds or other payment-enabled accounts).
pub fn bank_only(app: &App) -> Vec<Opt> {
    let ids: Vec<i64> = app.lk.accounts.iter().filter(|a| a["type"] == "BANK" && a["status"] == "ACTIVE").filter_map(|a| a["id"].as_i64()).collect();
    app.lk.bank_opts().into_iter().filter(|o| ids.contains(&o.id)).collect()
}

fn load_docs(app: &App, kind: &str, contact: Option<i64>) -> Task<Message> {
    app.be.call("docs.list", json!({"type": kind, "tab": "awaiting", "contact_id": contact, "limit": 500}), |r| Message::Pay(Msg::Docs(r)))
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    let style = app.date_style();
    let Some(mut s) = app.pay.take() else { return Task::none() };
    let m = match m { Msg::SaveNew => { s.new_after = true; Msg::Save } Msg::Save => { s.new_after = false; Msg::Save } m => m };
    let task = match m {
        Msg::SaveNew => Task::none(),
        Msg::Docs(Ok(v)) => {
            s.docs = v["rows"].as_array().cloned().unwrap_or_default().iter().map(|r| OpenDoc {
                id: i(&r["id"]), number: r["number"].as_str().unwrap_or("").into(), contact: r["contact_name"].as_str().unwrap_or("").into(),
                date: r["date"].as_str().unwrap_or("").into(), due_date: r["due_date"].as_str().unwrap_or("").into(), total: i(&r["total"]), due: i(&r["amount_due"]), pick: false, amount: String::new(),
            }).filter(|d| d.due > 0).collect();
            Task::none()
        }
        Msg::Docs(Err(e)) => app.toast(e, true),
        Msg::Undeposited(Ok(v)) => {
            s.docs = v["payments"].as_array().cloned().unwrap_or_default().iter().map(|p| OpenDoc {
                id: i(&p["id"]), number: p["number"].as_str().unwrap_or("").into(), contact: p["contact"].as_str().unwrap_or("").into(), date: p["date"].as_str().unwrap_or("").into(),
                due_date: String::new(), total: i(&p["amount"]), due: i(&p["amount"]), pick: true, amount: ui::plain(i(&p["amount"])),
            }).collect();
            Task::none()
        }
        Msg::Undeposited(Err(e)) => app.toast(e, true),
        Msg::Contact(c) => {
            let id = c.id;
            s.contact_text = c.label.clone();
            s.contact = Some(c);
            if matches!(s.mode, Mode::Receive) { app.pay = Some(s); return load_docs(app, "ACCREC", Some(id)); }
            Task::none()
        }
        Msg::ContactText(t) => { s.contact_text = t; s.contact = None; Task::none() }
        Msg::Account(a) => {
            let id = a.id;
            s.account = Some(a);
            if s.check { app.pay = Some(s); return app.be.call("banktx.nextCheck", json!({"account_id": id}), |r| Message::Pay(Msg::NextCheck(r))); }
            Task::none()
        }
        Msg::CheckNo(n) => { s.check_no = n; Task::none() }
        Msg::NextCheck(r) => { if let Ok(v) = r { s.check_no = v["number"].as_str().unwrap_or("").to_string(); } Task::none() }
        Msg::Date(d) => { s.date = ui::date_keys(&s.date, &d, app.date_style()); Task::none() }
        Msg::Reference(r) => { s.reference = r; Task::none() }
        Msg::Pick(n, on) => { let d = &mut s.docs[n]; d.pick = on; d.amount = if on { ui::plain(d.due) } else { String::new() }; Task::none() }
        Msg::Amount(n, a) => { let d = &mut s.docs[n]; d.pick = ui::parse_money(&a).unwrap_or(0) > 0; d.amount = a; Task::none() }
        Msg::LineAccount(n, a) => {
            let tax = a_tax(app, a.id);
            let l = &mut s.lines[n];
            l.account = Some(a);
            if tax.is_some() { l.tax = tax; }
            if n + 1 == s.lines.len() { s.lines.push(SpendLine::default()); }
            Task::none()
        }
        Msg::LineDesc(n, d) => { s.lines[n].description = d; Task::none() }
        Msg::LineAmount(n, a) => { s.lines[n].amount = a; Task::none() }
        Msg::LineTax(n, t) => { s.lines[n].tax = Some(t); Task::none() }
        Msg::AddLine => { s.lines.push(SpendLine::default()); Task::none() }
        Msg::RemoveLine(n) => { if s.lines.len() > 1 { s.lines.remove(n); } Task::none() }
        Msg::Save => {
            if s.busy { app.pay = Some(s); return Task::none(); }
            let Some(date) = ui::to_iso(&s.date, style) else { app.pay = Some(s); return app.toast("Check the payment date", true) };
            let Some(acc) = s.account.clone() else { app.pay = Some(s); return app.toast("Choose a bank account", true) };
            match s.mode {
                Mode::Deposit => {
                    let ids: Vec<i64> = s.docs.iter().filter(|d| d.pick).map(|d| d.id).collect();
                    if ids.is_empty() { app.pay = Some(s); return app.toast("Tick the payments to deposit", true); }
                    s.busy = true;
                    app.be.call("deposits.create", json!({"account_id": acc.id, "date": date, "reference": s.reference, "payment_ids": ids}), |r| Message::Pay(Msg::Saved(r)))
                }
                Mode::Receive | Mode::PayBills => {
                    let mut items = vec![];
                    for d in s.docs.iter().filter(|d| d.pick) {
                        match ui::parse_money(&d.amount) {
                            Some(a) if a > 0 && a <= d.due => items.push(json!({"document_id": d.id, "amount": ui::plain(a)})),
                            Some(a) if a > d.due => { let n = d.number.clone(); app.pay = Some(s); return app.toast(format!("{n}: payment is more than the open balance"), true); }
                            _ => {}
                        }
                    }
                    if items.is_empty() { app.pay = Some(s); return app.toast("Tick at least one transaction to pay", true); }
                    s.busy = true;
                    // Several payments in one go are a batch, so the bank sees (and reconciles) one amount.
                    if items.len() > 1 {
                        app.be.call("batch.create", json!({"account_id": acc.id, "date": date, "reference": s.reference, "items": items}), |r| Message::Pay(Msg::Saved(r)))
                    } else {
                        let it = &items[0];
                        app.be.call("payments.add", json!({"document_id": it["document_id"], "account_id": acc.id, "date": date, "amount": it["amount"], "reference": s.reference}), |r| Message::Pay(Msg::Saved(r)))
                    }
                }
                Mode::Spend => {
                    let lines: Vec<Value> = s.lines.iter().filter(|l| l.account.is_some() && ui::parse_money(&l.amount).unwrap_or(0) != 0)
                        .map(|l| json!({"account_id": l.account.as_ref().map(|a| a.id), "description": l.description, "amount": l.amount.replace(',', ""), "tax_rate_id": l.tax.as_ref().map(|t| t.id)})).collect();
                    if lines.is_empty() { app.pay = Some(s); return app.toast("Add a category and amount", true); }
                    s.busy = true;
                    let mut tx = json!({"type": "SPEND", "account_id": acc.id, "date": date, "reference": s.reference, "line_amount_type": s.lat, "lines": lines});
                    if s.check { tx["subtype"] = json!("CHECK"); tx["check_number"] = json!(s.check_no.trim()); }
                    match &s.contact { Some(c) => tx["contact_id"] = json!(c.id), None if !s.contact_text.trim().is_empty() => tx["contact_name"] = json!(s.contact_text.trim()), None => {} }
                    app.be.call("banktx.save", json!({"tx": tx}), |r| Message::Pay(Msg::Saved(r)))
                }
            }
        }
        Msg::Saved(r) => {
            s.busy = false;
            match r {
                Ok(_) if app.desktop() => {
                    // Desktop: Save & Close closes the window; Save & New starts a fresh form.
                    let msg = match s.mode { Mode::Receive => "Payment recorded", Mode::PayBills => "Bills paid", Mode::Spend if s.check => "Check saved", Mode::Spend => "Charge saved", Mode::Deposit => "Deposit saved" };
                    let t1 = app.toast(msg, false);
                    let t2 = app.load_lookups();
                    let t3 = if s.new_after { app.load() } else { let i = app.desk.active; app.close_window(i) };
                    return Task::batch([t1, t2, t3]);
                }
                Ok(_) => {
                    let (msg, next) = match s.mode { Mode::Receive => ("Payment recorded", Route::Invoices), Mode::PayBills => ("Bills paid", Route::Bills), Mode::Spend if s.check => ("Check saved", Route::Expenses), Mode::Spend => ("Expense saved", Route::Expenses), Mode::Deposit => ("Deposit saved", Route::Banking(None)) };
                    let t1 = app.toast(msg, false);
                    let t2 = app.go(next);
                    let t3 = app.load_lookups();
                    return Task::batch([t1, t2, t3]);
                }
                Err(e) => app.toast(e, true),
            }
        }
    };
    app.pay = Some(s);
    task
}

fn a_tax(app: &App, account: i64) -> Option<Opt> {
    let tid = app.lk.accounts.iter().find(|a| a["id"] == account).and_then(|a| a["tax_rate_id"].as_i64())?;
    app.lk.tax_opts().into_iter().find(|t| t.id == tid)
}

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(s) = &app.pay else { return ui::muted("…") };
    if app.desktop() { return crate::paydesk::view(app, s); }
    let k = t();
    let date_ph = match app.date_style() { ui::DateStyle::Us => "MM/DD/YYYY", ui::DateStyle::Intl => "DD/MM/YYYY", ui::DateStyle::Long => "12 Oct 2026" };
    let banks = app.lk.bank_opts();
    let account_pick = |label: &str| -> Element<'_, Message> {
        column![ui::label(label.to_string()), pick_list(banks.clone(), s.account.clone(), |a| Message::Pay(Msg::Account(a))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill).into()
    };
    let date = ui::field_ph(match s.mode { Mode::Spend => "Payment date", _ => "Payment date" }, date_ph, &s.date, |d| Message::Pay(Msg::Date(d)));
    let reference = ui::field("Ref no.", &s.reference, |r| Message::Pay(Msg::Reference(r)));
    let save_label = "Save and close";
    let footer = container(row![ui::btn("Cancel", Some(Message::Back)), ui::hspace(), ui::btn_primary(save_label, if s.busy { None } else { Some(Message::Pay(Msg::Save)) })].spacing(8))
        .padding([12, 16]).style(theme::topbar).width(Fill);

    match s.mode {
        Mode::Deposit => {
            let total: i64 = s.docs.iter().filter(|d| d.pick).map(|d| d.due).sum();
            let banks = bank_only(app);
            let head = row![
                column![ui::label("Account"), pick_list(banks, s.account.clone(), |a| Message::Pay(Msg::Account(a))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill),
                date, ui::field("Memo", &s.reference, |r| Message::Pay(Msg::Reference(r))),
            ].spacing(14);
            let amount_box = column![text("TOTAL").size(12).font(BOLD).color(k.muted), text(ui::money(total)).size(30).font(BOLD)].spacing(2).align_x(Alignment::End);
            let cols: Vec<(&str, u16, bool)> = vec![("", 3, false), ("Received from", 26, false), ("Date", 14, false), ("Type", 14, false), ("Ref no.", 14, false), ("Amount", 14, true)];
            let head_row = container(row(cols.iter().map(|(h, w, r)| {
                let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
                container(if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
            })).spacing(10).padding([8, 12])).style(theme::header_row).width(Fill);
            let mut table = column![head_row];
            if s.docs.is_empty() { table = table.push(container(ui::muted("No payments waiting. Receive payments into Undeposited Funds to group them into one deposit.")).padding(24).center_x(Fill)); }
            for (n, d) in s.docs.iter().enumerate() {
                let cells: Vec<Element<Message>> = vec![checkbox(d.pick).on_toggle(move |v| Message::Pay(Msg::Pick(n, v))).into(), ui::cell(d.contact.clone()), ui::cell(app.d(&d.date)),
                    ui::cell_muted("Payment"), ui::cell_muted(d.number.clone()), ui::cell_money(d.due)];
                let r = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(10).align_y(Alignment::Center).padding([6, 12]);
                table = table.push(column![r, ui::hr()]);
            }
            ui::page(ui::h1("Bank deposit"), vec![], column![
                ui::card(row![column![head].width(Fill), Space::new().width(24), amount_box].align_y(Alignment::Start)),
                ui::h2("Select the payments included in this deposit"),
                container(table).style(theme::card).width(Fill).clip(true),
                footer,
            ].spacing(14).into())
        }
        Mode::Receive | Mode::PayBills => {
            let receive = matches!(s.mode, Mode::Receive);
            let total: i64 = s.docs.iter().filter(|d| d.pick).map(|d| ui::parse_money(&d.amount).unwrap_or(0)).sum();
            let head: Element<Message> = if receive {
                row![
                    column![ui::label("Customer"), combo_box(&s.contacts, "Choose a customer", s.contact.as_ref(), |c| Message::Pay(Msg::Contact(c))).padding([7, 10]).size(14).input_style(theme::input)].spacing(5).width(Fill),
                    date, reference, account_pick("Deposit to"),
                ].spacing(14).into()
            } else {
                row![account_pick("Payment account"), date, reference].spacing(14).into()
            };
            let amount_box = column![text(if receive { "AMOUNT RECEIVED" } else { "TOTAL PAYMENT AMOUNT" }).size(12).font(BOLD).color(k.muted), text(ui::money(total)).size(30).font(BOLD)].spacing(2).align_x(Alignment::End);
            let cols: Vec<(&str, u16, bool)> = if receive {
                vec![("", 3, false), ("Description", 26, false), ("Due date", 12, false), ("Original amount", 14, true), ("Open balance", 14, true), ("Payment", 16, true)]
            } else {
                vec![("", 3, false), (if app.skin == Skin::QuickBooks { "Payee" } else { "Supplier" }, 22, false), ("Ref no.", 12, false), ("Due date", 12, false), ("Open balance", 14, true), ("Payment", 16, true)]
            };
            let head_row = container(row(cols.iter().map(|(h, w, r)| {
                let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
                container(if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
            })).spacing(10).padding([8, 12])).style(theme::header_row).width(Fill);
            let mut table = column![head_row];
            if s.docs.is_empty() {
                table = table.push(container(ui::muted(if receive && s.contact.is_none() { "Choose a customer to see their open invoices." } else { "Nothing outstanding." })).padding(24).center_x(Fill));
            }
            for (n, d) in s.docs.iter().enumerate() {
                let cb = checkbox(d.pick).on_toggle(move |v| Message::Pay(Msg::Pick(n, v)));
                let pay = text_input("", &d.amount).on_input(move |a| Message::Pay(Msg::Amount(n, a))).padding([5, 8]).size(14).align_x(iced::alignment::Horizontal::Right).style(theme::input);
                let cells: Vec<Element<Message>> = if receive {
                    vec![cb.into(), ui::cell(format!("Invoice # {} ({})", d.number, app.d(&d.date))), ui::cell_muted(app.d(&d.due_date)), ui::cell_money(d.total), ui::cell_money(d.due), pay.into()]
                } else {
                    vec![cb.into(), ui::cell(d.contact.clone()), ui::cell_muted(d.number.clone()), ui::cell_muted(app.d(&d.due_date)), ui::cell_money(d.due), pay.into()]
                };
                let r = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(10).align_y(Alignment::Center).padding([6, 12]);
                table = table.push(column![r, ui::hr()]);
            }
            let note = if !receive && s.docs.iter().filter(|d| d.pick).count() > 1 { ui::muted("Several bills paid together are saved as one batch payment, so they match a single line on your bank statement.") } else { Space::new().into() };
            let title = if receive { app.term("receive_payment") } else { app.term("pay_bill") };
            ui::page(ui::h1(title), vec![], column![
                ui::card(row![column![head].width(Fill), Space::new().width(24), amount_box].align_y(Alignment::Start)),
                ui::h2(if receive { "Outstanding transactions" } else { "Unpaid bills" }),
                container(table).style(theme::card).width(Fill).clip(true),
                note,
                footer,
            ].spacing(14).into())
        }
        Mode::Spend => {
            let taxes = app.lk.tax_opts();
            let accounts = app.lk.account_opts(|a| a["type"] != "BANK");
            let mut total = 0i64;
            let cols: Vec<(&str, u16, bool)> = vec![("#", 2, false), ("Category", 20, false), ("Description", 26, false), ("Amount", 12, true), ("Tax", 14, false), ("", 3, false)];
            let head_row = container(row(cols.iter().map(|(h, w, r)| {
                let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
                container(if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
            })).spacing(8).padding([8, 10])).style(theme::header_row).width(Fill);
            let mut table = column![head_row];
            for (n, l) in s.lines.iter().enumerate() {
                total += ui::parse_money(&l.amount).unwrap_or(0);
                let cells: Vec<Element<Message>> = vec![
                    ui::cell_muted((n + 1).to_string()),
                    pick_list(accounts.clone(), l.account.clone(), move |a| Message::Pay(Msg::LineAccount(n, a))).placeholder("Choose a category").padding([6, 6]).text_size(13).style(theme::pick).width(Fill).into(),
                    text_input("", &l.description).on_input(move |d| Message::Pay(Msg::LineDesc(n, d))).padding([6, 6]).size(14).style(theme::cell_input).into(),
                    text_input("", &l.amount).on_input(move |a| Message::Pay(Msg::LineAmount(n, a))).padding([6, 6]).size(14).align_x(iced::alignment::Horizontal::Right).style(theme::cell_input).into(),
                    pick_list(taxes.clone(), l.tax.clone(), move |t| Message::Pay(Msg::LineTax(n, t))).placeholder("Tax").padding([6, 6]).text_size(13).style(theme::pick).width(Fill).into(),
                    if s.lines.len() > 1 { button(icon("trash", 14.0, k.muted)).style(theme::ghost).padding(4).on_press(Message::Pay(Msg::RemoveLine(n))).into() } else { Space::new().into() },
                ];
                let r = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(8).align_y(Alignment::Center).padding([4, 10]);
                table = table.push(column![r, ui::hr()]);
            }
            let payee = column![ui::label("Payee"), combo_box(&s.contacts, "Who did you pay?", s.contact.as_ref(), |c| Message::Pay(Msg::Contact(c)))
                .on_input(|t| Message::Pay(Msg::ContactText(t))).padding([7, 10]).size(14).input_style(theme::input)].spacing(5).width(Fill);
            let amount_box = column![text("AMOUNT").size(12).font(BOLD).color(k.muted), text(ui::money(total)).size(30).font(BOLD)].spacing(2).align_x(Alignment::End);
            let second: Element<Message> = if s.check {
                row![date, ui::field("Check no.", &s.check_no, |v| Message::Pay(Msg::CheckNo(v))), reference].spacing(14).into()
            } else { row![date, reference].spacing(14).into() };
            ui::page(ui::h1(if s.check { "Check" } else { app.term("spend") }), vec![], column![
                ui::card(row![column![row![payee, account_pick(if s.check { "Bank account" } else { "Payment account" })].spacing(14), second].spacing(14).width(Fill), Space::new().width(24), amount_box].align_y(Alignment::Start)),
                ui::h2("Category details"),
                container(table).style(theme::card).width(Fill).clip(true),
                row![ui::btn("Add lines", Some(Message::Pay(Msg::AddLine))), ui::hspace(), ui::muted("Amounts include tax")],
                footer,
            ].spacing(14).into())
        }
    }
}
