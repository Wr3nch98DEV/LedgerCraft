//! List screens: transactions, customers/vendors, contact page, products, chart of accounts.

use crate::app::{App, Message, Route};
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD, col, coln};
use crate::vocab;
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Color, Element, Fill};
use serde_json::Value;

fn i(v: &Value) -> i64 { v.as_i64().unwrap_or(0) }
fn st<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }

pub fn status_color(label: &str) -> Color {
    let k = t();
    match label { "Overdue" => k.bad, "Paid" | "Closed" | "Accepted" | "Billed" | "Invoiced" => k.ok, "Completed" | "Approved" => k.ok, "Draft" | "On Hold" | "Voided" | "Deleted" => k.muted, "Pending approval" | "Pending Approval" | "Awaiting approval" => k.warn, _ => k.info }
}

fn loading_or_error<'a>(app: &'a App, title: &str) -> Option<Element<'a, Message>> {
    if app.data.is_some() { return None; }
    let body: Element<Message> = match &app.error { Some(e) => ui::card(column![ui::h2("Couldn't load"), ui::muted(e.clone())].spacing(6)), None => ui::muted("Loading…") };
    Some(ui::page(ui::h1(title.to_string()), vec![], body))
}

/// QuickBooks "All sales": invoices and credit memos, plus delayed charges/credits.
pub fn load_all_sales(app: &mut App) -> iced::Task<Message> {
    let be = app.be.clone();
    let g = app.page_gen;
    let qb = app.skin == Skin::QuickBooks;
    let (tab, q) = (app.list_tab, app.list_q.clone());
    iced::Task::perform(async move {
        tokio::task::spawn_blocking(move || -> Result<Value, String> {
            let mut sales = be.call_now("docs.list", serde_json::json!({"type": "SALES", "tab": tab, "q": q, "limit": 300}))?;
            if qb && tab == "all" {
                let delayed = be.call_now("docs.list", serde_json::json!({"type": "DELAYED", "tab": "all", "q": q, "limit": 300}))?;
                let mut rows = sales["rows"].as_array().cloned().unwrap_or_default();
                rows.extend(delayed["rows"].as_array().cloned().unwrap_or_default());
                rows.sort_by(|a, b| b["date"].as_str().cmp(&a["date"].as_str()));
                sales["rows"] = serde_json::json!(rows);
            }
            Ok(sales)
        }).await.unwrap_or_else(|e| Err(e.to_string()))
    }, move |r| Message::Loaded(g, r))
}

/// QuickBooks "Expense transactions": bills plus money already spent from bank accounts.
pub fn load_expenses(app: &mut App) -> iced::Task<Message> {
    let be = app.be.clone();
    let g = app.page_gen;
    let (tab, q) = (app.list_tab, app.list_q.to_lowercase());
    iced::Task::perform(async move {
        tokio::task::spawn_blocking(move || -> Result<Value, String> {
            let bills = be.call_now("docs.list", serde_json::json!({"type": "ACCPAY", "tab": tab, "limit": 300}))?;
            let mut rows: Vec<Value> = bills["rows"].as_array().cloned().unwrap_or_default();
            if tab == "all" || tab == "paid" {
                for b in be.call_now("accounts.bank", serde_json::json!({}))?.as_array().cloned().unwrap_or_default() {
                    for t in be.call_now("bank.transactions", serde_json::json!({"id": b["id"], "limit": 300}))?.as_array().cloned().unwrap_or_default() {
                        let amt = t["amount"].as_i64().unwrap_or(0);
                        if amt >= 0 || !(t["kind"] == "banktx" || (t["kind"] == "payment" && t["description"].as_str().unwrap_or("").starts_with("Payment"))) { continue; }
                        if t["kind"] == "payment" && t["description"].as_str().map(|d| !d.contains('-')).unwrap_or(true) { continue; }
                        let desc = t["description"].as_str().unwrap_or("");
                        let kind = if t["kind"] != "banktx" { "BILLPAYMENT" } else if desc.starts_with("Check ") { "CHECK" } else { "EXPENSE" };
                        let number = if kind == "CHECK" { serde_json::json!(desc.trim_start_matches("Check ")) } else { t["reference"].clone() };
                        rows.push(serde_json::json!({"type": kind, "date": t["date"], "number": number, "contact_name": t["contact"], "total": -amt, "amount_due": 0,
                            "status": "PAID", "document_id": t["document_id"], "account": b["name"], "id": t["id"]}));
                    }
                }
            }
            if !q.is_empty() { rows.retain(|r| r.to_string().to_lowercase().contains(&q)); }
            rows.sort_by(|a, b| b["date"].as_str().cmp(&a["date"].as_str()));
            Ok(serde_json::json!({"rows": rows, "counts": bills["counts"]}))
        }).await.unwrap_or_else(|e| Err(e.to_string()))
    }, move |r| Message::Loaded(g, r))
}

fn type_label(app: &App, kind: &str) -> &'static str {
    match kind {
        "EXPENSE" => if app.skin == Skin::QuickBooks { "Expense" } else { "Spend money" },
        "SALESRECEIPT_ROW" => "Sales receipt",
        "DELAYEDCHARGE" => "Delayed Charge",
        "DELAYEDCREDIT" => "Delayed Credit",
        "CHECK" => "Check",
        "BILLPAYMENT" => "Bill payment", "ACCREC" => app.term("invoice"), "QUOTE" => app.term("quote"), "ACCRECCREDIT" => app.term("credit_note"), "ACCPAY" => app.term("bill"),
        "ACCPAYCREDIT" => app.term("supplier_credit"), "PO" => app.term("po"), _ => "Transaction" }
}

pub(crate) fn search_and_tabs<'a>(app: &'a App, tabs: &[(&'static str, String)]) -> Element<'a, Message> {
    let k = t();
    let mut r = row![].spacing(2).align_y(Alignment::Center);
    for (key, label) in tabs {
        let active = app.list_tab == *key;
        r = r.push(column![
            button(text(label.clone()).size(14).font(if active { BOLD } else { iced::Font::DEFAULT })).style(theme::tab(active)).padding([8, 12]).on_press(Message::ListTab(key)),
            container(Space::new()).width(Fill).height(2).style(move |_: &iced::Theme| container::Style { background: if active { Some(iced::Background::Color(k.accent)) } else { None }, ..Default::default() }),
        ].width(iced::Shrink));
    }
    row![r, ui::hspace(), text_input("Search", &app.list_q).on_input(Message::ListQuery).padding([7, 10]).size(14).width(260).style(theme::input)].align_y(Alignment::Center).into()
}

/// Tabs without the search box (contact pages).
pub(crate) fn tab_row<'a>(app: &'a App, tabs: &[(&'static str, String)]) -> Element<'a, Message> {
    let k = t();
    let mut r = row![].spacing(2).align_y(Alignment::Center);
    for (key, label) in tabs {
        let active = app.list_tab == *key;
        r = r.push(column![
            button(text(label.clone()).size(14).font(if active { BOLD } else { iced::Font::DEFAULT })).style(theme::tab(active)).padding([8, 12]).on_press(Message::ListTab(key)),
            container(Space::new()).width(Fill).height(2).style(move |_: &iced::Theme| container::Style { background: if active { Some(iced::Background::Color(k.accent)) } else { None }, ..Default::default() }),
        ].width(iced::Shrink));
    }
    r.into()
}

/// Money summary bar (QuickBooks' coloured "money bar" above sales/expense lists).
fn money_bar<'a>(parts: Vec<(&'a str, String, i64, Color)>) -> Element<'a, Message> {
    row(parts.into_iter().map(|(label, sub, amount, c)| {
        column![
            text(ui::money(amount)).size(20).font(BOLD),
            text(label).size(13).color(t().muted),
            container(Space::new()).width(Fill).height(6).style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(c)), border: iced::Border { radius: 3.0.into(), ..Default::default() }, ..Default::default() }),
            text(sub).size(12).color(t().faint),
        ].spacing(4).width(Fill).into()
    })).spacing(14).into()
}

/// Xero's summary above invoices and bills: what's draft, waiting for approval, waiting for payment and overdue.
/// Each box opens its tab.
fn xero_summary<'a>(app: &App, counts: &serde_json::Value) -> Element<'a, Message> {
    let k = t();
    let boxes = [("draft", "Draft", "draft", "draft_total", k.muted), ("submitted", "Awaiting approval", "submitted", "submitted_total", k.warn),
        ("awaiting", "Awaiting payment", "awaiting", "awaiting_total", k.info), ("overdue", "Overdue", "overdue", "overdue_total", k.bad)];
    row(boxes.into_iter().map(|(tab, label, n, total, c)| {
        let active = app.list_tab == tab;
        let n = i(&counts[n]);
        button(column![
            text(label).size(13).color(if tab == "overdue" && n > 0 { k.bad } else { k.muted }),
            text(ui::money(i(&counts[total]))).size(20).font(BOLD),
            text(format!("{n} {}", if n == 1 { "item" } else { "items" })).size(12).color(k.faint),
            container(Space::new()).width(Fill).height(3).style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(c)), ..Default::default() }),
        ].spacing(4).width(Fill)).style(theme::tab(active)).padding([10, 14]).width(Fill).on_press(Message::ListTab(tab)).into()
    })).spacing(10).into()
}

pub fn docs(app: &App) -> Element<'_, Message> {
    let (title, new_kind): (&str, &str) = match app.route {
        Route::AllSales => (if app.skin == Skin::QuickBooks { "Sales transactions" } else { "Sales" }, "ACCREC"),
        Route::Invoices => (app.term("invoices"), "ACCREC"),
        Route::Quotes => (app.term("quotes"), "QUOTE"),
        Route::CreditNotes => (app.term("credit_notes"), "ACCRECCREDIT"),
        Route::Bills => (if app.skin == Skin::QuickBooks { "Bills" } else { "Bills to pay" }, "ACCPAY"),
        Route::Expenses => ("Expense transactions", "ACCPAY"),
        Route::PurchaseOrders => (app.term("pos"), "PO"),
        _ => ("Transactions", "ACCREC"),
    };
    if let Some(e) = loading_or_error(app, title) { return e; }
    let d = app.data.as_ref().unwrap();
    let rows_v = d["rows"].as_array().cloned().unwrap_or_default();
    let counts = &d["counts"];
    let purchases = matches!(new_kind, "ACCPAY" | "PO");
    let new_label = format!("New {}", type_label(app, new_kind).to_lowercase());
    let mut actions: Vec<Element<Message>> = vec![];
    if purchases && new_kind == "ACCPAY" && !(app.skin == Skin::Xero && matches!(app.route, Route::Bills)) { actions.push(ui::btn(app.term("pay_bill"), Some(Message::Nav(Route::PayBills)))); }
    let xero_list = app.skin == Skin::Xero && matches!(app.route, Route::Invoices | Route::Bills);
    if xero_list {
        // Xero's invoice and bill lists: Import / Export, statements for customers, then the new buttons.
        actions.push(ui::btn("Import", Some(Message::Nav(Route::ImportExport))));
        actions.push(ui::btn("Export", Some(Message::Nav(Route::ImportExport))));
        if new_kind == "ACCREC" {
            actions.push(ui::btn("Send statements", Some(Message::Nav(Route::Statements))));
            actions.push(ui::btn("New credit note", Some(Message::Nav(Route::Doc { kind: "ACCRECCREDIT".into(), id: None }))));
        } else {
            actions.push(ui::btn("New credit note", Some(Message::Nav(Route::Doc { kind: "ACCPAYCREDIT".into(), id: None }))));
        }
        actions.push(ui::btn(if new_kind == "ACCREC" { "New repeating invoice" } else { "New repeating bill" }, Some(Message::Nav(Route::Memorized))));
    }
    if !purchases && new_kind == "ACCREC" && !xero_list { actions.push(ui::btn(app.term("receive_payment"), Some(Message::Nav(Route::ReceivePayment(None))))); }
    if matches!(app.route, Route::Expenses) { actions.push(ui::btn("New expense", Some(Message::Nav(Route::Spend(None))))); }
    actions.push(ui::btn_primary(&new_label, Some(Message::Nav(Route::Doc { kind: new_kind.into(), id: None }))));

    let k = t();
    let top: Element<Message> = if app.skin == Skin::QuickBooks {
        if purchases {
            money_bar(vec![
                ("Overdue", format!("{} overdue", i(&counts["overdue"])), i(&counts["overdue_total"]), k.bad),
                ("Open bills", format!("{} open", i(&counts["awaiting"])), i(&counts["awaiting_total"]), k.warn),
                (if vocab::acumatica() { "On Hold" } else { "Draft" }, format!("{} draft", i(&counts["draft"])), i(&counts["draft_total"]), k.line2),
            ])
        } else {
            money_bar(vec![
                ("Overdue", format!("{} overdue", i(&counts["overdue"])), i(&counts["overdue_total"]), k.bad),
                ("Open invoices", format!("{} open", i(&counts["awaiting"])), i(&counts["awaiting_total"]), k.info),
                (if vocab::acumatica() { "On Hold" } else { "Draft" }, format!("{} draft", i(&counts["draft"])), i(&counts["draft_total"]), k.line2),
            ])
        }
    } else if xero_list {
        xero_summary(app, counts)
    } else { Space::new().into() };

    let tabs: Vec<(&'static str, String)> = match app.skin {
        Skin::QuickBooks if vocab::acumatica() => [("all", "All"), ("draft", "On Hold"), ("awaiting", "Open"), ("overdue", "Past due"), ("paid", "Closed")].into_iter().map(|(k, l)| (k, l.to_string())).collect(),
        Skin::QuickBooks => [("all", "All"), ("awaiting", "Unpaid"), ("overdue", "Overdue"), ("paid", "Paid"), ("draft", "Draft")].into_iter().map(|(k, l)| (k, l.to_string())).collect(),
        // Xero shows how many are waiting on the tabs that need action, e.g. "Awaiting payment (5)".
        Skin::Xero => [("all", "All"), ("draft", "Draft"), ("submitted", "Awaiting approval"), ("awaiting", "Awaiting payment"), ("paid", "Paid")].into_iter().map(|(k, l)| {
            let n = if matches!(k, "draft" | "submitted" | "awaiting") { i(&counts[k]) } else { 0 };
            (k, if n > 0 { format!("{l} ({n})") } else { l.to_string() })
        }).chain(xero_list.then(|| ("repeating", "Repeating".to_string()))).collect(),
    };
    let cols = match app.skin {
        Skin::QuickBooks => vec![col("Date", 10), col("Type", 10), col("No.", 9), col(if purchases { "Payee" } else { "Customer" }, 20), col("Due date", 10), coln("Balance", 11), coln("Total", 11), col("Status", 10), col("Action", 12)],
        Skin::Xero => vec![col("Number", 10), col("Ref", 10), col("To", 22), col("Date", 11), col("Due date", 11), coln("Paid", 11), coln("Due", 11), col("Status", 12)],
    };
    let rows = rows_v.iter().map(|r| {
        let kind = match st(r, "subtype") { "RECEIPT" => "SALESRECEIPT_ROW", s @ ("DELAYEDCHARGE" | "DELAYEDCREDIT") => s, _ => st(r, "type") };
        let label = vocab::status(app.skin, kind, st(r, "status"), i(&r["overdue"]) == 1);
        let msg = match kind {
            "EXPENSE" | "CHECK" => None,
            "BILLPAYMENT" => r["document_id"].as_i64().map(|id| Message::Nav(Route::Doc { kind: "ACCPAY".into(), id: Some(id) })),
            "SALESRECEIPT_ROW" => Some(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: r["id"].as_i64() })),
            _ => Some(Message::Nav(Route::Doc { kind: kind.into(), id: r["id"].as_i64() })),
        };
        let cells: Vec<Element<Message>> = match app.skin {
            Skin::QuickBooks => vec![
                ui::cell(app.d(st(r, "date"))), ui::cell_muted(type_label(app, kind)), ui::cell(st(r, "number")), ui::cell(st(r, "contact_name")),
                ui::cell_muted(if kind.ends_with("CREDIT") { String::new() } else { app.d(st(r, "due_date")) }), ui::cell_money(i(&r["amount_due"])), ui::cell_money(i(&r["total"])), ui::chip(label, status_color(label)),
                // QuickBooks Online's Action column: the next thing to do with the transaction.
                match (kind, st(r, "status")) {
                    ("ACCREC", "AUTHORISED") => ui::link("Receive payment", Message::Nav(Route::ReceivePayment(r["contact_id"].as_i64()))),
                    ("ACCPAY", "AUTHORISED") => ui::link("Make payment", Message::Nav(Route::PayBills)),
                    (_, "DRAFT") => match &msg { Some(m) => ui::link("Edit", m.clone()), None => ui::cell("") },
                    _ => match &msg { Some(m) => ui::link("View", m.clone()), None => ui::cell("") },
                },
            ],
            Skin::Xero => vec![
                ui::cell(st(r, "number")), ui::cell_muted(st(r, "reference")), ui::cell(st(r, "contact_name")), ui::cell(app.d(st(r, "date"))),
                ui::cell_muted(app.d(st(r, "due_date"))), ui::cell_money(i(&r["amount_paid"])), ui::cell_money(i(&r["amount_due"])), ui::chip(label, status_color(label)),
            ],
        };
        (cells, msg)
    }).collect();
    let body = column![top, search_and_tabs(app, &tabs), ui::table(&cols, rows, "Nothing here yet.")].spacing(16);
    ui::page(ui::h1(title.to_string()), actions, body.into())
}

pub fn contacts(app: &App) -> Element<'_, Message> {
    let (title, customers) = match app.route { Route::Customers => (app.term("customers"), Some(true)), Route::Vendors => (app.term("suppliers"), Some(false)), _ => (app.term("contacts"), None) };
    if let Some(e) = loading_or_error(app, title) { return e; }
    let list = app.data.as_ref().unwrap().as_array().cloned().unwrap_or_default();
    let k = t();
    let (recv, pay): (i64, i64) = list.iter().fold((0, 0), |a, c| (a.0 + i(&c["receivable"]), a.1 + i(&c["payable"])));
    let overdue: i64 = list.iter().map(|c| i(&c["overdue"])).sum();
    let top: Element<Message> = if app.skin == Skin::QuickBooks && customers.is_some() {
        if customers == Some(true) {
            money_bar(vec![("Overdue", "Unpaid, past due".into(), overdue, k.bad), ("Open invoices", "Unpaid".into(), recv, k.info)])
        } else { money_bar(vec![("Open bills", "Unpaid".into(), pay, k.warn)]) }
    } else { Space::new().into() };
    let cols = match (app.skin, customers) {
        (Skin::QuickBooks, Some(true)) => vec![col("Customer / company", 30), col("Phone", 14), col("Email", 24), coln("Open balance", 14), col("Action", 16)],
        (Skin::QuickBooks, _) => vec![col("Vendor / company", 30), col("Phone", 14), col("Email", 24), coln("Open balance", 14), col("Action", 16)],
        _ => vec![col("Name", 32), col("Email", 28), coln("You owe", 15), coln("They owe", 15)],
    };
    let rows = list.iter().map(|c| {
        let id = c["id"].as_i64().unwrap_or(0);
        let open = if customers == Some(false) { i(&c["payable"]) } else { i(&c["receivable"]) };
        let cells: Vec<Element<Message>> = match (app.skin, customers) {
            (Skin::QuickBooks, Some(cust)) => vec![
                ui::cell(st(c, "name")), ui::cell_muted(st(c, "phone")), ui::cell_muted(st(c, "email")), ui::cell_money(open),
                if cust {
                    if open > 0 { ui::link("Receive payment", Message::Nav(Route::ReceivePayment(Some(id)))) } else { ui::link("Create invoice", Message::NewDocFor("ACCREC".into(), id)) }
                } else if open > 0 { ui::link("Make payment", Message::Nav(Route::PayBills)) } else { ui::link("Create bill", Message::NewDocFor("ACCPAY".into(), id)) },
            ],
            _ => vec![ui::cell(st(c, "name")), ui::cell_muted(st(c, "email")), ui::cell_money(i(&c["payable"])), ui::cell_money(i(&c["receivable"]))],
        };
        (cells, Some(Message::Nav(Route::Contact(id))))
    }).collect();
    let tabs: Vec<(&'static str, String)> = if customers.is_none() { vec![("all", "All".into()), ("customers", "Customers".into()), ("suppliers", "Suppliers".into())] } else { vec![] };
    let body = column![top, search_and_tabs(app, &tabs), ui::table(&cols, rows, "No contacts yet.")].spacing(16);
    let new_label = match customers { Some(true) => "New customer", Some(false) => if app.skin == Skin::QuickBooks { "New vendor" } else { "New supplier" }, None => "New contact" };
    let mut actions: Vec<Element<Message>> = vec![];
    if app.skin == Skin::Xero { for b in ["Import", "Export"] { actions.push(ui::btn(b, Some(Message::Nav(Route::ImportExport)))); } }
    actions.push(ui::btn_primary(new_label, Some(Message::Nav(Route::NewContact(customers != Some(false))))));
    ui::page(ui::h1(title.to_string()), actions, body.into())
}

pub fn contact(app: &App) -> Element<'_, Message> {
    if let Some(e) = loading_or_error(app, "Contact") { return e; }
    if app.skin == Skin::Xero { return xero_contact(app); }
    let d = app.data.as_ref().unwrap();
    let c = &d["contact"];
    let id = c["id"].as_i64().unwrap_or(0);
    let k = t();
    let is_supplier = c["is_supplier"] == 1 && c["is_customer"] != 1;
    let mut txs: Vec<Value> = d["sales"].as_array().cloned().unwrap_or_default();
    txs.extend(d["bills"].as_array().cloned().unwrap_or_default());
    txs.sort_by(|a, b| st(b, "date").cmp(st(a, "date")));
    let open: i64 = txs.iter().filter(|r| matches!(st(r, "type"), "ACCREC" | "ACCPAY")).map(|r| i(&r["amount_due"])).sum();
    let overdue: i64 = txs.iter().filter(|r| i(&r["overdue"]) == 1).map(|r| i(&r["amount_due"])).sum();
    let cols = vec![col("Date", 12), col("Type", 12), col("No.", 12), col("Due date", 12), coln("Balance", 14), coln("Total", 14), col("Status", 14)];
    let rows = txs.iter().map(|r| {
        let kind = st(r, "type");
        let label = vocab::status(app.skin, kind, st(r, "status"), i(&r["overdue"]) == 1);
        let due = if kind.ends_with("CREDIT") { String::new() } else { app.d(st(r, "due_date")) };
        (vec![ui::cell(app.d(st(r, "date"))), ui::cell_muted(type_label(app, kind)), ui::cell(st(r, "number")), ui::cell_muted(due),
            ui::cell_money(i(&r["amount_due"])), ui::cell_money(i(&r["total"])), ui::chip(label, status_color(label))],
         Some(Message::Nav(Route::Doc { kind: kind.into(), id: r["id"].as_i64() })))
    }).collect();
    let info = ui::card(row![
        column![ui::h2(st(c, "name").to_string()), ui::muted(st(c, "email").to_string()), ui::muted(st(c, "phone").to_string())].spacing(4).width(Fill),
        column![text(ui::money(open)).size(20).font(BOLD), ui::muted("Open balance")].spacing(2).width(180),
        column![text(ui::money(overdue)).size(20).font(BOLD).color(if overdue > 0 { k.bad } else { k.text }), ui::muted("Overdue")].spacing(2).width(160),
    ].align_y(Alignment::Center));
    let mut actions: Vec<Element<Message>> = vec![ui::btn("Edit", Some(Message::Nav(Route::EditContact(id))))];
    if is_supplier {
        actions.push(ui::btn_primary("New bill", Some(Message::NewDocFor("ACCPAY".into(), id))));
    } else {
        actions.push(ui::btn(app.term("receive_payment"), Some(Message::Nav(Route::ReceivePayment(Some(id))))));
        actions.push(ui::btn_primary("New invoice", Some(Message::NewDocFor("ACCREC".into(), id))));
    }
    // US vendors: "Track payments for 1099" (QuickBooks).
    let info: Element<Message> = if is_supplier && app.country() == "US" {
        let on = i(&c["track_1099"]) == 1;
        column![info, iced::widget::checkbox(on).label("Track payments for 1099").on_toggle(move |v| Message::Set1099(id, v)).text_size(14)].spacing(10).into()
    } else { info };
    let back = if is_supplier { Route::Vendors } else { Route::Customers };
    let title = column![ui::link(if is_supplier { app.term("suppliers") } else { app.term("customers") }, Message::Nav(back)), ui::h1(st(c, "name").to_string())].spacing(4);
    // QuickBooks Online: Transaction list / Customer details / Notes tabs (Statements opens Create Statements).
    let who = if is_supplier { "Vendor details" } else { "Customer details" };
    let mut tabs: Vec<(&'static str, String)> = vec![("all", "Transaction list".into()), ("details", who.into()), ("notes", "Notes".into())];
    if !is_supplier { tabs.insert(1, ("statements", "Statements".into())); }
    let body: Element<Message> = match app.list_tab {
        "details" => ui::card(contact_details(c)).into(),
        "notes" => ui::card(contact_history(app, d, id)).into(),
        _ => ui::table(&cols, rows, "No transactions yet."),
    };
    ui::page(title.into(), actions, column![info, tab_row(app, &tabs), body].spacing(16).into())
}

/// A contact's details as label/value pairs (only the ones filled in).
fn contact_details(c: &Value) -> iced::widget::Column<'static, Message> {
    let mut details = column![ui::h2("Contact details")].spacing(10);
    let person = format!("{} {}", st(c, "first_name"), st(c, "last_name")).trim().to_string();
    for (label, v) in [("Primary person", person), ("Email", st(c, "email").to_string()), ("Phone", st(c, "phone").to_string()), ("Mobile", st(c, "mobile").to_string()),
        ("Website", st(c, "website").to_string()), ("Postal address", st(c, "postal_address").to_string()), ("Street address", st(c, "street_address").to_string()),
        ("Tax number", st(c, "tax_number").to_string()), ("Account number", st(c, "account_number").to_string())] {
        if !v.trim().is_empty() { details = details.push(column![ui::label(label), text(v).size(14)].spacing(2)); }
    }
    let groups: Vec<String> = c["groups"].as_array().cloned().unwrap_or_default().iter().map(|g| st(g, "name").to_string()).collect();
    if !groups.is_empty() { details = details.push(column![ui::label("Groups"), text(groups.join(", ")).size(14)].spacing(2)); }
    details
}

/// History & notes for a contact, with a box to add a note.
fn contact_history<'a>(app: &'a App, d: &'a Value, id: i64) -> iced::widget::Column<'a, Message> {
    let k = t();
    let mut hist = column![
        row![
            text_input("Add a note", &app.extras.contact_note).on_input(|s| Message::Extras(crate::extras::Msg::ContactNote(s)))
                .on_submit(Message::Extras(crate::extras::Msg::ContactNoteAdd(id))).padding([7, 10]).size(14).style(theme::input),
            ui::btn("Add note", Some(Message::Extras(crate::extras::Msg::ContactNoteAdd(id)))),
        ].spacing(8),
    ].spacing(8);
    let entries = d["history"].as_array().cloned().unwrap_or_default();
    if entries.is_empty() { hist = hist.push(ui::muted("No history yet.")); }
    for h in entries.iter().take(20) {
        let when = st(h, "at");
        let day = app.d(when.get(..10).unwrap_or(when));
        hist = hist.push(row![
            text(day).size(13).color(k.muted).width(110),
            text(if st(h, "kind") == "NOTE" { "Note" } else { "History" }).size(13).color(k.muted).width(70),
            text(st(h, "text").to_string()).size(14).width(Fill),
            text(st(h, "user").to_string()).size(13).color(k.muted),
        ].spacing(10));
    }
    hist
}

/// Xero's contact page: details down the left; money in / money out, recent activity and
/// History & notes on the right.
fn xero_contact(app: &App) -> Element<'_, Message> {
    let d = app.data.as_ref().unwrap();
    let c = &d["contact"];
    let id = c["id"].as_i64().unwrap_or(0);
    let k = t();
    let sum = &c["summary"];
    let customer = c["is_customer"] == 1 || c["is_supplier"] != 1;
    let supplier = c["is_supplier"] == 1;
    let details = contact_details(c);
    let left = column![ui::card(details)].width(320);
    // ---- money in / out ----
    let money = |title: &'static str, out: i64, over: i64, year: i64, new: Element<'static, Message>| -> Element<'static, Message> {
        ui::card(column![
            row![ui::h2(title), ui::hspace(), new].align_y(Alignment::Center),
            row![
                column![text(ui::money(out)).size(20).font(BOLD), ui::muted("Outstanding")].spacing(2).width(Fill),
                column![text(ui::money(over)).size(20).font(BOLD).color(if over > 0 { k.bad } else { k.text }), ui::muted("Overdue")].spacing(2).width(Fill),
            ],
            text(format!("{} in the last 12 months", ui::money(year))).size(13).color(k.muted),
        ].spacing(10)).into()
    };
    let mut panels = row![].spacing(14);
    if customer { panels = panels.push(container(money("Money in", i(&sum["receivable"]), i(&sum["receivable_overdue"]), i(&sum["sales_12m"]), ui::link("New invoice", Message::NewDocFor("ACCREC".into(), id)))).width(Fill)); }
    if supplier { panels = panels.push(container(money("Money out", i(&sum["payable"]), i(&sum["payable_overdue"]), i(&sum["purchases_12m"]), ui::link("New bill", Message::NewDocFor("ACCPAY".into(), id)))).width(Fill)); }
    // ---- recent activity ----
    let mut txs: Vec<Value> = d["sales"].as_array().cloned().unwrap_or_default();
    txs.extend(d["bills"].as_array().cloned().unwrap_or_default());
    txs.sort_by(|a, b| st(b, "date").cmp(st(a, "date")));
    let cols = vec![col("Date", 12), col("Type", 12), col("Number", 12), col("Due date", 12), coln("Due", 14), coln("Total", 14), col("Status", 14)];
    let rows = txs.iter().take(10).map(|r| {
        let kind = st(r, "type");
        let label = vocab::status(app.skin, kind, st(r, "status"), i(&r["overdue"]) == 1);
        let due = if kind.ends_with("CREDIT") { String::new() } else { app.d(st(r, "due_date")) };
        (vec![ui::cell(app.d(st(r, "date"))), ui::cell_muted(type_label(app, kind)), ui::cell(st(r, "number")), ui::cell_muted(due),
            ui::cell_money(i(&r["amount_due"])), ui::cell_money(i(&r["total"])), ui::chip(label, status_color(label))],
         Some(Message::Nav(Route::Doc { kind: kind.into(), id: r["id"].as_i64() })))
    }).collect();
    let hist = contact_history(app, d, id);
    let right = column![
        panels,
        ui::h2("Recent activity"), ui::table(&cols, rows, "No transactions yet."),
        ui::h2("History & notes"), ui::card(hist),
    ].spacing(14).width(Fill);
    let mut actions: Vec<Element<Message>> = vec![ui::btn("Edit", Some(Message::Nav(Route::EditContact(id))))];
    if customer { actions.push(ui::btn("Record payment", Some(Message::Nav(Route::ReceivePayment(Some(id)))))); }
    actions.push(ui::btn_primary(if customer { "New invoice" } else { "New bill" }, Some(Message::NewDocFor((if customer { "ACCREC" } else { "ACCPAY" }).into(), id))));
    let title = column![ui::link("Contacts", Message::Nav(Route::Contacts)), ui::h1(st(c, "name").to_string())].spacing(4);
    ui::page(title.into(), actions, row![left, right].spacing(18).into())
}

pub fn products(app: &App) -> Element<'_, Message> {
    let title = app.term("items");
    if let Some(e) = loading_or_error(app, title) { return e; }
    let list = app.data.as_ref().unwrap().as_array().cloned().unwrap_or_default();
    let k = t();
    let low = list.iter().filter(|x| i(&x["low_stock"]) == 1).count();
    let cols = vec![col("Name", 30), col("SKU", 12), col("Type", 14), coln("Sales price", 13), coln("Cost", 13), coln("Qty on hand", 12)];
    let rows = list.iter().map(|x| {
        let tracked = i(&x["is_tracked"]) == 1;
        let kind = if tracked { "Inventory" } else if i(&x["is_sold"]) == 1 && i(&x["is_purchased"]) == 0 { "Service" } else { "Non-inventory" };
        (vec![ui::cell(st(x, "name")), ui::cell_muted(st(x, "code")), ui::cell_muted(kind), ui::num(ui::money(i(&x["sale_unit_e2"]) / 100)), ui::num(ui::money(i(&x["purchase_unit_e2"]) / 100)),
            if tracked { ui::num(format!("{}", i(&x["qty_on_hand_e4"]) as f64 / 10000.0)) } else { ui::cell_muted("") }], x["id"].as_i64().map(|id| Message::Nav(Route::EditItem(id))))
    }).collect();
    let note: Element<Message> = if low > 0 { container(text(format!("{low} item(s) low on stock")).size(13).color(k.warn)).padding([6, 10]).style(theme::bad_soft).into() } else { Space::new().into() };
    ui::page(ui::h1(title), vec![ui::btn_primary("New", Some(Message::Nav(Route::NewProduct)))], column![note, search_and_tabs(app, &[]), ui::table(&cols, rows, "No products or services yet.")].spacing(16).into())
}


pub fn chart(app: &App) -> Element<'_, Message> {
    if let Some(e) = loading_or_error(app, "Chart of accounts") { return e; }
    let list = app.data.as_ref().unwrap().as_array().cloned().unwrap_or_default();
    let cols = match app.skin {
        Skin::QuickBooks => vec![col("Number", 8), col("Name", 28), col("Type", 18), col("Detail type", 22), coln("Balance", 14)],
        Skin::Xero => vec![col("Code", 8), col("Name", 30), col("Type", 18), col("Tax rate", 20), coln("YTD", 14)],
    };
    let rows = list.iter().filter(|a| a["status"] == "ACTIVE").map(|a| {
        let kind = st(a, "type");
        let sign = if matches!(st(a, "class"), "REVENUE" | "LIABILITY" | "EQUITY") { -1 } else { 1 };
        let bal = sign * i(&a["ytd_balance"]);
        let bank_id = if kind == "BANK" { a["id"].as_i64() } else { None };
        let cells: Vec<Element<Message>> = match app.skin {
            Skin::QuickBooks => vec![ui::cell_muted(st(a, "code")), ui::cell(st(a, "name")), ui::cell_muted(crate::coa::qb_type(kind, st(a, "bank_type"))),
                ui::cell_muted(crate::coa::detail_type(kind, st(a, "bank_type"), st(a, "detail_type")).to_string()), ui::cell_money(bal)],
            Skin::Xero => vec![ui::cell_muted(st(a, "code")), ui::cell(st(a, "name")), ui::cell_muted(crate::coa::xero_type(kind)), ui::cell_muted(st(a, "tax_rate_name")), ui::cell_money(bal)],
        };
        // Bank accounts open their transactions; other accounts open Edit account.
        (cells, Some(match bank_id { Some(id) => Message::Nav(Route::Banking(Some(id))), None => Message::Nav(Route::EditAccount(a["id"].as_i64().unwrap_or(0))) }))
    }).collect();
    let new = if app.skin == Skin::QuickBooks { "New" } else { "Add account" };
    ui::page(ui::h1("Chart of accounts"), vec![ui::btn_primary(new, Some(Message::Nav(Route::NewAccount)))], ui::table(&cols, rows, "No accounts."))
}
