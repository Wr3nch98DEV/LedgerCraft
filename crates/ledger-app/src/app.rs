//! Application state, messages, routing and data loading shared by both layouts.

use crate::backend::{Backend, Reply};
use crate::theme::{self, Skin};
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
    NewProduct,
    Journal,
    Placeholder(&'static str),
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
}
impl Lookups {
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
    Nav(Route),
    Back,
    Session(Reply),
    LookupsLoaded(Box<Lookups>),
    Loaded(u64, Reply),
    Refresh,
    ToggleTheme,
    SetSkin(Skin),
    ToggleNew,
    ToggleGear,
    OpenMenu(Option<&'static str>),
    CloseMenus,
    Search(String),
    SearchGo,
    ListQuery(String),
    ListTab(&'static str),
    Toast(String, bool),
    DropToast(u64),
    Key(keyboard::Event),
    Doc(doc::Msg),
    Pay(payments::Msg),
    Bank(banking::Msg),
    Report(reports::Msg),
    Setup(setup::Msg),
    Form(forms::Msg),
    People(people::Msg),
    /// New invoice/bill for a given contact.
    NewDocFor(String, i64),
    Noop,
}

pub struct App {
    pub be: Backend,
    pub skin: Skin,
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
    pub list_q: String,
    pub list_tab: &'static str,
    pub toasts: Vec<(u64, String, bool)>,
    pub toast_seq: u64,
    pub doc: Option<doc::Editor>,
    pub pay: Option<payments::State>,
    pub form: Option<forms::State>,
    pub people: people::State,
    pub prefill_contact: Option<i64>,
    pub bank: banking::State,
    pub report: reports::State,
    pub setup: setup::State,
    /// Screen to open once the company is loaded (`--route`, used for testing).
    pub start_route: Option<Route>,
    pub pending_nav: Option<Route>,
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
        "doc" => Route::Doc { kind: parts.get(1)?.to_string(), id: id(2) }, "journal" => Route::Journal, "payroll" => Route::Payroll, "payrun" => Route::PayRun(id(1)?), "taxes" => Route::Taxes, "taxreturn" => Route::TaxReturn(id(1)?), "newcustomer" => Route::NewContact(true), "newvendor" => Route::NewContact(false), "newproduct" => Route::NewProduct, "receive" => Route::ReceivePayment(id(1)), "paybills" => Route::PayBills, "spend" => Route::Spend(id(1)),
        _ => return None,
    })
}

fn prefs_path(be: &Backend) -> std::path::PathBuf { be.st.data_dir.join("ui-preferences.json") }

impl App {
    pub fn boot(data_dir: std::path::PathBuf, start: Option<Route>, skin: Option<Skin>) -> (App, Task<Message>) {
        let be = Backend::new(data_dir);
        let prefs: Value = std::fs::read_to_string(prefs_path(&be)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}));
        let app = App {
            skin: Skin::from_key(prefs["layout"].as_str().unwrap_or("quickbooks")),
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
            list_q: String::new(),
            list_tab: "all",
            toasts: vec![],
            toast_seq: 0,
            doc: None,
            pay: None,
            form: None,
            people: people::State::default(),
            prefill_contact: None,
            bank: banking::State::default(),
            report: reports::State::default(),
            setup: setup::State::default(),
            start_route: start,
            pending_nav: None,
        };
        let mut app = app;
        if let Some(s) = skin { app.skin = s; }
        let t = app.be.call("session.get", json!({}), Message::Session);
        (app, t)
    }

    pub fn title(&self) -> String {
        let org = self.session.as_ref().and_then(|s| s["org"]["name"].as_str()).unwrap_or("LedgerCraft");
        format!("{org} — LedgerCraft")
    }

    pub fn theme(&self) -> iced::Theme { theme::iced_theme(theme::tokens(self.skin, self.dark)) }

    pub fn setting(&self, k: &str) -> String { self.session.as_ref().and_then(|s| s["settings"][k].as_str()).unwrap_or("").to_string() }
    pub fn country(&self) -> String { self.setting("country") }
    pub fn date_style(&self) -> DateStyle {
        match self.skin { Skin::Xero => DateStyle::Long, Skin::QuickBooks => if self.country() == "US" { DateStyle::Us } else { DateStyle::Intl } }
    }
    pub fn d(&self, s: &str) -> String { crate::ui::date(s, self.date_style()) }
    pub fn term(&self, k: &str) -> &'static str { crate::vocab::term(self.skin, k) }
    pub fn org_name(&self) -> String { self.session.as_ref().and_then(|s| s["org"]["name"].as_str()).unwrap_or("").to_string() }
    pub fn user_name(&self) -> String { self.session.as_ref().and_then(|s| s["user"]["name"].as_str()).unwrap_or("").to_string() }

    pub fn save_prefs(&self) {
        let _ = std::fs::write(prefs_path(&self.be), json!({"layout": self.skin.key(), "dark": self.dark}).to_string());
    }

    pub fn toast(&mut self, text: impl Into<String>, error: bool) -> Task<Message> {
        self.toast_seq += 1;
        let id = self.toast_seq;
        self.toasts.push((id, text.into(), error));
        Task::perform(async move { tokio::time::sleep(Duration::from_millis(if error { 6000 } else { 3000 })).await; id }, Message::DropToast)
    }

    pub fn load_lookups(&self) -> Task<Message> {
        let be = self.be.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let get = |m: &str, p: Value| be.call_now(m, p).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
                    Lookups { accounts: get("accounts.list", json!({})), tax_rates: get("taxRates.list", json!({})), items: get("items.list", json!({})), contacts: get("contacts.list", json!({"filter": "all"})) }
                }).await.unwrap_or_default()
            },
            |l| Message::LookupsLoaded(Box::new(l)),
        )
    }

    /// Navigate, remembering where we came from.
    pub fn go(&mut self, r: Route) -> Task<Message> {
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
        self.page_gen += 1;
        self.data = None;
        self.error = None;
        let g = self.page_gen;
        let call = |m: &str, p: Value| self.be.call(m, p, move |r| Message::Loaded(g, r));
        match self.route.clone() {
            Route::Dashboard => crate::dashboard::load(self),
            Route::AllSales => call("docs.list", json!({"type": "SALES", "tab": self.list_tab, "q": self.list_q, "limit": 300})),
            Route::Invoices => call("docs.list", json!({"type": "ACCREC", "tab": self.list_tab, "q": self.list_q, "limit": 300})),
            Route::Quotes => call("docs.list", json!({"type": "QUOTE", "tab": self.list_tab, "q": self.list_q, "limit": 300})),
            Route::CreditNotes => call("docs.list", json!({"type": "ACCRECCREDIT", "tab": self.list_tab, "q": self.list_q, "limit": 300})),
            Route::Bills | Route::Expenses => call("docs.list", json!({"type": "ACCPAY", "tab": self.list_tab, "q": self.list_q, "limit": 300})),
            Route::PurchaseOrders => call("docs.list", json!({"type": "PO", "tab": self.list_tab, "q": self.list_q, "limit": 300})),
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
                        Ok(json!({"contact": c, "sales": sales["rows"], "bills": bills["rows"]}))
                    }).await.unwrap_or_else(|e| Err(e.to_string()))
                }, move |r| Message::Loaded(g, r))
            }
            Route::Banking(acc) => banking::load(self, acc),
            Route::Reports => call("reports.catalogue", json!({})),
            Route::Report(key) => reports::load(self, &key),
            Route::Doc { kind, id } => doc::open(self, kind, id),
            Route::ReceivePayment(contact) => payments::open_receive(self, contact),
            Route::PayBills => payments::open_pay_bills(self),
            Route::Spend(acc) => payments::open_spend(self, acc),
            Route::NewContact(_) | Route::NewProduct | Route::Journal => forms::open(self),
            Route::Payroll | Route::PayRun(_) | Route::Taxes | Route::TaxReturn(_) => people::load(self),
            Route::Setup => setup::load(self),
            Route::Settings | Route::Placeholder(_) | Route::Login => Task::none(),
        }
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Noop => Task::none(),
            Message::Nav(r) => self.go(r),
            Message::Back => {
                if let Some(r) = self.history.pop() { self.route = r; self.load() } else { Task::none() }
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
                    Ok(v) => { self.data = Some(v); Task::none() }
                    Err(e) => { self.error = Some(e); Task::none() }
                }
            }
            Message::Refresh => { let a = self.load(); Task::batch([a, self.load_lookups()]) }
            Message::ToggleTheme => { self.dark = !self.dark; self.save_prefs(); Task::none() }
            Message::SetSkin(s) => {
                self.skin = s;
                self.save_prefs();
                self.gear_menu = false;
                let t = self.toast(format!("Switched to the {} layout — same books, same data.", s.label()), false);
                Task::batch([t, self.load()])
            }
            Message::ToggleNew => { self.new_menu = !self.new_menu; self.gear_menu = false; self.menu = None; Task::none() }
            Message::ToggleGear => { self.gear_menu = !self.gear_menu; self.new_menu = false; self.menu = None; Task::none() }
            Message::OpenMenu(m) => { self.menu = if self.menu == m { None } else { m }; self.new_menu = false; self.gear_menu = false; Task::none() }
            Message::CloseMenus => { self.new_menu = false; self.gear_menu = false; self.menu = None; Task::none() }
            Message::Search(s) => { self.search = s; Task::none() }
            Message::SearchGo => {
                // Search runs as a filtered transaction list for now.
                let q = std::mem::take(&mut self.search);
                let t = self.go(if self.skin == Skin::QuickBooks { Route::AllSales } else { Route::Invoices });
                self.list_q = q;
                Task::batch([t, self.load()])
            }
            Message::ListQuery(q) => { self.list_q = q; self.load() }
            Message::ListTab(t) => { self.list_tab = t; self.load() }
            Message::Toast(s, e) => self.toast(s, e),
            Message::DropToast(id) => { self.toasts.retain(|t| t.0 != id); Task::none() }
            Message::Key(ev) => self.on_key(ev),
            Message::Doc(m) => doc::update(self, m),
            Message::Pay(m) => payments::update(self, m),
            Message::Bank(m) => banking::update(self, m),
            Message::Report(m) => reports::update(self, m),
            Message::Setup(m) => setup::update(self, m),
            Message::Form(m) => forms::update(self, m),
            Message::People(m) => people::update(self, m),
            Message::NewDocFor(kind, cid) => { self.prefill_contact = Some(cid); self.go(Route::Doc { kind, id: None }) }
        }
    }

    fn on_key(&mut self, ev: keyboard::Event) -> Task<Message> {
        if let keyboard::Event::KeyPressed { key, modifiers, .. } = ev {
            use keyboard::key::Named;
            match key.as_ref() {
                keyboard::Key::Named(Named::Escape) => return self.update(Message::CloseMenus),
                keyboard::Key::Character("s") if modifiers.command() => {
                    if matches!(self.route, Route::Doc { .. }) { return doc::update(self, doc::Msg::Save(false)); }
                }
                _ => {}
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> { keyboard::listen().map(Message::Key) }

    pub fn view(&self) -> Element<'_, Message> {
        theme::set_current(theme::tokens(self.skin, self.dark));
        shell::view(self)
    }
}
