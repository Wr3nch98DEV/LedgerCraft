//! MYOB Acumatica-style layout: a main menu of workspaces down the left (Receivables, Payables,
//! Banking, Finance…). Each workspace opens a panel listing its screens under Transactions,
//! Processes, Inquiries, Reports and Preferences, in Acumatica's wording. The screens themselves
//! are the shared ones; this module is the navigation and frame.

use crate::app::{App, Message, Route};
use crate::icons::icon;
use crate::theme::{self, t};
use crate::ui::{self, BOLD};
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Fill, Length};

/// The workspaces in the main menu: (key, label, icon).
pub const WORKSPACES: [(&str, &str, &str); 12] = [
    ("receivables", "Receivables", "sales"), ("payables", "Payables", "expenses"), ("banking", "Banking", "bank"),
    ("sales_orders", "Sales Orders", "docs"), ("purchases", "Purchases", "truck"), ("inventory", "Inventory", "box"),
    ("projects", "Projects", "projects"), ("time_expenses", "Time and Expenses", "clock"), ("payroll", "Payroll", "payroll"),
    ("taxes", "Taxes", "taxes"), ("finance", "Finance", "accounting"), ("configuration", "Configuration", "gear"),
];

/// The groups inside a workspace, in Acumatica's order.
pub const SECTIONS: [&str; 6] = ["Transactions", "Processes", "Inquiries", "Profiles", "Reports", "Preferences"];

/// Where each feature (by registry id) lives: (feature, workspace, section, Acumatica's name for it).
pub static PLACES: &[(&str, &str, &str, &str)] = &[
    // Receivables
    ("invoice", "receivables", "Transactions", "Invoices and Memos"),
    ("receive_payment", "receivables", "Transactions", "Payments and Applications"),
    ("sales_receipt", "receivables", "Transactions", "Cash Sales"),
    ("credit_memo", "receivables", "Transactions", "Credit Memos"),
    ("delayed_charge", "receivables", "Transactions", "Statement Charges"),
    ("delayed_credit", "receivables", "Transactions", "Statement Credits"),
    ("batch_invoices", "receivables", "Processes", "Create Batch Invoices"),
    ("statements", "receivables", "Processes", "Prepare Statements"),
    ("finance_charges", "receivables", "Processes", "Calculate Overdue Charges"),
    ("print_forms", "receivables", "Processes", "Print Invoices and Memos"),
    ("payment_reminders", "receivables", "Preferences", "Dunning Levels"),
    ("invoices", "receivables", "Inquiries", "Invoices"),
    ("all_sales", "receivables", "Inquiries", "Customer Details"),
    ("credit_notes", "receivables", "Inquiries", "Credit Memos List"),
    ("customers", "receivables", "Profiles", "Customers"),
    // Payables
    ("bill", "payables", "Transactions", "Bills and Adjustments"),
    ("vendor_credit", "payables", "Transactions", "Debit Adjustments"),
    ("check", "payables", "Transactions", "Quick Checks"),
    ("expense", "payables", "Transactions", "Card Expenses"),
    ("pay_bills", "payables", "Processes", "Prepare Payments"),
    ("print_checks", "payables", "Processes", "Print Checks"),
    ("batch_payments", "payables", "Processes", "Batch Payments and Bank Files"),
    ("bills", "payables", "Inquiries", "Bills"),
    ("expenses", "payables", "Inquiries", "Vendor Details"),
    ("vendors", "payables", "Profiles", "Vendors"),
    ("contractor_1099", "payables", "Reports", "1099 Year Summary"),
    ("other_names", "payables", "Profiles", "Other Business Accounts"),
    // Banking
    ("banking", "banking", "Processes", "Process Bank Transactions"),
    ("deposit", "banking", "Transactions", "Bank Deposits"),
    ("transfer", "banking", "Transactions", "Funds Transfers"),
    ("reconcile", "banking", "Processes", "Reconcile Cash Account"),
    ("register", "banking", "Inquiries", "Cash Account Details"),
    ("bank_rules", "banking", "Preferences", "Bank Transaction Rules"),
    // Sales orders / purchases / inventory
    ("sales_order", "sales_orders", "Transactions", "Sales Orders"),
    ("estimate", "sales_orders", "Transactions", "Sales Quotes"),
    ("quotes", "sales_orders", "Inquiries", "Sales Quotes List"),
    ("price_levels", "sales_orders", "Preferences", "Sales Prices"),
    ("purchase_order", "purchases", "Transactions", "Purchase Orders"),
    ("receive_items", "purchases", "Transactions", "Purchase Receipts"),
    ("purchase_orders", "purchases", "Inquiries", "Purchase Orders List"),
    ("products", "inventory", "Profiles", "Stock and Non-Stock Items"),
    ("adjust_inventory", "inventory", "Transactions", "Adjustments"),
    ("build_assemblies", "inventory", "Transactions", "Kit Assembly"),
    ("inventory", "inventory", "Inquiries", "Inventory Summary"),
    ("locations", "inventory", "Profiles", "Warehouses"),
    ("lots", "inventory", "Inquiries", "Lot/Serial Numbers"),
    // Projects / time and expenses
    ("projects", "projects", "Profiles", "Projects"),
    ("enter_time", "time_expenses", "Transactions", "Time Entries"),
    ("expense_claims", "time_expenses", "Transactions", "Expense Claims"),
    ("mileage", "time_expenses", "Transactions", "Mileage Receipts"),
    // Payroll
    ("payroll", "payroll", "Profiles", "Employees"),
    ("pay_liabilities", "payroll", "Processes", "Pay Tax Liabilities"),
    ("payroll_forms", "payroll", "Reports", "Payroll Tax Forms"),
    ("payroll_settings", "payroll", "Preferences", "Payroll Preferences"),
    ("payroll_items", "payroll", "Preferences", "Earning and Deduction Codes"),
    // Taxes
    ("taxes", "taxes", "Processes", "Prepare Tax Report"),
    ("pay_sales_tax", "taxes", "Transactions", "Tax Payments"),
    // Finance
    ("journal", "finance", "Transactions", "Journal Transactions"),
    ("recode", "finance", "Processes", "Reclassify Transactions"),
    ("revenue_recognition", "finance", "Processes", "Deferral Schedules"),
    ("chart", "finance", "Profiles", "Chart of Accounts"),
    ("budgets", "finance", "Transactions", "Budgets"),
    ("currencies", "finance", "Preferences", "Currencies and Rates"),
    ("fixed_assets", "finance", "Profiles", "Fixed Assets"),
    ("closing_date", "finance", "Processes", "Close Financial Periods"),
    ("classes", "finance", "Preferences", "Subaccounts and Tracking"),
    ("recurring", "finance", "Processes", "Recurring Transactions"),
    ("reports", "finance", "Reports", "Financial Reports"),
    ("memorized_reports", "finance", "Reports", "Saved Report Versions"),
    ("custom_reports", "finance", "Reports", "Report Designer"),
    ("insights", "finance", "Inquiries", "Company Dashboard"),
    ("find", "finance", "Inquiries", "Transaction Search"),
    // Configuration
    ("settings", "configuration", "Preferences", "Company and Preferences"),
    ("users", "configuration", "Profiles", "Users and Roles"),
    ("audit_log", "configuration", "Inquiries", "Audit History"),
    ("import_data", "configuration", "Processes", "Import Scenarios"),
    ("export_data", "configuration", "Processes", "Export Scenarios"),
    ("templates", "configuration", "Preferences", "Form Templates"),
    ("custom_fields", "configuration", "Preferences", "User-Defined Fields"),
    ("backup", "configuration", "Processes", "Back Up Tenant"),
    ("restore", "configuration", "Processes", "Restore Snapshot"),
    ("attachments", "configuration", "Inquiries", "Files"),
    ("calendar", "configuration", "Inquiries", "Calendar"),
    ("reminders", "configuration", "Inquiries", "Reminders"),
    ("help", "configuration", "Inquiries", "Help"),
];

/// A workspace's screens grouped by section (sections with nothing are left out).
pub fn entries(ws: &str) -> Vec<(&'static str, Vec<(&'static str, Message)>)> {
    SECTIONS.iter().map(|sec| {
        let items = PLACES.iter().filter(|p| p.1 == ws && p.2 == *sec)
            .filter_map(|p| crate::features::FEATURES.iter().find(|f| f.id == p.0).map(|f| (p.3, (f.go)()))).collect::<Vec<_>>();
        (*sec, items)
    }).filter(|(_, v)| !v.is_empty()).collect()
}

/// Which workspace a screen belongs to, for highlighting the main menu.
fn workspace_of(r: &Route) -> &'static str {
    match r {
        Route::Invoices | Route::AllSales | Route::CreditNotes | Route::Customers | Route::ReceivePayment(_) | Route::Statements | Route::BatchInvoices | Route::FinanceCharges => "receivables",
        Route::Doc { kind, .. } => match kind.as_str() {
            "ACCPAY" | "ACCPAYCREDIT" => "payables", "PO" => "purchases", "SALESORDER" | "QUOTE" => "sales_orders", _ => "receivables",
        },
        Route::Bills | Route::Expenses | Route::Vendors | Route::PayBills | Route::Spend(_) | Route::Check(_) => "payables",
        Route::Banking(_) | Route::Deposit | Route::Transfer | Route::Reconcile | Route::Register(_) | Route::BankRules | Route::BankTx(_) => "banking",
        Route::Quotes | Route::PriceLevels => "sales_orders",
        Route::PurchaseOrders => "purchases",
        Route::Products | Route::InventoryCenter | Route::AdjustInventory | Route::BuildAssemblies | Route::NewProduct | Route::EditItem(_) | Route::Locations | Route::Lots => "inventory",
        Route::Projects => "projects",
        Route::EnterTime | Route::ExpenseClaims | Route::Mileage => "time_expenses",
        Route::Payroll | Route::PayRun(_) | Route::PayLiabilities | Route::PayrollForms | Route::PayrollSettings | Route::PayrollItems => "payroll",
        Route::Taxes | Route::PaySalesTax | Route::TaxReturn(_) => "taxes",
        Route::Settings | Route::AuditLog | Route::ImportExport | Route::Templates | Route::Help | Route::Calendar | Route::DocCenter => "configuration",
        Route::Dashboard => "",
        _ => "finance",
    }
}

/// The frame: top bar, main menu down the left, the screen on the right.
pub fn frame<'a>(app: &'a App, content: Element<'a, Message>) -> Element<'a, Message> {
    let k = t();
    let current = workspace_of(&app.route);
    let item = |key: &'static str, label: &'static str, ic: &'static str, msg: Message| -> Element<'a, Message> {
        let active = app.menu == Some(key) || (app.menu.is_none() && current == key) || (key == "dashboard" && app.route == Route::Dashboard && app.menu.is_none());
        button(column![icon(ic, 20.0, if active { k.nav_active } else { k.nav_muted }), text(label).size(11).color(if active { k.nav_text } else { k.nav_muted }).align_x(iced::alignment::Horizontal::Center).width(Fill)]
            .spacing(4).align_x(Alignment::Center).width(Fill))
            .style(theme::nav_item(active)).padding([8, 4]).width(Fill).on_press(msg).into()
    };
    let mut menu = column![item("dashboard", "Dashboard", "home", Message::Nav(Route::Dashboard))].spacing(2);
    for (key, label, ic) in WORKSPACES { menu = menu.push(item(key, label, ic, Message::OpenMenu(Some(key)))); }
    let left = container(scrollable(menu.padding([6, 4])).style(theme::scroll).height(Fill)).width(96).height(Fill).style(theme::nav);
    let top = container(row![
        container(text(app.org_name().chars().next().unwrap_or('L').to_string()).size(14).font(BOLD).color(k.accent_ink)).width(30).height(30).center_x(30).center_y(30)
            .style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.accent)), border: iced::Border { radius: 4.0.into(), ..Default::default() }, ..Default::default() }),
        button(column![text(app.org_name()).size(14).font(BOLD).color(k.nav_text), text("Company").size(11).color(k.nav_muted)].spacing(0)).style(theme::nav_item(false)).padding([4, 8]).on_press(Message::Nav(Route::Setup)),
        ui::hspace(),
        text_input("Search for screens, documents and records", &app.search).on_input(Message::Search).on_submit(Message::SearchGo).padding([6, 10]).size(13).width(Length::Fixed(360.0)).style(theme::input),
        button(icon("help", 18.0, k.nav_muted)).style(theme::nav_item(false)).padding(7).on_press(Message::Nav(Route::Help)),
        button(icon(if app.dark { "sun" } else { "moon" }, 18.0, k.nav_muted)).style(theme::nav_item(false)).padding(7).on_press(Message::ToggleTheme),
        button(icon("gear", 18.0, k.nav_muted)).style(theme::nav_item(false)).padding(7).on_press(Message::Nav(Route::Settings)),
        crate::shell::avatar(app),
    ].spacing(6).align_y(Alignment::Center).padding([0, 14])).height(50).center_y(50).width(Fill).style(theme::nav);
    column![top, row![left, content].height(Fill)].into()
}

/// The workspace panel that opens beside the main menu.
pub fn workspace_panel(app: &App, ws: &'static str) -> Element<'static, Message> {
    let k = t();
    let title = WORKSPACES.iter().find(|w| w.0 == ws).map(|w| w.1).unwrap_or("");
    // Sections side by side, four to a row so wide workspaces (Finance) still fit.
    let mut rows = column![].spacing(18);
    let mut cols = row![].spacing(28);
    for (n, (sec, items)) in entries(ws).into_iter().enumerate() {
        if n > 0 && n % 4 == 0 { rows = rows.push(cols); cols = row![].spacing(28); }
        let mut c = column![text(sec.to_uppercase()).size(11).font(BOLD).color(k.muted)].spacing(2).width(220);
        for (label, msg) in items {
            c = c.push(button(text(label).size(13).color(k.text)).style(theme::menu_item).padding([5, 6]).width(Fill).on_press(msg));
        }
        cols = cols.push(c);
    }
    rows = rows.push(cols);
    let _ = app;
    container(column![
        row![text(title).size(18).font(BOLD), ui::hspace(), button(icon("x", 16.0, k.muted)).style(theme::menu_item).padding(4).on_press(Message::CloseMenus)].align_y(Alignment::Center),
        rows,
    ].spacing(14)).padding(18).style(theme::menu).into()
}

/// Space reserved left of the panel (the main menu's width).
pub const PANEL_LEFT: f32 = 100.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_feature_has_a_place_once() {
        for f in crate::features::FEATURES {
            let n = PLACES.iter().filter(|p| p.0 == f.id).count();
            assert_eq!(n, 1, "feature {} has {n} Acumatica places", f.id);
        }
        for p in PLACES {
            assert!(crate::features::FEATURES.iter().any(|f| f.id == p.0), "unknown feature {}", p.0);
            assert!(WORKSPACES.iter().any(|w| w.0 == p.1), "unknown workspace {}", p.1);
            assert!(SECTIONS.contains(&p.2), "unknown section {}", p.2);
        }
    }
}
