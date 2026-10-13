//! Sage Accounting-style layout: a dark top bar with Summary, Contacts, Sales, Purchases,
//! Banking, Products & Services, Payroll, Reporting and Accounting menus, and a settings menu
//! on the right. Sage's wording comes from `vocab`; the screens are the shared ones.

use crate::app::{App, Message, Route};
use crate::icons::icon;
use crate::theme::{self, t};
use crate::ui::{self, BOLD};
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length};

/// The top-bar menus: (key, label). "settings" is the gear on the right.
pub const MENUS: [(&str, &str); 9] = [
    ("contacts", "Contacts"), ("sales", "Sales"), ("purchases", "Purchases"), ("banking", "Banking"), ("products", "Products & Services"),
    ("payroll", "Payroll"), ("reporting", "Reporting"), ("accounting", "Accounting"), ("settings", "Settings"),
];

/// Where each feature (by registry id) lives: (feature, menu, Sage's name for it).
pub static PLACES: &[(&str, &str, &str)] = &[
    // Contacts
    ("customers", "contacts", "Customers"), ("vendors", "contacts", "Suppliers"), ("other_names", "contacts", "Other Contacts"),
    ("statements", "contacts", "Customer Statements"),
    // Sales
    ("all_sales", "sales", "Sales Overview"), ("invoices", "sales", "Sales Invoices"), ("invoice", "sales", "New Sales Invoice"),
    ("quotes", "sales", "Quotes & Estimates"), ("estimate", "sales", "New Quote"), ("credit_notes", "sales", "Sales Credit Notes"),
    ("credit_memo", "sales", "New Sales Credit Note"), ("receive_payment", "sales", "Customer Receipts"), ("sales_receipt", "sales", "Cash Sale"),
    ("sales_order", "sales", "Sales Orders"), ("delayed_charge", "sales", "Pending Charges"), ("delayed_credit", "sales", "Pending Credits"),
    ("batch_invoices", "sales", "Batch Invoicing"), ("print_forms", "sales", "Print & Email Invoices"), ("finance_charges", "sales", "Late Payment Charges"),
    ("payment_reminders", "sales", "Payment Reminders"), ("projects", "sales", "Projects"),
    // Purchases
    ("bills", "purchases", "Purchase Invoices"), ("bill", "purchases", "New Purchase Invoice"), ("vendor_credit", "purchases", "New Purchase Credit Note"),
    ("pay_bills", "purchases", "Supplier Payments"), ("batch_payments", "purchases", "Batch Payments"), ("purchase_orders", "purchases", "Purchase Orders"), ("purchase_order", "purchases", "New Purchase Order"),
    ("receive_items", "purchases", "Goods Received"), ("landed_costs", "purchases", "Landed Costs"), ("expenses", "purchases", "Purchases Overview"), ("expense", "purchases", "Other Payment"),
    ("check", "purchases", "Cheque Payment"), ("print_checks", "purchases", "Print Cheques"), ("expense_claims", "purchases", "Expense Claims"),
    ("mileage", "purchases", "Mileage"), ("contractor_1099", "purchases", "1099 Reporting"),
    // Banking
    ("banking", "banking", "Bank Accounts"), ("reconcile", "banking", "Reconcile"), ("deposit", "banking", "Bank Deposit"),
    ("transfer", "banking", "Bank Transfer"), ("register", "banking", "Bank Activity"), ("bank_rules", "banking", "Bank Rules"),
    // Products & services
    ("products", "products", "Products & Services"), ("inventory", "products", "Stock"), ("locations", "products", "Stock Locations"), ("lots", "products", "Batches & Serial Numbers"), ("adjust_inventory", "products", "Stock Movements"),
    ("build_assemblies", "products", "Build Assemblies"), ("price_levels", "products", "Price Bands"),
    // Payroll
    ("payroll", "payroll", "Employees & Pay Runs"), ("pay_liabilities", "payroll", "Pay Liabilities"), ("payroll_forms", "payroll", "Filings"),
    ("payroll_settings", "payroll", "Payroll Settings"), ("payroll_items", "payroll", "Pay Elements"), ("enter_time", "payroll", "Timesheets"),
    // Reporting
    ("reports", "reporting", "All Reports"), ("memorized_reports", "reporting", "Saved Reports"), ("custom_reports", "reporting", "Report Designer"), ("consolidation", "reporting", "Consolidated Reports"),
    ("insights", "reporting", "Business Summary"), ("audit_log", "reporting", "Audit Trail"),
    // Accounting
    ("journal", "accounting", "Journals"), ("chart", "accounting", "Chart of Accounts"), ("recode", "accounting", "Reallocate Transactions"), ("revenue_recognition", "accounting", "Deferred Income"),
    ("budgets", "accounting", "Budgets"), ("currencies", "accounting", "Currencies"), ("fixed_assets", "accounting", "Fixed Assets"),
    ("taxes", "accounting", "Tax Returns"), ("pay_sales_tax", "accounting", "Tax Payments"), ("closing_date", "accounting", "Year End & Lock Date"),
    ("classes", "accounting", "Analysis Types"), ("recurring", "accounting", "Recurring Transactions"), ("find", "accounting", "Search Transactions"),
    // Settings (gear)
    ("settings", "settings", "Business Settings"), ("users", "settings", "Manage Users"), ("import_data", "settings", "Import Data"),
    ("export_data", "settings", "Export Data"), ("templates", "settings", "Invoice Templates"), ("custom_fields", "settings", "Custom Fields"), ("approval_rules", "settings", "Approval Rules"), ("backup", "settings", "Back Up"),
    ("restore", "settings", "Restore"), ("attachments", "settings", "Attachments"), ("calendar", "settings", "Calendar"), ("reminders", "settings", "Reminders List"), ("help", "settings", "Help Centre"),
];

/// A menu's entries in table order.
pub fn entries(menu: &str) -> Vec<(&'static str, Message)> {
    PLACES.iter().filter(|p| p.1 == menu)
        .filter_map(|p| crate::features::FEATURES.iter().find(|f| f.id == p.0).map(|f| (p.2, (f.go)()))).collect()
}

/// Which menu a screen belongs to, for highlighting.
fn menu_of(r: &Route) -> &'static str {
    match r {
        Route::Dashboard => "summary",
        Route::Customers | Route::Vendors | Route::Contacts | Route::Contact(_) | Route::Statements | Route::OtherNames => "contacts",
        Route::Invoices | Route::AllSales | Route::Quotes | Route::CreditNotes | Route::ReceivePayment(_) | Route::BatchInvoices | Route::Projects => "sales",
        Route::Doc { kind, .. } => if matches!(kind.as_str(), "ACCPAY" | "ACCPAYCREDIT" | "PO") { "purchases" } else { "sales" },
        Route::Bills | Route::Expenses | Route::PayBills | Route::PurchaseOrders | Route::Spend(_) | Route::Check(_) | Route::ExpenseClaims | Route::Mileage => "purchases",
        Route::Banking(_) | Route::Reconcile | Route::Deposit | Route::Transfer | Route::Register(_) | Route::BankRules | Route::BankTx(_) => "banking",
        Route::Products | Route::InventoryCenter | Route::AdjustInventory | Route::BuildAssemblies | Route::PriceLevels | Route::Locations | Route::Lots => "products",
        Route::Payroll | Route::PayRun(_) | Route::PayLiabilities | Route::PayrollForms | Route::PayrollSettings | Route::PayrollItems | Route::EnterTime => "payroll",
        Route::Reports | Route::Report(_) | Route::MemorizedReports | Route::CustomReports | Route::AuditLog | Route::Consolidation => "reporting",
        Route::Settings | Route::ImportExport | Route::Templates | Route::Help | Route::Calendar | Route::DocCenter => "settings",
        _ => "accounting",
    }
}

/// Approximate left edge of a menu's button (for placing its dropdown under it).
pub fn menu_left(key: &str) -> f32 {
    if key == "settings" { return 0.0; }
    let mut x = 240.0;
    for (k, label) in MENUS {
        if k == key { return x; }
        x += label.len() as f32 * 7.6 + 30.0;
    }
    x
}

pub fn topbar(app: &App) -> Element<'_, Message> {
    let k = t();
    let cur = menu_of(&app.route);
    let tab = |key: &'static str, label: &'static str, msg: Message| -> Element<'_, Message> {
        let active = app.menu == Some(key) || (app.menu.is_none() && cur == key);
        let mut r = row![text(label).size(14).font(if active { BOLD } else { iced::Font::DEFAULT })].spacing(4).align_y(Alignment::Center);
        if key != "summary" { r = r.push(icon("down", 11.0, k.nav_muted)); }
        button(r).style(theme::nav_item(active)).padding([8, 10]).on_press(msg).into()
    };
    let mut bar = row![
        container(text("S").size(15).font(BOLD).color(k.accent_ink)).width(30).height(30).center_x(30).center_y(30)
            .style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.accent)), border: iced::Border { radius: 15.0.into(), ..Default::default() }, ..Default::default() }),
        button(text(app.org_name()).size(14).font(BOLD).color(k.nav_text)).style(theme::nav_item(false)).padding([6, 8]).on_press(Message::Nav(Route::Setup)),
        tab("summary", "Summary", Message::Nav(Route::Dashboard)),
    ].spacing(2).align_y(Alignment::Center);
    for (key, label) in MENUS.iter().filter(|m| m.0 != "settings") { bar = bar.push(tab(key, label, Message::OpenMenu(Some(key)))); }
    container(row![
        bar, ui::hspace(),
        text_input("Search", &app.search).on_input(Message::Search).on_submit(Message::SearchGo).padding([6, 10]).size(13).width(Length::Fixed(200.0)).style(theme::input),
        button(icon(if app.dark { "sun" } else { "moon" }, 18.0, k.nav_muted)).style(theme::nav_item(false)).padding(7).on_press(Message::ToggleTheme),
        button(icon("gear", 18.0, if app.menu == Some("settings") { k.nav_active } else { k.nav_muted })).style(theme::nav_item(app.menu == Some("settings"))).padding(7).on_press(Message::OpenMenu(Some("settings"))),
        crate::shell::avatar(app),
    ].spacing(6).align_y(Alignment::Center).padding([0, 14])).height(56).center_y(56).width(Length::Fill).style(theme::nav).into()
}

/// A top-bar menu's dropdown.
pub fn dropdown(menu: &'static str) -> Element<'static, Message> {
    let k = t();
    let mut col = column![].spacing(1).width(250);
    for (label, msg) in entries(menu) {
        // "New …" entries sit apart, as Sage puts its create actions first.
        let new = label.starts_with("New ");
        col = col.push(button(text(label).size(13).color(if new { k.accent } else { k.text })).style(theme::menu_item).padding([6, 10]).width(iced::Fill).on_press(msg));
    }
    container(col).padding(8).style(theme::menu).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_feature_has_a_place_once() {
        for f in crate::features::FEATURES {
            let n = PLACES.iter().filter(|p| p.0 == f.id).count();
            assert_eq!(n, 1, "feature {} has {n} Sage places", f.id);
        }
        for p in PLACES {
            assert!(crate::features::FEATURES.iter().any(|f| f.id == p.0), "unknown feature {}", p.0);
            assert!(MENUS.iter().any(|m| m.0 == p.1), "unknown menu {}", p.1);
        }
    }
}
