//! The feature registry: every feature LedgerCraft has, and where each layout puts it.
//!
//! The QuickBooks Online-style and Xero-style menus are built from this list, so a feature
//! added here shows up in every layout. The QuickBooks Desktop menus are hand-laid-out (with
//! separators and shortcuts), so for Desktop the registry names the menu entry, and a test
//! checks it's there.

use crate::app::{Message, Route};
use crate::desktop::Msg as Desk;

/// Where a feature lives in QuickBooks Online-style:
/// * `"top"` — a top-level sidebar item (Dashboard, Banking, Projects…), laid out by hand;
/// * `"sales"`, `"expenses"`, `"payroll"`, `"taxes"`, `"reports"`, `"accounting"` — under that sidebar section;
/// * `"new:Customers"`, `"new:Vendors"`, `"new:Employees"`, `"new:Other"` — a column of the + New menu;
/// * `"gear:Your company"`, `"gear:Lists"`, `"gear:Tools"` — a column of the gear menu.
///
/// Where it lives in Xero-style: `"top"`, `"business"`, `"accounting"`, `"payroll"`, `"contacts"`
/// (the top-bar menus), `"new"` (the + New menu) or `"settings"` (the organisation/gear menu).
pub struct Feature {
    pub id: &'static str,
    pub go: fn() -> Message,
    pub online: (&'static str, &'static str),
    pub xero: (&'static str, &'static str),
    /// QuickBooks Desktop: (menu, entry label).
    pub desktop: (&'static str, &'static str),
}

fn nav(r: Route) -> Message { Message::Nav(r) }
fn doc(kind: &str) -> Message { Message::Nav(Route::Doc { kind: kind.into(), id: None }) }
fn rep(key: &str) -> Message { Message::Nav(Route::Report(key.into())) }

pub static FEATURES: &[Feature] = &[
    // ---- customers & sales ----
    Feature { id: "invoice", go: || doc("ACCREC"), online: ("new:Customers", "Invoice"), xero: ("new", "Invoice"), desktop: ("Customers", "Create Invoices") },
    Feature { id: "receive_payment", go: || nav(Route::ReceivePayment(None)), online: ("new:Customers", "Receive payment"), xero: ("business", "Receive a payment"), desktop: ("Customers", "Receive Payments") },
    Feature { id: "statements", go: || nav(Route::Statements), online: ("new:Customers", "Statement"), xero: ("contacts", "Send statements"), desktop: ("Customers", "Create Statements…") },
    Feature { id: "estimate", go: || doc("QUOTE"), online: ("new:Customers", "Estimate"), xero: ("new", "Quote"), desktop: ("Customers", "Create Estimates") },
    Feature { id: "credit_memo", go: || doc("ACCRECCREDIT"), online: ("new:Customers", "Credit memo"), xero: ("new", "Credit note"), desktop: ("Customers", "Create Credit Memos/Refunds") },
    Feature { id: "sales_receipt", go: || doc("SALESRECEIPT"), online: ("new:Customers", "Sales receipt"), xero: ("new", "Receive money"), desktop: ("Customers", "Enter Sales Receipts") },
    Feature { id: "sales_order", go: || doc("SALESORDER"), online: ("new:Customers", "Sales order"), xero: ("new", "Sales order"), desktop: ("Customers", "Create Sales Orders") },
    Feature { id: "delayed_charge", go: || doc("DELAYEDCHARGE"), online: ("new:Customers", "Delayed charge"), xero: ("new", "Statement charge"), desktop: ("Customers", "Enter Statement Charges") },
    Feature { id: "delayed_credit", go: || doc("DELAYEDCREDIT"), online: ("new:Customers", "Delayed credit"), xero: ("new", "Statement credit"), desktop: ("Customers", "Create Credit Memos/Refunds") },
    Feature { id: "all_sales", go: || nav(Route::AllSales), online: ("sales", "All sales"), xero: ("business", "Sales overview"), desktop: ("Customers", "Income Tracker") },
    Feature { id: "invoices", go: || nav(Route::Invoices), online: ("sales", "Invoices"), xero: ("business", "Invoices"), desktop: ("Customers", "Income Tracker") },
    Feature { id: "quotes", go: || nav(Route::Quotes), online: ("sales", "Estimates"), xero: ("business", "Quotes"), desktop: ("Customers", "Create Estimates") },
    Feature { id: "credit_notes", go: || nav(Route::CreditNotes), online: ("sales", "Credit memos"), xero: ("business", "Credit notes"), desktop: ("Customers", "Create Credit Memos/Refunds") },
    Feature { id: "customers", go: || nav(Route::Customers), online: ("sales", "Customers"), xero: ("contacts", "Customers"), desktop: ("Customers", "Customer Center") },
    Feature { id: "products", go: || nav(Route::Products), online: ("sales", "Products and services"), xero: ("business", "Products and services"), desktop: ("Lists", "Item List") },
    Feature { id: "inventory", go: || nav(Route::InventoryCenter), online: ("sales", "Inventory"), xero: ("business", "Inventory"), desktop: ("Inventory", "Inventory Center") },
    Feature { id: "build_assemblies", go: || nav(Route::BuildAssemblies), online: ("sales", "Build assemblies"), xero: ("business", "Build assemblies"), desktop: ("Inventory", "Build Assemblies…") },
    Feature { id: "batch_invoices", go: || nav(Route::BatchInvoices), online: ("sales", "Batch invoices"), xero: ("business", "Batch invoices"), desktop: ("Customers", "Create Batch Invoices") },
    Feature { id: "print_forms", go: || nav(Route::PrintForms), online: ("sales", "Print or send"), xero: ("business", "Print invoices"), desktop: ("File", "Print Forms") },
    Feature { id: "finance_charges", go: || nav(Route::FinanceCharges), online: ("sales", "Late fees"), xero: ("business", "Late fees"), desktop: ("Customers", "Assess Finance Charges…") },
    // ---- vendors & expenses ----
    Feature { id: "expense", go: || nav(Route::Spend(None)), online: ("new:Vendors", "Expense"), xero: ("new", "Spend money"), desktop: ("Banking", "Enter Credit Card Charges") },
    Feature { id: "check", go: || nav(Route::Check(None)), online: ("new:Vendors", "Check"), xero: ("new", "Write a cheque"), desktop: ("Banking", "Write Checks") },
    Feature { id: "bill", go: || doc("ACCPAY"), online: ("new:Vendors", "Bill"), xero: ("new", "Bill"), desktop: ("Vendors", "Enter Bills") },
    Feature { id: "pay_bills", go: || nav(Route::PayBills), online: ("new:Vendors", "Pay bills"), xero: ("business", "Pay bills"), desktop: ("Vendors", "Pay Bills") },
    Feature { id: "purchase_order", go: || doc("PO"), online: ("new:Vendors", "Purchase order"), xero: ("new", "Purchase order"), desktop: ("Vendors", "Create Purchase Orders") },
    Feature { id: "vendor_credit", go: || doc("ACCPAYCREDIT"), online: ("new:Vendors", "Vendor credit"), xero: ("new", "Bill credit note"), desktop: ("Vendors", "Enter Bills") },
    Feature { id: "print_checks", go: || nav(Route::Check(None)), online: ("new:Vendors", "Print checks"), xero: ("accounting", "Print cheques"), desktop: ("Banking", "Write Checks") },
    Feature { id: "expenses", go: || nav(Route::Expenses), online: ("expenses", "Expenses"), xero: ("business", "Spend money"), desktop: ("Vendors", "Bill Tracker") },
    Feature { id: "bills", go: || nav(Route::Bills), online: ("expenses", "Bills"), xero: ("business", "Bills to pay"), desktop: ("Vendors", "Bill Tracker") },
    Feature { id: "vendors", go: || nav(Route::Vendors), online: ("expenses", "Vendors"), xero: ("contacts", "Suppliers"), desktop: ("Vendors", "Vendor Center") },
    Feature { id: "purchase_orders", go: || nav(Route::PurchaseOrders), online: ("expenses", "Purchase orders"), xero: ("business", "Purchase orders"), desktop: ("Vendors", "Purchase Orders List") },
    Feature { id: "receive_items", go: || Message::ReceiveItems, online: ("expenses", "Receive items"), xero: ("business", "Receive items"), desktop: ("Inventory", "Receive Items") },
    Feature { id: "contractor_1099", go: || rep("contractor-1099"), online: ("expenses", "1099 contractors"), xero: ("accounting", "1099 summary"), desktop: ("Vendors", "Print/E-file 1099s: 1099 Summary") },
    // ---- employees & payroll ----
    Feature { id: "payroll", go: || nav(Route::Payroll), online: ("payroll", "Employees"), xero: ("payroll", "Employees"), desktop: ("Employees", "Employee Center") },
    Feature { id: "pay_liabilities", go: || nav(Route::PayLiabilities), online: ("payroll", "Payroll tax"), xero: ("payroll", "Pay liabilities"), desktop: ("Employees", "Pay Liabilities") },
    Feature { id: "payroll_forms", go: || nav(Route::PayrollForms), online: ("payroll", "Filings"), xero: ("payroll", "Single Touch Payroll"), desktop: ("Employees", "Process Payroll Forms") },
    Feature { id: "payroll_items", go: || nav(Route::PayrollItems), online: ("payroll", "Pay types"), xero: ("payroll", "Pay items"), desktop: ("Lists", "Payroll Item List") },
    Feature { id: "enter_time", go: || nav(Route::EnterTime), online: ("new:Employees", "Single time activity"), xero: ("payroll", "Timesheets"), desktop: ("Employees", "Enter Time") },
    Feature { id: "mileage", go: || nav(Route::Mileage), online: ("top", "Mileage"), xero: ("business", "Expense claims"), desktop: ("Company", "Enter Vehicle Mileage…") },
    Feature { id: "projects", go: || nav(Route::Projects), online: ("top", "Projects"), xero: ("business", "Projects"), desktop: ("Customers", "Job Costing Center") },
    // ---- banking & accounting ----
    Feature { id: "banking", go: || nav(Route::Banking(None)), online: ("top", "Banking"), xero: ("accounting", "Bank accounts"), desktop: ("Banking", "Bank Feeds Center") },
    Feature { id: "deposit", go: || nav(Route::Deposit), online: ("new:Other", "Bank deposit"), xero: ("new", "Bank deposit"), desktop: ("Banking", "Make Deposits") },
    Feature { id: "transfer", go: || nav(Route::Transfer), online: ("new:Other", "Transfer"), xero: ("new", "Transfer money"), desktop: ("Banking", "Transfer Funds") },
    Feature { id: "journal", go: || nav(Route::Journal), online: ("new:Other", "Journal entry"), xero: ("new", "Manual journal"), desktop: ("Company", "Make General Journal Entries…") },
    Feature { id: "adjust_inventory", go: || nav(Route::AdjustInventory), online: ("new:Other", "Inventory qty adjustment"), xero: ("business", "Inventory adjustment"), desktop: ("Inventory", "Adjust Quantity/Value on Hand…") },
    Feature { id: "register", go: || nav(Route::Register(None)), online: ("accounting", "Registers"), xero: ("accounting", "Account transactions"), desktop: ("Banking", "Use Register") },
    Feature { id: "reconcile", go: || nav(Route::Reconcile), online: ("accounting", "Reconcile"), xero: ("accounting", "Reconcile account"), desktop: ("Banking", "Reconcile") },
    Feature { id: "chart", go: || nav(Route::Chart), online: ("accounting", "Chart of accounts"), xero: ("accounting", "Chart of accounts"), desktop: ("Lists", "Chart of Accounts") },
    Feature { id: "fixed_assets", go: || rep("fixed-asset-register"), online: ("accounting", "Fixed assets"), xero: ("accounting", "Fixed assets"), desktop: ("Lists", "Fixed Asset Item List") },
    Feature { id: "find", go: || nav(Route::Find), online: ("accounting", "Advanced search"), xero: ("accounting", "Find and recode"), desktop: ("Edit", "Find…") },
    Feature { id: "taxes", go: || nav(Route::Taxes), online: ("taxes", "Sales tax"), xero: ("accounting", "Tax returns"), desktop: ("Vendors", "Sales Tax: Manage Sales Tax…") },
    Feature { id: "pay_sales_tax", go: || nav(Route::PaySalesTax), online: ("taxes", "Record tax payment"), xero: ("accounting", "Pay tax"), desktop: ("Vendors", "Sales Tax: Pay Sales Tax…") },
    // ---- reports ----
    Feature { id: "reports", go: || nav(Route::Reports), online: ("reports", "Standard reports"), xero: ("accounting", "Reports"), desktop: ("Reports", "Report Center") },
    Feature { id: "memorized_reports", go: || nav(Route::MemorizedReports), online: ("reports", "Custom reports"), xero: ("accounting", "Custom reports"), desktop: ("Reports", "Memorized Reports") },
    Feature { id: "insights", go: || Message::Desk(Desk::Insights(true)), online: ("top", "Dashboard"), xero: ("top", "Dashboard"), desktop: ("Company", "Company Snapshot") },
    // ---- lists, company and tools ----
    Feature { id: "templates", go: || nav(Route::Templates), online: ("gear:Your company", "Custom form styles"), xero: ("settings", "Invoice settings"), desktop: ("Lists", "Templates") },
    Feature { id: "users", go: || Message::Desk(Desk::OpenPrefs("Users & Roles")), online: ("gear:Your company", "Manage users"), xero: ("settings", "Users"), desktop: ("Company", "Set Up Users and Roles…") },
    Feature { id: "settings", go: || nav(Route::Settings), online: ("gear:Your company", "Account and settings"), xero: ("settings", "Organisation settings"), desktop: ("Edit", "Preferences…") },
    Feature { id: "closing_date", go: || Message::Desk(Desk::OpenPrefs("Accounting")), online: ("gear:Your company", "Close the books"), xero: ("settings", "Lock dates"), desktop: ("Company", "Set Closing Date…") },
    Feature { id: "backup", go: || Message::BackupCompany, online: ("gear:Your company", "Back up company"), xero: ("settings", "Back up organisation"), desktop: ("File", "Back Up Company…") },
    Feature { id: "restore", go: || Message::Setup(crate::setup::Msg::Restore), online: ("gear:Your company", "Restore a backup"), xero: ("settings", "Restore a backup"), desktop: ("File", "Restore a Backup Copy…") },
    Feature { id: "price_levels", go: || nav(Route::PriceLevels), online: ("gear:Lists", "Price rules"), xero: ("settings", "Price levels"), desktop: ("Lists", "Price Level List") },
    Feature { id: "classes", go: || nav(Route::ClassList), online: ("gear:Lists", "Classes"), xero: ("settings", "Tracking categories"), desktop: ("Lists", "Class List") },
    Feature { id: "recurring", go: || nav(Route::Memorized), online: ("gear:Lists", "Recurring transactions"), xero: ("business", "Repeating transactions"), desktop: ("Lists", "Memorized Transaction List") },
    Feature { id: "attachments", go: || nav(Route::DocCenter), online: ("gear:Lists", "Attachments"), xero: ("business", "Files"), desktop: ("Company", "Documents") },
    Feature { id: "other_names", go: || nav(Route::OtherNames), online: ("gear:Lists", "Other names"), xero: ("contacts", "Other contacts"), desktop: ("Lists", "Other Names List") },
    Feature { id: "reminders", go: || nav(Route::Reminders), online: ("gear:Tools", "Reminders"), xero: ("business", "Reminders"), desktop: ("Company", "Reminders") },
    Feature { id: "calendar", go: || nav(Route::Calendar), online: ("gear:Tools", "Calendar"), xero: ("business", "Calendar"), desktop: ("Company", "Calendar") },
    Feature { id: "help", go: || nav(Route::Help), online: ("gear:Tools", "Help"), xero: ("settings", "Help"), desktop: ("Help", "LedgerCraft Help") },
];

/// The QuickBooks Online-style entries for one place, in registry order.
pub fn online(place: &str) -> Vec<(&'static str, Option<Message>)> {
    FEATURES.iter().filter(|f| f.online.0 == place).map(|f| (f.online.1, Some((f.go)()))).collect()
}

/// Xero's own order for its Business menu: sales, then purchases, then products and projects.
const XERO_BUSINESS: &[&str] = &["Sales overview", "Invoices", "Quotes", "Credit notes", "Repeating transactions", "Receive a payment", "Batch invoices", "Print invoices",
    "Late fees", "Reminders", "Bills to pay", "Pay bills", "Purchase orders", "Receive items", "Spend money", "Expense claims", "Products and services", "Inventory",
    "Inventory adjustment", "Build assemblies", "Projects", "Files", "Calendar"];

/// The Xero-style entries for one place (the Business menu in Xero's order, others in registry order).
pub fn xero(place: &str) -> Vec<(&'static str, Option<Message>)> {
    let mut v: Vec<(&'static str, Option<Message>)> = FEATURES.iter().filter(|f| f.xero.0 == place).map(|f| (f.xero.1, Some((f.go)()))).collect();
    if place == "business" { v.sort_by_key(|(l, _)| XERO_BUSINESS.iter().position(|x| x == l).unwrap_or(usize::MAX)); }
    v
}
