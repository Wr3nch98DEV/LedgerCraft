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
        Route::Templates => crate::desktop::templates(app),
        Route::PriceLevels => crate::desktop::price_levels(app),
        Route::PrintForms => crate::desktop::print_forms(app),
        Route::PayrollForms => crate::desktop::payroll_forms(app),
        Route::CustomizeFavorites => crate::desktop::customize_favorites(app),
        Route::BuildAssemblies => crate::desktop::build_assemblies(app),
        Route::InventoryCenter => crate::desktop::inventory_center(app),
        Route::BatchInvoices => crate::desktop::batch_invoices(app),
        Route::Reminders => crate::desktop::reminders(app),
        Route::Find => crate::desktop::find(app),
        Route::Apps => apps(app),
        Route::FixedAssets => crate::extras::fixed_assets(app),
        Route::AuditLog => crate::extras::audit_log(app),
        Route::Currencies => crate::extras::currencies(app),
        Route::Budgets => crate::extras::budgets(app),
        Route::ExpenseClaims => crate::extras::claims(app, false),
        Route::BankRules => crate::extras::bank_rules(app),
        Route::ImportExport => crate::extras::import_export(app),
        Route::CustomReports => crate::extras::custom_reports(app),
        Route::EnterTime => crate::desktop::enter_time(app),
        Route::Doc { .. } => doc::view(app),
        Route::ReceivePayment(_) | Route::PayBills | Route::Spend(_) | Route::Check(_) | Route::BankTx(_) | Route::Deposit => payments::view(app),
        Route::NewContact(_) | Route::EditContact(_) | Route::EditItem(_) | Route::EditAccount(_) | Route::NewProduct | Route::NewAccount | Route::Journal | Route::Transfer => forms::view(app),
        Route::Payroll if app.desktop() => crate::desktop::employee_center(app),
        Route::Payroll => people::payroll(app),
        Route::PayRun(_) => people::pay_run(app),
        Route::Taxes => people::taxes(app),
        Route::TaxReturn(_) => people::tax_return(app),
        Route::Projects => people::projects(app),
        Route::Mileage => crate::extras::claims(app, true),
        Route::Setup => setup::view(app),
        Route::Login => setup::login(app),
    }
}

/// Apps: what LedgerCraft connects to, and where each connection is set up.
fn apps(app: &App) -> Element<'_, Message> {
    let card = |title: &'static str, body: &'static str, action: Option<(&'static str, Message)>| -> Element<'_, Message> {
        let mut c = column![ui::h2(title), ui::muted(body)].spacing(6);
        if let Some((label, m)) = action { c = c.push(row![ui::btn(label, Some(m))]); }
        ui::card(c)
    };
    ui::page(ui::h1(if app.skin == Skin::Xero { "Connected apps" } else { "Apps" }), vec![], column![
        card("Bank statements", "Import OFX, QIF, CSV or QBO statement files; bank rules and suggested matches do the rest. Live bank feeds need an aggregator plugin (Basiq, Plaid, GoCardless).", Some(("Open banking", Message::Nav(Route::Banking(None))))),
        card("Email", "Send invoices, statements and reminders from your own mail server or a Gmail app password.", Some(("Email settings", Message::Nav(Route::Settings)))),
        card("Online payments", "Pay-now buttons on invoices come from a payment-gateway plugin (Stripe, PayPal…) — see PLUGINS.md in the LedgerCraft folder.", None),
        card("REST API", "Other software can read and write your books through LedgerCraft's API with a token — see API.md.", None),
        card("Import your books", "Bring customers, suppliers, items, the chart of accounts and history across from your previous software (CSV today; direct imports from each product are on the plan).", Some(("Import", Message::Nav(Route::ImportExport)))),
    ].spacing(14).max_width(900).into())
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
        Route::Settings => "accountant",
        Route::Projects => "projects",
        Route::Mileage => "mileage",
        Route::Chart | Route::Journal | Route::Reconcile | Route::NewAccount => "accounting",
        Route::Register(_) => "banking",
        Route::Calendar => "dashboard",
        Route::NewContact(true) | Route::NewProduct => "sales",
        Route::NewContact(false) => "expenses",
        Route::Apps => "apps",
        _ => {
            // Anything else: the sidebar section the registry puts it in.
            let here = format!("{:?}", Message::Nav(r.clone()));
            crate::features::FEATURES.iter().find(|f| format!("{:?}", (f.go)()) == here).map(|f| f.online.0)
                .filter(|p| matches!(*p, "sales" | "expenses" | "payroll" | "taxes" | "reports" | "accounting")).unwrap_or("")
        }
    }
}

fn qb_sidebar<'a>(app: &'a App) -> Element<'a, Message> {
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
    let here = format!("{:?}", Message::Nav(app.route.clone()));
    let sub = |label: &'static str, msg: Message| -> Element<'_, Message> {
        let active = format!("{msg:?}") == here;
        button(text(label).size(13).font(if active { BOLD } else { iced::Font::DEFAULT })).style(theme::nav_item(active)).padding(Padding { top: 6.0, bottom: 6.0, left: 45.0, right: 10.0 }).width(Fill).on_press(msg).into()
    };
    // Each expandable section lists its features from the registry.
    let subs = |nav: iced::widget::Column<'a, Message>, key: &str| -> iced::widget::Column<'a, Message> {
        let mut nav = nav;
        if sec == key { for (label, m) in crate::features::online(key) { if let Some(m) = m { nav = nav.push(sub(label, m)); } } }
        nav
    };
    let mut nav = column![].spacing(2);
    nav = nav.push(item("home", "Dashboard", "dashboard", Route::Dashboard, false));
    nav = nav.push(item("bank", "Banking", "banking", Route::Banking(None), false));
    nav = nav.push(item("sales", "Sales", "sales", Route::AllSales, true));
    nav = subs(nav, "sales");
    nav = nav.push(item("expenses", "Expenses", "expenses", Route::Expenses, true));
    nav = subs(nav, "expenses");
    nav = nav.push(item("projects", "Projects", "projects", Route::Projects, false));
    nav = nav.push(item("payroll", "Payroll", "payroll", Route::Payroll, true));
    nav = subs(nav, "payroll");
    nav = nav.push(item("reports", "Reports", "reports", Route::Reports, true));
    nav = subs(nav, "reports");
    nav = nav.push(item("taxes", "Taxes", "taxes", Route::Taxes, true));
    nav = subs(nav, "taxes");
    nav = nav.push(item("mileage", "Mileage", "mileage", Route::Mileage, false));
    nav = nav.push(item("accounting", "Accounting", "accounting", Route::Chart, true));
    nav = subs(nav, "accounting");
    nav = nav.push(item("users", "My accountant", "accountant", Route::Settings, false));
    nav = nav.push(item("apps", "Apps", "apps", Route::Apps, false));

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
        top_icon("help", Message::Nav(Route::Help)),
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
        Skin::QuickBooks => {
            let col = |title: &'static str, extra: Vec<(&'static str, Option<Message>)>| { let mut v = crate::features::online(&format!("new:{title}")); v.extend(extra); menu_col(title, v) };
            row![
                col("Customers", vec![("Refund receipt", doc_route("ACCRECCREDIT"))]),
                col("Vendors", vec![]),
                col("Employees", vec![("Payroll", Some(Message::Nav(Route::Payroll))), ("Weekly timesheet", Some(Message::Nav(Route::EnterTime)))]),
                col("Other", vec![("Pay down credit card", Some(Message::Nav(Route::Transfer))), ("Contact", Some(Message::Nav(Route::NewContact(true))))]),
            ].spacing(18).into()
        }
        Skin::Xero => {
            let mut items = crate::features::xero("new");
            items.insert(2, ("Contact", Some(Message::Nav(Route::NewContact(true)))));
            columns_of("Create new", items, 9)
        }
    };
    container(body).padding(16).style(theme::menu).into()
}

fn gear_menu(app: &App) -> Element<'_, Message> {
    let other = if app.skin == Skin::QuickBooks { Skin::Xero } else { Skin::QuickBooks };
    let profile_items = vec![
            (if other == Skin::Xero { "Switch to Xero-style layout" } else { "Switch to QuickBooks Online-style layout" }, Some(Message::SetSkin(other))),
            ("Switch to QuickBooks Desktop-style layout", Some(Message::SetLayout(crate::theme::Layout::Desktop))),
            (if app.dark { "Light mode" } else { "Dark mode" }, Some(Message::ToggleTheme)),
            ("Switch company", Some(Message::Nav(Route::Setup))),
        ];
    if app.skin == Skin::Xero {
        // Xero's organisation settings menu.
        let body = row![columns_of("Settings", crate::features::xero("settings"), 10), menu_col("Profile", profile_items)].spacing(18);
        return container(body).padding(16).style(theme::menu).into();
    }
    let mut lists = crate::features::online("gear:Lists");
    lists.insert(0, ("Products and services", Some(Message::Nav(Route::Products))));
    let mut company = crate::features::online("gear:Your company");
    company.push(("Chart of accounts", Some(Message::Nav(Route::Chart))));
    let body = row![
        menu_col("Your company", company),
        menu_col("Lists", lists),
        menu_col("Tools", crate::features::online("gear:Tools")),
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

fn xero_menu_left(m: &str) -> f32 { match m { "business" => 250.0, "accounting" => 345.0, "payroll" => 460.0, "contacts" => 555.0, _ => 200.0 } }

/// A menu with long lists split into side-by-side columns.
fn columns_of<'a>(title: &'a str, items: Vec<(&'a str, Option<Message>)>, per: usize) -> Element<'a, Message> {
    let mut r = row![].spacing(14);
    for (n, chunk) in items.chunks(per).enumerate() { r = r.push(menu_col(if n == 0 { title } else { " " }, chunk.to_vec())); }
    r.into()
}

fn xero_menu(_app: &App, m: &'static str) -> Element<'static, Message> {
    let mut items = crate::features::xero(m);
    if m == "contacts" { items.insert(0, ("All contacts", Some(Message::Nav(Route::Contacts)))); }
    let title = match m { "business" => "Business", "accounting" => "Accounting", "payroll" => "Payroll", _ => "Contacts" };
    container(columns_of(title, items, 10)).padding(14).style(theme::menu).into()
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
        tab("Payroll", "payroll", Message::OpenMenu(Some("payroll")), true),
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
