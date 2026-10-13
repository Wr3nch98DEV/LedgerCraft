//! Terminology per layout. Screens ask for a concept and get the word each product uses.

use crate::theme::Skin;
use std::cell::Cell;

thread_local! { static LAYOUT: Cell<crate::theme::Layout> = const { Cell::new(crate::theme::Layout::Desktop) }; }

/// MYOB Acumatica-style shares the QuickBooks skin (Customers, Vendors, Bills) and Sage-style
/// shares Xero's (Suppliers, Credit notes), but each has its own words for screens, buttons and
/// statuses. The app publishes which layout is showing.
pub fn set_layout(l: crate::theme::Layout) { LAYOUT.with(|c| c.set(l)); }
pub fn acumatica() -> bool { LAYOUT.with(|c| c.get()) == crate::theme::Layout::Acumatica }
pub fn sage() -> bool { LAYOUT.with(|c| c.get()) == crate::theme::Layout::Sage }

/// Sage Accounting's words, where they differ from Xero's.
fn sage_term(key: &str) -> Option<&'static str> {
    Some(match key {
        "invoices" => "Sales Invoices", "quote" => "Quote", "quotes" => "Quotes & Estimates", "credit_notes" => "Sales Credit Notes",
        "bill" => "Purchase invoice", "bills" => "Purchase Invoices", "supplier_credit" => "Purchase credit note",
        "spend" => "Other payment", "receive" => "Other receipt", "journal" => "Journal", "journals" => "Journals",
        "items" => "Products & Services", "tracking" => "Analysis types", "repeating" => "Recurring",
        "receive_payment" => "Customer receipt", "pay_bill" => "Supplier payment", "approve" => "Save", "draft" => "Save as draft",
        "due" => "Outstanding", "aged_ar" => "Aged Debtors", "aged_ap" => "Aged Creditors", "settings" => "Settings",
        _ => return None,
    })
}

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
    if skin == Xero && sage() { if let Some(w) = sage_term(key) { return w; } }
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
            // Taken off hold, waiting to be released to the general ledger.
            (_, "SUBMITTED") => "Balanced",
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
    // Sage: unpaid invoices are Outstanding (Overdue once due); quotes are Pending until accepted.
    if skin == Skin::Xero && sage() {
        return match (kind, status) {
            (_, "DRAFT") => "Draft",
            (_, "SUBMITTED") => "Pending approval",
            ("QUOTE", "SENT" | "AUTHORISED") => "Pending",
            ("QUOTE", "ACCEPTED") => "Accepted",
            ("QUOTE", "DECLINED") => "Declined",
            ("QUOTE", "INVOICED") => "Invoiced",
            ("PO", "AUTHORISED") => "Ordered",
            ("PO", "BILLED") => "Completed",
            (_, "AUTHORISED") if overdue => "Overdue",
            (_, "AUTHORISED" | "SENT") => "Outstanding",
            (_, "PAID" | "INVOICED") => "Paid",
            (_, "VOIDED") => "Void",
            (_, "DELETED") => "Deleted",
            _ => "—",
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
