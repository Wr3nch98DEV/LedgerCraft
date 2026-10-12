//! QuickBooks Desktop-style transaction forms (Create Invoices, Create Estimates, Enter Sales
//! Receipts, Create Credit Memos/Refunds, Enter Bills, Create Purchase Orders). Same editor state
//! and messages as the other layouts; laid out the way Desktop users know: ribbon toolbar, form
//! header with CUSTOMER:JOB / BILL TO, line grid, totals block and Save & Close / Save & New.

use crate::app::{App, Message, Opt, Route};
use crate::doc::{Editor, Msg, delayed, ledger_doc, line_cents, parse_e4, sales};
use crate::icons::icon;
use crate::theme::{self, t};
use crate::ui::{self, BOLD};
use iced::widget::{Space, button, column, combo_box, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Element, Fill, Length, Padding};

/// The big form title Desktop prints on the form itself.
fn form_name(e: &Editor) -> &'static str {
    let receipt = e.kind == "SALESRECEIPT" || e.doc.as_ref().map(|d| d["subtype"] == "RECEIPT").unwrap_or(false);
    match e.kind.as_str() {
        _ if receipt => "Sales Receipt",
        "ACCREC" => "Invoice", "QUOTE" => "Estimate", "ACCRECCREDIT" => "Credit Memo", "DELAYEDCHARGE" => "Statement Charge", "DELAYEDCREDIT" => "Statement Credit",
        "ACCPAY" => "Bill", "ACCPAYCREDIT" => "Credit", "PO" => "Purchase Order", _ => "Transaction",
    }
}

fn label<'a>(s: &str) -> Element<'a, Message> { text(s.to_uppercase()).size(11).font(BOLD).color(t().info).into() }

/// A labelled field: editable input, or plain value once the form is saved and locked.
fn field<'a>(editable: bool, lbl: &str, val: &str, width: f32, f: fn(String) -> Msg) -> Element<'a, Message> {
    let input: Element<Message> = if editable {
        text_input("", val).on_input(move |s| Message::Doc(f(s))).padding([5, 7]).size(13).style(theme::input).into()
    } else { container(text(val.to_string()).size(13)).padding([5, 7]).into() };
    column![label(lbl), input].spacing(3).width(Length::Fixed(width)).into()
}

/// Ribbon button: icon over a short label (Desktop's Main tab).
fn rib<'a>(ic: &'static str, lbl: &'static str, msg: Option<Message>) -> Element<'a, Message> {
    let k = t();
    let on = msg.is_some();
    let c = column![icon(ic, 20.0, if on { k.info } else { k.faint }), text(lbl).size(11).color(if on { k.text } else { k.faint })].spacing(2).align_x(Alignment::Center);
    match msg {
        Some(m) => button(c).style(theme::ghost).padding([4, 8]).on_press(m).into(),
        None => container(c).padding([4, 8]).into(),
    }
}

/// "Net 30" style terms from the date and due date.
fn terms(app: &App, e: &Editor) -> String {
    let st = app.date_style();
    match (ui::to_iso(&e.date, st), ui::to_iso(&e.due, st)) {
        (Some(d), Some(due)) => {
            let days = chrono::NaiveDate::parse_from_str(&due, "%Y-%m-%d").ok().zip(chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()).map(|(a, b)| (a - b).num_days()).unwrap_or(0);
            if days <= 0 { "Due on receipt".into() } else { format!("Net {days}") }
        }
        _ => String::new(),
    }
}

pub fn view<'a>(app: &'a App, e: &'a Editor) -> Element<'a, Message> {
    let k = t();
    let editable = e.editable();
    let is_sales = sales(&e.kind);
    let saved = e.doc.is_some();
    let status = e.doc.as_ref().and_then(|d| d["status"].as_str()).unwrap_or("");
    let busy = e.busy;
    let m = |x: Msg| if busy { None } else { Some(Message::Doc(x)) };

    // ---- ribbon ----
    let mut ribbon = row![
        rib("plus", "New", Some(Message::Nav(Route::Doc { kind: e.kind.clone(), id: None }))),
        rib("check", "Save", if editable { m(Msg::Save(true)) } else { None }),
        if saved && matches!(status, "DRAFT" | "SUBMITTED" | "SENT") && !ledger_doc(&e.kind) || saved && status == "DRAFT" { rib("trash", "Delete", m(Msg::Action("delete"))) }
            else if saved && ledger_doc(&e.kind) && matches!(status, "AUTHORISED" | "PAID") { rib("trash", "Void", m(Msg::Action("void"))) } else { rib("trash", "Delete", None) },
        rib("docs", "Create a Copy", if saved { m(Msg::Action("copy")) } else { None }),
        rib("list", "Memorize", None),
        container(Space::new()).width(1).height(36).style(theme::rule),
        rib("file", "Print", if saved { m(Msg::Print) } else { None }),
        rib("docs", "Email", if saved && is_sales && matches!(status, "AUTHORISED" | "SENT" | "PAID") { m(Msg::Send) } else { None }),
        rib("docs", "Attach File", None),
    ].spacing(2).align_y(Alignment::Center);
    if !editable && saved && matches!(status, "AUTHORISED") && !(e.doc.as_ref().map(|d| d["amount_paid"].as_i64().unwrap_or(0) != 0 || d["amount_credited"].as_i64().unwrap_or(0) != 0).unwrap_or(false)) {
        ribbon = ribbon.push(rib("swap", "Edit", m(Msg::Edit)));
    }
    if delayed(&e.kind) && saved && status == "SENT" && !editable { ribbon = ribbon.push(rib("swap", "Edit", m(Msg::Edit))); }
    ribbon = ribbon.push(container(Space::new()).width(1).height(36).style(theme::rule));
    match e.kind.as_str() {
        "ACCREC" => { ribbon = ribbon.push(rib("receive", "Receive Payments", Some(Message::Nav(Route::ReceivePayment(e.contact.as_ref().map(|c| c.id)))))); }
        "QUOTE" => { ribbon = ribbon.push(rib("sales", "Create Invoice", if saved && matches!(status, "SENT" | "ACCEPTED" | "DRAFT") { m(Msg::Action("to_invoice")) } else { None })); }
        "PO" => { ribbon = ribbon.push(rib("expenses", "Enter Bill", if saved && status == "AUTHORISED" { m(Msg::Action("to_bill")) } else { None })); }
        "ACCPAY" => { ribbon = ribbon.push(rib("check-write", "Pay Bills", Some(Message::Nav(Route::PayBills)))); }
        _ => {}
    }
    let ribbon_tabs = row![text("Main").size(12).font(BOLD), text("Formatting").size(12).color(k.muted), text("Send/Ship").size(12).color(k.muted), text("Reports").size(12).color(k.muted)].spacing(18);
    let ribbon = container(column![ribbon_tabs, ribbon].spacing(4)).padding([6, 12]).width(Fill).style(theme::topbar);

    // ---- header ----
    let who = if is_sales { "Customer:Job" } else { "Vendor" };
    let contact: Element<Message> = if editable {
        combo_box(&e.contacts, "", e.contact.as_ref(), |c| Message::Doc(Msg::Contact(c))).on_input(|s| Message::Doc(Msg::ContactText(s))).padding([5, 7]).size(13).input_style(theme::input).into()
    } else {
        container(ui::link(e.contact.as_ref().map(|c| c.label.clone()).unwrap_or_default(), e.contact.as_ref().map(|c| Message::Nav(Route::Contact(c.id))).unwrap_or(Message::Noop))).padding([5, 0]).into()
    };
    let address = e.contact.as_ref().and_then(|c| app.lk.contacts.iter().find(|x| x["id"].as_i64() == Some(c.id)))
        .and_then(|c| c["postal_address"].as_str()).map(String::from)
        .or_else(|| e.doc.as_ref().and_then(|d| d["contact_address"].as_str()).map(String::from)).unwrap_or_default();
    let addr_box = |lbl: &str| -> Element<'_, Message> {
        column![label(lbl), container(text(format!("{}\n{}", e.contact.as_ref().map(|c| c.label.as_str()).unwrap_or(e.contact_text.as_str()), address.replace(", ", "\n"))).size(13))
            .padding([6, 8]).width(260).height(84).style(theme::soft)].spacing(3).into()
    };
    let title = row![text(form_name(e)).size(30).font(BOLD)].spacing(12).align_y(Alignment::Center);
    let status_label = if status.is_empty() { "" } else { crate::vocab::status(app.skin, &e.kind, status, e.doc.as_ref().map(|d| d["overdue"].as_i64() == Some(1)).unwrap_or(false)) };
    let title = if status_label.is_empty() || status_label == "—" { title } else { title.push(ui::chip(&status_label.to_uppercase(), crate::lists::status_color(status_label))) };
    let number_label = match e.kind.as_str() { "ACCREC" => "Invoice #", "QUOTE" => "Estimate No.", "ACCRECCREDIT" => "Credit No.", "SALESRECEIPT" => "Sale No.", "PO" => "P.O. No.", "ACCPAY" | "ACCPAYCREDIT" => "Ref. No.", _ => "No." };
    let header: Element<Message> = if is_sales {
        let mut right = column![row![field(editable, "Date", &e.date, 120.0, Msg::Date), field(editable, number_label, &e.number, 110.0, Msg::Number)].spacing(10)].spacing(8);
        match e.kind.as_str() {
            "ACCREC" => { right = right.push(row![field(editable, "P.O. No.", &e.reference, 110.0, Msg::Reference), column![label("Terms"), container(text(terms(app, e)).size(13)).padding([5, 7])].spacing(3).width(100), field(editable, "Due Date", &e.due, 120.0, Msg::Due)].spacing(10)); }
            "QUOTE" => { right = right.push(row![field(editable, "Expiration Date", &e.due, 130.0, Msg::Due), field(editable, "Reference", &e.reference, 110.0, Msg::Reference)].spacing(10)); }
            "SALESRECEIPT" if editable => {
                right = right.push(column![label("Deposit To"), pick_list(app.lk.bank_opts(), e.pay_account.clone(), |a| Message::Doc(Msg::PayAccount(a))).padding([5, 7]).text_size(13).style(theme::pick).width(240)].spacing(3));
            }
            _ => { right = right.push(field(editable, "P.O. No.", &e.reference, 110.0, Msg::Reference)); }
        }
        column![
            row![column![label(who), contact].spacing(3).width(320), Space::new().width(20),
                column![label("Template"), container(text(format!("LedgerCraft {}", form_name(e))).size(13).color(k.muted)).padding([5, 7]).style(theme::soft)].spacing(3).width(220)].align_y(Alignment::End),
            row![column![title, addr_box(if e.kind == "SALESRECEIPT" { "Sold To" } else { "Bill To" })].spacing(10), ui::hspace(), right].align_y(Alignment::Start),
        ].spacing(10).into()
    } else {
        let amount = if editable { e.totals(app).2 } else { e.doc.as_ref().and_then(|d| d["total"].as_i64()).unwrap_or(0) };
        let mut right = column![row![field(editable, "Date", &e.date, 120.0, Msg::Date), field(editable, number_label, &if e.kind == "PO" { e.number.clone() } else { e.reference.clone() }, 120.0, if e.kind == "PO" { Msg::Number } else { Msg::Reference })].spacing(10),
            column![label("Amount Due"), container(text(ui::money(amount)).size(15).font(BOLD)).padding([4, 7])].spacing(3)].spacing(8);
        if e.kind == "ACCPAY" { right = right.push(row![field(editable, "Bill Due", &e.due, 120.0, Msg::Due), column![label("Terms"), container(text(terms(app, e)).size(13)).padding([5, 7])].spacing(3).width(100)].spacing(10)); }
        column![
            row![title, ui::hspace()],
            row![column![column![label(who), contact].spacing(3).width(320), addr_box("Address")].spacing(10), ui::hspace(), right].align_y(Alignment::Start),
        ].spacing(10).into()
    };

    // ---- line grid ----
    let taxes = app.lk.tax_opts();
    let items = app.lk.item_opts();
    let accounts = app.lk.account_opts(|a| a["type"] != "BANK");
    let bills = !is_sales;
    // Bills/POs: Expenses tab (account lines) or Items tab (item lines), as in Desktop.
    let cols: Vec<(&str, u16, bool)> = if !bills { vec![("Item", 16, false), ("Description", 30, false), ("Quantity", 8, true), ("Rate", 10, true), ("Amount", 11, true), ("Tax", 12, false), ("", 3, false)] }
        else if e.items_tab { vec![("Item", 18, false), ("Description", 32, false), ("Qty", 8, true), ("Cost", 10, true), ("Amount", 11, true), ("", 3, false)] }
        else { vec![("Account", 22, false), ("Amount", 11, true), ("Memo", 32, false), ("Tax", 14, false), ("", 3, false)] };
    let head = container(row(cols.iter().map(|(h, w, right)| {
        let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
        container(if *right { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
    })).spacing(6).padding([6, 8])).style(theme::header_row).width(Fill);
    let mut grid = column![head].width(Fill);
    for (n, l) in e.lines.iter().enumerate() {
        if bills && l.on_items != e.items_tab { continue; }
        let amount = line_cents(parse_e4(&l.qty).unwrap_or(0), parse_e4(&l.price).unwrap_or(0));
        let input = |val: &str, f: fn(usize, String) -> Msg, right: bool| -> Element<'_, Message> {
            if editable {
                let ti = text_input("", val).on_input(move |s| Message::Doc(f(n, s))).padding([4, 5]).size(13).style(theme::cell_input);
                if right { ti.align_x(iced::alignment::Horizontal::Right).into() } else { ti.into() }
            } else if right { ui::num(val.to_string()) } else { ui::cell(val.to_string()) }
        };
        let pick = |opts: &Vec<Opt>, sel: &Option<Opt>, f: fn(usize, Opt) -> Msg| -> Element<'_, Message> {
            if editable { pick_list(opts.clone(), sel.clone(), move |o| Message::Doc(f(n, o))).padding([4, 5]).text_size(13).style(theme::pick).width(Fill).into() }
            else { ui::cell(sel.as_ref().map(|o| o.label.clone()).unwrap_or_default()) }
        };
        let del: Element<Message> = if editable { button(icon("trash", 13.0, k.faint)).style(theme::ghost).padding(3).on_press(Message::Doc(Msg::RemoveLine(n))).into() } else { Space::new().into() };
        let cells: Vec<Element<Message>> = if !bills {
            vec![pick(&items, &l.item, Msg::Item), input(&l.description, Msg::Desc, false), input(&l.qty, Msg::Qty, true), input(&l.price, Msg::Price, true), ui::num(ui::money(amount)), pick(&taxes, &l.tax, Msg::Tax), del]
        } else if e.items_tab {
            vec![pick(&items, &l.item, Msg::Item), input(&l.description, Msg::Desc, false), input(&l.qty, Msg::Qty, true), input(&l.price, Msg::Price, true), ui::num(ui::money(amount)), del]
        } else {
            let amt = if parse_e4(&l.qty).unwrap_or(0) == 10000 { l.price.clone() } else { ui::plain(amount) };
            vec![pick(&accounts, &l.account, Msg::Account), input(&amt, Msg::Amount, true), input(&l.description, Msg::Desc, false), pick(&taxes, &l.tax, Msg::Tax), del]
        };
        let r = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(6).align_y(Alignment::Center).padding([2, 8]);
        grid = grid.push(column![r, ui::hr()]);
    }
    let grid_box = container(scrollable(grid).style(theme::scroll)).height(Length::Fixed(210.0)).style(theme::card).width(Fill).clip(true);
    let grid_area: Element<Message> = if bills {
        let tab_total = |items: bool| -> i64 { e.lines.iter().filter(|l| l.on_items == items).map(|l| line_cents(parse_e4(&l.qty).unwrap_or(0), parse_e4(&l.price).unwrap_or(0))).sum() };
        let tab = |lbl: &'static str, items: bool| -> Element<'_, Message> {
            let on = e.items_tab == items;
            button(text(format!("{lbl}   {}", ui::money(tab_total(items)))).size(13).font(if on { BOLD } else { iced::Font::DEFAULT })).style(theme::tab(on)).padding([5, 10]).on_press(Message::Doc(Msg::BillTab(items))).into()
        };
        column![row![tab("Expenses", false), tab("Items", true)].spacing(4), grid_box].spacing(4).into()
    } else { grid_box.into() };
    let add: Element<Message> = if editable { row![ui::btn("Add lines", Some(Message::Doc(Msg::AddLine)))].into() } else { Space::new().into() };

    // ---- totals + message ----
    let (_, tax, total) = match (&e.doc, editable) { (Some(d), false) => (0, d["total_tax"].as_i64().unwrap_or(0), d["total"].as_i64().unwrap_or(0)), _ => e.totals(app) };
    let paid = e.doc.as_ref().map(|d| d["amount_paid"].as_i64().unwrap_or(0) + d["amount_credited"].as_i64().unwrap_or(0)).unwrap_or(0);
    let line = |l: String, v: i64, big: bool| -> Element<'_, Message> {
        row![text(l).size(if big { 14 } else { 13 }).font(if big { BOLD } else { iced::Font::DEFAULT }).width(Fill), text(ui::money(v)).size(if big { 16 } else { 13 }).font(if big { BOLD } else { iced::Font::DEFAULT })].into()
    };
    let tax_label = app.setting("tax_label");
    let mut tot = column![line(if tax_label.is_empty() { "Tax".into() } else { tax_label }, tax, false), line("Total".into(), total, true)].spacing(6).width(280);
    if ledger_doc(&e.kind) {
        tot = tot.push(line(if is_sales { "Payments Applied".into() } else { "Payments/Credits".into() }, paid, false)).push(ui::hr()).push(line("Balance Due".into(), total - paid, true));
    }
    let msg_label = if is_sales { "Customer Message" } else { "Memo" };
    let memo: Element<Message> = if editable {
        column![label(msg_label), text_input("", &e.terms).on_input(|s| Message::Doc(Msg::Terms(s))).padding([5, 7]).size(13).style(theme::input)].spacing(3).width(420).into()
    } else { column![label(msg_label), text(e.terms.clone()).size(13)].spacing(3).width(420).into() };

    // ---- buttons ----
    let mut buttons = row![ui::hspace()].spacing(8);
    if editable {
        buttons = buttons.push(ui::btn("Save & New", m(Msg::SaveAndNew)));
        buttons = buttons.push(ui::btn_primary("Save & Close", m(Msg::SaveClose)));
        buttons = buttons.push(ui::btn(if saved { "Revert" } else { "Clear" }, Some(Message::Refresh)));
    } else {
        buttons = buttons.push(ui::btn("Close", Some(Message::Back)));
    }
    let error: Element<Message> = match &e.error { Some(err) => container(text(err.clone()).size(13).color(k.bad)).padding([6, 10]).style(theme::bad_soft).width(Fill).into(), None => Space::new().into() };

    let form = column![
        error,
        header,
        doc_extra(app, e),
        grid_area,
        add,
        row![memo, ui::hspace(), tot].align_y(Alignment::Start),
        buttons,
    ].spacing(12).padding(Padding { top: 12.0, left: 16.0, right: 16.0, bottom: 16.0 });
    column![ribbon, row![scrollable(container(form).max_width(1250)).style(theme::scroll).height(Fill).width(Fill), history_panel(app, e)]].into()
}

/// The history panel down the right of a form: summary and recent transactions for the name.
fn history_panel<'a>(app: &'a App, e: &'a Editor) -> Element<'a, Message> {
    let Some(c) = &e.contact else { return Space::new().into() };
    let k = t();
    let info = app.lk.contacts.iter().find(|x| x["id"].as_i64() == Some(c.id));
    let g = |key: &str| info.and_then(|x| x[key].as_str()).unwrap_or("").to_string();
    let n = |key: &str| info.and_then(|x| x[key].as_i64()).unwrap_or(0);
    let is_sales = sales(&e.kind);
    let line = |l: &'static str, v: String| -> Element<'a, Message> { row![text(l).size(12).color(k.muted).width(90), text(if v.is_empty() { "—".into() } else { v }).size(12)].spacing(6).into() };
    let mut recent = column![text("RECENT TRANSACTIONS").size(11).font(BOLD).color(k.info)].spacing(4);
    for r in e.history.iter().filter(|r| r["id"].as_i64() != e.id).take(5) {
        let kind = r["type"].as_str().unwrap_or("");
        recent = recent.push(button(row![
            column![text(format!("{} {}", match kind { "ACCREC" => "Invoice", "ACCRECCREDIT" => "Credit Memo", "ACCPAY" => "Bill", _ => "Transaction" }, r["number"].as_str().unwrap_or(""))).size(12),
                text(app.d(r["date"].as_str().unwrap_or(""))).size(11).color(k.muted)].spacing(0).width(Fill),
            text(ui::money(r["total"].as_i64().unwrap_or(0))).size(12),
        ].align_y(Alignment::Center)).style(theme::ghost).padding([3, 2]).width(Fill).on_press(Message::Nav(Route::Doc { kind: kind.into(), id: r["id"].as_i64() })));
    }
    if e.history.iter().all(|r| r["id"].as_i64() == e.id) { recent = recent.push(text("No other transactions").size(12).color(k.faint)); }
    container(column![
        text(c.label.clone()).size(15).font(BOLD),
        ui::link("Edit / QuickReport", Message::Nav(Route::Contact(c.id))),
        text("SUMMARY").size(11).font(BOLD).color(k.info),
        line("Phone", g("phone")),
        line("Email", g("email")),
        line("Open balance", ui::money(n(if is_sales { "receivable" } else { "payable" }))),
        if is_sales { line("Overdue", ui::money(n("overdue"))) } else { Space::new().into() },
        ui::hr(),
        recent,
    ].spacing(8)).padding(12).width(260).height(Fill).style(theme::soft).into()
}

/// Extras between the header and the grid: open statement charges for a new invoice.
fn doc_extra<'a>(app: &'a App, e: &'a Editor) -> Element<'a, Message> {
    if e.delayed.is_empty() || !e.editable() { return Space::new().into(); }
    let k = t();
    let mut c = column![text("This customer has open statement charges — add them to the invoice:").size(13).font(BOLD)].spacing(6);
    for d in &e.delayed {
        let credit = d["subtype"] == "DELAYEDCREDIT";
        let total = d["total"].as_i64().unwrap_or(0);
        c = c.push(row![
            text(if credit { "Statement Credit" } else { "Statement Charge" }).size(13).width(150),
            text(app.d(d["date"].as_str().unwrap_or(""))).size(13).color(k.muted).width(110),
            text(d["lines"][0]["description"].as_str().unwrap_or("").to_string()).size(13).width(Fill),
            text(ui::money(if credit { -total } else { total })).size(13).width(100),
            ui::btn("Add", Some(Message::Doc(Msg::AddDelayed(d["id"].as_i64().unwrap_or(0))))),
        ].spacing(10).align_y(Alignment::Center));
    }
    container(c).padding([10, 12]).style(theme::accent_soft).width(Fill).into()
}
