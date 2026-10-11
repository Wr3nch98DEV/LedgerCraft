//! Organisation setup: currencies, country chart-of-accounts templates, tax rates, defaults.

use crate::{accounts::class_of, error::invalid, Ledger, Result};
use rusqlite::params;
use serde::Deserialize;

/// ISO 4217 currencies: code|name|decimals
pub const CURRENCIES: &str = "AED|UAE Dirham|2;AFN|Afghan Afghani|2;ALL|Albanian Lek|2;AMD|Armenian Dram|2;ANG|Netherlands Antillean Guilder|2;AOA|Angolan Kwanza|2;ARS|Argentine Peso|2;AUD|Australian Dollar|2;AWG|Aruban Florin|2;AZN|Azerbaijani Manat|2;BAM|Bosnia-Herzegovina Mark|2;BBD|Barbadian Dollar|2;BDT|Bangladeshi Taka|2;BGN|Bulgarian Lev|2;BHD|Bahraini Dinar|3;BIF|Burundian Franc|0;BMD|Bermudan Dollar|2;BND|Brunei Dollar|2;BOB|Bolivian Boliviano|2;BRL|Brazilian Real|2;BSD|Bahamian Dollar|2;BTN|Bhutanese Ngultrum|2;BWP|Botswanan Pula|2;BYN|Belarusian Ruble|2;BZD|Belize Dollar|2;CAD|Canadian Dollar|2;CDF|Congolese Franc|2;CHF|Swiss Franc|2;CLP|Chilean Peso|0;CNY|Chinese Yuan|2;COP|Colombian Peso|2;CRC|Costa Rican Colón|2;CUP|Cuban Peso|2;CVE|Cape Verdean Escudo|2;CZK|Czech Koruna|2;DJF|Djiboutian Franc|0;DKK|Danish Krone|2;DOP|Dominican Peso|2;DZD|Algerian Dinar|2;EGP|Egyptian Pound|2;ERN|Eritrean Nakfa|2;ETB|Ethiopian Birr|2;EUR|Euro|2;FJD|Fijian Dollar|2;FKP|Falkland Islands Pound|2;GBP|British Pound|2;GEL|Georgian Lari|2;GHS|Ghanaian Cedi|2;GIP|Gibraltar Pound|2;GMD|Gambian Dalasi|2;GNF|Guinean Franc|0;GTQ|Guatemalan Quetzal|2;GYD|Guyanaese Dollar|2;HKD|Hong Kong Dollar|2;HNL|Honduran Lempira|2;HTG|Haitian Gourde|2;HUF|Hungarian Forint|2;IDR|Indonesian Rupiah|2;ILS|Israeli New Shekel|2;INR|Indian Rupee|2;IQD|Iraqi Dinar|3;IRR|Iranian Rial|2;ISK|Icelandic Króna|0;JMD|Jamaican Dollar|2;JOD|Jordanian Dinar|3;JPY|Japanese Yen|0;KES|Kenyan Shilling|2;KGS|Kyrgystani Som|2;KHR|Cambodian Riel|2;KMF|Comorian Franc|0;KPW|North Korean Won|2;KRW|South Korean Won|0;KWD|Kuwaiti Dinar|3;KYD|Cayman Islands Dollar|2;KZT|Kazakhstani Tenge|2;LAK|Laotian Kip|2;LBP|Lebanese Pound|2;LKR|Sri Lankan Rupee|2;LRD|Liberian Dollar|2;LSL|Lesotho Loti|2;LYD|Libyan Dinar|3;MAD|Moroccan Dirham|2;MDL|Moldovan Leu|2;MGA|Malagasy Ariary|2;MKD|Macedonian Denar|2;MMK|Myanmar Kyat|2;MNT|Mongolian Tugrik|2;MOP|Macanese Pataca|2;MRU|Mauritanian Ouguiya|2;MUR|Mauritian Rupee|2;MVR|Maldivian Rufiyaa|2;MWK|Malawian Kwacha|2;MXN|Mexican Peso|2;MYR|Malaysian Ringgit|2;MZN|Mozambican Metical|2;NAD|Namibian Dollar|2;NGN|Nigerian Naira|2;NIO|Nicaraguan Córdoba|2;NOK|Norwegian Krone|2;NPR|Nepalese Rupee|2;NZD|New Zealand Dollar|2;OMR|Omani Rial|3;PAB|Panamanian Balboa|2;PEN|Peruvian Sol|2;PGK|Papua New Guinean Kina|2;PHP|Philippine Peso|2;PKR|Pakistani Rupee|2;PLN|Polish Zloty|2;PYG|Paraguayan Guarani|0;QAR|Qatari Riyal|2;RON|Romanian Leu|2;RSD|Serbian Dinar|2;RUB|Russian Ruble|2;RWF|Rwandan Franc|0;SAR|Saudi Riyal|2;SBD|Solomon Islands Dollar|2;SCR|Seychellois Rupee|2;SDG|Sudanese Pound|2;SEK|Swedish Krona|2;SGD|Singapore Dollar|2;SHP|St. Helena Pound|2;SLE|Sierra Leonean Leone|2;SOS|Somali Shilling|2;SRD|Surinamese Dollar|2;SSP|South Sudanese Pound|2;STN|São Tomé Dobra|2;SVC|Salvadoran Colón|2;SYP|Syrian Pound|2;SZL|Swazi Lilangeni|2;THB|Thai Baht|2;TJS|Tajikistani Somoni|2;TMT|Turkmenistani Manat|2;TND|Tunisian Dinar|3;TOP|Tongan Paʻanga|2;TRY|Turkish Lira|2;TTD|Trinidad & Tobago Dollar|2;TWD|New Taiwan Dollar|2;TZS|Tanzanian Shilling|2;UAH|Ukrainian Hryvnia|2;UGX|Ugandan Shilling|0;USD|US Dollar|2;UYU|Uruguayan Peso|2;UZS|Uzbekistani Som|2;VES|Venezuelan Bolívar|2;VND|Vietnamese Dong|0;VUV|Vanuatu Vatu|0;WST|Samoan Tala|2;XAF|Central African CFA Franc|0;XCD|East Caribbean Dollar|2;XOF|West African CFA Franc|0;XPF|CFP Franc|0;YER|Yemeni Rial|2;ZAR|South African Rand|2;ZMW|Zambian Kwacha|2;ZWG|Zimbabwe Gold|2";

#[derive(Debug, Clone, Deserialize)]
pub struct SetupInput {
    pub org_name: String,
    #[serde(default = "default_country")]
    pub country: String,
    #[serde(default)]
    pub base_currency: Option<String>,
    #[serde(default)]
    pub fy_end_month: Option<u32>,
    #[serde(default)]
    pub tax_basis: Option<String>,
    #[serde(default)]
    pub tax_registered: Option<bool>,
    #[serde(default)]
    pub tax_number: Option<String>,
    #[serde(default)]
    pub tax_period: Option<String>,
    #[serde(default)]
    pub industry: Option<String>,
    #[serde(default)]
    pub owner_name: Option<String>,
    #[serde(default)]
    pub owner_email: Option<String>,
    #[serde(default)]
    pub conversion_date: Option<String>,
}
fn default_country() -> String { "AU".into() }

pub struct CountryProfile {
    pub currency: &'static str,
    pub fy_end_month: u32,
    pub tax_label: &'static str,     // GST / VAT / Sales Tax
    pub tax_period: &'static str,
    /// (name, report_type, rate_bp, key)
    pub rates: &'static [(&'static str, &'static str, i64, &'static str)],
    pub payroll: &'static str,
}

pub fn profile(country: &str) -> CountryProfile {
    match country {
        "NZ" => CountryProfile { currency: "NZD", fy_end_month: 3, tax_label: "GST", tax_period: "TWOMONTHLY", payroll: "NZ",
            rates: &[("GST on Income", "OUTPUT", 1500, "SALES"), ("GST on Expenses", "INPUT", 1500, "PURCHASES"),
                ("Zero Rated", "ZERORATED", 0, "ZERO"), ("No GST", "NONE", 0, "NONE"), ("GST on Imports", "GSTONIMPORTS", 0, "IMPORTS"),
                ("Zero Rated - Exempt Expenses", "EXEMPTINPUT", 0, "EXEMPTPURCH")] },
        "GB" | "UK" => CountryProfile { currency: "GBP", fy_end_month: 3, tax_label: "VAT", tax_period: "QUARTERLY", payroll: "UK",
            rates: &[("20% (VAT on Income)", "OUTPUT", 2000, "SALES"), ("20% (VAT on Expenses)", "INPUT", 2000, "PURCHASES"),
                ("5% (VAT on Income)", "OUTPUT", 500, "SALES5"), ("5% (VAT on Expenses)", "INPUT", 500, "PURCH5"),
                ("Zero Rated Income", "ZERORATED", 0, "ZERO"), ("Zero Rated Expenses", "ZERORATEDINPUT", 0, "ZEROPURCH"),
                ("Exempt Income", "EXEMPTOUTPUT", 0, "EXEMPT"), ("Exempt Expenses", "EXEMPTINPUT", 0, "EXEMPTPURCH"),
                ("No VAT", "NONE", 0, "NONE"), ("Reverse Charge Expenses (20%)", "REVERSECHARGE", 2000, "REVERSE"),
                ("20% (VAT on Capital Purchases)", "CAPEXINPUT", 2000, "CAPEX")] },
        "IE" => CountryProfile { currency: "EUR", fy_end_month: 12, tax_label: "VAT", tax_period: "TWOMONTHLY", payroll: "GENERIC",
            rates: &[("23% VAT on Income", "OUTPUT", 2300, "SALES"), ("23% VAT on Expenses", "INPUT", 2300, "PURCHASES"),
                ("13.5% VAT on Income", "OUTPUT", 1350, "SALES135"), ("13.5% VAT on Expenses", "INPUT", 1350, "PURCH135"),
                ("9% VAT on Income", "OUTPUT", 900, "SALES9"), ("9% VAT on Expenses", "INPUT", 900, "PURCH9"),
                ("Zero Rated", "ZERORATED", 0, "ZERO"), ("Exempt", "EXEMPTOUTPUT", 0, "EXEMPT"), ("No VAT", "NONE", 0, "NONE"),
                ("EU Acquisitions 23%", "REVERSECHARGE", 2300, "REVERSE")] },
        "CA" => CountryProfile { currency: "CAD", fy_end_month: 12, tax_label: "GST/HST", tax_period: "QUARTERLY", payroll: "GENERIC",
            rates: &[("GST on Sales 5%", "OUTPUT", 500, "SALES"), ("GST on Purchases 5%", "INPUT", 500, "PURCHASES"),
                ("HST (ON) on Sales 13%", "OUTPUT", 1300, "HSTSALES"), ("HST (ON) on Purchases 13%", "INPUT", 1300, "HSTPURCH"),
                ("HST (Atlantic) on Sales 15%", "OUTPUT", 1500, "HST15S"), ("HST (Atlantic) on Purchases 15%", "INPUT", 1500, "HST15P"),
                ("Zero Rated", "ZERORATED", 0, "ZERO"), ("Exempt", "EXEMPTOUTPUT", 0, "EXEMPT"), ("No Tax", "NONE", 0, "NONE")] },
        "US" => CountryProfile { currency: "USD", fy_end_month: 12, tax_label: "Sales Tax", tax_period: "QUARTERLY", payroll: "US",
            rates: &[("Sales Tax", "OUTPUT", 0, "SALES"), ("Tax Exempt", "NONE", 0, "NONE"), ("Tax on Purchases", "INPUT", 0, "PURCHASES")] },
        "AU" => CountryProfile { currency: "AUD", fy_end_month: 6, tax_label: "GST", tax_period: "QUARTERLY", payroll: "AU",
            rates: &[("GST on Income", "OUTPUT", 1000, "SALES"), ("GST on Expenses", "INPUT", 1000, "PURCHASES"),
                ("GST Free Income", "EXEMPTOUTPUT", 0, "EXEMPT"), ("GST Free Expenses", "EXEMPTINPUT", 0, "EXEMPTPURCH"),
                ("GST on Capital", "CAPEXINPUT", 1000, "CAPEX"), ("GST Free Capital", "EXEMPTCAPITAL", 0, "EXEMPTCAPEX"),
                ("Input Taxed", "INPUTTAXED", 0, "INPUTTAXED"), ("BAS Excluded", "BASEXCLUDED", 0, "NONE"),
                ("GST on Imports", "GSTONIMPORTS", 0, "IMPORTS"), ("GST Free Exports", "EXPORTS", 0, "EXPORTS")] },
        _ => CountryProfile { currency: "USD", fy_end_month: 12, tax_label: "Tax", tax_period: "QUARTERLY", payroll: "GENERIC",
            rates: &[("Tax on Sales", "OUTPUT", 1000, "SALES"), ("Tax on Purchases", "INPUT", 1000, "PURCHASES"),
                ("Zero Rated", "ZERORATED", 0, "ZERO"), ("Exempt", "EXEMPTOUTPUT", 0, "EXEMPT"), ("No Tax", "NONE", 0, "NONE")] },
    }
}

/// (code, name, type, default-tax-key, system_account, show_in_expense_claims)
fn chart(tax_label: &str) -> Vec<(&'static str, String, &'static str, &'static str, Option<&'static str>, bool)> {
    let t = tax_label;
    vec![
        // Local naming: "current account" in VAT countries (UK/IE), "checking" where tax is sales tax (US), "cheque" otherwise.
        ("090", match t { "VAT" => "Business Current Account", "GST" => "Business Cheque Account", "GST/HST" => "Business Chequing Account", _ => "Business Checking Account" }.into(), "BANK", "NONE", None, false),
        ("091", "Business Savings Account".into(), "BANK", "NONE", None, false),
        ("200", "Sales".into(), "REVENUE", "SALES", None, false),
        ("260", "Other Revenue".into(), "REVENUE", "SALES", None, false),
        ("270", "Interest Income".into(), "OTHERINCOME", "EXEMPT", None, false),
        ("310", "Cost of Goods Sold".into(), "DIRECTCOSTS", "PURCHASES", None, false),
        ("320", "Subcontractors".into(), "DIRECTCOSTS", "PURCHASES", None, false),
        ("400", "Advertising & Marketing".into(), "EXPENSE", "PURCHASES", None, true),
        ("404", "Bank Fees".into(), "EXPENSE", "EXEMPTPURCH", None, false),
        ("408", "Cleaning".into(), "EXPENSE", "PURCHASES", None, true),
        ("412", "Accounting & Consulting".into(), "EXPENSE", "PURCHASES", None, false),
        ("416", "Depreciation Expense".into(), "DEPRECIATN", "NONE", None, false),
        ("420", "Entertainment".into(), "EXPENSE", "NONE", None, true),
        ("425", "Freight & Postage".into(), "EXPENSE", "PURCHASES", None, true),
        ("429", "General Expenses".into(), "EXPENSE", "PURCHASES", None, true),
        ("433", "Insurance".into(), "EXPENSE", "PURCHASES", None, false),
        ("437", "Interest Expense".into(), "EXPENSE", "EXEMPTPURCH", None, false),
        ("441", "Legal Fees".into(), "EXPENSE", "PURCHASES", None, false),
        ("445", "Electricity & Gas".into(), "EXPENSE", "PURCHASES", None, false),
        ("449", "Motor Vehicle Expenses".into(), "EXPENSE", "PURCHASES", None, true),
        ("453", "Office Expenses".into(), "EXPENSE", "PURCHASES", None, true),
        ("461", "Printing & Stationery".into(), "EXPENSE", "PURCHASES", None, true),
        ("469", "Rent".into(), "EXPENSE", "PURCHASES", None, false),
        ("473", "Repairs & Maintenance".into(), "EXPENSE", "PURCHASES", None, true),
        ("477", "Wages & Salaries".into(), "EXPENSE", "NONE", Some("WAGESEXPENSE"), false),
        ("478", "Superannuation / Pension".into(), "EXPENSE", "NONE", Some("SUPEREXPENSE"), false),
        ("485", "Subscriptions & Software".into(), "EXPENSE", "PURCHASES", None, true),
        ("489", "Phone & Internet".into(), "EXPENSE", "PURCHASES", None, true),
        ("493", "Travel - Domestic".into(), "EXPENSE", "PURCHASES", None, true),
        ("494", "Travel - International".into(), "EXPENSE", "NONE", None, true),
        ("496", "Mileage Reimbursements".into(), "EXPENSE", "NONE", None, true),
        ("497", "Bank Revaluations".into(), "EXPENSE", "NONE", Some("BANKREVAL"), false),
        ("498", "Unrealised Currency Gains".into(), "EXPENSE", "NONE", Some("UNREALISEDFX"), false),
        ("499", "Realised Currency Gains".into(), "EXPENSE", "NONE", Some("FXGAIN"), false),
        ("505", "Income Tax Expense".into(), "EXPENSE", "NONE", None, false),
        ("610", "Accounts Receivable".into(), "CURRENT", "NONE", Some("AR"), false),
        ("620", "Prepayments".into(), "CURRENT", "NONE", None, false),
        ("630", "Inventory".into(), "INVENTORY", "NONE", Some("INVENTORY"), false),
        ("710", "Office Equipment".into(), "FIXED", "CAPEX", None, false),
        ("711", "Accumulated Depreciation - Office Equipment".into(), "FIXED", "NONE", None, false),
        ("720", "Computer Equipment".into(), "FIXED", "CAPEX", None, false),
        ("721", "Accumulated Depreciation - Computer Equipment".into(), "FIXED", "NONE", None, false),
        ("730", "Motor Vehicles".into(), "FIXED", "CAPEX", None, false),
        ("731", "Accumulated Depreciation - Motor Vehicles".into(), "FIXED", "NONE", None, false),
        ("800", "Accounts Payable".into(), "CURRLIAB", "NONE", Some("AP"), false),
        ("801", "Unpaid Expense Claims".into(), "CURRLIAB", "NONE", Some("EXPENSECLAIMS"), false),
        ("804", "Wages Payable".into(), "CURRLIAB", "NONE", Some("WAGESPAYABLE"), false),
        ("820", t.to_string(), "CURRLIAB", "NONE", Some("GST"), false),
        ("825", "Payroll Tax Withholdings Payable".into(), "CURRLIAB", "NONE", Some("PAYGWITHHOLDING"), false),
        ("826", "Superannuation / Pension Payable".into(), "CURRLIAB", "NONE", Some("SUPERPAYABLE"), false),
        ("830", "Income Tax Payable".into(), "CURRLIAB", "NONE", None, false),
        ("835", "Customer Deposits & Prepayments".into(), "CURRLIAB", "NONE", None, false),
        ("840", "Historical Adjustment".into(), "CURRLIAB", "NONE", Some("HISTORICAL"), false),
        ("850", "Suspense".into(), "CURRLIAB", "NONE", Some("SUSPENSE"), false),
        ("860", "Rounding".into(), "CURRLIAB", "NONE", Some("ROUNDING"), false),
        ("877", "Tracking Transfers".into(), "CURRLIAB", "NONE", Some("TRACKINGTRANSFERS"), false),
        ("880", "Owner Drawings".into(), "EQUITY", "NONE", None, false),
        ("881", "Owner Funds Introduced".into(), "EQUITY", "NONE", None, false),
        ("900", "Loan".into(), "TERMLIAB", "NONE", None, false),
        ("960", "Retained Earnings".into(), "EQUITY", "NONE", Some("RETAINED"), false),
        ("970", "Owner Share Capital".into(), "EQUITY", "NONE", None, false),
    ]
}

impl Ledger {
    pub fn is_setup(&self) -> Result<bool> {
        Ok(self.setting("org_name")?.is_some())
    }

    /// Initialise a brand-new organisation (setup wizard).
    pub fn setup_org(&mut self, s: &SetupInput) -> Result<()> {
        if self.is_setup()? { return invalid("organisation is already set up"); }
        if s.org_name.trim().is_empty() { return invalid("organisation name is required"); }
        let country = if s.country == "UK" { "GB".to_string() } else { s.country.clone() };
        let p = profile(&country);
        let base = s.base_currency.clone().unwrap_or_else(|| p.currency.to_string());
        self.atomic(|| {
            // currencies
            for c in CURRENCIES.split(';') {
                let parts: Vec<&str> = c.split('|').collect();
                self.conn.execute("INSERT OR IGNORE INTO currencies(code, name, decimals, enabled) VALUES (?1, ?2, ?3, ?4)",
                    params![parts[0], parts[1], parts[2].parse::<i64>().unwrap_or(2), (parts[0] == base) as i64])?;
            }
            let fy = s.fy_end_month.unwrap_or(p.fy_end_month).clamp(1, 12);
            let settings: Vec<(&str, String)> = vec![
                ("org_name", s.org_name.trim().to_string()), ("legal_name", s.org_name.trim().to_string()),
                ("country", country.clone()), ("base_currency", base.clone()), ("fy_end_month", fy.to_string()), ("fy_end_day", "31".into()),
                ("tax_basis", s.tax_basis.clone().unwrap_or_else(|| "ACCRUAL".into())),
                ("tax_registered", if s.tax_registered.unwrap_or(true) { "1".into() } else { "0".into() }),
                ("tax_number", s.tax_number.clone().unwrap_or_default()), ("tax_label", p.tax_label.into()),
                ("tax_period", s.tax_period.clone().unwrap_or_else(|| p.tax_period.into())),
                ("industry", s.industry.clone().unwrap_or_default()),
                ("sales_terms_type", "DAYSAFTERBILLDATE".into()), ("sales_terms_days", "14".into()),
                ("bill_terms_type", "DAYSAFTERBILLDATE".into()), ("bill_terms_days", "30".into()),
                ("default_sales_lat", if country == "US" || country == "CA" { "EXCLUSIVE".into() } else { "EXCLUSIVE".into() }),
                ("default_purchases_lat", "INCLUSIVE".into()), ("payroll_jurisdiction", p.payroll.into()),
                ("conversion_date", s.conversion_date.clone().unwrap_or_default()),
            ];
            for (k, v) in settings { self.conn.execute("INSERT OR REPLACE INTO settings(key, value) VALUES (?1, ?2)", params![k, v])?; }

            // tax rates
            let mut key_to_id = std::collections::HashMap::new();
            for (name, rt, bp, key) in p.rates {
                self.conn.execute("INSERT INTO tax_rates(name, report_type, rate_bp, system) VALUES (?1, ?2, ?3, 1)", params![name, rt, bp])?;
                let id = self.conn.last_insert_rowid();
                self.conn.execute("INSERT INTO tax_components(tax_rate_id, name, rate_bp) VALUES (?1, ?2, ?3)", params![id, p.tax_label, bp])?;
                key_to_id.entry(*key).or_insert(id);
            }
            let fallback = |k: &str| -> Option<i64> {
                key_to_id.get(k).copied().or_else(|| match k {
                    "EXEMPT" | "EXEMPTPURCH" | "CAPEX" | "ZERO" => key_to_id.get("NONE").copied(),
                    _ => None,
                })
            };
            // accounts
            for (code, name, ty, taxk, sys, exp) in chart(&format!("{} Payable", p.tax_label)) {
                let class = class_of(ty)?;
                let tax = if ty == "BANK" { None } else { fallback(taxk) };
                self.conn.execute(
                    "INSERT INTO accounts(code, name, type, class, tax_rate_id, system_account, show_in_expense_claims, enable_payments, dashboard_watch, bank_type, currency)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![code, name, ty, class, tax, sys, exp as i64, (ty == "BANK") as i64, (ty == "BANK") as i64,
                        if ty == "BANK" { Some("BANK") } else { None }, if ty == "BANK" { Some(base.clone()) } else { None }],
                )?;
                let id = self.conn.last_insert_rowid();
                self.index_search("account", id, &format!("{code} {name}"), "")?;
            }
            // sequences
            for (n, pfx) in [("INV", "INV-"), ("BILL", ""), ("CN", "CN-"), ("QU", "QU-"), ("PO", "PO-"), ("MJ", "MJ-"), ("EXP", "EXP-"), ("ASSET", "FA-"), ("PAYRUN", "PR-")] {
                self.conn.execute("INSERT OR IGNORE INTO sequences(name, prefix, next, width) VALUES (?1, ?2, 1, 4)", params![n, pfx])?;
            }
            // default branding theme & templates
            self.conn.execute("INSERT INTO branding_themes(name, is_default, terms, payment_instructions, invoice_title) VALUES ('Standard', 1, '', 'Please pay by bank transfer quoting the invoice number.', ?1)",
                [if p.tax_label == "Sales Tax" { "Invoice" } else { "Tax Invoice" }])?;
            for (kind, name, subject, body) in DEFAULT_EMAILS {
                self.conn.execute("INSERT INTO email_templates(kind, name, subject, body, is_default) VALUES (?1, ?2, ?3, ?4, 1)", params![kind, name, subject, body])?;
            }
            self.conn.execute("INSERT INTO reminder_rules(days, template_id, active) SELECT 7, id, 0 FROM email_templates WHERE kind = 'REMINDER' LIMIT 1", [])?;
            // owner user
            let owner = s.owner_name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Owner".into());
            let email = s.owner_email.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "owner@localhost".into());
            self.conn.execute("INSERT INTO users(name, email, role) VALUES (?1, ?2, 'ADVISER')", params![owner, email])?;
            self.audit("SETUP", "organisation", None, None, Some(serde_json::json!({"name": s.org_name, "country": country})))?;
            Ok(())
        })?;
        self.user_id = Some(1);
        Ok(())
    }
}

const DEFAULT_EMAILS: &[(&str, &str, &str, &str)] = &[
    ("INVOICE", "Standard invoice", "Invoice [Invoice Number] from [Trading Name] for [Contact Name]",
     "Hi [Contact First Name],\n\nHere's invoice [Invoice Number] for [Currency Code] [Invoice Total].\n\nThe amount outstanding of [Amount Due] is due on [Due Date].\n\nView your invoice online: [Online Invoice Link]\n\nIf you have any questions, please let us know.\n\nThanks,\n[Trading Name]"),
    ("QUOTE", "Standard quote", "Quote [Quote Number] from [Trading Name] for [Contact Name]",
     "Hi [Contact First Name],\n\nThank you for your enquiry.\n\nHere's quote [Quote Number] for [Currency Code] [Quote Total].\n\nIf you have any questions, please let us know.\n\nThanks,\n[Trading Name]"),
    ("STATEMENT", "Standard statement", "Statement from [Trading Name] for [Contact Name]",
     "Hi [Contact First Name],\n\nHere's your statement of account. The balance outstanding is [Currency Code] [Amount Due].\n\nThanks,\n[Trading Name]"),
    ("REMINDER", "Overdue reminder", "Reminder: invoice [Invoice Number] is overdue",
     "Hi [Contact First Name],\n\nThis is a friendly reminder that invoice [Invoice Number] for [Currency Code] [Amount Due] was due on [Due Date].\n\nView and pay online: [Online Invoice Link]\n\nIf you've already paid, please ignore this email.\n\nThanks,\n[Trading Name]"),
    ("PO", "Standard purchase order", "Purchase Order [Purchase Order Number] from [Trading Name]",
     "Hi [Contact First Name],\n\nPlease find purchase order [Purchase Order Number] attached.\n\nThanks,\n[Trading Name]"),
    ("CREDIT", "Standard credit note", "Credit Note [Credit Note Number] from [Trading Name]",
     "Hi [Contact First Name],\n\nHere's credit note [Credit Note Number] for [Currency Code] [Credit Note Total].\n\nThanks,\n[Trading Name]"),
    ("RECEIPT", "Payment receipt", "Payment receipt for invoice [Invoice Number]",
     "Hi [Contact First Name],\n\nThank you for your payment of [Currency Code] [Payment Amount] for invoice [Invoice Number].\n\nThanks,\n[Trading Name]"),
    ("PAYSLIP", "Payslip", "Your payslip for [Pay Period]",
     "Hi [Employee First Name],\n\nYour payslip for the period [Pay Period] is attached.\n\nThanks,\n[Trading Name]"),
];
