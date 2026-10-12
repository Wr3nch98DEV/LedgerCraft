//! QuickBooks Desktop-style layout (modelled on Enterprise 2024): menu bar, left icon bar with
//! My Shortcuts / View Balances / Open Windows, several open windows shown as tabs, and the Home
//! page workflow diagram. Screens themselves are shared with the other layouts unless a
//! Desktop-specific version exists.

use crate::app::{App, Message, Route};
use crate::icons::icon;
use crate::theme::{self, t};
use crate::ui::{self, BOLD};
use crate::{doc, forms, payments};
use iced::widget::{Space, button, column, container, mouse_area, opaque, pick_list, row, scrollable, stack, text, text_input};
use iced::{Alignment, Element, Fill, Length, Padding, Task};
use serde_json::Value;

/// One open window: its screen plus any unsaved form state, kept while another window is in front.
pub struct Win {
    pub route: Route,
    pub doc: Option<doc::Editor>,
    pub pay: Option<payments::State>,
    pub form: Option<forms::State>,
    pub data: Option<Value>,
}
impl Win { pub fn new(route: Route) -> Win { Win { route, doc: None, pay: None, form: None, data: None } } }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane { #[default] Shortcuts, Balances, Windows }

#[derive(Default)]
pub struct State {
    pub windows: Vec<Win>,
    pub active: usize,
    pub pane: Pane,
    /// Home window: Insights tab instead of the Home Page diagram.
    pub insights: bool,
    /// Customer/Vendor Center: selected name and its details + transactions.
    pub sel: Option<i64>,
    pub detail: Option<Value>,
    /// Report Center category.
    pub report_cat: Option<&'static str>,
    /// Income/Bill Tracker: which summary box is selected ("all" when none).
    pub tracker: &'static str,
}

#[derive(Debug, Clone)]
pub enum Msg { Activate(usize), Close(usize), CloseAll, Pane(Pane), Insights(bool), Exit, Select(i64), Detail(i64, crate::backend::Reply), ReportCat(&'static str), Tracker(&'static str) }

impl App {
    /// Keep the current screen's unsaved state with its window.
    pub fn stash_window(&mut self) {
        let i = self.desk.active;
        let route = self.route.clone();
        let (doc, pay, form, data) = (self.doc.take(), self.pay.take(), self.form.take(), self.data.take());
        if let Some(w) = self.desk.windows.get_mut(i) {
            if w.route == route { w.doc = doc; w.pay = pay; w.form = form; w.data = data; }
        }
    }

    /// Bring window `i` to the front. Returns true when it had saved state (no reload needed).
    pub fn restore_window(&mut self, i: usize) -> bool {
        let Some(w) = self.desk.windows.get_mut(i) else { return false };
        self.desk.active = i;
        self.route = w.route.clone();
        let had = w.doc.is_some() || w.pay.is_some() || w.form.is_some();
        self.doc = w.doc.take();
        self.pay = w.pay.take();
        self.form = w.form.take();
        let data = w.data.take();
        if had { self.data = data.or(Some(serde_json::json!({}))); self.error = None; }
        had
    }

    /// Make sure the current route has a window and it's the active one.
    pub fn ensure_window(&mut self) {
        if matches!(self.route, Route::Setup | Route::Login) { return; }
        match self.desk.windows.iter().position(|w| w.route == self.route) {
            Some(i) => self.desk.active = i,
            None => { self.desk.windows.push(Win::new(self.route.clone())); self.desk.active = self.desk.windows.len() - 1; }
        }
    }

    /// Close window `i` and show the one before it (Home when none are left).
    pub fn close_window(&mut self, i: usize) -> Task<Message> {
        if i >= self.desk.windows.len() { return Task::none(); }
        let was_active = i == self.desk.active;
        if was_active { self.doc = None; self.pay = None; self.form = None; } else { self.stash_window(); }
        self.desk.windows.remove(i);
        if self.desk.windows.is_empty() { self.route = Route::Dashboard; return self.load(); }
        let next = if was_active { i.saturating_sub(1).min(self.desk.windows.len() - 1) } else if i < self.desk.active { self.desk.active - 1 } else { self.desk.active };
        if self.restore_window(next) { Task::none() } else { self.load() }
    }
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    match m {
        Msg::Activate(i) => match app.desk.windows.get(i) { Some(w) => { let r = w.route.clone(); app.go(r) } None => Task::none() },
        Msg::Close(i) => app.close_window(i),
        Msg::CloseAll => {
            app.doc = None; app.pay = None; app.form = None;
            app.desk.windows.clear();
            app.route = Route::Dashboard;
            app.menu = None;
            app.load()
        }
        Msg::Pane(p) => { app.desk.pane = p; Task::none() }
        Msg::Insights(v) => { app.desk.insights = v; Task::none() }
        Msg::Exit => iced::exit(),
        Msg::Select(id) => {
            app.desk.sel = Some(id);
            let be = app.be.clone();
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<Value, String> {
                    let c = be.call_now("contacts.get", serde_json::json!({"id": id}))?;
                    let mut rows = vec![];
                    for t in ["SALES", "ACCPAY", "ACCPAYCREDIT", "QUOTE", "DELAYED", "PO"] {
                        rows.extend(be.call_now("docs.list", serde_json::json!({"type": t, "contact_id": id, "limit": 300})).ok().and_then(|v| v["rows"].as_array().cloned()).unwrap_or_default());
                    }
                    rows.sort_by(|a, b| b["date"].as_str().cmp(&a["date"].as_str()));
                    Ok(serde_json::json!({"contact": c, "rows": rows}))
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, move |r| Message::Desk(Msg::Detail(id, r)))
        }
        Msg::ReportCat(c) => { app.desk.report_cat = Some(c); Task::none() }
        Msg::Tracker(f) => { app.desk.tracker = if app.desk.tracker == f { "all" } else { f }; Task::none() }
        Msg::Detail(id, r) => {
            if app.desk.sel == Some(id) { app.desk.detail = r.ok(); }
            Task::none()
        }
    }
}

/// After a Center's list loads, select the remembered name (or the first one).
pub fn after_load(app: &mut App) -> Task<Message> {
    if !app.desktop() || !matches!(app.route, Route::Customers | Route::Vendors) { return Task::none(); }
    let list = app.data.as_ref().and_then(|d| d.as_array().cloned()).unwrap_or_default();
    let keep = app.desk.sel.filter(|s| list.iter().any(|c| c["id"].as_i64() == Some(*s)));
    match keep.or_else(|| list.first().and_then(|c| c["id"].as_i64())) {
        Some(id) if app.desk.detail.as_ref().and_then(|d| d["contact"]["id"].as_i64()) != Some(id) => update(app, Msg::Select(id)),
        _ => Task::none(),
    }
}

/// Window title as QuickBooks Desktop shows it.
pub fn title(app: &App, r: &Route) -> String {
    match r {
        Route::Dashboard => "Home".into(),
        Route::AllSales | Route::Invoices => "Income Tracker".into(),
        Route::Quotes => "Estimates".into(),
        Route::CreditNotes => "Credit Memos".into(),
        Route::Bills | Route::Expenses | Route::PurchaseOrders => "Bill Tracker".into(),
        Route::Customers | Route::Contacts => "Customer Center".into(),
        Route::Vendors => "Vendor Center".into(),
        Route::Contact(_) => "Customer Center".into(),
        Route::Products => "Item List".into(),
        Route::Chart => "Chart of Accounts".into(),
        Route::Banking(_) => "Bank Feeds".into(),
        Route::Reports => "Report Center".into(),
        Route::Report(k) => match k.as_str() {
            "profit-and-loss" => "Profit & Loss".into(), "balance-sheet" => "Balance Sheet".into(), "cash-flow-statement" => "Statement of Cash Flows".into(),
            "trial-balance" => "Trial Balance".into(), "general-ledger-detail" => "General Ledger".into(), "general-ledger" => "General Ledger Summary".into(),
            _ => {
                // The report's own title once it's loaded, else the QuickBooks name.
                let loaded = app.data.as_ref().filter(|_| app.route == *r).and_then(|d| d["title"].as_str()).map(String::from);
                let n = crate::reports::name(app.skin, k, "");
                if n.is_empty() { loaded.unwrap_or_else(|| "Report".into()) } else { n }
            }
        },
        Route::Settings => "Preferences".into(),
        Route::Reconcile => "Reconcile".into(),
        Route::Register(_) => app.data.as_ref().filter(|_| app.route == *r).and_then(|d| d["banks"].as_array().and_then(|b| b.iter().find(|x| x["id"] == d["account"]).and_then(|x| x["name"].as_str().map(String::from)))).unwrap_or_else(|| "Register".into()),
        Route::Doc { kind, .. } => match kind.as_str() {
            "ACCREC" => "Create Invoices", "QUOTE" => "Create Estimates", "ACCRECCREDIT" => "Create Credit Memos/Refunds", "SALESRECEIPT" => "Enter Sales Receipts",
            "ACCPAY" => "Enter Bills", "ACCPAYCREDIT" => "Enter Bills (Credit)", "PO" => "Create Purchase Orders", "DELAYEDCHARGE" | "DELAYEDCREDIT" => "Enter Statement Charges", _ => "Transaction",
        }.into(),
        Route::ReceivePayment(_) => "Receive Payments".into(),
        Route::PayBills => "Pay Bills".into(),
        Route::Spend(_) => "Enter Credit Card Charges".into(),
        Route::Check(_) => "Write Checks".into(),
        Route::Deposit => "Make Deposits".into(),
        Route::Transfer => "Transfer Funds Between Accounts".into(),
        Route::Journal => "Make General Journal Entries".into(),
        Route::NewContact(true) => "New Customer".into(),
        Route::NewContact(false) => "New Vendor".into(),
        Route::NewProduct => "New Item".into(),
        Route::NewAccount => "Add New Account".into(),
        Route::Payroll | Route::PayRun(_) => "Payroll Center".into(),
        Route::Taxes | Route::TaxReturn(_) => "Manage Sales Tax".into(),
        Route::Projects => "Projects".into(),
        Route::Mileage => "Enter Vehicle Mileage".into(),
        Route::Placeholder(p) => p.to_string(),
        Route::Setup | Route::Login => "Company".into(),
    }
}

// ---------------- menu bar ----------------

const MENUS: [&str; 14] = ["File", "Edit", "View", "Lists", "Favorites", "Company", "Inventory", "Customers", "Vendors", "Employees", "Banking", "Reports", "Window", "Help"];

/// Width of a menu bar title (fixed so dropdowns can line up under it).
fn menu_w(name: &str) -> f32 { name.len() as f32 * 6.9 + 16.0 }

fn nav(r: Route) -> Option<Message> { Some(Message::Nav(r)) }
fn doc_new(kind: &str) -> Option<Message> { nav(Route::Doc { kind: kind.into(), id: None }) }
fn rep(key: &str) -> Option<Message> { nav(Route::Report(key.into())) }

/// Menu entries: (label, shortcut, action). "-" is a separator; a label starting with "#" is a group heading.
fn menu_items(app: &App, name: &str) -> Vec<(String, &'static str, Option<Message>)> {
    let e = |l: &str, k: &'static str, m: Option<Message>| (l.to_string(), k, m);
    let sep = || ("-".to_string(), "", None);
    match name {
        "File" => vec![e("New Company…", "", nav(Route::Setup)), e("Open or Restore Company…", "", nav(Route::Setup)), e("Open Previous Company", "", nav(Route::Setup)), sep(),
            e("Back Up Company", "", None), e("Print Forms", "", None), sep(), e("Exit", "Alt+F4", Some(Message::Desk(Msg::Exit)))],
        "Edit" => vec![e("Undo", "Ctrl+Z", None), sep(), e("Find…", "Ctrl+F", None), e("Search…", "F3", None), sep(), e("Preferences…", "", nav(Route::Settings))],
        "View" => vec![e("Open Window List", "", Some(Message::Desk(Msg::Pane(Pane::Windows)))), e("View Balances", "", Some(Message::Desk(Msg::Pane(Pane::Balances)))),
            e("My Shortcuts", "", Some(Message::Desk(Msg::Pane(Pane::Shortcuts)))), sep(), e(if app.dark { "Light Mode" } else { "Dark Mode" }, "Alt+T", Some(Message::ToggleTheme)), sep(),
            e("Switch to QuickBooks Online-style layout", "", Some(Message::SetLayout(theme::Layout::Online))), e("Switch to Xero-style layout", "", Some(Message::SetLayout(theme::Layout::Xero)))],
        "Lists" => vec![e("Chart of Accounts", "Ctrl+A", nav(Route::Chart)), e("Item List", "", nav(Route::Products)), e("Fixed Asset Item List", "", None), e("Price Level List", "", None),
            e("Sales Tax Code List", "", nav(Route::Taxes)), e("Payroll Item List", "", None), e("Class List", "", None), e("Other Names List", "", None), e("Templates", "", None), e("Memorized Transaction List", "Ctrl+T", None)],
        "Favorites" => vec![e("Customize Favorites…", "", None)],
        "Company" => vec![e("Home Page", "", nav(Route::Dashboard)), e("Company Snapshot", "", Some(Message::Desk(Msg::Insights(true)))), sep(), e("My Company", "", nav(Route::Settings)),
            e("Company Information…", "", nav(Route::Settings)), e("Set Up Users and Roles…", "", nav(Route::Settings)), sep(), e("Chart of Accounts", "Ctrl+A", nav(Route::Chart)),
            e("Make General Journal Entries…", "", nav(Route::Journal)), e("Enter Vehicle Mileage…", "", nav(Route::Mileage)), e("Calendar", "", None), e("Documents", "", None)],
        "Inventory" => vec![e("Inventory Center", "", nav(Route::Products)), e("Item List", "", nav(Route::Products)), sep(), e("Create Purchase Orders", "", doc_new("PO")),
            e("Purchase Orders List", "", nav(Route::PurchaseOrders)), e("Receive Items", "", None), e("Adjust Quantity/Value on Hand…", "", None)],
        "Customers" => vec![e("Customer Center", "Ctrl+J", nav(Route::Customers)), sep(), e("Create Estimates", "", doc_new("QUOTE")), e("Create Sales Orders", "", None),
            e("Create Invoices", "Ctrl+I", doc_new("ACCREC")), e("Enter Sales Receipts", "", doc_new("SALESRECEIPT")), e("Enter Statement Charges", "", doc_new("DELAYEDCHARGE")),
            e("Create Statements…", "", None), e("Assess Finance Charges…", "", None), sep(), e("Receive Payments", "", nav(Route::ReceivePayment(None))),
            e("Create Credit Memos/Refunds", "", doc_new("ACCRECCREDIT")), sep(), e("Income Tracker", "", nav(Route::AllSales)), e("Item List", "", nav(Route::Products))],
        "Vendors" => vec![e("Vendor Center", "", nav(Route::Vendors)), sep(), e("Enter Bills", "", doc_new("ACCPAY")), e("Pay Bills", "", nav(Route::PayBills)), e("Bill Tracker", "", nav(Route::Bills)),
            sep(), e("Sales Tax: Manage Sales Tax…", "", nav(Route::Taxes)), sep(), e("Create Purchase Orders", "", doc_new("PO")), e("Purchase Orders List", "", nav(Route::PurchaseOrders)),
            sep(), e("Print/E-file 1099s: 1099 Summary", "", rep("contractor-1099")), e("Enter Bills for Received Items", "", None)],
        "Employees" => vec![e("Employee Center", "", nav(Route::Payroll)), e("Payroll Center", "", nav(Route::Payroll)), sep(), e("Pay Employees", "", nav(Route::Payroll)),
            e("Pay Liabilities", "", None), e("Process Payroll Forms", "", None), sep(), e("Enter Time", "", None)],
        "Banking" => vec![e("Write Checks", "Ctrl+W", nav(Route::Check(None))), e("Order Checks & Envelopes", "", None), e("Enter Credit Card Charges", "", nav(Route::Spend(None))), sep(),
            e("Use Register", "Ctrl+R", nav(Route::Register(None))), e("Make Deposits", "", nav(Route::Deposit)), e("Transfer Funds", "", nav(Route::Transfer)), e("Reconcile", "", nav(Route::Reconcile)),
            sep(), e("Bank Feeds Center", "", nav(Route::Banking(None)))],
        "Reports" => vec![e("Report Center", "", nav(Route::Reports)), e("Memorized Reports", "", None), e("Company Snapshot", "", Some(Message::Desk(Msg::Insights(true)))), sep(),
            e("#Company & Financial", "", None), e("Profit & Loss Standard", "", rep("profit-and-loss")), e("Balance Sheet Standard", "", rep("balance-sheet")), e("Statement of Cash Flows", "", rep("cash-flow-statement")),
            e("#Customers & Receivables", "", None), e("A/R Aging Summary", "", rep("aged-receivables")), e("Customer Balance Summary", "", rep("contact-transactions")),
            e("#Sales", "", None), e("Sales by Customer Summary", "", rep("sales-by-contact")), e("Sales by Item Summary", "", rep("sales-by-item")),
            e("#Vendors & Payables", "", None), e("A/P Aging Summary", "", rep("aged-payables")), e("1099 Summary", "", rep("contractor-1099")),
            e("#Banking", "", None), e("Reconciliation Discrepancy", "", rep("bank-reconciliation")),
            e("#Accountant & Taxes", "", None), e("Trial Balance", "", rep("trial-balance")), e("General Ledger", "", rep("general-ledger-detail")), e("Journal", "", rep("journal-report"))],
        "Window" => {
            let mut v = vec![e("Close All", "", Some(Message::Desk(Msg::CloseAll))), sep()];
            for (n, w) in app.desk.windows.iter().enumerate() { v.push((format!("{}{}", if n == app.desk.active { "✓ " } else { "   " }, title(app, &w.route)), "", Some(Message::Desk(Msg::Activate(n))))); }
            v
        }
        _ => vec![e("LedgerCraft Help", "F1", None), e("About LedgerCraft", "", None)],
    }
}

fn menu_bar(app: &App) -> Element<'_, Message> {
    let k = t();
    let mut r = row![].spacing(0).align_y(Alignment::Center);
    for m in MENUS {
        let open = app.menu == Some(m);
        r = r.push(button(text(m).size(13).width(Fill).align_x(iced::alignment::Horizontal::Center)).padding([4, 0]).width(Length::Fixed(menu_w(m)))
            .style(move |_, s| {
                let hot = open || matches!(s, button::Status::Hovered);
                button::Style { background: if hot { Some(iced::Background::Color(k.hover)) } else { None }, text_color: k.text, border: iced::Border { radius: 3.0.into(), ..Default::default() }, shadow: Default::default(), snap: true }
            })
            .on_press(Message::OpenMenu(if open { None } else { Some(m) })));
    }
    container(row![r, ui::hspace(), text(app.org_name()).size(12).color(k.muted)].align_y(Alignment::Center).padding([0, 8])).height(28).center_y(28).width(Fill).style(theme::topbar).into()
}

fn dropdown<'a>(app: &'a App, name: &'static str) -> Element<'a, Message> {
    let k = t();
    let mut c = column![].spacing(0).width(300);
    for (label, key, msg) in menu_items(app, name) {
        if label == "-" { c = c.push(container(ui::hr()).padding([4, 6])); continue; }
        if let Some(h) = label.strip_prefix('#') { c = c.push(container(text(h.to_string()).size(11).font(BOLD).color(k.muted)).padding(Padding { top: 6.0, bottom: 2.0, left: 10.0, right: 10.0 })); continue; }
        let enabled = msg.is_some();
        let mut b = button(row![text(label).size(13).color(if enabled { k.text } else { k.faint }), ui::hspace(), text(key).size(12).color(k.faint)].align_y(Alignment::Center))
            .style(theme::menu_item).padding([5, 10]).width(Fill);
        if let Some(m) = msg { b = b.on_press(m); }
        c = c.push(b);
    }
    let left: f32 = 8.0 + MENUS.iter().take_while(|m| **m != name).map(|m| menu_w(m)).sum::<f32>();
    container(opaque(container(scrollable(c).style(theme::scroll)).max_height(620).padding(4).style(theme::menu))).padding(Padding { top: 28.0, left, ..Padding::ZERO }).into()
}

// ---------------- left icon bar ----------------

fn icon_bar(app: &App) -> Element<'_, Message> {
    let k = t();
    let item = |ic: &'static str, label: &'static str, msg: Option<Message>, active: bool| -> Element<'_, Message> {
        let col_ = if msg.is_some() { k.nav_text } else { k.nav_muted };
        let mut b = button(row![icon(ic, 18.0, if active { k.nav_active } else { col_ }), text(label).size(13).color(col_)].spacing(10).align_y(Alignment::Center))
            .style(theme::nav_item(active)).padding([6, 10]).width(Fill);
        if let Some(m) = msg { b = b.on_press(m); }
        b.into()
    };
    let r = &app.route;
    let body: Element<Message> = match app.desk.pane {
        Pane::Shortcuts => column![
            text("My Shortcuts").size(11).font(BOLD).color(k.nav_muted),
            item("home", "Home", nav(Route::Dashboard), *r == Route::Dashboard),
            item("apps", "My Company", nav(Route::Settings), *r == Route::Settings),
            item("sales", "Income Tracker", nav(Route::AllSales), matches!(r, Route::AllSales | Route::Invoices)),
            item("expenses", "Bill Tracker", nav(Route::Bills), matches!(r, Route::Bills)),
            item("calendar", "Calendar", None, false),
            item("chart", "Snapshots", Some(Message::Desk(Msg::Insights(true))), false),
            item("contacts", "Customers", nav(Route::Customers), matches!(r, Route::Customers | Route::Contact(_))),
            item("users", "Vendors", nav(Route::Vendors), matches!(r, Route::Vendors)),
            item("payroll", "Employees", nav(Route::Payroll), matches!(r, Route::Payroll | Route::PayRun(_))),
            item("bank", "Bank Feeds", nav(Route::Banking(None)), matches!(r, Route::Banking(_))),
            item("docs", "Docs", None, false),
            item("reports", "Reports", nav(Route::Reports), matches!(r, Route::Reports | Route::Report(_))),
            item("check-write", "Write Checks", nav(Route::Check(None)), matches!(r, Route::Check(_))),
            item("reconcile", "Reconcile", nav(Route::Reconcile), matches!(r, Route::Reconcile)),
        ].spacing(1).into(),
        Pane::Balances => {
            let mut c = column![text("Account Balances").size(11).font(BOLD).color(k.nav_muted)].spacing(4);
            for (head, pred) in [("Bank Accounts", "BANK"), ("Accounts Receivable", "AR"), ("Accounts Payable", "AP")] {
                c = c.push(text(head).size(12).font(BOLD).color(k.nav_text));
                for a in app.lk.accounts.iter().filter(|a| a["status"] == "ACTIVE" && match pred {
                    "BANK" => a["type"] == "BANK", "AR" => a["system_account"] == "AR", _ => a["system_account"] == "AP" }) {
                    let bal = a["ytd_balance"].as_i64().unwrap_or(0) * if pred == "AP" { -1 } else { 1 };
                    c = c.push(row![text(a["name"].as_str().unwrap_or("").to_string()).size(12).color(k.nav_muted).width(Fill), text(ui::money(bal)).size(12).color(k.nav_text)].spacing(6));
                }
            }
            c.into()
        }
        Pane::Windows => {
            let mut c = column![text("Open Windows").size(11).font(BOLD).color(k.nav_muted)].spacing(1);
            for (n, w) in app.desk.windows.iter().enumerate() {
                let active = n == app.desk.active;
                c = c.push(button(text(title(app, &w.route)).size(13).color(k.nav_text)).style(theme::nav_item(active)).padding([6, 10]).width(Fill).on_press(Message::Desk(Msg::Activate(n))));
            }
            c.into()
        }
    };
    let pane_btn = |label: &'static str, p: Pane, ic: &'static str| -> Element<'_, Message> {
        let active = app.desk.pane == p;
        button(row![icon(ic, 15.0, if active { k.nav_active } else { k.nav_muted }), text(label).size(12).color(if active { k.nav_text } else { k.nav_muted })].spacing(8).align_y(Alignment::Center))
            .style(theme::nav_item(active)).padding([5, 10]).width(Fill).on_press(Message::Desk(Msg::Pane(p))).into()
    };
    let search = text_input("Search Company or Help", &app.search).on_input(Message::Search).on_submit(Message::SearchGo).padding([5, 8]).size(12).style(theme::input);
    container(column![
        container(search).padding(8),
        scrollable(container(body).padding([4, 8])).style(theme::scroll).height(Fill),
        container(ui::hr()).padding([0, 8]),
        container(column![
            pane_btn("My Shortcuts", Pane::Shortcuts, "list"),
            pane_btn("View Balances", Pane::Balances, "bank"),
            button(row![icon("reports", 15.0, k.nav_muted), text("Run Favorite Reports").size(12).color(k.nav_muted)].spacing(8).align_y(Alignment::Center)).style(theme::nav_item(false)).padding([5, 10]).width(Fill).on_press(Message::Nav(Route::Reports)),
            pane_btn("Open Windows", Pane::Windows, "layout"),
        ].spacing(1)).padding(Padding { top: 4.0, bottom: 8.0, left: 8.0, right: 8.0 }),
    ]).width(208).height(Fill).style(theme::nav).into()
}

// ---------------- window tabs ----------------

fn window_tabs(app: &App) -> Element<'_, Message> {
    let k = t();
    let mut r = row![].spacing(2).align_y(Alignment::End);
    for (n, w) in app.desk.windows.iter().enumerate() {
        let active = n == app.desk.active;
        let tab = row![
            button(text(title(app, &w.route)).size(13).font(if active { BOLD } else { iced::Font::DEFAULT }).color(if active { k.text } else { k.muted })).style(theme::ghost).padding([5, 4]).on_press(Message::Desk(Msg::Activate(n))),
            button(icon("x", 12.0, k.muted)).style(theme::ghost).padding(3).on_press(Message::Desk(Msg::Close(n))),
        ].spacing(2).align_y(Alignment::Center);
        r = r.push(container(tab).padding(Padding { top: 1.0, bottom: 1.0, left: 8.0, right: 4.0 }).style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(if active { k.panel } else { k.bg2 })),
            border: iced::Border { color: if active { k.line2 } else { k.line }, width: 1.0, radius: iced::border::Radius { top_left: 5.0, top_right: 5.0, bottom_left: 0.0, bottom_right: 0.0 } },
            ..Default::default()
        }));
    }
    container(scrollable(r).direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new().width(3).scroller_width(3))).style(theme::scroll))
        .padding(Padding { top: 5.0, left: 6.0, right: 6.0, bottom: 0.0 }).width(Fill).style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.bg2)), ..Default::default() }).into()
}

/// The whole Desktop frame around the current screen.
pub fn frame<'a>(app: &'a App, content: Element<'a, Message>) -> Element<'a, Message> {
    let k = t();
    let work = column![window_tabs(app), container(ui::hr()).width(Fill), content];
    let base: Element<Message> = column![menu_bar(app), row![icon_bar(app), work]].into();
    let mut layers = stack![base].width(Fill).height(Fill);
    if let Some(m) = app.menu {
        layers = layers.push(mouse_area(container(Space::new()).width(Fill).height(Fill)).on_press(Message::CloseMenus));
        layers = layers.push(dropdown(app, m));
    }
    if !app.search_results.is_empty() && !app.search.is_empty() {
        layers = layers.push(mouse_area(container(Space::new()).width(Fill).height(Fill)).on_press(Message::CloseMenus));
        let list = column(app.search_results.iter().map(|r| {
            let entity = r["entity"].as_str().unwrap_or("").to_string();
            button(column![text(r["title"].as_str().unwrap_or("").to_string()).size(13), text(r["snippet"].as_str().unwrap_or("").replace(['[', ']'], "")).size(11).color(k.muted)].spacing(1))
                .style(theme::menu_item).padding([5, 8]).width(Fill).on_press(Message::OpenResult(entity, r["id"].as_i64().unwrap_or(0))).into()
        })).spacing(1);
        layers = layers.push(container(opaque(container(list).padding(8).width(400).style(theme::menu))).padding(Padding { top: 72.0, left: 8.0, ..Padding::ZERO }));
    }
    layers.into()
}

// ---------------- Income / Bill Tracker ----------------

/// Everything the trackers show: sales (or purchases) with their open estimates (or POs).
pub fn load_tracker(app: &mut App, sales: bool) -> Task<Message> {
    let be = app.be.clone();
    let g = app.page_gen;
    Task::perform(async move {
        tokio::task::spawn_blocking(move || -> Result<Value, String> {
            let kinds: &[&str] = if sales { &["SALES", "QUOTE", "DELAYED"] } else { &["ACCPAY", "ACCPAYCREDIT", "PO"] };
            let mut rows = vec![];
            for k in kinds { rows.extend(be.call_now("docs.list", serde_json::json!({"type": k, "limit": 1000}))?["rows"].as_array().cloned().unwrap_or_default()); }
            rows.retain(|r| !matches!(r["status"].as_str(), Some("DELETED")));
            rows.sort_by(|a, b| b["date"].as_str().cmp(&a["date"].as_str()));
            Ok(serde_json::json!({"rows": rows}))
        }).await.unwrap_or_else(|e| Err(e.to_string()))
    }, move |r| Message::Loaded(g, r))
}

/// Which tracker box a row falls in.
fn tracker_bucket(r: &Value, since: &str) -> &'static str {
    let kind = r["type"].as_str().unwrap_or("");
    let st = r["status"].as_str().unwrap_or("");
    match (kind, st) {
        ("QUOTE", "SENT" | "DRAFT" | "ACCEPTED") if !r["subtype"].as_str().unwrap_or("").starts_with("DELAYED") => "estimates",
        ("PO", "AUTHORISED" | "DRAFT" | "SUBMITTED") => "estimates",
        ("ACCREC" | "ACCPAY", "AUTHORISED") if r["overdue"].as_i64() == Some(1) => "overdue",
        ("ACCREC" | "ACCPAY", "AUTHORISED" | "DRAFT" | "SUBMITTED") => "open",
        ("ACCREC" | "ACCPAY", "PAID") if r["paid_date"].as_str().unwrap_or("") >= since => "paid",
        _ => "",
    }
}

/// Customers ▸ Income Tracker / Vendors ▸ Bill Tracker.
pub fn tracker(app: &App, sales: bool) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let rows = d["rows"].as_array().cloned().unwrap_or_default();
    let since = ui::add_days_iso(&ui::today_iso(), -30);
    let sum = |b: &str| -> (usize, i64) {
        rows.iter().filter(|r| in_box(r, &since, b)).fold((0, 0), |(n, s), r| (n + 1, s + if b == "paid" { r["total"].as_i64().unwrap_or(0) } else if b == "estimates" { r["total"].as_i64().unwrap_or(0) } else { r["amount_due"].as_i64().unwrap_or(0) }))
    };
    let filter = if app.desk.tracker.is_empty() { "all" } else { app.desk.tracker };
    let boxes: Vec<(&'static str, &'static str, iced::Color)> = if sales {
        vec![("estimates", "ESTIMATES", hexc(0x2f6fb5)), ("open", "OPEN INVOICES", hexc(0x7a52b0)), ("overdue", "OVERDUE", hexc(0xd7502b)), ("paid", "PAID LAST 30 DAYS", hexc(0x3c9a3e))]
    } else {
        vec![("estimates", "PURCHASE ORDERS", hexc(0x2f6fb5)), ("open", "OPEN BILLS", hexc(0x7a52b0)), ("overdue", "OVERDUE", hexc(0xd7502b)), ("paid", "PAID LAST 30 DAYS", hexc(0x3c9a3e))]
    };
    let bar = row(boxes.into_iter().map(|(key, lbl, c)| {
        let (n, amt) = sum(key);
        let on = filter == key;
        button(column![text(ui::money(amt)).size(20).font(BOLD).color(iced::Color::WHITE), text(format!("{n} {lbl}")).size(11).font(BOLD).color(iced::Color::WHITE)].spacing(2))
            .padding([10, 14]).width(Fill)
            .style(move |_, s| button::Style { background: Some(iced::Background::Color(if matches!(s, button::Status::Hovered) || on { iced::Color { a: 0.85, ..c } } else { c })),
                text_color: iced::Color::WHITE, border: iced::Border { color: if on { k.text } else { c }, width: if on { 2.0 } else { 0.0 }, radius: 3.0.into() }, shadow: Default::default(), snap: true })
            .on_press(Message::Desk(Msg::Tracker(key))).into()
    })).spacing(4);
    let q = app.list_q.to_lowercase();
    let shown: Vec<&Value> = rows.iter().filter(|r| filter == "all" || in_box(r, &since, filter))
        .filter(|r| q.is_empty() || r["contact_name"].as_str().unwrap_or("").to_lowercase().contains(&q) || r["number"].as_str().unwrap_or("").to_lowercase().contains(&q)).collect();
    let cols: Vec<(&str, u16, bool)> = vec![(if sales { "Customer:Job" } else { "Vendor" }, 20, false), ("Type", 12, false), ("Number", 10, false), ("Date", 10, false), ("Due Date", 10, false),
        ("Amount", 11, true), ("Open Balance", 11, true), ("Status", 10, false), ("Action", 12, false)];
    let rows_el: Vec<(Vec<Element<Message>>, Option<Message>)> = shown.iter().map(|r| {
        let kind = r["type"].as_str().unwrap_or("");
        let sub = r["subtype"].as_str().unwrap_or("");
        let route_kind = if sub.starts_with("DELAYED") { sub.to_string() } else { kind.to_string() };
        let status = crate::vocab::status(app.skin, if sub.starts_with("DELAYED") { sub } else { kind }, r["status"].as_str().unwrap_or(""), r["overdue"].as_i64() == Some(1));
        let open = r["amount_due"].as_i64().unwrap_or(0);
        let cid = r["contact_id"].as_i64();
        let action: Element<Message> = match kind {
            "ACCREC" if open > 0 && r["status"] == "AUTHORISED" => ui::link("Receive Payment", Message::Nav(Route::ReceivePayment(cid))),
            "ACCPAY" if open > 0 && r["status"] == "AUTHORISED" => ui::link("Pay Bill", Message::Nav(Route::PayBills)),
            "QUOTE" if bucket_is(r, &since, "estimates") => ui::link("Create Invoice", Message::Nav(Route::Doc { kind: route_kind.clone(), id: r["id"].as_i64() })),
            _ => Space::new().into(),
        };
        (vec![ui::cell(r["contact_name"].as_str().unwrap_or("").to_string()), ui::cell_muted(type_name(kind, sub)), ui::cell(r["number"].as_str().unwrap_or("").to_string()),
            ui::cell(app.d(r["date"].as_str().unwrap_or(""))), ui::cell_muted(app.d(r["due_date"].as_str().unwrap_or(""))),
            ui::cell_money(r["total"].as_i64().unwrap_or(0)), ui::cell_money(if matches!(kind, "ACCREC" | "ACCPAY" | "ACCRECCREDIT" | "ACCPAYCREDIT") { open } else { 0 }),
            text(status).size(13).color(crate::lists::status_color(status)).into(), action],
         Some(Message::Nav(Route::Doc { kind: route_kind, id: r["id"].as_i64() })))
    }).collect();
    let filters = row![
        text(if sales { "CUSTOMER:JOB" } else { "VENDOR" }).size(11).font(BOLD).color(k.info),
        text_input("All", &app.list_q).on_input(Message::ListQuery).padding([4, 7]).size(13).style(theme::input).width(220),
        text("STATUS").size(11).font(BOLD).color(k.info),
        text(match filter { "estimates" => if sales { "Estimates" } else { "Purchase Orders" }, "open" => "Open", "overdue" => "Overdue", "paid" => "Paid", _ => "All" }).size(13),
        ui::hspace(),
        if filter != "all" { ui::btn("Clear Filter", Some(Message::Desk(Msg::Tracker(filter)))) } else { Space::new().into() },
    ].spacing(10).align_y(Alignment::Center);
    let manage = if sales {
        row![ui::btn("Create Invoices", doc_new("ACCREC")), ui::btn("Create Estimates", doc_new("QUOTE")), ui::btn("Receive Payments", nav(Route::ReceivePayment(None))), ui::btn("Enter Sales Receipts", doc_new("SALESRECEIPT"))]
    } else {
        row![ui::btn("Enter Bills", doc_new("ACCPAY")), ui::btn("Pay Bills", nav(Route::PayBills)), ui::btn("Create Purchase Orders", doc_new("PO")), ui::btn("Write Checks", nav(Route::Check(None)))]
    }.spacing(8);
    let cols_ui: Vec<ui::Col> = cols.iter().map(|(h, w, r)| ui::Col { title: *h, width: *w, right: *r }).collect();
    scrollable(column![bar, filters, ui::table(&cols_ui, rows_el, "Nothing to show."), row![text("Manage Transactions").size(12).color(k.muted), manage].spacing(12).align_y(Alignment::Center)]
        .spacing(10).padding(12)).style(theme::scroll).height(Fill).into()
}

fn bucket_is(r: &Value, since: &str, b: &str) -> bool { tracker_bucket(r, since) == b }
/// Tracker box membership: Open includes Overdue (as in QuickBooks Desktop).
fn in_box(r: &Value, since: &str, b: &str) -> bool { let x = tracker_bucket(r, since); x == b || (b == "open" && x == "overdue") }
fn hexc(v: u32) -> iced::Color { iced::Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8) }

// ---------------- Report Center ----------------

const REPORT_CATS: [&str; 11] = ["Company & Financial", "Customers & Receivables", "Sales", "Jobs, Time & Mileage", "Vendors & Payables", "Purchases", "Inventory",
    "Employees & Payroll", "Banking", "Accountant & Taxes", "Budgets & Forecasts"];

/// The Report Center category a report sits in (as in QuickBooks Desktop).
fn report_cat(key: &str) -> &'static str {
    match key {
        "profit-and-loss" | "balance-sheet" | "cash-flow-statement" | "executive-summary" | "cash-summary" | "income-by-contact" => "Company & Financial",
        "aged-receivables" | "receivable-invoice-detail" | "contact-transactions" => "Customers & Receivables",
        "sales-by-contact" | "sales-by-item" => "Sales",
        "project-profitability" | "expense-claims" => "Jobs, Time & Mileage",
        "aged-payables" | "payable-invoice-detail" | "contractor-1099" | "sales-tax-by-agency" | "tax-reconciliation" => "Vendors & Payables",
        "purchases-by-contact" | "purchases-by-item" => "Purchases",
        "inventory-summary" | "fixed-asset-register" | "fixed-asset-reconciliation" | "depreciation-schedule" => "Inventory",
        "payroll-activity" | "leave-balances" => "Employees & Payroll",
        "bank-summary" | "bank-reconciliation" => "Banking",
        "budget-summary" | "budget-variance" => "Budgets & Forecasts",
        _ => "Accountant & Taxes",
    }
}

/// Reports ▸ Report Center.
pub fn report_center(app: &App) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let us = app.country() == "US";
    let cat = app.desk.report_cat.unwrap_or("Company & Financial");
    let mut left = column![].spacing(1);
    for c in REPORT_CATS {
        let on = c == cat;
        left = left.push(button(text(c).size(13).font(if on { BOLD } else { iced::Font::DEFAULT })).style(theme::nav_item(on)).padding([6, 10]).width(Fill).on_press(Message::Desk(Msg::ReportCat(c))));
    }
    let mut right = column![text(cat).size(18).font(BOLD)].spacing(8);
    let mut any = false;
    for r in d.as_array().cloned().unwrap_or_default() {
        let key = r["key"].as_str().unwrap_or("").to_string();
        if report_cat(&key) != cat || (!us && matches!(key.as_str(), "contractor-1099" | "sales-tax-by-agency")) || key == "bank-reconciliation" { continue; }
        any = true;
        let title = title(app, &Route::Report(key.clone()));
        let title = if title == "Report" { r["title"].as_str().unwrap_or("").to_string() } else { title };
        right = right.push(container(row![
            column![text(title).size(14).font(BOLD), text(r["description"].as_str().unwrap_or("").to_string()).size(12).color(k.muted)].spacing(2).width(Fill),
            ui::btn("Run", Some(Message::Nav(Route::Report(key)))),
        ].spacing(10).align_y(Alignment::Center)).padding([8, 12]).width(Fill).style(theme::card));
    }
    if !any { right = right.push(ui::muted("No reports in this category yet.")); }
    let tabs = row![text("Standard").size(13).font(BOLD), text("Memorized").size(13).color(k.faint), text("Favorites").size(13).color(k.faint), text("Recent").size(13).color(k.faint), text("Contributed").size(13).color(k.faint)].spacing(20);
    column![
        tabs,
        row![
            container(scrollable(left).style(theme::scroll)).width(230).height(Fill).padding(6).style(theme::card),
            scrollable(right.width(Fill)).style(theme::scroll).height(Fill),
        ].spacing(12).height(Fill),
    ].spacing(10).padding(12).height(Fill).into()
}

// ---------------- Lists ----------------

/// A Desktop list window: header row, rows, and the button bar along the bottom.
fn list_window<'a>(cols: &[(&str, u16, bool)], rows: Vec<(Vec<Element<'a, Message>>, Option<Message>)>, bottom: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let k = t();
    let head = container(row(cols.iter().map(|(h, w, r)| {
        let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
        container(if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
    })).spacing(8).padding([6, 10])).style(theme::header_row).width(Fill);
    let mut list = column![].spacing(0);
    for (n, (cells, msg)) in rows.into_iter().enumerate() {
        let shade = n % 2 == 1;
        let r = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(8).align_y(Alignment::Center).padding([4, 10]);
        list = list.push(button(r).padding(0).width(Fill).on_press_maybe(msg)
            .style(move |_, s| button::Style { background: Some(iced::Background::Color(if matches!(s, button::Status::Hovered) { k.hover } else if shade { k.panel2 } else { k.panel })),
                text_color: k.text, border: iced::Border::default(), shadow: Default::default(), snap: true }));
    }
    column![
        container(column![head, scrollable(list).style(theme::scroll).height(Fill)]).style(theme::card).width(Fill).height(Fill).clip(true),
        row(bottom).spacing(8).align_y(Alignment::Center),
    ].spacing(8).padding(12).height(Fill).into()
}

/// Lists ▸ Chart of Accounts.
pub fn chart(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let mut accts: Vec<Value> = d.as_array().cloned().unwrap_or_default().into_iter().filter(|a| a["status"] == "ACTIVE").collect();
    let s = |a: &Value, k: &str| a[k].as_str().unwrap_or("").to_string();
    accts.sort_by_key(|a| (crate::coa::desktop_type(a["type"].as_str().unwrap_or(""), a["bank_type"].as_str().unwrap_or(""), a["system_account"].as_str().unwrap_or("")).1, s(a, "code")));
    let cols: Vec<(&str, u16, bool)> = vec![("Name", 40), ("Type", 22), ("Balance Total", 16)].into_iter().map(|(h, w)| (h, w, h == "Balance Total")).collect();
    let rows = accts.iter().map(|a| {
        let kind = a["type"].as_str().unwrap_or("");
        let (ty, _) = crate::coa::desktop_type(kind, a["bank_type"].as_str().unwrap_or(""), a["system_account"].as_str().unwrap_or(""));
        // Desktop shows balances only for balance sheet accounts.
        let bs = matches!(a["class"].as_str(), Some("ASSET" | "LIABILITY" | "EQUITY"));
        let sign = if matches!(a["class"].as_str(), Some("LIABILITY" | "EQUITY")) { -1 } else { 1 };
        let bal: Element<Message> = if bs { ui::cell_money(sign * a["ytd_balance"].as_i64().unwrap_or(0)) } else { Space::new().into() };
        let msg = if kind == "BANK" { a["id"].as_i64().map(|id| Message::Nav(Route::Register(Some(id)))) } else { None };
        (vec![ui::cell(format!("{} · {}", s(a, "code"), s(a, "name"))), ui::cell_muted(ty), bal], msg)
    }).collect();
    list_window(&cols, rows, vec![
        ui::btn("Account: New", nav(Route::NewAccount)),
        ui::btn("Activities: Write Checks", nav(Route::Check(None))), ui::btn("Make Deposits", nav(Route::Deposit)), ui::btn("Use Register", nav(Route::Register(None))),
        ui::btn("Make General Journal Entries", nav(Route::Journal)), ui::btn("Reconcile", nav(Route::Reconcile)),
        ui::hspace(), ui::btn("Reports: Trial Balance", rep("trial-balance")),
    ])
}

/// Lists ▸ Item List.
pub fn item_list(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let items = d.as_array().cloned().unwrap_or_default();
    let acct = |id: Option<i64>| id.and_then(|id| app.lk.accounts.iter().find(|a| a["id"].as_i64() == Some(id))).map(|a| format!("{} · {}", a["code"].as_str().unwrap_or(""), a["name"].as_str().unwrap_or(""))).unwrap_or_default();
    let cols: Vec<(&str, u16, bool)> = vec![("Name", 20, false), ("Description", 28, false), ("Type", 12, false), ("Account", 22, false), ("On Hand", 8, true), ("Price", 10, true)];
    let rows = items.iter().map(|x| {
        let tracked = x["is_tracked"].as_i64() == Some(1);
        let ty = if tracked { "Inventory Part" } else if x["is_purchased"].as_i64() == Some(1) && x["is_sold"].as_i64() == Some(1) { "Non-inventory Part" } else { "Service" };
        (vec![
            ui::cell(x["name"].as_str().unwrap_or("").to_string()),
            ui::cell_muted(x["sales_description"].as_str().unwrap_or("").to_string()),
            ui::cell_muted(ty),
            ui::cell_muted(acct(x["sale_account_id"].as_i64())),
            if tracked { ui::num(format!("{}", x["qty_on_hand_e4"].as_i64().unwrap_or(0) as f64 / 10000.0)) } else { Space::new().into() },
            ui::num(ui::money(x["sale_unit_e2"].as_i64().unwrap_or(0) / 100)),
        ], None)
    }).collect();
    list_window(&cols, rows, vec![
        ui::btn("Item: New", nav(Route::NewProduct)),
        ui::btn("Activities: Create Invoices", doc_new("ACCREC")), ui::btn("Enter Sales Receipts", doc_new("SALESRECEIPT")), ui::btn("Create Purchase Orders", doc_new("PO")),
        ui::hspace(), ui::btn("Reports: Sales by Item", rep("sales-by-item")),
    ])
}

// ---------------- Register ----------------

/// A bank account register: every transaction with a running balance, newest first.
pub fn register(app: &App) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let banks: Vec<crate::app::Opt> = d["banks"].as_array().cloned().unwrap_or_default().iter()
        .map(|b| crate::app::Opt { id: b["id"].as_i64().unwrap_or(0), label: b["name"].as_str().unwrap_or("").to_string() }).collect();
    let sel = banks.iter().find(|b| Some(b.id) == d["account"].as_i64()).cloned();
    let cols: Vec<(&str, u16, bool)> = vec![("Date", 10, false), ("Number", 9, false), ("Payee", 20, false), ("Memo", 22, false), ("Payment", 11, true), ("✓", 3, false), ("Deposit", 11, true), ("Balance", 12, true)];
    let head = container(row(cols.iter().map(|(h, w, r)| {
        let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
        container(if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
    })).spacing(6).padding([6, 10])).style(theme::header_row).width(Fill);
    let mut list = column![].spacing(0);
    let mut bal = d["balance"].as_i64().unwrap_or(0);
    for (n, r) in d["rows"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let amt = r["amount"].as_i64().unwrap_or(0);
        let desc = r["description"].as_str().unwrap_or("");
        let number = if let Some(no) = desc.strip_prefix("Check ") { no.to_string() } else { r["reference"].as_str().unwrap_or("").to_string() };
        let kind = match r["kind"].as_str().unwrap_or("") { "payment" if amt > 0 => "PMT", "payment" => "BILLPMT", "deposit" => "DEP", "transfer_from" | "transfer_to" => "TRANSFR", _ if desc.starts_with("Check") => "CHK", _ if amt > 0 => "DEP", _ => "EXP" };
        let cells: Vec<Element<Message>> = vec![
            column![ui::cell(app.d(r["date"].as_str().unwrap_or(""))), text(kind).size(10).color(k.faint)].spacing(0).into(),
            ui::cell_muted(number),
            ui::cell(r["contact"].as_str().unwrap_or("").to_string()),
            ui::cell_muted(desc.to_string()),
            if amt < 0 { ui::cell_money(-amt) } else { Space::new().into() },
            text(if r["reconciled"].as_i64() == Some(1) { "✓" } else { "" }).size(13).color(k.ok).into(),
            if amt > 0 { ui::cell_money(amt) } else { Space::new().into() },
            text(ui::money(bal)).size(13).color(if bal < 0 { k.bad } else { k.text }).align_x(iced::alignment::Horizontal::Right).width(Fill).into(),
        ];
        bal -= amt;
        let msg = r["document_id"].as_i64().map(|id| Message::Nav(Route::Doc { kind: if amt > 0 { "ACCREC".into() } else { "ACCPAY".into() }, id: Some(id) }));
        let rr = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(6).align_y(Alignment::Center).padding([4, 10]);
        let shade = n % 2 == 1;
        list = list.push(button(rr).padding(0).width(Fill).on_press_maybe(msg)
            .style(move |_, s| button::Style { background: Some(iced::Background::Color(if matches!(s, button::Status::Hovered) { k.hover } else if shade { k.panel2 } else { k.panel })),
                text_color: k.text, border: iced::Border::default(), shadow: Default::default(), snap: true }));
    }
    let top = row![
        text("Go to").size(13).color(k.muted),
        pick_list(banks, sel, |b: crate::app::Opt| Message::Nav(Route::Register(Some(b.id)))).padding([5, 7]).text_size(13).style(theme::pick).width(300),
        ui::hspace(),
        ui::btn("Write Checks", nav(Route::Check(d["account"].as_i64()))),
        ui::btn("Make Deposits", nav(Route::Deposit)),
        ui::btn("Reconcile", nav(Route::Reconcile)),
    ].spacing(10).align_y(Alignment::Center);
    let ending = row![ui::hspace(), text("ENDING BALANCE").size(12).font(BOLD).color(k.info), text(ui::money(d["balance"].as_i64().unwrap_or(0))).size(15).font(BOLD)].spacing(14).align_y(Alignment::Center);
    column![
        top,
        container(column![head, scrollable(list).style(theme::scroll).height(Fill)]).style(theme::card).width(Fill).height(Fill).clip(true),
        ending,
    ].spacing(10).padding(12).height(Fill).into()
}

// ---------------- Customer / Vendor Center ----------------

fn type_name(kind: &str, sub: &str) -> &'static str {
    match (kind, sub) {
        (_, "RECEIPT") => "Sales Receipt", (_, "DELAYEDCHARGE") => "Statement Charge", (_, "DELAYEDCREDIT") => "Statement Credit",
        ("ACCREC", _) => "Invoice", ("ACCRECCREDIT", _) => "Credit Memo", ("QUOTE", _) => "Estimate", ("ACCPAY", _) => "Bill", ("ACCPAYCREDIT", _) => "Credit", ("PO", _) => "Purchase Order", _ => "Transaction",
    }
}

pub fn center(app: &App, customers: bool) -> Element<'_, Message> {
    let k = t();
    let list = app.data.as_ref().and_then(|d| d.as_array().cloned()).unwrap_or_default();
    let tool = |label: &'static str, msg: Option<Message>| -> Element<'_, Message> { ui::btn(label, msg) };
    let toolbar = if customers {
        row![tool("New Customer & Job", nav(Route::NewContact(true))), tool("Create Invoices", doc_new("ACCREC")), tool("Receive Payments", nav(Route::ReceivePayment(app.desk.sel))),
            tool("Create Estimates", doc_new("QUOTE")), tool("Enter Sales Receipts", doc_new("SALESRECEIPT")), ui::hspace(), tool("Income Tracker", nav(Route::AllSales))]
    } else {
        row![tool("New Vendor", nav(Route::NewContact(false))), tool("Enter Bills", doc_new("ACCPAY")), tool("Pay Bills", nav(Route::PayBills)),
            tool("Write Checks", nav(Route::Check(None))), tool("Create Purchase Orders", doc_new("PO")), ui::hspace(), tool("Bill Tracker", nav(Route::Bills))]
    }.spacing(6).align_y(Alignment::Center);

    // ---- left: names with balances ----
    let mut names = column![row![
        text("NAME").size(11).font(BOLD).color(k.muted).width(Fill),
        text("BALANCE TOTAL").size(11).font(BOLD).color(k.muted),
    ].padding([6, 10])].spacing(0);
    for c in &list {
        let id = c["id"].as_i64().unwrap_or(0);
        let bal = if customers { c["receivable"].as_i64().unwrap_or(0) } else { c["payable"].as_i64().unwrap_or(0) };
        let on = app.desk.sel == Some(id);
        names = names.push(button(row![text(c["name"].as_str().unwrap_or("").to_string()).size(13).width(Fill), text(ui::money(bal)).size(13)].spacing(8))
            .padding([4, 10]).width(Fill)
            .style(move |_, s| button::Style { background: if on { Some(iced::Background::Color(k.accent_soft)) } else if matches!(s, button::Status::Hovered) { Some(iced::Background::Color(k.hover)) } else { None },
                text_color: k.text, border: iced::Border::default(), shadow: Default::default(), snap: true })
            .on_press(Message::Desk(Msg::Select(id))));
    }
    let left = container(column![
        row![text(if customers { "Customers & Jobs" } else { "Vendors" }).size(13).font(BOLD), ui::hspace(), text(if customers { "Active Customers" } else { "Active Vendors" }).size(12).color(k.muted)].padding([8, 10]),
        container(text_input("Search", &app.list_q).on_input(Message::ListQuery).padding([5, 8]).size(13).style(theme::input)).padding([0, 10]),
        scrollable(names).style(theme::scroll).height(Fill),
    ].spacing(6)).width(340).height(Fill).style(theme::card);

    // ---- right: information + transactions ----
    let right: Element<Message> = match &app.desk.detail {
        None => container(ui::muted(if list.is_empty() { "No names yet — use the New button above." } else { "Loading…" })).padding(20).width(Fill).height(Fill).style(theme::card).into(),
        Some(d) => {
            let c = &d["contact"];
            let id = c["id"].as_i64().unwrap_or(0);
            let s = |key: &str| c[key].as_str().unwrap_or("").to_string();
            let info_row = |label: &'static str, v: String| -> Element<'_, Message> { row![text(label).size(12).color(k.info).width(110), text(if v.is_empty() { "—".into() } else { v }).size(13)].spacing(6).into() };
            let full = format!("{} {}", s("first_name"), s("last_name")).trim().to_string();
            let info = container(column![
                row![text(if customers { "Customer Information" } else { "Vendor Information" }).size(15).font(BOLD), ui::hspace(),
                    ui::link("QuickReport", Message::Nav(Route::Report("contact-transactions".into()))),
                    ui::link("Open Balance", Message::Nav(Route::Report(if customers { "aged-receivables" } else { "aged-payables" }.into())))].spacing(14).align_y(Alignment::Center),
                row![
                    column![info_row("Company Name", s("name")), info_row("Full Name", full), info_row(if customers { "Bill To" } else { "Billed From" }, s("postal_address").replace('\n', ", "))].spacing(6).width(Fill),
                    column![info_row("Main Phone", s("phone")), info_row("Main Email", s("email")), info_row("Account No.", s("account_number"))].spacing(6).width(Fill),
                ].spacing(20),
            ].spacing(10)).padding(14).width(Fill).style(theme::card);
            let rows: Vec<(Vec<Element<Message>>, Option<Message>)> = d["rows"].as_array().cloned().unwrap_or_default().iter().map(|r| {
                let kind = r["type"].as_str().unwrap_or("");
                let sub = r["subtype"].as_str().unwrap_or("");
                let route_kind = if sub.starts_with("DELAYED") { sub.to_string() } else { kind.to_string() };
                (vec![ui::cell(type_name(kind, sub)), ui::cell_muted(r["number"].as_str().unwrap_or("").to_string()), ui::cell(app.d(r["date"].as_str().unwrap_or(""))),
                    ui::cell_muted(crate::vocab::status(app.skin, if sub.starts_with("DELAYED") { sub } else { kind }, r["status"].as_str().unwrap_or(""), r["overdue"].as_i64() == Some(1))),
                    ui::cell_money(r["amount_due"].as_i64().unwrap_or(0)), ui::cell_money(r["total"].as_i64().unwrap_or(0))],
                 Some(Message::Nav(Route::Doc { kind: route_kind, id: r["id"].as_i64() })))
            }).collect();
            let actions = if customers { row![ui::btn("Create Invoice", Some(Message::NewDocFor("ACCREC".into(), id))), ui::btn("Receive Payment", nav(Route::ReceivePayment(Some(id))))] }
                else { row![ui::btn("Enter Bill", Some(Message::NewDocFor("ACCPAY".into(), id))), ui::btn("Pay Bills", nav(Route::PayBills))] };
            column![
                info,
                row![text("Transactions").size(14).font(BOLD), ui::hspace(), actions.spacing(6)].align_y(Alignment::Center),
                ui::table(&[ui::col("Type", 16), ui::col("Num", 10), ui::col("Date", 12), ui::col("Status", 12), ui::coln("Open Balance", 14), ui::coln("Amount", 14)], rows, "No transactions for this name yet."),
            ].spacing(10).width(Fill).into()
        }
    };
    let right = scrollable(right).style(theme::scroll).height(Fill).width(Fill);
    column![toolbar, row![left, right].spacing(10).height(Fill)].spacing(10).padding(12).height(Fill).into()
}

// ---------------- Home page ----------------

/// A clickable workflow icon on the Home page.
fn node<'a>(ic: &'static str, label: &'static str, msg: Option<Message>) -> Element<'a, Message> {
    let k = t();
    let on = msg.is_some();
    let c = column![
        container(icon(ic, 30.0, if on { k.info } else { k.faint })).padding(8).style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(k.panel2)), border: iced::Border { color: k.line, width: 1.0, radius: 8.0.into() }, ..Default::default() }),
        text(label).size(12).color(if on { k.text } else { k.faint }).align_x(iced::alignment::Horizontal::Center).width(Fill),
    ].spacing(4).align_x(Alignment::Center).width(96);
    match msg {
        Some(m) => button(c).style(theme::ghost).padding(4).on_press(m).into(),
        None => container(c).padding(4).into(),
    }
}
fn arrow<'a>() -> Element<'a, Message> { container(icon("arrow", 22.0, t().faint)).padding(Padding { top: 18.0, ..Padding::ZERO }).into() }
fn gap<'a>() -> Element<'a, Message> { Space::new().width(30).into() }

fn band<'a>(name: &'static str, rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let k = t();
    let head = container(text(name).size(12).font(BOLD).color(k.accent_ink)).padding([5, 10]).width(Fill)
        .style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.info)), border: iced::Border { radius: iced::border::Radius { top_left: 6.0, top_right: 6.0, bottom_left: 0.0, bottom_right: 0.0 }, ..Default::default() }, ..Default::default() });
    container(column![head, container(column(rows).spacing(6)).padding([10, 14])]).width(Fill).style(theme::card).into()
}

pub fn home(app: &App) -> Element<'_, Message> {
    let k = t();
    let tab = |label: &'static str, on: bool, v: bool| -> Element<'_, Message> {
        column![
            button(text(label).size(14).font(if on { BOLD } else { iced::Font::DEFAULT })).style(theme::tab(on)).padding([6, 12]).on_press(Message::Desk(Msg::Insights(v))),
            container(Space::new()).width(Fill).height(2).style(move |_: &iced::Theme| container::Style { background: if on { Some(iced::Background::Color(k.accent)) } else { None }, ..Default::default() }),
        ].width(iced::Shrink).into()
    };
    let tabs = row![tab("Home Page", !app.desk.insights, false), tab("Insights", app.desk.insights, true)].spacing(4);
    if app.desk.insights {
        return column![container(tabs).padding(Padding { top: 10.0, left: 20.0, ..Padding::ZERO }), crate::dashboard::view(app)].into();
    }
    let vendors = band("VENDORS", vec![
        row![node("box", "Purchase Orders", doc_new("PO")), arrow(), node("truck", "Receive Inventory", None), arrow(), node("expenses", "Enter Bills Against Inventory", None), gap(), node("taxes", "Manage Sales Tax", nav(Route::Taxes))].align_y(Alignment::Start).into(),
        row![node("expenses", "Enter Bills", doc_new("ACCPAY")), arrow(), node("check-write", "Pay Bills", nav(Route::PayBills)), gap(), node("file", "1099 Summary", rep("contractor-1099"))].align_y(Alignment::Start).into(),
    ]);
    let customers = band("CUSTOMERS", vec![
        row![node("file", "Estimates", doc_new("QUOTE")), arrow(), node("sales", "Create Invoices", doc_new("ACCREC")), arrow(), node("receive", "Receive Payments", nav(Route::ReceivePayment(None))), arrow(), node("deposit", "Record Deposits", nav(Route::Deposit))].align_y(Alignment::Start).into(),
        row![node("receive", "Create Sales Receipts", doc_new("SALESRECEIPT")), gap(), node("list", "Statement Charges", doc_new("DELAYEDCHARGE")), arrow(), node("docs", "Statements", None), gap(), node("refresh", "Refunds & Credits", doc_new("ACCRECCREDIT"))].align_y(Alignment::Start).into(),
    ]);
    let employees = band("EMPLOYEES", vec![
        row![node("payroll", "Payroll Center", nav(Route::Payroll)), gap(), node("check-write", "Pay Employees", nav(Route::Payroll)), arrow(), node("taxes", "Pay Liabilities", None), arrow(), node("docs", "Process Payroll Forms", None)].align_y(Alignment::Start).into(),
    ]);
    let company = band("COMPANY", vec![
        row![node("accounting", "Chart of Accounts", nav(Route::Chart)), node("box", "Items & Services", nav(Route::Products))].into(),
        row![node("calendar", "Calendar", None), node("mileage", "Vehicle Mileage", nav(Route::Mileage))].into(),
    ]);
    let banking = band("BANKING", vec![
        row![node("deposit", "Record Deposits", nav(Route::Deposit)), node("reconcile", "Reconcile", nav(Route::Reconcile))].into(),
        row![node("check-write", "Write Checks", nav(Route::Check(None))), node("register", "Check Register", nav(Route::Register(None)))].into(),
        row![node("card", "Enter Credit Card Charges", nav(Route::Spend(None))), node("swap", "Transfer Funds", nav(Route::Transfer))].into(),
    ]);
    let body = row![
        column![vendors, customers, employees].spacing(12).width(Length::FillPortion(3)),
        column![company, banking].spacing(12).width(Length::FillPortion(1)),
    ].spacing(12);
    scrollable(column![tabs, body].spacing(12).padding(Padding { top: 10.0, left: 20.0, right: 20.0, bottom: 24.0 }).max_width(1400)).style(theme::scroll).height(Fill).into()
}
