//! Application state, messages, routing and data loading shared by both layouts.

use crate::backend::{Backend, Reply};
use crate::theme::{self, Layout, Skin};
use crate::ui::DateStyle;
use crate::{banking, doc, forms, payments, people, reports, setup, shell};
use iced::{Element, Subscription, Task, keyboard};
use serde_json::{Value, json};
use std::time::Duration;

/// Where the user is. Both layouts share routes; each shows them in its own navigation.
#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    Setup,
    Login,
    Dashboard,
    AllSales,
    Invoices,
    Quotes,
    CreditNotes,
    Customers,
    Products,
    Expenses,
    Bills,
    Vendors,
    PurchaseOrders,
    Contacts,
    Contact(i64),
    Banking(Option<i64>),
    Chart,
    Reports,
    Report(String),
    Settings,
    Doc { kind: String, id: Option<i64> },
    ReceivePayment(Option<i64>),
    PayBills,
    Spend(Option<i64>),
    NewContact(bool),
    Payroll,
    PayRun(i64),
    Taxes,
    TaxReturn(i64),
    Projects,
    Mileage,
    Transfer,
    Check(Option<i64>),
    Deposit,
    Reconcile,
    /// Bank account register (QuickBooks Desktop "Use Register").
    Register(Option<i64>),
    /// QuickBooks Desktop Calendar (transactions due by day).
    Calendar,
    /// QuickBooks Desktop Create Statements.
    Statements,
    /// QuickBooks Desktop Memorized Transaction List.
    Memorized,
    /// QuickBooks Desktop Pay Sales Tax.
    PaySalesTax,
    /// QuickBooks Desktop Adjust Quantity/Value on Hand.
    AdjustInventory,
    /// Help: keyboard shortcuts and where things are.
    Help,
    /// QuickBooks Desktop Reports ▸ Memorized Reports.
    MemorizedReports,
    /// QuickBooks Desktop Doc Center (the files inbox).
    DocCenter,
    /// QuickBooks Desktop Pay Liabilities (payroll withholding and pension).
    PayLiabilities,
    /// QuickBooks Desktop Lists ▸ Class List (tracking categories).
    ClassList,
    /// QuickBooks Desktop Lists ▸ Payroll Item List.
    PayrollItems,
    /// QuickBooks Desktop Lists ▸ Other Names List (contacts that are neither customers nor vendors).
    OtherNames,
    /// QuickBooks Desktop Customers ▸ Assess Finance Charges.
    FinanceCharges,
    /// QuickBooks Desktop Lists ▸ Templates (invoice/quote/PO form designs).
    Templates,
    /// QuickBooks Desktop Lists ▸ Price Level List.
    PriceLevels,
    /// QuickBooks Desktop File ▸ Print Forms (invoices not yet printed or sent).
    PrintForms,
    /// QuickBooks Desktop Employees ▸ Process Payroll Forms (filing files per pay run).
    PayrollForms,
    /// QuickBooks Desktop Favorites ▸ Customize Favorites.
    CustomizeFavorites,
    /// QuickBooks Desktop Inventory ▸ Build Assemblies.
    BuildAssemblies,
    /// QuickBooks Desktop Inventory ▸ Inventory Center.
    InventoryCenter,
    /// QuickBooks Desktop Edit Customer / Edit Vendor.
    EditContact(i64),
    /// QuickBooks Desktop Edit Item.
    EditItem(i64),
    /// QuickBooks Desktop Edit Account.
    EditAccount(i64),
    /// QuickBooks Desktop Customers ▸ Create Batch Invoices.
    BatchInvoices,
    /// A saved check or expense (spend money), opened from the register.
    BankTx(i64),
    /// QuickBooks Desktop Company ▸ Reminders.
    Reminders,
    /// QuickBooks Desktop Edit ▸ Find (Ctrl+F): transactions by name, number, amount or date.
    Find,
    /// Apps / connected apps: what LedgerCraft connects to and how.
    Apps,
    /// Fixed assets register: draft, registered and disposed assets; depreciation runs.
    FixedAssets,
    /// Audit log / History and notes.
    AuditLog,
    /// Currencies and exchange rates.
    Currencies,
    /// Budgets: income and expense accounts by month for a financial year.
    Budgets,
    /// Expense claims (receipts and mileage) with submit/approve.
    ExpenseClaims,
    /// Bank rules.
    BankRules,
    /// Import and export data (CSV).
    ImportExport,
    /// Find and recode (Xero) / Reclassify transactions (QuickBooks).
    Recode,
    /// Custom reports: build your own report rows and formulas.
    CustomReports,
    /// Payroll settings: pay schedules and leave types.
    PayrollSettings,
    /// QuickBooks Desktop Enter Time (single activity against a project task).
    EnterTime,
    NewProduct,
    NewAccount,
    Journal,
}

/// A choice in a picker (accounts, tax rates, items, contacts…).
#[derive(Debug, Clone, PartialEq)]
pub struct Opt { pub id: i64, pub label: String }
impl std::fmt::Display for Opt { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.label) } }

/// Reference data used by forms and pickers.
#[derive(Debug, Clone, Default)]
pub struct Lookups {
    pub accounts: Vec<Value>,
    pub tax_rates: Vec<Value>,
    pub items: Vec<Value>,
    pub contacts: Vec<Value>,
    pub price_levels: Vec<Value>,
    /// Form templates (branding themes), default first.
    pub templates: Vec<Value>,
}
impl Lookups {
    pub fn template_opts(&self) -> Vec<Opt> {
        self.templates.iter().map(|t| Opt { id: t["id"].as_i64().unwrap_or(0), label: t["name"].as_str().unwrap_or("").to_string() }).collect()
    }
    /// The price level adjustment (basis points) for a customer, if they have one.
    pub fn price_level_bp(&self, contact_id: i64) -> Option<i64> {
        let level = self.contacts.iter().find(|c| c["id"].as_i64() == Some(contact_id))?["price_level_id"].as_i64()?;
        self.price_levels.iter().find(|p| p["id"].as_i64() == Some(level))?["percent_bp"].as_i64()
    }
    pub fn account_opts(&self, f: impl Fn(&Value) -> bool) -> Vec<Opt> {
        self.accounts.iter().filter(|a| a["status"] == "ACTIVE" && f(a)).map(|a| Opt { id: a["id"].as_i64().unwrap_or(0), label: format!("{} {}", a["code"].as_str().unwrap_or(""), a["name"].as_str().unwrap_or("")) }).collect()
    }
    pub fn bank_opts(&self) -> Vec<Opt> {
        self.accounts.iter().filter(|a| a["status"] == "ACTIVE" && (a["type"] == "BANK" || a["enable_payments"] == 1))
            .map(|a| Opt { id: a["id"].as_i64().unwrap_or(0), label: a["name"].as_str().unwrap_or("").to_string() }).collect()
    }
    pub fn tax_opts(&self) -> Vec<Opt> {
        self.tax_rates.iter().filter(|t| t["status"] == "ACTIVE").map(|t| Opt { id: t["id"].as_i64().unwrap_or(0), label: t["name"].as_str().unwrap_or("").to_string() }).collect()
    }
    pub fn item_opts(&self) -> Vec<Opt> {
        self.items.iter().filter(|i| i["status"] == "ACTIVE").map(|i| Opt { id: i["id"].as_i64().unwrap_or(0), label: format!("{} — {}", i["code"].as_str().unwrap_or(""), i["name"].as_str().unwrap_or("")) }).collect()
    }
    pub fn contact_opts(&self, customers: bool) -> Vec<Opt> {
        let key = if customers { "is_customer" } else { "is_supplier" };
        let mut v: Vec<Opt> = self.contacts.iter().filter(|c| c["status"] == "ACTIVE" && c[key] == 1).map(|c| Opt { id: c["id"].as_i64().unwrap_or(0), label: c["name"].as_str().unwrap_or("").to_string() }).collect();
        // Contacts that are neither yet still appear, so a new contact can be used either way.
        v.extend(self.contacts.iter().filter(|c| c["status"] == "ACTIVE" && c["is_customer"] != 1 && c["is_supplier"] != 1).map(|c| Opt { id: c["id"].as_i64().unwrap_or(0), label: c["name"].as_str().unwrap_or("").to_string() }));
        v
    }
    pub fn tax_rate_bp(&self, id: Option<i64>) -> i64 {
        id.and_then(|id| self.tax_rates.iter().find(|t| t["id"] == id)).and_then(|t| t["rate_bp"].as_i64()).unwrap_or(0)
    }
    pub fn item(&self, id: i64) -> Option<&Value> { self.items.iter().find(|i| i["id"] == id) }
    pub fn account_name(&self, id: Option<i64>) -> String {
        id.and_then(|id| self.accounts.iter().find(|a| a["id"] == id)).map(|a| a["name"].as_str().unwrap_or("").to_string()).unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    /// The fixed assets, audit log and currencies screens.
    Extras(crate::extras::Msg),
    Nav(Route),
    Back,
    Session(Reply),
    LookupsLoaded(Box<Lookups>),
    Loaded(u64, Reply),
    Refresh,
    ToggleTheme,
    SetSkin(Skin),
    SetLayout(Layout),
    /// QuickBooks Desktop layout: windows, icon bar panes, Home tabs.
    Desk(crate::desktop::Msg),
    ToggleNew,
    ToggleGear,
    OpenMenu(Option<&'static str>),
    CloseMenus,
    Search(String),
    SearchGo,
    SearchResults(String, Reply),
    OpenResult(String, i64),
    ListQuery(String),
    ListTab(&'static str),
    DropToast(u64),
    Key(keyboard::Event),
    Doc(doc::Msg),
    Pay(payments::Msg),
    Bank(banking::Msg),
    Report(reports::Msg),
    Setup(setup::Msg),
    Form(forms::Msg),
    Rec(crate::reconcile::Msg),
    Settings(crate::settings::Msg),
    People(people::Msg),
    /// New invoice/bill for a given contact.
    NewDocFor(String, i64),
    /// Turn 1099 tracking on/off for a vendor (US).
    Set1099(i64, bool),
    /// Show an engine error as a toast.
    Failed(String),
    /// Desktop Receive Items: Enter Bills opened on the Items tab.
    ReceiveItems,
    /// File ▸ Back Up Company: choose where, then write a backup copy.
    BackupCompany,
    BackedUp(Result<Option<String>, String>),
    /// Put the cursor in the search box (Desktop Ctrl+F / Edit ▸ Find).
    FocusSearch,
    Noop,
}

pub struct App {
    pub be: Backend,
    /// Vocabulary family (QuickBooks or Xero); always `layout.skin()`.
    pub skin: Skin,
    pub layout: Layout,
    pub desk: crate::desktop::State,
    pub extras: crate::extras::State,
    pub dark: bool,
    pub route: Route,
    pub history: Vec<Route>,
    pub session: Option<Value>,
    pub lk: Lookups,
    /// Data for the current list/dashboard/report page.
    pub data: Option<Value>,
    pub error: Option<String>,
    pub page_gen: u64,
    pub new_menu: bool,
    pub gear_menu: bool,
    pub menu: Option<&'static str>,
    pub search: String,
    pub search_results: Vec<Value>,
    pub list_q: String,
    pub list_tab: &'static str,
    pub toasts: Vec<(u64, String, bool)>,
    pub toast_seq: u64,
    pub doc: Option<doc::Editor>,
    pub pay: Option<payments::State>,
    pub form: Option<forms::State>,
    pub rec: crate::reconcile::State,
    pub people: people::State,
    pub settings: crate::settings::State,
    pub prefill_contact: Option<i64>,
    pub bank: banking::State,
    pub report: reports::State,
    pub setup: setup::State,
    /// Screen to open once the company is loaded (`--route`, used for testing).
    pub start_route: Option<Route>,
    pub pending_nav: Option<Route>,
    /// Layout chosen on the command line (`--skin`), which wins over saved preferences.
    pub skin_locked: bool,
}

/// Parse a `--route` argument such as `invoices`, `doc:ACCREC:117`, `doc:ACCPAY:new`, `contact:3`, `report:balance-sheet`.
pub fn parse_route(s: &str) -> Option<Route> {
    let parts: Vec<&str> = s.split(':').collect();
    let id = |i: usize| parts.get(i).and_then(|x| x.parse::<i64>().ok());
    Some(match parts[0] {
        "dashboard" => Route::Dashboard, "sales" => Route::AllSales, "invoices" => Route::Invoices, "quotes" => Route::Quotes, "credits" => Route::CreditNotes,
        "customers" => Route::Customers, "products" => Route::Products, "expenses" => Route::Expenses, "bills" => Route::Bills, "vendors" => Route::Vendors,
        "pos" => Route::PurchaseOrders, "contacts" => Route::Contacts, "contact" => Route::Contact(id(1)?), "banking" => Route::Banking(id(1)), "chart" => Route::Chart,
        "reports" => Route::Reports, "report" => Route::Report(parts.get(1)?.to_string()), "settings" => Route::Settings, "setup" => Route::Setup,
        "doc" => Route::Doc { kind: parts.get(1)?.to_string(), id: id(2) }, "journal" => Route::Journal, "payroll" => Route::Payroll, "payrun" => Route::PayRun(id(1)?), "taxes" => Route::Taxes, "projects" => Route::Projects, "mileage" => Route::Mileage, "transfer" => Route::Transfer, "check" => Route::Check(id(1)), "deposit" => Route::Deposit, "reconcile" => Route::Reconcile, "register" => Route::Register(id(1)), "calendar" => Route::Calendar, "statements" => Route::Statements, "memorized" => Route::Memorized, "paysalestax" => Route::PaySalesTax, "adjustinventory" => Route::AdjustInventory, "help" => Route::Help, "memreports" => Route::MemorizedReports, "docs" => Route::DocCenter, "payliabilities" => Route::PayLiabilities, "classes" => Route::ClassList, "payitems" => Route::PayrollItems, "othernames" => Route::OtherNames, "financecharges" => Route::FinanceCharges, "templates" => Route::Templates, "pricelevels" => Route::PriceLevels, "printforms" => Route::PrintForms, "payrollforms" => Route::PayrollForms, "favorites" => Route::CustomizeFavorites, "build" => Route::BuildAssemblies, "inventory" => Route::InventoryCenter, "editcontact" => Route::EditContact(id(1)?), "edititem" => Route::EditItem(id(1)?), "editaccount" => Route::EditAccount(id(1)?), "batchinvoices" => Route::BatchInvoices, "banktx" => Route::BankTx(id(1)?), "reminders" => Route::Reminders, "find" => Route::Find, "apps" => Route::Apps, "assets" => Route::FixedAssets, "audit" => Route::AuditLog, "currencies" => Route::Currencies, "budgets" => Route::Budgets, "claims" => Route::ExpenseClaims, "bankrules" => Route::BankRules, "importexport" => Route::ImportExport, "recode" => Route::Recode, "customreports" => Route::CustomReports, "payrollsettings" => Route::PayrollSettings, "time" => Route::EnterTime, "taxreturn" => Route::TaxReturn(id(1)?), "newcustomer" => Route::NewContact(true), "newvendor" => Route::NewContact(false), "newproduct" => Route::NewProduct, "newaccount" => Route::NewAccount, "receive" => Route::ReceivePayment(id(1)), "paybills" => Route::PayBills, "spend" => Route::Spend(id(1)),
        _ => return None,
    })
}

fn prefs_path(be: &Backend) -> std::path::PathBuf { be.st.data_dir.join("ui-preferences.json") }

impl App {
    /// The preferences file (layout, theme, memorized reports…).
    pub fn prefs(&self) -> Value {
        std::fs::read_to_string(prefs_path(&self.be)).ok().and_then(|s| serde_json::from_str(&s).ok()).filter(Value::is_object).unwrap_or(json!({}))
    }
    pub fn write_prefs(&self, v: &Value) { let _ = std::fs::write(prefs_path(&self.be), v.to_string()); }
    /// Memorized reports for the open company.
    pub fn memorized_reports(&self) -> Vec<Value> {
        let org = self.org_name();
        self.prefs()["memorized_reports"][org.as_str()].as_array().cloned().unwrap_or_default()
    }
}

/// A saved layout. Files written before the Desktop layout existed ("v" < 2) said "quickbooks" for
/// the only QuickBooks layout there was; those now open in Desktop, the new default.
fn saved_layout(prefs: &Value, v: &Value) -> Layout {
    match v.as_str() {
        None => Layout::Desktop,
        Some("quickbooks") if prefs["v"].as_i64().unwrap_or(1) < 2 => Layout::Desktop,
        Some(k) => Layout::from_key(k),
    }
}

impl App {
    /// Boot with an optional layout family override (tests use this: QuickBooks → the Online layout).
    #[cfg(test)]
    pub fn boot(data_dir: std::path::PathBuf, start: Option<Route>, skin: Option<Skin>) -> (App, Task<Message>) {
        Self::boot_layout(data_dir, start, skin.map(Layout::from))
    }

    pub fn boot_layout(data_dir: std::path::PathBuf, start: Option<Route>, layout: Option<Layout>) -> (App, Task<Message>) {
        let be = Backend::new(data_dir);
        let prefs: Value = std::fs::read_to_string(prefs_path(&be)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}));
        let saved = saved_layout(&prefs, &prefs["layout"]);
        let app = App {
            skin: saved.skin(),
            layout: saved,
            desk: Default::default(),
            extras: Default::default(),
            dark: prefs["dark"].as_bool().unwrap_or(true),
            be,
            route: Route::Dashboard,
            history: vec![],
            session: None,
            lk: Lookups::default(),
            data: None,
            error: None,
            page_gen: 0,
            new_menu: false,
            gear_menu: false,
            menu: None,
            search: String::new(),
            search_results: vec![],
            list_q: String::new(),
            list_tab: "all",
            toasts: vec![],
            toast_seq: 0,
            doc: None,
            pay: None,
            form: None,
            rec: Default::default(),
            people: people::State::default(),
            settings: crate::settings::State::default(),
            prefill_contact: None,
            bank: banking::State::default(),
            report: reports::State::default(),
            setup: setup::State::default(),
            start_route: start,
            pending_nav: None,
            skin_locked: layout.is_some(),
        };
        let mut app = app;
        if let Some(l) = layout { app.set_layout(l); }
        app.load_desk_prefs();
        // Background work: repeating transactions, reminders, outgoing email and daily backups
        // (every 5 minutes, as in the web version). Not in unit tests.
        #[cfg(not(test))]
        ledger_server::jobs::start(app.be.st.clone());
        let t = app.be.call("session.get", json!({}), Message::Session);
        (app, t)
    }

    pub fn title(&self) -> String {
        let org = self.session.as_ref().and_then(|s| s["org"]["name"].as_str()).unwrap_or("LedgerCraft");
        format!("{org} — LedgerCraft")
    }

    pub fn theme(&self) -> iced::Theme { theme::iced_theme(theme::tokens_for(self.layout, self.dark)) }
    pub fn desktop(&self) -> bool { self.layout == Layout::Desktop }
    pub fn set_layout(&mut self, l: Layout) {
        self.layout = l;
        self.skin = l.skin();
        // Desktop keeps a list of open windows; other layouts show one screen at a time.
        if l != Layout::Desktop { self.desk.windows.clear(); }
    }

    pub fn setting(&self, k: &str) -> String { self.session.as_ref().and_then(|s| s["settings"][k].as_str()).unwrap_or("").to_string() }
    pub fn country(&self) -> String { self.setting("country") }
    pub fn date_style(&self) -> DateStyle {
        match self.skin { Skin::Xero => DateStyle::Long, Skin::QuickBooks => if self.country() == "US" { DateStyle::Us } else { DateStyle::Intl } }
    }
    pub fn d(&self, s: &str) -> String { crate::ui::date(s, self.date_style()) }
    pub fn term(&self, k: &str) -> &'static str { crate::vocab::term(self.skin, k) }
    pub fn org_name(&self) -> String { self.session.as_ref().and_then(|s| s["org"]["name"].as_str()).unwrap_or("").to_string() }
    pub fn user_name(&self) -> String { self.session.as_ref().and_then(|s| s["user"]["name"].as_str()).unwrap_or("").to_string() }

    /// Saves layout and theme for the signed-in user (and as the default for this computer,
    /// used before anyone has signed in).
    pub fn save_prefs(&self) {
        let path = prefs_path(&self.be);
        let mut prefs: Value = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).filter(Value::is_object).unwrap_or(json!({}));
        prefs["layout"] = json!(self.layout.key());
        prefs["dark"] = json!(self.dark);
        prefs["v"] = json!(2);
        if let Some(u) = self.user_key() {
            if !prefs["users"].is_object() { prefs["users"] = json!({}); }
            prefs["users"][u] = json!({"layout": self.layout.key(), "dark": self.dark});
        }
        let _ = std::fs::write(path, prefs.to_string());
    }

    /// Who the preferences belong to: the signed-in user's email (or name).
    fn user_key(&self) -> Option<String> {
        let u = &self.session.as_ref()?["user"];
        [&u["email"], &u["name"]].into_iter().filter_map(|v| v.as_str()).map(|s| s.trim().to_lowercase()).find(|s| !s.is_empty())
    }

    /// Applies the signed-in user's saved layout and theme.
    fn apply_user_prefs(&mut self) {
        let Some(u) = self.user_key() else { return };
        let prefs: Value = std::fs::read_to_string(prefs_path(&self.be)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}));
        let mine = &prefs["users"][u.as_str()];
        if !mine["layout"].is_null() && !self.skin_locked { let l = saved_layout(&prefs, &mine["layout"]); self.set_layout(l); }
        if let Some(d) = mine["dark"].as_bool() { self.dark = d; }
    }

    pub fn toast(&mut self, text: impl Into<String>, error: bool) -> Task<Message> {
        self.toast_seq += 1;
        let id = self.toast_seq;
        self.toasts.push((id, text.into(), error));
        Task::perform(async move { tokio::time::sleep(Duration::from_millis(if error { 6000 } else { 3000 })).await; id }, Message::DropToast)
    }

    pub fn load_lookups(&self) -> Task<Message> {
        let be = self.be.clone();
        let qb = self.skin == Skin::QuickBooks;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    if qb { let _ = be.call_now("accounts.undeposited", json!({})); }
                    let get = |m: &str, p: Value| be.call_now(m, p).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
                    Lookups { accounts: get("accounts.list", json!({})), tax_rates: get("taxRates.list", json!({})), items: get("items.list", json!({})), contacts: get("contacts.list", json!({"filter": "all"})), price_levels: get("priceLevels.list", json!({})), templates: get("branding.list", json!({})) }
                }).await.unwrap_or_default()
            },
            |l| Message::LookupsLoaded(Box::new(l)),
        )
    }

    /// Navigate, remembering where we came from.
    pub fn go(&mut self, r: Route) -> Task<Message> {
        // Desktop shows a customer/vendor inside its Center.
        let r = match r {
            Route::Contact(id) if self.desktop() => {
                self.desk.sel = Some(id);
                self.desk.detail = None;
                let vendor = self.lk.contacts.iter().find(|c| c["id"].as_i64() == Some(id)).map(|c| c["is_supplier"] == 1 && c["is_customer"] != 1).unwrap_or(false);
                if vendor { Route::Vendors } else { Route::Customers }
            }
            r => r,
        };
        if self.desktop() && !matches!(r, Route::Setup | Route::Login) && !matches!(self.route, Route::Setup | Route::Login) {
            self.new_menu = false; self.gear_menu = false; self.menu = None;
            let same = self.desk.windows.get(self.desk.active).map(|w| w.route == r).unwrap_or(false);
            if !same {
                // A new document that's just been saved keeps its window.
                let saved_new = matches!((&self.route, &r), (Route::Doc { kind: a, id: None }, Route::Doc { kind: b, id: Some(_) }) if a == b);
                if saved_new {
                    let i = self.desk.active;
                    if let Some(w) = self.desk.windows.get_mut(i) { w.route = r.clone(); }
                } else {
                    self.stash_window();
                    if let Some(i) = self.desk.windows.iter().position(|w| w.route == r) {
                        self.history.push(self.route.clone());
                        if self.restore_window(i) { return Task::none(); }
                    }
                }
            }
        }
        if self.route != r { self.history.push(self.route.clone()); if self.history.len() > 50 { self.history.remove(0); } }
        self.route = r;
        self.list_q.clear();
        self.list_tab = "all";
        self.new_menu = false;
        self.gear_menu = false;
        self.menu = None;
        self.load()
    }

    /// Fetch the data for the current route.
    pub fn load(&mut self) -> Task<Message> {
        if self.desktop() { self.ensure_window(); }
        self.page_gen += 1;
        self.data = None;
        self.error = None;
        let g = self.page_gen;
        let call = |m: &str, p: Value| self.be.call(m, p, move |r| Message::Loaded(g, r));
        match self.route.clone() {
            Route::Dashboard if self.desktop() && self.desk.insights => crate::desktop::load_insights(self),
            Route::Dashboard => crate::dashboard::load(self),
            Route::AllSales | Route::Invoices if self.desktop() => crate::desktop::load_tracker(self, true),
            Route::Bills | Route::Expenses if self.desktop() => crate::desktop::load_tracker(self, false),
            Route::AllSales => crate::lists::load_all_sales(self),
            Route::Invoices => call("docs.list", json!({"type": "ACCREC", "tab": self.list_tab, "q": self.list_q, "limit": 300})),
            Route::Quotes => call("docs.list", json!({"type": "QUOTE", "tab": self.list_tab, "q": self.list_q, "limit": 300})),
            Route::CreditNotes => call("docs.list", json!({"type": "ACCRECCREDIT", "tab": self.list_tab, "q": self.list_q, "limit": 300})),
            Route::Bills => call("docs.list", json!({"type": "ACCPAY", "tab": self.list_tab, "q": self.list_q, "limit": 300})),
            Route::Expenses => crate::lists::load_expenses(self),
            Route::PurchaseOrders => call("docs.list", json!({"type": "PO", "tab": self.list_tab, "q": self.list_q, "limit": 300})),
            Route::Customers if self.desktop() => {
                // Customer:Job — the jobs (projects) are listed under each customer.
                let t1 = call("contacts.list", json!({"filter": "customers", "q": self.list_q}));
                let t2 = self.be.call("projects.list", json!({"status": "ALL"}), |r| Message::Desk(crate::desktop::Msg::JobsLoaded(r)));
                Task::batch([t1, t2])
            }
            Route::Customers => call("contacts.list", json!({"filter": "customers", "q": self.list_q})),
            Route::Vendors => call("contacts.list", json!({"filter": "suppliers", "q": self.list_q})),
            Route::Contacts => call("contacts.list", json!({"filter": self.list_tab, "q": self.list_q})),
            Route::Products => call("items.list", json!({"q": self.list_q})),
            Route::Chart => call("accounts.list", json!({})),
            Route::Contact(id) => {
                let be = self.be.clone();
                Task::perform(async move {
                    tokio::task::spawn_blocking(move || {
                        let c = be.call_now("contacts.get", json!({"id": id}))?;
                        let sales = be.call_now("docs.list", json!({"type": "SALES", "contact_id": id, "limit": 200})).unwrap_or(json!({"rows": []}));
                        let bills = be.call_now("docs.list", json!({"type": "ACCPAY", "contact_id": id, "limit": 200})).unwrap_or(json!({"rows": []}));
                        let history = be.call_now("history.get", json!({"entity": "contact", "id": id})).unwrap_or(json!([]));
                        Ok(json!({"contact": c, "sales": sales["rows"], "bills": bills["rows"], "history": history}))
                    }).await.unwrap_or_else(|e| Err(e.to_string()))
                }, move |r| Message::Loaded(g, r))
            }
            Route::Banking(acc) => banking::load(self, acc),
            Route::Reports => call("reports.catalogue", json!({})),
            Route::Report(key) => reports::load(self, &key),
            Route::Doc { kind, id } => doc::open(self, kind, id),
            Route::ReceivePayment(contact) => payments::open_receive(self, contact),
            Route::PayBills => payments::open_pay_bills(self),
            Route::Spend(acc) => payments::open_spend(self, acc, false),
            Route::Check(acc) => payments::open_spend(self, acc, true),
            Route::BankTx(id) => payments::open_tx(self, id),
            Route::Deposit => payments::open_deposit(self),
            Route::Reconcile => crate::reconcile::load(self),
            Route::Calendar => crate::desktop::load_calendar(self),
            Route::ClassList => call("tracking.list", json!({})),
            Route::PayrollItems => call("payroll.settings", json!({})),
            Route::Reminders => crate::desktop::load_reminders(self),
            Route::FixedAssets | Route::AuditLog | Route::Currencies | Route::Budgets | Route::ExpenseClaims | Route::BankRules | Route::ImportExport | Route::Recode | Route::CustomReports | Route::PayrollSettings => crate::extras::load(self),
            Route::Apps => { self.data = Some(json!({})); Task::none() }
            Route::Find => { self.data = Some(json!({"rows": null})); Task::none() }
            Route::BatchInvoices => call("contacts.list", json!({"filter": "customers"})),
            Route::Templates => call("branding.list", json!({})),
            Route::PriceLevels => call("priceLevels.list", json!({})),
            Route::CustomizeFavorites => { self.data = Some(json!({})); Task::none() }
            Route::PayrollForms => call("payroll.payRuns", json!({})),
            Route::PrintForms => call("docs.list", json!({"type": "ACCREC", "limit": 5000})),
            Route::OtherNames => call("contacts.list", json!({"filter": "all"})),
            Route::FinanceCharges => {
                // Overdue invoices, plus earlier finance charges so interest isn't charged twice.
                let be = self.be.clone();
                Task::perform(async move {
                    tokio::task::spawn_blocking(move || -> Result<Value, String> {
                        let mut v = be.call_now("docs.list", json!({"type": "ACCREC", "tab": "overdue", "limit": 5000}))?;
                        v["charged"] = be.call_now("docs.list", json!({"type": "ACCREC", "q": crate::desktop::FC_REFERENCE, "limit": 5000}))?["rows"].clone();
                        Ok(v)
                    }).await.unwrap_or_else(|e| Err(e.to_string()))
                }, move |r| Message::Loaded(g, r))
            }
            Route::EnterTime => {
                let be = self.be.clone();
                Task::perform(async move {
                    tokio::task::spawn_blocking(move || -> Result<Value, String> {
                        let mut out = vec![];
                        for p in be.call_now("projects.list", json!({"status": "INPROGRESS"}))?["projects"].as_array().cloned().unwrap_or_default() {
                            let full = be.call_now("projects.get", json!({"id": p["id"]}))?;
                            out.push(json!({"id": p["id"], "name": p["name"], "contact": p["contact_name"], "tasks": full["tasks"]}));
                        }
                        Ok(json!({"projects": out}))
                    }).await.unwrap_or_else(|e| Err(e.to_string()))
                }, move |r| Message::Loaded(g, r))
            }
            Route::PayLiabilities => call("accounts.list", json!({})),
            Route::DocCenter => call("inbox.list", json!({})),
            Route::MemorizedReports => { self.data = Some(json!(self.memorized_reports())); Task::none() }
            Route::Help => { self.data = Some(json!({})); Task::none() }
            Route::AdjustInventory | Route::BuildAssemblies | Route::InventoryCenter => call("items.list", json!({})),
            Route::PaySalesTax => call("agencies.list", json!({})),
            Route::Memorized => call("repeating.list", json!({"kind": "ALL"})),
            Route::Statements => call("contacts.list", json!({"filter": "customers"})),
            Route::Register(acc) => {
                let be = self.be.clone();
                Task::perform(async move {
                    tokio::task::spawn_blocking(move || -> Result<Value, String> {
                        let banks: Vec<Value> = be.call_now("accounts.bank", json!({}))?.as_array().cloned().unwrap_or_default().into_iter().filter(|b| b["status"] == "ACTIVE").collect();
                        let acc = acc.or_else(|| banks.first().and_then(|b| b["id"].as_i64()));
                        let Some(acc) = acc else { return Ok(json!({"banks": banks, "rows": []})) };
                        let rows = be.call_now("bank.transactions", json!({"id": acc, "limit": 1000}))?;
                        let bal = banks.iter().find(|b| b["id"].as_i64() == Some(acc)).and_then(|b| b["ledger_balance_fx"].as_i64()).unwrap_or(0);
                        Ok(json!({"banks": banks, "account": acc, "balance": bal, "rows": rows}))
                    }).await.unwrap_or_else(|e| Err(e.to_string()))
                }, move |r| Message::Loaded(g, r))
            }
            Route::NewContact(_) | Route::NewProduct | Route::NewAccount | Route::Journal | Route::Transfer => forms::open(self),
            Route::EditContact(id) => forms::open_edit(self, id),
            Route::EditItem(id) => forms::open_edit_item(self, id),
            Route::EditAccount(id) => forms::open_edit_account(self, id),
            Route::Payroll | Route::PayRun(_) | Route::Taxes | Route::TaxReturn(_) | Route::Projects | Route::Mileage => people::load(self),
            Route::Setup => setup::load(self),
            Route::Settings => crate::settings::load(self),
            Route::Login => Task::none(),
        }
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        let t = self.update_inner(msg);
        // Desktop grids' type-ahead boxes follow their lines (cheap when nothing changed).
        doc::sync_boxes(self);
        payments::sync_boxes(self);
        t
    }

    fn update_inner(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Noop => Task::none(),
            Message::Nav(r) => self.go(r),
            Message::Back => {
                // Desktop: Cancel/Back closes the window.
                if self.desktop() && !matches!(self.route, Route::Setup | Route::Login) { let i = self.desk.active; return self.close_window(i); }
                if let Some(r) = self.history.pop() { self.route = r; self.load() } else { Task::none() }
            }
            Message::Desk(m) => crate::desktop::update(self, m),
            Message::Extras(m) => crate::extras::update(self, m),
            Message::SetLayout(l) => {
                self.set_layout(l);
                self.save_prefs();
                self.gear_menu = false;
                self.menu = None;
                let t = self.toast(format!("Switched to the {} layout — same books, same data.", l.label()), false);
                Task::batch([t, self.load()])
            }
            Message::Session(r) => match r {
                Ok(v) if v["login_required"] == true => {
                    self.session = None;
                    self.setup.orgs = v["orgs"].as_array().cloned().unwrap_or_default();
                    self.setup.login_org = v["org"]["name"].as_str().unwrap_or("").to_string();
                    self.route = Route::Login;
                    self.data = Some(json!({}));
                    Task::none()
                }
                Ok(v) if !v["org"]["name"].is_null() => {
                    self.session = Some(v);
                    self.apply_user_prefs();
                    // Open the first screen once reference data (accounts, contacts…) is loaded.
                    let next = self.start_route.take().unwrap_or(if self.route == Route::Setup { Route::Dashboard } else { self.route.clone() });
                    self.pending_nav = Some(next);
                    self.load_lookups()
                }
                Ok(_) => { self.route = Route::Setup; self.load() }
                Err(e) if e.contains("sign in") => { self.route = Route::Login; self.data = Some(json!({})); Task::none() }
                Err(e) => {
                    // No organisation open yet (or it could not be opened): show the organisation picker.
                    self.route = Route::Setup;
                    let t = self.load();
                    if e.contains("no organisation") { t } else { Task::batch([t, self.toast(e, true)]) }
                }
            },
            Message::LookupsLoaded(l) => {
                self.lk = *l;
                if let Some(r) = self.pending_nav.take() { self.route = r; return self.load(); }
                if let Some(e) = self.doc.as_mut() { e.contacts = iced::widget::combo_box::State::with_selection(self.lk.contact_opts(matches!(e.kind.as_str(), "ACCREC" | "QUOTE" | "ACCRECCREDIT")), e.contact.as_ref()); }
                Task::none()
            }
            Message::Loaded(g, r) => {
                if g != self.page_gen { return Task::none(); }
                match r {
                    Ok(v) => { self.data = Some(v); crate::desktop::after_load(self) }
                    Err(e) => { self.error = Some(e); Task::none() }
                }
            }
            Message::Refresh => { let a = self.load(); Task::batch([a, self.load_lookups()]) }
            Message::ToggleTheme => { self.dark = !self.dark; self.save_prefs(); Task::none() }
            Message::SetSkin(s) => {
                self.set_layout(Layout::from(s));
                self.save_prefs();
                self.gear_menu = false;
                let t = self.toast(format!("Switched to the {} layout — same books, same data.", s.label()), false);
                Task::batch([t, self.load()])
            }
            Message::ToggleNew => { self.new_menu = !self.new_menu; self.gear_menu = false; self.menu = None; Task::none() }
            Message::ToggleGear => { self.gear_menu = !self.gear_menu; self.new_menu = false; self.menu = None; Task::none() }
            Message::OpenMenu(m) => { self.desk.menu_sel = None; self.menu = if self.menu == m { None } else { m }; self.new_menu = false; self.gear_menu = false; Task::none() }
            Message::CloseMenus => { self.new_menu = false; self.gear_menu = false; self.menu = None; self.search_results.clear(); Task::none() }
            Message::Search(s) => {
                self.search = s.clone();
                if s.trim().chars().count() < 2 { self.search_results.clear(); return Task::none(); }
                self.be.call("search", json!({"q": s.trim(), "limit": 12}), move |r| Message::SearchResults(s.clone(), r))
            }
            Message::SearchResults(q, r) => {
                // Ignore replies for text the user has since changed.
                if q == self.search { self.search_results = r.ok().and_then(|v| v.as_array().cloned()).unwrap_or_default(); }
                Task::none()
            }
            Message::SearchGo => {
                if let Some(first) = self.search_results.first().cloned() {
                    return self.update(Message::OpenResult(first["entity"].as_str().unwrap_or("").to_string(), first["id"].as_i64().unwrap_or(0)));
                }
                Task::none()
            }
            Message::OpenResult(entity, id) => {
                self.search.clear();
                self.search_results.clear();
                let r = match entity.as_str() {
                    "contact" => Route::Contact(id),
                    "document" => Route::Doc { kind: "ACCREC".into(), id: Some(id) },
                    "account" => Route::EditAccount(id),
                    "item" => Route::EditItem(id),
                    "banktx" => Route::BankTx(id),
                    "asset" => Route::FixedAssets,
                    "manual_journal" => Route::Report("journal-report".into()),
                    "project" => { let t = self.go(Route::Projects); let t2 = self.update(Message::Extras(crate::extras::Msg::PjSelect(id))); return Task::batch([t, t2]); }
                    _ => return self.toast("That record can't be opened from search", false),
                };
                self.go(r)
            }
            Message::ListQuery(q) => { self.list_q = q; self.load() }
            // Xero's "Repeating" tab on invoices and bills is the repeating transactions list.
            Message::ListTab("repeating") => self.update(Message::Nav(Route::Memorized)),
            Message::ListTab(t) => { self.list_tab = t; self.load() }
            Message::DropToast(id) => { self.toasts.retain(|t| t.0 != id); Task::none() }
            Message::Key(ev) => self.on_key(ev),
            Message::Doc(m) => doc::update(self, m),
            Message::Pay(m) => payments::update(self, m),
            Message::Bank(m) => banking::update(self, m),
            Message::Report(m) => reports::update(self, m),
            Message::Setup(m) => setup::update(self, m),
            Message::Form(m) => forms::update(self, m),
            Message::Rec(m) => crate::reconcile::update(self, m),
            Message::People(m) => people::update(self, m),
            Message::Settings(m) => crate::settings::update(self, m),
            Message::NewDocFor(kind, cid) => { self.prefill_contact = Some(cid); self.go(Route::Doc { kind, id: None }) }
            Message::Failed(e) => self.toast(e, true),
            Message::BackupCompany => {
                self.menu = None;
                let st = self.be.st.clone();
                let name = format!("{} {}.lcdb", self.org_name().replace(|c: char| !c.is_alphanumeric() && c != ' ', "-"), crate::ui::today_iso());
                Task::perform(async move {
                    let Some(file) = rfd::AsyncFileDialog::new().set_title("Back Up Company").set_file_name(&name).add_filter("LedgerCraft company file", &["lcdb"]).save_file().await else { return Ok(None) };
                    let path = file.path().to_path_buf();
                    tokio::task::spawn_blocking(move || ledger_server::jobs::backup_to(&st, &path).map(|_| Some(path.display().to_string()))).await.unwrap_or_else(|e| Err(e.to_string()))
                }, Message::BackedUp)
            }
            Message::BackedUp(r) => match r {
                Ok(Some(p)) => self.toast(format!("Backup saved to {p}"), false),
                Ok(None) => Task::none(),
                Err(e) => self.toast(format!("Backup failed: {e}"), true),
            },
            Message::ReceiveItems => { let t = self.go(Route::Doc { kind: "ACCPAY".into(), id: None }); let t2 = doc::update(self, doc::Msg::BillTab(true)); Task::batch([t, t2]) }
            Message::FocusSearch => { self.menu = None; iced::widget::operation::focus(crate::desktop::SEARCH_ID) }
            Message::Set1099(cid, on) => {
                if let Some(d) = self.data.as_mut() { d["contact"]["track_1099"] = json!(on as i64); }
                self.be.call("contacts.set1099", json!({"id": cid, "on": on}), |r| match r { Ok(_) => Message::Noop, Err(e) => Message::Failed(e) })
            }
        }
    }

    fn on_key(&mut self, ev: keyboard::Event) -> Task<Message> {
        if let keyboard::Event::KeyPressed { key, modifiers, .. } = ev {
            use keyboard::key::Named;
            match key.as_ref() {
                keyboard::Key::Named(Named::Escape) => return self.update(Message::CloseMenus),
                // Arrow keys and Enter inside an open Desktop menu.
                keyboard::Key::Named(k @ (Named::ArrowUp | Named::ArrowDown | Named::ArrowLeft | Named::ArrowRight | Named::Enter)) if self.desktop() && self.menu.is_some() => {
                    let k = match k { Named::ArrowUp => "up", Named::ArrowDown => "down", Named::ArrowLeft => "left", Named::ArrowRight => "right", _ => "enter" };
                    return crate::desktop::menu_key(self, k);
                }
                keyboard::Key::Named(Named::F5) => return self.update(Message::Refresh),
                keyboard::Key::Named(Named::F3) if self.desktop() => return self.update(Message::FocusSearch),
                keyboard::Key::Character("t") if modifiers.alt() && !modifiers.command() => return self.update(Message::ToggleTheme),
                keyboard::Key::Named(Named::F1) if self.session.is_some() => return self.go(Route::Help),
                // Tab / Shift+Tab move between fields, as in QuickBooks Desktop forms.
                keyboard::Key::Named(Named::Tab) => return if modifiers.shift() { iced::widget::operation::focus_previous() } else { iced::widget::operation::focus_next() },
                keyboard::Key::Character("s") if modifiers.command() => {
                    if matches!(self.route, Route::Doc { .. }) { return doc::update(self, doc::Msg::Save(false)); }
                }
                // QuickBooks Desktop form keys: Alt+A Save & Close, Alt+S Save & New.
                keyboard::Key::Character(c @ ("a" | "s")) if modifiers.alt() && !modifiers.command() && self.desktop() => {
                    let close = c == "a";
                    let m = match &self.route {
                        Route::Doc { .. } if self.doc.is_some() => Some(Message::Doc(if close { doc::Msg::SaveClose } else { doc::Msg::SaveAndNew })),
                        Route::Check(_) | Route::Spend(_) | Route::BankTx(_) | Route::ReceivePayment(_) | Route::PayBills | Route::Deposit if self.pay.is_some() =>
                            Some(Message::Pay(if close { crate::payments::Msg::Save } else { crate::payments::Msg::SaveNew })),
                        Route::NewContact(_) | Route::EditContact(_) | Route::EditItem(_) | Route::EditAccount(_) | Route::NewProduct | Route::Transfer | Route::Journal | Route::NewAccount if self.form.is_some() =>
                            Some(Message::Form(if close { crate::forms::Msg::Save } else { crate::forms::Msg::SaveNew })),
                        _ => None,
                    };
                    if let Some(m) = m { return self.update(m); }
                    if close { return self.update(Message::OpenMenu(Some("Favorites"))); }
                }
                // Alt+letter opens a menu (the underlined letter in QuickBooks Desktop's menu bar).
                keyboard::Key::Character(c) if modifiers.alt() && !modifiers.command() && self.desktop() && self.session.is_some() => {
                    let m = match c { "f" => Some("File"), "e" => Some("Edit"), "v" => Some("View"), "l" => Some("Lists"), "c" => Some("Company"), "i" => Some("Inventory"),
                        "u" => Some("Customers"), "n" => Some("Vendors"), "m" => Some("Employees"), "b" => Some("Banking"), "r" => Some("Reports"), "w" => Some("Window"), "h" => Some("Help"), _ => None };
                    if m.is_some() { return self.update(Message::OpenMenu(m)); }
                }
                // In a transaction: Ctrl+P print, Ctrl+M memorize, Ctrl+N a new one of the same kind.
                keyboard::Key::Character(c @ ("p" | "m" | "n")) if modifiers.command() && self.desktop() && matches!(self.route, Route::Doc { .. }) && self.doc.is_some() => {
                    return match c {
                        "p" => doc::update(self, doc::Msg::Print),
                        "m" => doc::update(self, doc::Msg::Memorize),
                        _ => match self.doc.as_ref() {
                            // Already a new, unsaved form: nothing to do (and nothing to lose).
                            Some(e) if e.id.is_some() => { let kind = e.kind.clone(); self.go(Route::Doc { kind, id: None }) }
                            _ => Task::none(),
                        },
                    };
                }
                // QuickBooks Desktop shortcuts.
                keyboard::Key::Character(c) if modifiers.command() && self.desktop() && self.session.is_some() => {
                    let r = match c { "i" => Some(Route::Doc { kind: "ACCREC".into(), id: None }), "w" => Some(Route::Check(None)), "a" => Some(Route::Chart),
                        "j" => Some(Route::Customers), "r" => Some(Route::Register(None)), "t" => Some(Route::Memorized), "e" => Some(Route::Doc { kind: "ACCPAY".into(), id: None }), _ => None };
                    if let Some(r) = r { return self.go(r); }
                    if c == "f" { return self.go(Route::Find); }
                }
                _ => {}
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> { keyboard::listen().map(Message::Key) }

    pub fn view(&self) -> Element<'_, Message> {
        theme::set_current(theme::tokens_for(self.layout, self.dark));
        shell::view(self)
    }
}
