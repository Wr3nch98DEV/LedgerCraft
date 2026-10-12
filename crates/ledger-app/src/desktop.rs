//! QuickBooks Desktop-style layout (modelled on Enterprise 2024): menu bar, left icon bar with
//! My Shortcuts / View Balances / Open Windows, several open windows shown as tabs, and the Home
//! page workflow diagram. Screens themselves are shared with the other layouts unless a
//! Desktop-specific version exists.

use crate::app::{App, Message, Route};
use crate::icons::icon;
use crate::theme::{self, t};
use crate::ui::{self, BOLD};
use crate::{doc, forms, payments};
use iced::widget::{Space, button, column, container, mouse_area, opaque, row, scrollable, stack, text, text_input};
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
}

#[derive(Debug, Clone)]
pub enum Msg { Activate(usize), Close(usize), CloseAll, Pane(Pane), Insights(bool), Exit }

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
        Route::Report(k) => crate::reports::name(app.skin, k, "Report"),
        Route::Settings => "Preferences".into(),
        Route::Reconcile => "Reconcile".into(),
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
fn menu_w(name: &str) -> f32 { name.len() as f32 * 6.9 + 18.0 }

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
            e("Use Register", "Ctrl+R", nav(Route::Banking(None))), e("Make Deposits", "", nav(Route::Deposit)), e("Transfer Funds", "", nav(Route::Transfer)), e("Reconcile", "", nav(Route::Reconcile)),
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
        r = r.push(button(text(m).size(13)).padding([4, 0]).width(Length::Fixed(menu_w(m)))
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
        row![node("check-write", "Write Checks", nav(Route::Check(None))), node("register", "Check Register", nav(Route::Banking(None)))].into(),
        row![node("card", "Enter Credit Card Charges", nav(Route::Spend(None))), node("swap", "Transfer Funds", nav(Route::Transfer))].into(),
    ]);
    let body = row![
        column![vendors, customers, employees].spacing(12).width(Length::FillPortion(3)),
        column![company, banking].spacing(12).width(Length::FillPortion(1)),
    ].spacing(12);
    scrollable(column![tabs, body].spacing(12).padding(Padding { top: 10.0, left: 20.0, right: 20.0, bottom: 24.0 }).max_width(1400)).style(theme::scroll).height(Fill).into()
}
