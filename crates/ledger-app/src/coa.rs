//! Chart of accounts vocabulary: QuickBooks account types with their detail types, and Xero's
//! account types, both mapped onto the engine's account types.

/// A QuickBooks account type: its name and detail types, each with the engine type it creates.
pub struct QbType { pub name: &'static str, pub details: &'static [(&'static str, &'static str)], pub credit_card: bool }

pub const QB_TYPES: &[QbType] = &[
    QbType { name: "Bank", credit_card: false, details: &[("Checking", "BANK"), ("Savings", "BANK"), ("Money Market", "BANK"), ("Cash on hand", "BANK"), ("Trust account", "BANK")] },
    QbType { name: "Credit Card", credit_card: true, details: &[("Credit Card", "BANK")] },
    QbType { name: "Other Current Assets", credit_card: false, details: &[("Prepaid Expenses", "PREPAYMENT"), ("Inventory", "INVENTORY"), ("Employee Cash Advances", "CURRENT"),
        ("Loans to Others", "CURRENT"), ("Allowance for Bad Debts", "CURRENT"), ("Other Current Assets", "CURRENT")] },
    QbType { name: "Fixed Assets", credit_card: false, details: &[("Machinery & Equipment", "FIXED"), ("Vehicles", "FIXED"), ("Buildings", "FIXED"), ("Furniture & Fixtures", "FIXED"),
        ("Leasehold Improvements", "FIXED"), ("Land", "FIXED"), ("Accumulated Depreciation", "FIXED"), ("Other fixed assets", "FIXED")] },
    QbType { name: "Other Assets", credit_card: false, details: &[("Security Deposits", "NONCURRENT"), ("Goodwill", "NONCURRENT"), ("Organizational Costs", "NONCURRENT"), ("Other Long-term Assets", "NONCURRENT")] },
    QbType { name: "Other Current Liabilities", credit_card: false, details: &[("Accrued Liabilities", "CURRLIAB"), ("Loan Payable", "CURRLIAB"), ("Line of Credit", "CURRLIAB"),
        ("Payroll Tax Payable", "CURRLIAB"), ("Sales Tax Payable", "CURRLIAB"), ("Other Current Liabilities", "CURRLIAB")] },
    QbType { name: "Long Term Liabilities", credit_card: false, details: &[("Notes Payable", "TERMLIAB"), ("Shareholder Notes Payable", "TERMLIAB"), ("Other Long Term Liabilities", "TERMLIAB")] },
    QbType { name: "Equity", credit_card: false, details: &[("Owner's Equity", "EQUITY"), ("Partner Contributions", "EQUITY"), ("Partner Distributions", "EQUITY"), ("Common Stock", "EQUITY"),
        ("Paid-In Capital or Surplus", "EQUITY"), ("Opening Balance Equity", "EQUITY")] },
    QbType { name: "Income", credit_card: false, details: &[("Sales of Product Income", "REVENUE"), ("Service/Fee Income", "REVENUE"), ("Discounts/Refunds Given", "REVENUE"),
        ("Non-Profit Income", "REVENUE"), ("Other Primary Income", "REVENUE")] },
    QbType { name: "Cost of Goods Sold", credit_card: false, details: &[("Supplies & Materials - COGS", "DIRECTCOSTS"), ("Cost of labor - COS", "DIRECTCOSTS"),
        ("Equipment Rental - COS", "DIRECTCOSTS"), ("Shipping, Freight & Delivery - COS", "DIRECTCOSTS"), ("Other Costs of Services - COS", "DIRECTCOSTS")] },
    QbType { name: "Expenses", credit_card: false, details: &[("Advertising/Promotional", "EXPENSE"), ("Auto", "EXPENSE"), ("Bank Charges", "EXPENSE"), ("Dues & Subscriptions", "EXPENSE"),
        ("Entertainment", "EXPENSE"), ("Insurance", "EXPENSE"), ("Legal & Professional Fees", "EXPENSE"), ("Office/General Administrative Expenses", "EXPENSE"),
        ("Payroll Expenses", "EXPENSE"), ("Rent or Lease of Buildings", "EXPENSE"), ("Repair & Maintenance", "EXPENSE"), ("Supplies & Materials", "EXPENSE"),
        ("Travel", "EXPENSE"), ("Utilities", "EXPENSE"), ("Other Business Expenses", "EXPENSE")] },
    QbType { name: "Other Income", credit_card: false, details: &[("Interest Earned", "OTHERINCOME"), ("Dividend Income", "OTHERINCOME"), ("Other Investment Income", "OTHERINCOME"), ("Other Miscellaneous Income", "OTHERINCOME")] },
    QbType { name: "Other Expense", credit_card: false, details: &[("Depreciation", "DEPRECIATN"), ("Amortization", "DEPRECIATN"), ("Penalties & Settlements", "OVERHEADS"), ("Other Miscellaneous Expense", "OVERHEADS")] },
];

/// Xero account types (engine type, Xero's name), in Xero's order.
pub const XERO_TYPES: &[(&str, &str)] = &[
    ("BANK", "Bank"), ("CURRENT", "Current Asset"), ("FIXED", "Fixed Asset"), ("INVENTORY", "Inventory"), ("NONCURRENT", "Non-current Asset"), ("PREPAYMENT", "Prepayment"),
    ("CURRLIAB", "Current Liability"), ("LIABILITY", "Liability"), ("TERMLIAB", "Non-current Liability"), ("EQUITY", "Equity"),
    ("DEPRECIATN", "Depreciation"), ("DIRECTCOSTS", "Direct Costs"), ("EXPENSE", "Expense"), ("OVERHEADS", "Overhead"),
    ("OTHERINCOME", "Other Income"), ("REVENUE", "Revenue"), ("SALES", "Sales"),
];

/// QuickBooks account type name for an engine account.
pub fn qb_type(kind: &str, bank_type: &str) -> &'static str {
    match kind {
        "BANK" if bank_type == "CREDITCARD" => "Credit Card",
        "BANK" => "Bank", "CURRENT" | "PREPAYMENT" | "INVENTORY" => "Other Current Assets", "FIXED" => "Fixed Assets", "NONCURRENT" => "Other Assets",
        "CURRLIAB" => "Other Current Liabilities", "TERMLIAB" | "LIABILITY" => "Long Term Liabilities", "EQUITY" => "Equity",
        "REVENUE" | "SALES" => "Income", "OTHERINCOME" => "Other Income", "DIRECTCOSTS" => "Cost of Goods Sold", "EXPENSE" => "Expenses", "OVERHEADS" | "DEPRECIATN" => "Other Expense", _ => "Other",
    }
}

/// Detail type shown for an account: the one chosen, else the usual one for its type.
pub fn detail_type<'a>(kind: &str, bank_type: &str, chosen: &'a str) -> &'a str {
    if !chosen.is_empty() { return chosen; }
    match kind {
        "BANK" if bank_type == "CREDITCARD" => "Credit Card",
        "BANK" => "Checking", "CURRENT" => "Other Current Assets", "INVENTORY" => "Inventory", "PREPAYMENT" => "Prepaid Expenses", "FIXED" => "Machinery & Equipment",
        "NONCURRENT" => "Other Long-term Assets", "CURRLIAB" => "Other Current Liabilities", "TERMLIAB" | "LIABILITY" => "Other Long Term Liabilities", "EQUITY" => "Owner's Equity",
        "REVENUE" | "SALES" => "Sales of Product Income", "OTHERINCOME" => "Other Miscellaneous Income", "DIRECTCOSTS" => "Supplies & Materials - COGS",
        "EXPENSE" => "Other Business Expenses", "OVERHEADS" => "Other Miscellaneous Expense", "DEPRECIATN" => "Depreciation", _ => "",
    }
}

/// QuickBooks Desktop account type (singular, with A/R and A/P), and its sort order in lists.
pub fn desktop_type(kind: &str, bank_type: &str, system: &str) -> (&'static str, u8) {
    match (kind, system) {
        ("BANK", _) if bank_type == "CREDITCARD" => ("Credit Card", 6),
        ("BANK", _) => ("Bank", 0),
        (_, "AR") => ("Accounts Receivable", 1),
        (_, "AP") => ("Accounts Payable", 5),
        ("CURRENT" | "PREPAYMENT" | "INVENTORY", _) => ("Other Current Asset", 2),
        ("FIXED", _) => ("Fixed Asset", 3),
        ("NONCURRENT", _) => ("Other Asset", 4),
        ("CURRLIAB", _) => ("Other Current Liability", 7),
        ("TERMLIAB" | "LIABILITY", _) => ("Long Term Liability", 8),
        ("EQUITY", _) => ("Equity", 9),
        ("REVENUE" | "SALES", _) => ("Income", 10),
        ("DIRECTCOSTS", _) => ("Cost of Goods Sold", 11),
        ("EXPENSE", _) => ("Expense", 12),
        ("OTHERINCOME", _) => ("Other Income", 13),
        _ => ("Other Expense", 14),
    }
}

/// Xero's name for an engine account type.
pub fn xero_type(kind: &str) -> &'static str { XERO_TYPES.iter().find(|(k, _)| *k == kind).map(|(_, n)| *n).unwrap_or("Other") }

#[cfg(test)]
mod tests {
    use super::*;
    /// Every detail type creates a valid engine type that maps back to the same QuickBooks type.
    #[test]
    fn detail_types_round_trip() {
        for t in QB_TYPES {
            assert!(!t.details.is_empty());
            for (d, kind) in t.details {
                assert!(ledger_core::accounts::class_of(kind).is_ok(), "{d}: {kind}");
                assert_eq!(qb_type(kind, if t.credit_card { "CREDITCARD" } else { "BANK" }), t.name, "{d}");
            }
        }
        for (k, _) in XERO_TYPES { assert!(ledger_core::accounts::class_of(k).is_ok()); }
    }
}
