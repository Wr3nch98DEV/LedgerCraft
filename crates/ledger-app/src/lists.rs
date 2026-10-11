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
    match label { "Overdue" => k.bad, "Paid" | "Closed" | "Accepted" | "Billed" | "Invoiced" => k.ok, "Draft" | "Voided" | "Deleted" => k.muted, "Pending approval" | "Awaiting approval" => k.warn, _ => k.info }
}

fn loading_or_error<'a>(app: &'a App, title: &str) -> Option<Element<'a, Message>> {
    if app.data.is_some() { return None; }
    let body: Element<Message> = match &app.error { Some(e) => ui::card(column![ui::h2("Couldn't load"), ui::muted(e.clone())].spacing(6)), None => ui::muted("Loading…") };
    Some(ui::page(ui::h1(title.to_string()), vec![], body))
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
                        let kind = if t["kind"] == "banktx" { "EXPENSE" } else { "BILLPAYMENT" };
                        rows.push(serde_json::json!({"type": kind, "date": t["date"], "number": t["reference"], "contact_name": t["contact"], "total": -amt, "amount_due": 0,
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
        "BILLPAYMENT" => "Bill payment", "ACCREC" => app.term("invoice"), "QUOTE" => app.term("quote"), "ACCRECCREDIT" => app.term("credit_note"), "ACCPAY" => app.term("bill"),
        "ACCPAYCREDIT" => app.term("supplier_credit"), "PO" => app.term("po"), _ => "Transaction" }
}

fn search_and_tabs<'a>(app: &'a App, tabs: &[(&'static str, &'static str)]) -> Element<'a, Message> {
    let k = t();
    let mut r = row![].spacing(2).align_y(Alignment::Center);
    for (key, label) in tabs {
        let active = app.list_tab == *key;
        r = r.push(column![
            button(text(*label).size(14).font(if active { BOLD } else { iced::Font::DEFAULT })).style(theme::tab(active)).padding([8, 12]).on_press(Message::ListTab(key)),
            container(Space::new()).width(Fill).height(2).style(move |_: &iced::Theme| container::Style { background: if active { Some(iced::Background::Color(k.accent)) } else { None }, ..Default::default() }),
        ].width(iced::Shrink));
    }
    row![r, ui::hspace(), text_input("Search", &app.list_q).on_input(Message::ListQuery).padding([7, 10]).size(14).width(260).style(theme::input)].align_y(Alignment::Center).into()
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
    if purchases && new_kind == "ACCPAY" { actions.push(ui::btn(app.term("pay_bill"), Some(Message::Nav(Route::PayBills)))); }
    if !purchases && new_kind == "ACCREC" { actions.push(ui::btn(app.term("receive_payment"), Some(Message::Nav(Route::ReceivePayment(None))))); }
    if matches!(app.route, Route::Expenses) { actions.push(ui::btn("New expense", Some(Message::Nav(Route::Spend(None))))); }
    actions.push(ui::btn_primary(&new_label, Some(Message::Nav(Route::Doc { kind: new_kind.into(), id: None }))));

    let k = t();
    let top: Element<Message> = if app.skin == Skin::QuickBooks {
        if purchases {
            money_bar(vec![
                ("Overdue", format!("{} overdue", i(&counts["overdue"])), i(&counts["overdue_total"]), k.bad),
                ("Open bills", format!("{} open", i(&counts["awaiting"])), i(&counts["awaiting_total"]), k.warn),
                ("Draft", format!("{} draft", i(&counts["draft"])), i(&counts["draft_total"]), k.line2),
            ])
        } else {
            money_bar(vec![
                ("Overdue", format!("{} overdue", i(&counts["overdue"])), i(&counts["overdue_total"]), k.bad),
                ("Open invoices", format!("{} open", i(&counts["awaiting"])), i(&counts["awaiting_total"]), k.info),
                ("Draft", format!("{} draft", i(&counts["draft"])), i(&counts["draft_total"]), k.line2),
            ])
        }
    } else { Space::new().into() };

    let tabs: Vec<(&str, &str)> = match app.skin {
        Skin::QuickBooks => vec![("all", "All"), ("awaiting", "Unpaid"), ("overdue", "Overdue"), ("paid", "Paid"), ("draft", "Draft")],
        Skin::Xero => vec![("all", "All"), ("draft", "Draft"), ("submitted", "Awaiting approval"), ("awaiting", "Awaiting payment"), ("paid", "Paid")],
    };
    let cols = match app.skin {
        Skin::QuickBooks => vec![col("Date", 10), col("Type", 10), col("No.", 9), col(if purchases { "Payee" } else { "Customer" }, 22), col("Due date", 10), coln("Balance", 11), coln("Total", 11), col("Status", 11)],
        Skin::Xero => vec![col("Number", 10), col("Ref", 10), col("To", 22), col("Date", 11), col("Due date", 11), coln("Paid", 11), coln("Due", 11), col("Status", 12)],
    };
    let rows = rows_v.iter().map(|r| {
        let kind = st(r, "type");
        let label = vocab::status(app.skin, kind, st(r, "status"), i(&r["overdue"]) == 1);
        let msg = match kind {
            "EXPENSE" => None,
            "BILLPAYMENT" => r["document_id"].as_i64().map(|id| Message::Nav(Route::Doc { kind: "ACCPAY".into(), id: Some(id) })),
            _ => Some(Message::Nav(Route::Doc { kind: kind.into(), id: r["id"].as_i64() })),
        };
        let cells: Vec<Element<Message>> = match app.skin {
            Skin::QuickBooks => vec![
                ui::cell(app.d(st(r, "date"))), ui::cell_muted(type_label(app, kind)), ui::cell(st(r, "number")), ui::cell(st(r, "contact_name")),
                ui::cell_muted(if kind.ends_with("CREDIT") { String::new() } else { app.d(st(r, "due_date")) }), ui::cell_money(i(&r["amount_due"])), ui::cell_money(i(&r["total"])), ui::chip(label, status_color(label)),
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
    let tabs: Vec<(&str, &str)> = if customers.is_none() { vec![("all", "All"), ("customers", "Customers"), ("suppliers", "Suppliers")] } else { vec![] };
    let body = column![top, search_and_tabs(app, &tabs), ui::table(&cols, rows, "No contacts yet.")].spacing(16);
    let new_label = match customers { Some(true) => "New customer", Some(false) => if app.skin == Skin::QuickBooks { "New vendor" } else { "New supplier" }, None => "New contact" };
    ui::page(ui::h1(title.to_string()), vec![ui::btn_primary(new_label, Some(Message::Nav(Route::NewContact(customers != Some(false)))))], body.into())
}

pub fn contact(app: &App) -> Element<'_, Message> {
    if let Some(e) = loading_or_error(app, "Contact") { return e; }
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
    let mut actions: Vec<Element<Message>> = vec![];
    if is_supplier {
        actions.push(ui::btn_primary("New bill", Some(Message::NewDocFor("ACCPAY".into(), id))));
    } else {
        actions.push(ui::btn(app.term("receive_payment"), Some(Message::Nav(Route::ReceivePayment(Some(id))))));
        actions.push(ui::btn_primary("New invoice", Some(Message::NewDocFor("ACCREC".into(), id))));
    }
    let back = if is_supplier { Route::Vendors } else { Route::Customers };
    let title = column![ui::link(if is_supplier { app.term("suppliers") } else { app.term("customers") }, Message::Nav(back)), ui::h1(st(c, "name").to_string())].spacing(4);
    ui::page(title.into(), actions, column![info, ui::h2("Transaction list"), ui::table(&cols, rows, "No transactions yet.")].spacing(16).into())
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
            if tracked { ui::num(format!("{}", i(&x["qty_on_hand_e4"]) as f64 / 10000.0)) } else { ui::cell_muted("") }], None)
    }).collect();
    let note: Element<Message> = if low > 0 { container(text(format!("{low} item(s) low on stock")).size(13).color(k.warn)).padding([6, 10]).style(theme::bad_soft).into() } else { Space::new().into() };
    ui::page(ui::h1(title), vec![ui::btn_primary("New", Some(Message::Nav(Route::NewProduct)))], column![note, search_and_tabs(app, &[]), ui::table(&cols, rows, "No products or services yet.")].spacing(16).into())
}

/// QuickBooks shows a "detail type" under each account type; derived from the engine's type.
fn detail_type(kind: &str) -> &'static str {
    match kind {
        "BANK" => "Checking / savings", "CURRENT" => "Other current assets", "INVENTORY" => "Inventory", "PREPAYMENT" => "Prepaid expenses", "FIXED" => "Machinery & equipment",
        "NONCURRENT" => "Other long-term assets", "CURRLIAB" => "Other current liabilities", "TERMLIAB" | "LIABILITY" => "Long-term liabilities", "EQUITY" => "Owner's equity",
        "REVENUE" | "SALES" => "Sales of product income", "OTHERINCOME" => "Other primary income", "DIRECTCOSTS" => "Supplies & materials - COGS", "EXPENSE" | "OVERHEADS" => "Other business expenses",
        "DEPRECIATN" => "Depreciation", _ => "",
    }
}
fn qb_type(kind: &str) -> &'static str {
    match kind {
        "BANK" => "Bank", "CURRENT" | "PREPAYMENT" | "INVENTORY" => "Other Current Assets", "FIXED" => "Fixed Assets", "NONCURRENT" => "Other Assets",
        "CURRLIAB" => "Other Current Liabilities", "TERMLIAB" | "LIABILITY" => "Long Term Liabilities", "EQUITY" => "Equity",
        "REVENUE" | "SALES" => "Income", "OTHERINCOME" => "Other Income", "DIRECTCOSTS" => "Cost of Goods Sold", "EXPENSE" | "OVERHEADS" | "DEPRECIATN" => "Expenses", _ => "Other",
    }
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
            Skin::QuickBooks => vec![ui::cell_muted(st(a, "code")), ui::cell(st(a, "name")), ui::cell_muted(qb_type(kind)), ui::cell_muted(detail_type(kind)), ui::cell_money(bal)],
            Skin::Xero => vec![ui::cell_muted(st(a, "code")), ui::cell(st(a, "name")), ui::cell_muted(kind.to_string()), ui::cell_muted(st(a, "tax_rate_name")), ui::cell_money(bal)],
        };
        (cells, bank_id.map(|id| Message::Nav(Route::Banking(Some(id)))))
    }).collect();
    ui::page(ui::h1("Chart of accounts"), vec![ui::btn_primary("New", None)], ui::table(&cols, rows, "No accounts."))
}
