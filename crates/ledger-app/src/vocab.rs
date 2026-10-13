//! Terminology per layout. Screens ask for a concept and get the word each product uses.

use crate::theme::Skin;
use std::cell::Cell;

thread_local! { static ACUMATICA: Cell<bool> = const { Cell::new(false) }; }

/// MYOB Acumatica-style shares the QuickBooks skin (Customers, Vendors, Bills) but has its own
/// words for screens, buttons and statuses. The view publishes which layout is showing.
pub fn set_acumatica(on: bool) { ACUMATICA.with(|c| c.set(on)); }
pub fn acumatica() -> bool { ACUMATICA.with(|c| c.get()) }

/// Acumatica's words, where they differ from QuickBooks'.
fn acumatica_term(key: &str) -> Option<&'static str> {
    Some(match key {
        "contact" => "Business account", "contacts" => "Business accounts",
        "invoices" => "Invoices and Memos", "quote" => "Sales quote", "quotes" => "Sales Quotes",
        "credit_note" => "Credit memo", "credit_notes" => "Credit Memos",
        "supplier_credit" => "Debit adjustment", "bills" => "Bills and Adjustments", "pos" => "Purchase Orders",
        "spend" => "Cash disbursement", "receive" => "Cash receipt",
        "journal" => "Journal transaction", "journals" => "Journal Transactions",
        "items" => "Stock and Non-Stock Items", "tracking" => "Subaccounts", "repeating" => "Recurring Transactions",
        "reconcile" => "Process Bank Transactions", "receive_payment" => "Enter Payment", "pay_bill" => "Pay",
        "approve" => "Release", "draft" => "Save", "due" => "Balance",
        "aged_ar" => "AR Aging", "aged_ap" => "AP Aging", "settings" => "Company and Preferences",
        _ => return None,
    })
}

pub fn term(skin: Skin, key: &str) -> &'static str {
    use Skin::*;
    if skin == QuickBooks && acumatica() { if let Some(w) = acumatica_term(key) { return w; } }
    match (key, skin) {
        ("contact", QuickBooks) => "Customer/vendor", ("contact", Xero) => "Contact",
        ("contacts", QuickBooks) => "Customers & vendors", ("contacts", Xero) => "Contacts",
        ("customer", _) => "Customer",
        ("customers", _) => "Customers",
        ("supplier", QuickBooks) => "Vendor", ("supplier", Xero) => "Supplier",
        ("suppliers", QuickBooks) => "Vendors", ("suppliers", Xero) => "Suppliers",
        ("invoice", _) => "Invoice",
        ("invoices", _) => "Invoices",
        ("quote", QuickBooks) => "Estimate", ("quote", Xero) => "Quote",
        ("quotes", QuickBooks) => "Estimates", ("quotes", Xero) => "Quotes",
        ("credit_note", QuickBooks) => "Credit memo", ("credit_note", Xero) => "Credit note",
        ("credit_notes", QuickBooks) => "Credit memos", ("credit_notes", Xero) => "Credit notes",
        ("bill", _) => "Bill",
        ("bills", _) => "Bills",
        ("supplier_credit", QuickBooks) => "Vendor credit", ("supplier_credit", Xero) => "Supplier credit",
        ("po", _) => "Purchase order",
        ("pos", _) => "Purchase orders",
        ("spend", QuickBooks) => "Expense", ("spend", Xero) => "Spend money",
        ("receive", QuickBooks) => "Deposit", ("receive", Xero) => "Receive money",
        ("journal", QuickBooks) => "Journal entry", ("journal", Xero) => "Manual journal",
        ("journals", QuickBooks) => "Journal entries", ("journals", Xero) => "Manual journals",
        ("items", _) => "Products and services",
        ("tracking", QuickBooks) => "Classes and locations", ("tracking", Xero) => "Tracking categories",
        ("repeating", QuickBooks) => "Recurring transactions", ("repeating", Xero) => "Repeating",
        ("reconcile", QuickBooks) => "For review", ("reconcile", Xero) => "Reconcile",
        ("receive_payment", QuickBooks) => "Receive payment", ("receive_payment", Xero) => "Record payment",
        ("pay_bill", QuickBooks) => "Pay bills", ("pay_bill", Xero) => "Make payment",
        ("approve", QuickBooks) => "Save", ("approve", Xero) => "Approve",
        ("draft", QuickBooks) => "Save draft", ("draft", Xero) => "Save draft",
        ("due", QuickBooks) => "Open balance", ("due", Xero) => "Amount due",
        ("aged_ar", QuickBooks) => "A/R Aging Summary", ("aged_ar", Xero) => "Aged Receivables",
        ("aged_ap", QuickBooks) => "A/P Aging Summary", ("aged_ap", Xero) => "Aged Payables",
        ("settings", QuickBooks) => "Account and settings", ("settings", Xero) => "Settings",
        _ => "?",
    }
}

/// Status names as each product shows them.
pub fn status(skin: Skin, kind: &str, status: &str, overdue: bool) -> &'static str {
    // Acumatica: documents start On Hold, are Open once released and Closed when settled.
    if skin == Skin::QuickBooks && acumatica() {
        return match (kind, status) {
            (_, "DRAFT") => "On Hold",
            (_, "SUBMITTED") => "Pending Approval",
            ("QUOTE", "ACCEPTED") => "Approved",
            ("QUOTE", "DECLINED") => "Rejected",
            ("QUOTE", "INVOICED") | ("DELAYEDCHARGE" | "DELAYEDCREDIT", "INVOICED") => "Closed",
            ("SALESORDER", "INVOICED") => "Completed",
            (_, "SENT" | "AUTHORISED") => "Open",
            (_, "PAID" | "BILLED") => "Closed",
            (_, "VOIDED") => "Voided",
            (_, "DELETED") => "Deleted",
            _ => { let _ = overdue; "—" }
        };
    }
    match skin {
        Skin::QuickBooks => match (kind, status) {
            (_, "DRAFT") => "Draft",
            (_, "SUBMITTED") => "Pending approval",
            ("QUOTE", "SENT") => "Pending",
            ("QUOTE", "ACCEPTED") => "Accepted",
            ("QUOTE", "DECLINED") => "Rejected",
            ("QUOTE", "INVOICED") => "Closed",
            ("PO", "AUTHORISED") => "Open",
            ("PO", "BILLED") => "Closed",
            ("DELAYEDCHARGE" | "DELAYEDCREDIT" | "SALESORDER", "SENT") => "Open",
            ("DELAYEDCHARGE" | "DELAYEDCREDIT" | "SALESORDER", "INVOICED") => "Closed",
            ("ACCRECCREDIT" | "ACCPAYCREDIT", "PAID") => "Closed",
            ("ACCRECCREDIT" | "ACCPAYCREDIT", "AUTHORISED") => "Unapplied",
            (_, "AUTHORISED") if overdue => "Overdue",
            (_, "AUTHORISED") => "Open",
            (_, "PAID") => "Paid",
            (_, "VOIDED") => "Voided",
            (_, "DELETED") => "Deleted",
            _ => "—",
        },
        Skin::Xero => match (kind, status) {
            (_, "DRAFT") => "Draft",
            (_, "SUBMITTED") => "Awaiting approval",
            ("QUOTE", "SENT") => "Sent",
            ("QUOTE", "ACCEPTED") => "Accepted",
            ("QUOTE", "DECLINED") => "Declined",
            ("QUOTE", "INVOICED") => "Invoiced",
            ("PO", "AUTHORISED") => "Approved",
            ("PO", "BILLED") => "Billed",
            (_, "AUTHORISED") if overdue => "Overdue",
            (_, "AUTHORISED") => "Awaiting payment",
            (_, "PAID") => "Paid",
            (_, "VOIDED") => "Voided",
            (_, "DELETED") => "Deleted",
            _ => "—",
        },
    }
}
