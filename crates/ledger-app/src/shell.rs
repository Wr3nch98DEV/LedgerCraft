//! The window frame for each layout: navigation, top bar, menus, toasts.

use crate::app::{App, Message, Route};
use crate::icons::icon;
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD};
use crate::{banking, dashboard, doc, forms, lists, payments, people, reports, settings, setup};
use iced::widget::{Space, button, column, container, mouse_area, opaque, row, scrollable, stack, text, text_input};
use iced::{Alignment, Element, Fill, Length, Padding};

pub fn view(app: &App) -> Element<'_, Message> {
    if app.route == Route::Login {
        return with_toasts(app, container(setup::login(app)).style(theme::page).width(Fill).height(Fill).into());
    }
    if app.route == Route::Setup {
        return with_toasts(app, container(setup::view(app)).style(theme::page).width(Fill).height(Fill).into());
    }
    if app.desktop() {
        let content = container(page(app)).style(theme::page).width(Fill).height(Fill);
        return with_toasts(app, crate::desktop::frame(app, content.into()));
    }
    let content = container(page(app)).style(theme::page).width(Fill).height(Fill);
    let base: Element<Message> = match app.skin {
        Skin::QuickBooks => row![qb_sidebar(app), column![qb_topbar(app), content]].into(),
        Skin::Xero => column![xero_topbar(app), content].into(),
    };
    let mut layers = stack![base].width(Fill).height(Fill);
    if !app.search_results.is_empty() && !app.search.is_empty() {
        layers = layers.push(mouse_area(container(Space::new()).width(Fill).height(Fill)).on_press(Message::CloseMenus));
        let k = t();
        let list = column(app.search_results.iter().map(|r| {
            let entity = r["entity"].as_str().unwrap_or("").to_string();
            let label = match entity.as_str() { "contact" => "Contact", "document" => "Transaction", "account" => "Account", "item" => "Product", "project" => "Project", "banktx" => "Bank", "manual_journal" => "Journal", _ => "Record" };
            button(row![
                text(label.to_uppercase()).size(10).font(BOLD).color(k.faint).width(84),
                column![text(r["title"].as_str().unwrap_or("").to_string()).size(14), text(r["snippet"].as_str().unwrap_or("").replace(['[', ']'], "")).size(12).color(k.muted)].spacing(1),
            ].spacing(8).align_y(Alignment::Center)).style(theme::menu_item).padding([6, 8]).width(Fill)
                .on_press(Message::OpenResult(entity, r["id"].as_i64().unwrap_or(0))).into()
        })).spacing(2);
        let panel = container(column![list, ui::muted("Press Enter to open the first result")].spacing(6)).padding(10).width(460).style(theme::menu);
        let pad = match app.skin { Skin::QuickBooks => Padding { top: 54.0, right: 170.0, ..Padding::ZERO }, Skin::Xero => Padding { top: 54.0, right: 120.0, ..Padding::ZERO } };
        layers = layers.push(container(opaque(panel)).padding(pad).width(Fill).align_x(iced::alignment::Horizontal::Right));
    }
    if app.new_menu || app.gear_menu || app.menu.is_some() {
        layers = layers.push(mouse_area(container(Space::new()).width(Fill).height(Fill)).on_press(Message::CloseMenus));
        let (menu, pad) = match (app.skin, app.new_menu, app.gear_menu, app.menu) {
            (Skin::QuickBooks, true, _, _) => (new_menu(app), Padding { top: 104.0, left: 14.0, ..Padding::ZERO }),
            (Skin::Xero, true, _, _) => (new_menu(app), Padding { top: 56.0, left: 320.0, ..Padding::ZERO }),
            (_, _, true, _) => (gear_menu(app), Padding { top: 56.0, left: 180.0, ..Padding::ZERO }),
            (_, _, _, Some(m)) => (xero_menu(app, m), Padding { top: 56.0, left: xero_menu_left(m), ..Padding::ZERO }),
            _ => (Space::new().into(), Padding::ZERO),
        };
        let anchor: Element<Message> = if app.gear_menu {
            container(opaque(menu)).padding(Padding { top: 56.0, right: 16.0, ..Padding::ZERO }).width(Fill).align_x(iced::alignment::Horizontal::Right).into()
        } else {
            container(opaque(menu)).padding(pad).into()
        };
        layers = layers.push(anchor);
    }
    with_toasts(app, layers.into())
}

fn with_toasts<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    if app.toasts.is_empty() { return base; }
    let list = column(app.toasts.iter().map(|(id, msg, err)| {
        let k = t();
        container(row![
            container(Space::new()).width(4).height(20).style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(if *err { k.bad } else { k.ok })), ..Default::default() }),
            text(msg.clone()).size(14).width(Length::Fixed(340.0)),
            button(icon("x", 14.0, k.muted)).style(theme::ghost).padding(4).on_press(Message::DropToast(*id)),
        ].spacing(10).align_y(Alignment::Center)).padding([10, 12]).style(theme::toast).into()
    })).spacing(8);
    // Toasts don't block clicks (only their × does). Desktop forms keep their buttons bottom-right,
    // so there the toasts show top-right, under the window tabs.
    let (v, pad) = if app.desktop() { (iced::alignment::Vertical::Top, Padding { top: 72.0, right: 20.0, bottom: 0.0, left: 0.0 }) } else { (iced::alignment::Vertical::Bottom, Padding::from(20)) };
    stack![base, container(list).width(Fill).height(Fill).align_x(iced::alignment::Horizontal::Right).align_y(v).padding(pad)].into()
}

/// Routes to the screen for the current route.
fn page(app: &App) -> Element<'_, Message> {
    match &app.route {
        Route::Dashboard if app.desktop() => crate::desktop::home(app),
        Route::Dashboard => dashboard::view(app),
        Route::AllSales | Route::Invoices if app.desktop() => crate::desktop::tracker(app, true),
        Route::Bills | Route::Expenses if app.desktop() => crate::desktop::tracker(app, false),
        Route::Quotes | Route::CreditNotes | Route::PurchaseOrders if app.desktop() => crate::desktop::doc_list(app),
        Route::AllSales | Route::Invoices | Route::Quotes | Route::CreditNotes | Route::Bills | Route::Expenses | Route::PurchaseOrders => lists::docs(app),
        Route::Customers | Route::Contacts if app.desktop() => crate::desktop::center(app, true),
        Route::Vendors if app.desktop() => crate::desktop::center(app, false),
        Route::Customers | Route::Vendors | Route::Contacts => lists::contacts(app),
        Route::Contact(_) => lists::contact(app),
        Route::Products if app.desktop() => crate::desktop::item_list(app),
        Route::Products => lists::products(app),
        Route::Chart if app.desktop() => crate::desktop::chart(app),
        Route::Chart => lists::chart(app),
        Route::Banking(_) => banking::view(app),
        Route::Reports if app.desktop() => crate::desktop::report_center(app),
        Route::Reports => reports::centre(app),
        Route::Report(_) => reports::view(app),
        Route::Settings => settings::view(app),
        Route::Reconcile => crate::reconcile::view(app),
        Route::Register(_) => crate::desktop::register(app),
        Route::Calendar => crate::desktop::calendar(app),
        Route::Statements => crate::desktop::statements(app),
        Route::Memorized => crate::desktop::memorized(app),
        Route::PaySalesTax => crate::desktop::pay_sales_tax(app),
        Route::AdjustInventory => crate::desktop::adjust_inventory(app),
        Route::Help => crate::desktop::help(app),
        Route::MemorizedReports => crate::desktop::memorized_reports(app),
        Route::DocCenter => crate::desktop::doc_center(app),
        Route::PayLiabilities => crate::desktop::pay_liabilities(app),
        Route::ClassList => crate::desktop::class_list(app),
        Route::PayrollItems => crate::desktop::payroll_items(app),
        Route::OtherNames => crate::desktop::other_names(app),
        Route::FinanceCharges => crate::desktop::finance_charges(app),
        Route::EnterTime => crate::desktop::enter_time(app),
        Route::Doc { .. } => doc::view(app),
        Route::ReceivePayment(_) | Route::PayBills | Route::Spend(_) | Route::Check(_) | Route::Deposit => payments::view(app),
        Route::NewContact(_) | Route::NewProduct | Route::NewAccount | Route::Journal | Route::Transfer => forms::view(app),
        Route::Payroll if app.desktop() => crate::desktop::employee_center(app),
        Route::Payroll => people::payroll(app),
        Route::PayRun(_) => people::pay_run(app),
        Route::Taxes => people::taxes(app),
        Route::TaxReturn(_) => people::tax_return(app),
        Route::Projects => people::projects(app),
        Route::Mileage => people::mileage(app),
        Route::Placeholder(name) => placeholder(app, name),
        Route::Setup => setup::view(app),
        Route::Login => setup::login(app),
    }
}

fn placeholder<'a>(_app: &'a App, name: &'a str) -> Element<'a, Message> {
    ui::page(ui::h1(name.to_string()), vec![], ui::card(column![
        ui::h2("Not in the native app yet"),
        ui::muted(format!("{name} works in the current web version (start.bat web) and is scheduled for a later phase of the native rewrite. Your data is shared, so anything you do there shows up here.")),
        ui::gap(6.0),
        ui::btn("Back", Some(Message::Back)),
    ].spacing(8)))
}

// ===================== QuickBooks-style =====================

/// Which sidebar section a route belongs to.
fn qb_section(r: &Route) -> &'static str {
    match r {
        Route::Dashboard => "dashboard",
        Route::Banking(_) | Route::Transfer | Route::Deposit => "banking",
        Route::AllSales | Route::Invoices | Route::Quotes | Route::CreditNotes | Route::Customers | Route::Products | Route::ReceivePayment(_) => "sales",
        Route::Expenses | Route::Bills | Route::Vendors | Route::PurchaseOrders | Route::PayBills | Route::Spend(_) | Route::Check(_) => "expenses",
        Route::Doc { kind, .. } => if matches!(kind.as_str(), "ACCREC" | "QUOTE" | "ACCRECCREDIT") { "sales" } else { "expenses" },
        Route::Contact(_) | Route::Contacts => "sales",
        Route::Reports | Route::Report(_) => "reports",
        Route::Payroll | Route::PayRun(_) => "payroll",
        Route::Taxes | Route::TaxReturn(_) => "taxes",
        Route::Projects => "projects",
        Route::Mileage => "mileage",
        Route::Chart | Route::Journal | Route::Reconcile | Route::NewAccount => "accounting",
        Route::Register(_) => "banking",
        Route::Calendar => "dashboard",
        Route::NewContact(true) | Route::NewProduct => "sales",
        Route::NewContact(false) => "expenses",
        Route::Placeholder(p) => match *p { "Projects" => "projects", "Payroll" => "payroll", "Taxes" => "taxes", "Mileage" => "mileage", "My accountant" => "accountant", "Apps" => "apps", _ => "" },
        _ => "",
    }
}

fn qb_sidebar(app: &App) -> Element<'_, Message> {
    let k = t();
    let sec = qb_section(&app.route);
    let item = |ic: &'static str, label: &'static str, key: &'static str, route: Route, expandable: bool| -> Element<'_, Message> {
        let active = sec == key;
        let mut r = row![icon(ic, 18.0, if active { k.nav_active } else { k.nav_muted }), text(label).size(14).font(if active { BOLD } else { iced::Font::DEFAULT })].spacing(12).align_y(Alignment::Center);
        if expandable { r = r.push(ui::hspace()).push(icon(if active { "down" } else { "chevron" }, 14.0, k.nav_muted)); }
        let b = button(r).style(theme::nav_item(active)).padding([9, 12]).width(Fill).on_press(Message::Nav(route));
        if active {
            row![container(Space::new()).width(3).height(36).style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.nav_active)), ..Default::default() }), b].align_y(Alignment::Center).into()
        } else { row![Space::new().width(3), b].into() }
    };
    let sub = |label: &'static str, route: Route| -> Element<'_, Message> {
        let active = app.route == route;
        button(text(label).size(13).font(if active { BOLD } else { iced::Font::DEFAULT })).style(theme::nav_item(active)).padding(Padding { top: 6.0, bottom: 6.0, left: 45.0, right: 10.0 }).width(Fill).on_press(Message::Nav(route)).into()
    };
    let mut nav = column![].spacing(2);
    nav = nav.push(item("home", "Dashboard", "dashboard", Route::Dashboard, false));
    nav = nav.push(item("bank", "Banking", "banking", Route::Banking(None), false));
    nav = nav.push(item("sales", "Sales", "sales", Route::AllSales, true));
    if sec == "sales" {
        nav = nav.push(sub("All sales", Route::AllSales)).push(sub("Invoices", Route::Invoices)).push(sub("Estimates", Route::Quotes))
            .push(sub("Customers", Route::Customers)).push(sub("Products and services", Route::Products));
    }
    nav = nav.push(item("expenses", "Expenses", "expenses", Route::Expenses, true));
    if sec == "expenses" {
        nav = nav.push(sub("Expenses", Route::Expenses)).push(sub("Bills", Route::Bills)).push(sub("Vendors", Route::Vendors)).push(sub("Purchase orders", Route::PurchaseOrders));
    }
    nav = nav.push(item("projects", "Projects", "projects", Route::Projects, false));
    nav = nav.push(item("payroll", "Payroll", "payroll", Route::Payroll, false));
    nav = nav.push(item("reports", "Reports", "reports", Route::Reports, false));
    nav = nav.push(item("taxes", "Taxes", "taxes", Route::Taxes, false));
    nav = nav.push(item("mileage", "Mileage", "mileage", Route::Mileage, false));
    nav = nav.push(item("accounting", "Accounting", "accounting", Route::Chart, true));
    if sec == "accounting" {
        nav = nav.push(sub("Chart of accounts", Route::Chart)).push(sub("Reconcile", Route::Reconcile));
    }
    nav = nav.push(item("users", "My accountant", "accountant", Route::Placeholder("My accountant"), false));
    nav = nav.push(item("apps", "Apps", "apps", Route::Placeholder("Apps"), false));

    let brand = row![
        container(text("L").size(15).font(BOLD).color(k.accent_ink)).width(28).height(28).center_x(28).center_y(28)
            .style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.accent)), border: iced::Border { radius: 6.0.into(), ..Default::default() }, ..Default::default() }),
        text("LedgerCraft").size(16).font(BOLD).color(k.nav_text),
    ].spacing(10).align_y(Alignment::Center);
    let new_btn = button(row![icon("plus", 16.0, k.nav_text), text("New").size(14).font(BOLD).color(k.nav_text)].spacing(8).align_y(Alignment::Center))
        .padding([8, 18])
        .style(move |_, s| {
            let hover = matches!(s, button::Status::Hovered | button::Status::Pressed);
            button::Style { background: Some(iced::Background::Color(if hover { k.nav_hover } else { iced::Color::TRANSPARENT })), text_color: k.nav_text,
                border: iced::Border { color: k.nav_muted, width: 1.0, radius: 999.0.into() }, shadow: Default::default(), snap: true }
        })
        .on_press(Message::ToggleNew);
    container(column![
        container(brand).padding([16, 16]),
        container(new_btn).padding(Padding { top: 0.0, bottom: 14.0, left: 14.0, right: 14.0 }),
        scrollable(nav.padding(Padding { right: 8.0, ..Padding::ZERO })).style(theme::scroll).height(Fill),
    ])
    .width(224)
    .height(Fill)
    .style(theme::nav)
    .into()
}

fn top_icon<'a>(name: &str, msg: Message) -> Element<'a, Message> {
    button(icon(name, 19.0, t().muted)).style(theme::ghost).padding(7).on_press(msg).into()
}

fn avatar<'a>(app: &App) -> Element<'a, Message> {
    let k = t();
    let initials: String = app.user_name().split_whitespace().filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase();
    container(text(if initials.is_empty() { "?".into() } else { initials }).size(12).font(BOLD).color(k.accent_ink)).width(30).height(30).center_x(30).center_y(30)
        .style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.accent)), border: iced::Border { radius: 999.0.into(), ..Default::default() }, ..Default::default() })
        .into()
}

fn qb_topbar(app: &App) -> Element<'_, Message> {
    let search = text_input("Navigate or search for transactions, contacts, reports…", &app.search)
        .on_input(Message::Search).on_submit(Message::SearchGo).padding([7, 12]).size(14).width(Length::Fixed(380.0)).style(theme::input);
    container(row![
        text(app.org_name()).size(15).font(BOLD),
        ui::hspace(),
        search,
        top_icon("help", Message::Nav(Route::Placeholder("Help"))),
        top_icon("bell", Message::Nav(Route::Dashboard)),
        top_icon(if app.dark { "sun" } else { "moon" }, Message::ToggleTheme),
        top_icon("gear", Message::ToggleGear),
        avatar(app),
    ].spacing(8).align_y(Alignment::Center).padding([0, 20]))
    .height(58)
    .center_y(58)
    .width(Fill)
    .style(theme::topbar)
    .into()
}

/// A column in a mega-menu.
fn menu_col<'a>(title: &'a str, items: Vec<(&'a str, Option<Message>)>) -> Element<'a, Message> {
    let k = t();
    let mut c = column![text(title.to_uppercase()).size(11).font(BOLD).color(k.muted)].spacing(2).width(190);
    c = c.push(ui::gap(4.0));
    for (label, msg) in items {
        let enabled = msg.is_some();
        let mut b = button(text(label.to_string()).size(14).color(if enabled { k.text } else { k.faint })).style(theme::menu_item).padding([6, 8]).width(Fill);
        if let Some(m) = msg { b = b.on_press(m); }
        c = c.push(b);
    }
    c.into()
}

fn doc_route(kind: &str) -> Option<Message> { Some(Message::Nav(Route::Doc { kind: kind.into(), id: None })) }

fn new_menu(app: &App) -> Element<'_, Message> {
    let body: Element<Message> = match app.skin {
        Skin::QuickBooks => row![
            menu_col("Customers", vec![("Invoice", doc_route("ACCREC")), ("Receive payment", Some(Message::Nav(Route::ReceivePayment(None)))), ("Estimate", doc_route("QUOTE")),
                ("Credit memo", doc_route("ACCRECCREDIT")), ("Sales receipt", doc_route("SALESRECEIPT")), ("Refund receipt", None), ("Delayed credit", doc_route("DELAYEDCREDIT")), ("Delayed charge", doc_route("DELAYEDCHARGE"))]),
            menu_col("Vendors", vec![("Expense", Some(Message::Nav(Route::Spend(None)))), ("Check", Some(Message::Nav(Route::Check(None)))), ("Bill", doc_route("ACCPAY")), ("Pay bills", Some(Message::Nav(Route::PayBills))),
                ("Purchase order", doc_route("PO")), ("Vendor credit", doc_route("ACCPAYCREDIT")), ("Credit card credit", None), ("Print checks", None)]),
            menu_col("Employees", vec![("Payroll", Some(Message::Nav(Route::Payroll))), ("Single time activity", None), ("Weekly timesheet", None)]),
            menu_col("Other", vec![("Bank deposit", Some(Message::Nav(Route::Deposit))), ("Transfer", Some(Message::Nav(Route::Transfer))), ("Journal entry", Some(Message::Nav(Route::Journal))), ("Statement", None), ("Inventory qty adjustment", None), ("Pay down credit card", None)]),
        ].spacing(18).into(),
        Skin::Xero => column![
            menu_col("Create new", vec![("Invoice", doc_route("ACCREC")), ("Bill", doc_route("ACCPAY")), ("Contact", Some(Message::Nav(Route::NewContact(true)))), ("Quote", doc_route("QUOTE")),
                ("Purchase order", doc_route("PO")), ("Credit note", doc_route("ACCRECCREDIT")), ("Spend money", Some(Message::Nav(Route::Spend(None)))), ("Manual journal", Some(Message::Nav(Route::Journal)))]),
        ].into(),
    };
    let foot = if app.skin == Skin::QuickBooks { ui::muted("Greyed-out items are planned for the next phase (see UI_REWRITE_PLAN.md).") } else { Space::new().into() };
    container(column![body, foot].spacing(10)).padding(16).style(theme::menu).into()
}

fn gear_menu(app: &App) -> Element<'_, Message> {
    let other = if app.skin == Skin::QuickBooks { Skin::Xero } else { Skin::QuickBooks };
    let body = row![
        menu_col("Your company", vec![("Account and settings", Some(Message::Nav(Route::Settings))), ("Manage users", Some(Message::Nav(Route::Settings))), ("Custom form styles", Some(Message::Nav(Route::Settings))), ("Chart of accounts", Some(Message::Nav(Route::Chart)))]),
        menu_col("Lists", vec![("All lists", None), ("Products and services", Some(Message::Nav(Route::Products))), ("Recurring transactions", None), ("Attachments", None)]),
        menu_col("Tools", vec![("Import data", None), ("Export data", None), ("Reconcile", Some(Message::Nav(Route::Reconcile))), ("Budgeting", None), ("Audit log", None)]),
        menu_col("Profile", vec![
            (if other == Skin::Xero { "Switch to Xero-style layout" } else { "Switch to QuickBooks Online-style layout" }, Some(Message::SetSkin(other))),
            ("Switch to QuickBooks Desktop-style layout", Some(Message::SetLayout(crate::theme::Layout::Desktop))),
            (if app.dark { "Light mode" } else { "Dark mode" }, Some(Message::ToggleTheme)),
            ("Switch company", Some(Message::Nav(Route::Setup))),
        ]),
    ].spacing(18);
    container(body).padding(16).style(theme::menu).into()
}

// ===================== Xero-style =====================

fn xero_section(r: &Route) -> &'static str {
    match r {
        Route::Dashboard => "dashboard",
        Route::Invoices | Route::Quotes | Route::CreditNotes | Route::Bills | Route::PurchaseOrders | Route::Products | Route::AllSales | Route::Expenses
        | Route::ReceivePayment(_) | Route::PayBills | Route::Doc { .. } => "business",
        Route::Banking(_) | Route::Reports | Route::Report(_) | Route::Chart | Route::Spend(_) | Route::Journal | Route::Reconcile | Route::NewAccount | Route::Register(_) => "accounting",
        Route::Calendar => "dashboard",
        Route::NewContact(_) => "contacts", Route::NewProduct => "business",
        Route::Payroll | Route::PayRun(_) => "payroll",
        Route::Taxes | Route::TaxReturn(_) => "accounting",
        Route::Projects | Route::Mileage => "business",
        Route::Contacts | Route::Customers | Route::Vendors | Route::Contact(_) => "contacts",
        _ => "",
    }
}

fn xero_menu_left(m: &str) -> f32 { match m { "business" => 250.0, "accounting" => 345.0, "contacts" => 540.0, _ => 200.0 } }

fn xero_menu(_app: &App, m: &'static str) -> Element<'static, Message> {
    let items: Vec<(&str, Option<Message>)> = match m {
        "business" => vec![("Invoices", Some(Message::Nav(Route::Invoices))), ("Quotes", Some(Message::Nav(Route::Quotes))), ("Credit notes", Some(Message::Nav(Route::CreditNotes))),
            ("Bills to pay", Some(Message::Nav(Route::Bills))), ("Purchase orders", Some(Message::Nav(Route::PurchaseOrders))), ("Products and services", Some(Message::Nav(Route::Products))),
            ("Projects", Some(Message::Nav(Route::Projects))), ("Expense claims", Some(Message::Nav(Route::Mileage)))],
        "accounting" => vec![("Bank accounts", Some(Message::Nav(Route::Banking(None)))), ("Reconcile account", Some(Message::Nav(Route::Reconcile))), ("Reports", Some(Message::Nav(Route::Reports))), ("Chart of accounts", Some(Message::Nav(Route::Chart))),
            ("Manual journal", Some(Message::Nav(Route::Journal))), ("Tax returns", Some(Message::Nav(Route::Taxes))), ("Fixed assets", None)],
        _ => vec![("All contacts", Some(Message::Nav(Route::Contacts))), ("Customers", Some(Message::Nav(Route::Customers))), ("Suppliers", Some(Message::Nav(Route::Vendors)))],
    };
    container(menu_col(match m { "business" => "Business", "accounting" => "Accounting", _ => "Contacts" }, items)).padding(14).style(theme::menu).into()
}

fn xero_topbar(app: &App) -> Element<'_, Message> {
    let k = t();
    let sec = xero_section(&app.route);
    let tab = |label: &'static str, key: &'static str, msg: Message, dropdown: bool| -> Element<'_, Message> {
        let active = sec == key;
        let mut r = row![text(label).size(14).font(if active { BOLD } else { iced::Font::DEFAULT })].spacing(4).align_y(Alignment::Center);
        if dropdown { r = r.push(icon("down", 12.0, k.nav_muted)); }
        button(r).style(theme::nav_item(active)).padding([8, 12]).on_press(msg).into()
    };
    let initial = app.org_name().chars().next().unwrap_or('L').to_string();
    container(row![
        container(text(initial).size(14).font(BOLD).color(k.accent_ink)).width(30).height(30).center_x(30).center_y(30)
            .style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.accent)), border: iced::Border { radius: 6.0.into(), ..Default::default() }, ..Default::default() }),
        button(text(app.org_name()).size(15).font(BOLD).color(k.nav_text)).style(theme::nav_item(false)).padding([6, 8]).on_press(Message::Nav(Route::Setup)),
        Space::new().width(10),
        tab("Dashboard", "dashboard", Message::Nav(Route::Dashboard), false),
        tab("Business", "business", Message::OpenMenu(Some("business")), true),
        tab("Accounting", "accounting", Message::OpenMenu(Some("accounting")), true),
        tab("Payroll", "payroll", Message::Nav(Route::Payroll), false),
        tab("Contacts", "contacts", Message::OpenMenu(Some("contacts")), true),
        ui::hspace(),
        button(row![icon("plus", 15.0, k.accent_ink), text("New").size(14).font(BOLD)].spacing(6).align_y(Alignment::Center)).style(theme::primary).padding([6, 12]).on_press(Message::ToggleNew),
        text_input("Search", &app.search).on_input(Message::Search).on_submit(Message::SearchGo).padding([6, 10]).size(13).width(Length::Fixed(220.0)).style(theme::input),
        button(icon(if app.dark { "sun" } else { "moon" }, 18.0, k.nav_muted)).style(theme::nav_item(false)).padding(7).on_press(Message::ToggleTheme),
        button(icon("gear", 18.0, k.nav_muted)).style(theme::nav_item(false)).padding(7).on_press(Message::ToggleGear),
        avatar(app),
    ].spacing(4).align_y(Alignment::Center).padding([0, 16]))
    .height(56)
    .center_y(56)
    .width(Fill)
    .style(theme::nav)
    .into()
}
