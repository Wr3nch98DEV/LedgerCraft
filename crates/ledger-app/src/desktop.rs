//! QuickBooks Desktop-style layout (modelled on Enterprise 2024): menu bar, left icon bar with
//! My Shortcuts / View Balances / Open Windows, several open windows shown as tabs, and the Home
//! page workflow diagram. Screens themselves are shared with the other layouts unless a
//! Desktop-specific version exists.

use crate::app::{App, Message, Route};
use crate::icons::icon;
use crate::theme::{self, t};
use crate::ui::{self, BOLD};
use crate::{doc, forms, payments};
use iced::widget::{Space, button, column, container, mouse_area, opaque, pick_list, row, scrollable, stack, text, text_input};
use iced::{Alignment, Element, Fill, Length, Padding, Task};
use serde_json::Value;

/// One open window: its screen plus any unsaved form state, kept while another window is in front.
pub struct Win {
    pub route: Route,
    pub doc: Option<doc::Editor>,
    pub pay: Option<payments::State>,
    pub form: Option<forms::State>,
    pub data: Option<Value>,
}
impl Win { pub fn new(route: Route) -> Win { Win { route, doc: None, pay: None, form: None, data: None } } }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane { #[default] Shortcuts, Balances, Windows }

#[derive(Default)]
pub struct State {
    pub windows: Vec<Win>,
    pub active: usize,
    pub pane: Pane,
    /// Home window: Insights tab instead of the Home Page diagram.
    pub insights: bool,
    /// Customer/Vendor Center: selected name and its details + transactions.
    pub sel: Option<i64>,
    pub detail: Option<Value>,
    /// Report Center category.
    pub report_cat: Option<&'static str>,
    /// Income/Bill Tracker: which summary box is selected ("all" when none).
    pub tracker: &'static str,
    /// Employee Center: selected employee.
    pub emp: Option<i64>,
    /// Calendar: months away from the current month.
    pub cal_offset: i32,
    /// Create Statements: one customer (None = all customers with a balance) and the statement date.
    pub stmt_one: Option<crate::app::Opt>,
    pub stmt_date: String,
    pub stmt_busy: bool,
    /// Register: the blank entry row at the bottom.
    pub reg: RegEntry,
    /// Customer/Vendor Center Transactions tab: the chosen type and its rows.
    pub tx_type: Option<&'static str>,
    pub tx_rows: Vec<Value>,
    /// View ▸ One Window: each screen replaces the last (no window list).
    pub one_window: bool,
    /// Pay Sales Tax: per agency (pay?, amount), the account to pay from and the date.
    pub tax_pay: Vec<(i64, bool, String)>,
    pub tax_from: Option<crate::app::Opt>,
    pub tax_date: String,
    pub tax_busy: bool,
    /// Adjust Quantity/Value on Hand: per item the new quantity, plus date, account and memo.
    pub adj: Vec<(i64, String)>,
    pub adj_date: String,
    pub adj_account: Option<crate::app::Opt>,
    pub adj_memo: String,
    pub adj_busy: bool,
}

/// A transaction typed straight into the register.
#[derive(Default, Clone)]
pub struct RegEntry { pub date: String, pub number: String, pub payee: String, pub account: Option<crate::app::Opt>, pub payment: String, pub deposit: String, pub memo: String, pub busy: bool }

#[derive(Debug, Clone)]
pub enum RegField { Date, Number, Payee, Payment, Deposit, Memo }

#[derive(Debug, Clone)]
pub enum Msg { Activate(usize), Close(usize), CloseAll, Pane(Pane), Insights(bool), Exit, Select(i64), Detail(i64, crate::backend::Reply), ReportCat(&'static str), Tracker(&'static str), Employee(i64), CalMonth(i32), StmtOne(Option<crate::app::Opt>), StmtDate(String), StmtPrint, StmtEmail, StmtDone(Result<String, String>), MemDelete(i64), MemRun, MemDone(crate::backend::Reply, &'static str), Reg(RegField, String), RegAccount(crate::app::Opt), RegRecord, RegSaved(crate::backend::Reply), TxType(Option<&'static str>), TxRows(&'static str, crate::backend::Reply), OneWindow(bool), TaxPick(i64, bool), TaxAmt(i64, String), TaxFrom(crate::app::Opt), TaxDate(String), TaxPay, TaxPaid(crate::backend::Reply), AdjQty(i64, String), AdjDate(String), AdjAccount(crate::app::Opt), AdjMemo(String), AdjSave, AdjSaved(crate::backend::Reply) }

impl App {
    /// Keep the current screen's unsaved state with its window.
    pub fn stash_window(&mut self) {
        let i = self.desk.active;
        let route = self.route.clone();
        let (doc, pay, form, data) = (self.doc.take(), self.pay.take(), self.form.take(), self.data.take());
        if let Some(w) = self.desk.windows.get_mut(i) {
            if w.route == route { w.doc = doc; w.pay = pay; w.form = form; w.data = data; }
        }
    }

    /// Bring window `i` to the front. Returns true when it had saved state (no reload needed).
    pub fn restore_window(&mut self, i: usize) -> bool {
        let Some(w) = self.desk.windows.get_mut(i) else { return false };
        self.desk.active = i;
        self.route = w.route.clone();
        let had = w.doc.is_some() || w.pay.is_some() || w.form.is_some();
        self.doc = w.doc.take();
        self.pay = w.pay.take();
        self.form = w.form.take();
        let data = w.data.take();
        if had { self.data = data.or(Some(serde_json::json!({}))); self.error = None; }
        had
    }

    /// Make sure the current route has a window and it's the active one.
    pub fn ensure_window(&mut self) {
        if matches!(self.route, Route::Setup | Route::Login) { return; }
        // One Window view: the new screen replaces whatever was showing.
        if self.desk.one_window { self.desk.windows = vec![Win::new(self.route.clone())]; self.desk.active = 0; return; }
        match self.desk.windows.iter().position(|w| w.route == self.route) {
            Some(i) => self.desk.active = i,
            None => { self.desk.windows.push(Win::new(self.route.clone())); self.desk.active = self.desk.windows.len() - 1; }
        }
    }

    /// Close window `i` and show the one before it (Home when none are left).
    pub fn close_window(&mut self, i: usize) -> Task<Message> {
        if i >= self.desk.windows.len() { return Task::none(); }
        let was_active = i == self.desk.active;
        if was_active { self.doc = None; self.pay = None; self.form = None; } else { self.stash_window(); }
        self.desk.windows.remove(i);
        if self.desk.windows.is_empty() { self.route = Route::Dashboard; return self.load(); }
        let next = if was_active { i.saturating_sub(1).min(self.desk.windows.len() - 1) } else if i < self.desk.active { self.desk.active - 1 } else { self.desk.active };
        if self.restore_window(next) { Task::none() } else { self.load() }
    }
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    match m {
        Msg::Activate(i) => match app.desk.windows.get(i) { Some(w) => { let r = w.route.clone(); app.go(r) } None => Task::none() },
        Msg::Close(i) => app.close_window(i),
        Msg::CloseAll => {
            app.doc = None; app.pay = None; app.form = None;
            app.desk.windows.clear();
            app.route = Route::Dashboard;
            app.menu = None;
            app.load()
        }
        Msg::Pane(p) => { app.desk.pane = p; Task::none() }
        Msg::Insights(v) => { app.desk.insights = v; Task::none() }
        Msg::Exit => iced::exit(),
        Msg::Select(id) => {
            app.desk.sel = Some(id);
            let be = app.be.clone();
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<Value, String> {
                    let c = be.call_now("contacts.get", serde_json::json!({"id": id}))?;
                    let mut rows = vec![];
                    for t in ["SALES", "ACCPAY", "ACCPAYCREDIT", "QUOTE", "DELAYED", "PO"] {
                        rows.extend(be.call_now("docs.list", serde_json::json!({"type": t, "contact_id": id, "limit": 300})).ok().and_then(|v| v["rows"].as_array().cloned()).unwrap_or_default());
                    }
                    rows.sort_by(|a, b| b["date"].as_str().cmp(&a["date"].as_str()));
                    Ok(serde_json::json!({"contact": c, "rows": rows}))
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, move |r| Message::Desk(Msg::Detail(id, r)))
        }
        Msg::ReportCat(c) => { app.desk.report_cat = Some(c); Task::none() }
        Msg::AdjQty(id, v) => { match app.desk.adj.iter_mut().find(|x| x.0 == id) { Some(x) => x.1 = v, None => app.desk.adj.push((id, v)) } Task::none() }
        Msg::AdjDate(v) => { app.desk.adj_date = v; Task::none() }
        Msg::AdjAccount(a) => { app.desk.adj_account = Some(a); Task::none() }
        Msg::AdjMemo(v) => { app.desk.adj_memo = v; Task::none() }
        Msg::AdjSave => {
            let items = app.data.as_ref().and_then(|d| d.as_array().cloned()).unwrap_or_default();
            let Some(acc) = app.desk.adj_account.clone().or_else(|| default_adj_account(app)) else { return app.toast("Choose the adjustment account", true) };
            let date_in = if app.desk.adj_date.is_empty() { app.d(&ui::today_iso()) } else { app.desk.adj_date.clone() };
            let Some(date) = ui::to_iso(&date_in, app.date_style()) else { return app.toast("Check the adjustment date", true) };
            let mut calls = vec![];
            for (id, new_qty) in &app.desk.adj {
                let Some(item) = items.iter().find(|x| x["id"].as_i64() == Some(*id)) else { continue };
                let Some(new_e4) = crate::doc::parse_e4(new_qty).filter(|_| !new_qty.trim().is_empty()) else { continue };
                let diff = new_e4 - item["qty_on_hand_e4"].as_i64().unwrap_or(0);
                if diff == 0 { continue; }
                let mut p = serde_json::json!({"id": id, "date": date, "quantity": format!("{}", diff as f64 / 10000.0), "account_id": acc.id, "reason": app.desk.adj_memo.trim()});
                // Added stock is valued at the item's purchase cost.
                if diff > 0 { p["unit_cost"] = serde_json::json!(format!("{}", item["purchase_unit_e2"].as_i64().unwrap_or(0) as f64 / 10000.0)); }
                calls.push(p);
            }
            if calls.is_empty() { return app.toast("Enter a new quantity for at least one item", true); }
            let be = app.be.clone();
            app.desk.adj_busy = true;
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<Value, String> {
                    for p in &calls { be.call_now("items.adjust", p.clone())?; }
                    Ok(serde_json::json!({"adjusted": calls.len()}))
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, |r| Message::Desk(Msg::AdjSaved(r)))
        }
        Msg::AdjSaved(r) => {
            app.desk.adj_busy = false;
            match r {
                Ok(v) => { app.desk.adj.clear(); app.desk.adj_memo.clear(); let t1 = app.toast(format!("{} item(s) adjusted", v["adjusted"].as_i64().unwrap_or(0)), false); let t2 = app.load_lookups(); let i = app.desk.active; Task::batch([t1, t2, app.close_window(i)]) }
                Err(e) => app.toast(e, true),
            }
        }
        Msg::TaxPick(id, on) => { tax_row(app, id).1 = on; Task::none() }
        Msg::TaxAmt(id, v) => { let r = tax_row(app, id); r.2 = v; r.1 = true; Task::none() }
        Msg::TaxFrom(a) => { app.desk.tax_from = Some(a); Task::none() }
        Msg::TaxDate(v) => { app.desk.tax_date = v; Task::none() }
        Msg::TaxPay => {
            let Some(liab) = app.lk.accounts.iter().find(|a| a["system_account"] == "GST").and_then(|a| a["id"].as_i64()) else { return app.toast("No sales tax liability account found", true) };
            let Some(from) = app.desk.tax_from.clone().or_else(|| crate::payments::bank_only(app).into_iter().next()) else { return app.toast("Choose the account to pay from", true) };
            let date_in = if app.desk.tax_date.is_empty() { app.d(&ui::today_iso()) } else { app.desk.tax_date.clone() };
            let Some(date) = ui::to_iso(&date_in, app.date_style()) else { return app.toast("Check the payment date", true) };
            let names: Vec<(i64, String)> = app.data.as_ref().and_then(|d| d.as_array().cloned()).unwrap_or_default().iter().map(|a| (a["id"].as_i64().unwrap_or(0), a["name"].as_str().unwrap_or("").to_string())).collect();
            let pays: Vec<(String, String)> = app.desk.tax_pay.iter().filter(|(_, on, amt)| *on && ui::parse_money(amt).unwrap_or(0) > 0)
                .filter_map(|(id, _, amt)| names.iter().find(|(i, _)| i == id).map(|(_, n)| (n.clone(), amt.replace(',', "")))).collect();
            if pays.is_empty() { return app.toast("Tick an agency and enter the amount to pay", true); }
            let be = app.be.clone();
            app.desk.tax_busy = true;
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<Value, String> {
                    for (agency, amt) in &pays {
                        be.call_now("banktx.save", serde_json::json!({"tx": {"type": "SPEND", "account_id": from.id, "date": date, "reference": "Sales tax payment", "contact_name": agency, "line_amount_type": "NOTAX",
                            "lines": [{"account_id": liab, "description": format!("Sales tax payment — {agency}"), "amount": amt}]}}))?;
                    }
                    Ok(serde_json::json!({"paid": pays.len()}))
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, |r| Message::Desk(Msg::TaxPaid(r)))
        }
        Msg::TaxPaid(r) => {
            app.desk.tax_busy = false;
            match r {
                Ok(v) => { app.desk.tax_pay.clear(); let t1 = app.toast(format!("{} sales tax payment(s) recorded", v["paid"].as_i64().unwrap_or(0)), false); let t2 = app.load_lookups(); let i = app.desk.active; Task::batch([t1, t2, app.close_window(i)]) }
                Err(e) => app.toast(e, true),
            }
        }
        Msg::OneWindow(on) => {
            app.desk.one_window = on;
            app.menu = None;
            if on { let i = app.desk.active; let keep = app.desk.windows.remove(i); app.desk.windows = vec![keep]; app.desk.active = 0; }
            Task::none()
        }
        Msg::TxType(t) => {
            app.desk.tx_type = t;
            app.desk.tx_rows.clear();
            let Some(t) = t else { return Task::none() };
            let (kind, _) = tx_kind(t);
            app.be.call("docs.list", serde_json::json!({"type": kind, "limit": 1000}), move |r| Message::Desk(Msg::TxRows(t, r)))
        }
        Msg::TxRows(t, r) => {
            if app.desk.tx_type == Some(t) {
                let (_, receipts) = tx_kind(t);
                app.desk.tx_rows = r.ok().and_then(|v| v["rows"].as_array().cloned()).unwrap_or_default().into_iter()
                    .filter(|d| match receipts { Some(true) => d["subtype"] == "RECEIPT", Some(false) => d["subtype"] != "RECEIPT", None => true }).collect();
            }
            Task::none()
        }
        Msg::Reg(f, v) => {
            let r = &mut app.desk.reg;
            match f { RegField::Date => r.date = v, RegField::Number => r.number = v, RegField::Payee => r.payee = v, RegField::Memo => r.memo = v,
                // Payment and Deposit are either/or, as in the register.
                RegField::Payment => { r.payment = v; if !r.payment.trim().is_empty() { r.deposit.clear(); } }
                RegField::Deposit => { r.deposit = v; if !r.deposit.trim().is_empty() { r.payment.clear(); } } }
            Task::none()
        }
        Msg::RegAccount(a) => { app.desk.reg.account = Some(a); Task::none() }
        Msg::RegRecord => {
            let Some(bank) = app.data.as_ref().and_then(|d| d["account"].as_i64()) else { return Task::none() };
            let r = app.desk.reg.clone();
            let date_in = if r.date.trim().is_empty() { app.d(&ui::today_iso()) } else { r.date.clone() };
            let Some(date) = ui::to_iso(&date_in, app.date_style()) else { return app.toast("Check the date", true) };
            let Some(acc) = r.account.clone() else { return app.toast("Choose the account (category) for this transaction", true) };
            let pay = ui::parse_money(&r.payment).unwrap_or(0);
            let dep = ui::parse_money(&r.deposit).unwrap_or(0);
            if pay <= 0 && dep <= 0 { return app.toast("Enter a payment or a deposit amount", true); }
            let amount = if pay > 0 { r.payment.replace(',', "") } else { r.deposit.replace(',', "") };
            let tax = app.lk.accounts.iter().find(|a| a["id"].as_i64() == Some(acc.id)).and_then(|a| a["tax_rate_id"].as_i64());
            let mut tx = serde_json::json!({"type": if pay > 0 { "SPEND" } else { "RECEIVE" }, "account_id": bank, "date": date, "reference": r.memo.trim(), "line_amount_type": "INCLUSIVE",
                "lines": [{"account_id": acc.id, "description": r.memo.trim(), "amount": amount, "tax_rate_id": tax}]});
            // A number on a payment makes it a check.
            if pay > 0 && !r.number.trim().is_empty() && r.number.trim().chars().all(|c| c.is_ascii_digit()) { tx["subtype"] = serde_json::json!("CHECK"); tx["check_number"] = serde_json::json!(r.number.trim()); }
            else if !r.number.trim().is_empty() { tx["reference"] = serde_json::json!(r.number.trim()); }
            if !r.payee.trim().is_empty() { tx["contact_name"] = serde_json::json!(r.payee.trim()); }
            app.desk.reg.busy = true;
            app.be.call("banktx.save", serde_json::json!({"tx": tx}), |r| Message::Desk(Msg::RegSaved(r)))
        }
        Msg::RegSaved(r) => {
            app.desk.reg.busy = false;
            match r {
                Ok(_) => {
                    let date = app.desk.reg.date.clone();
                    app.desk.reg = RegEntry { date, ..Default::default() };
                    let t1 = app.toast("Recorded", false);
                    Task::batch([t1, app.load(), app.load_lookups()])
                }
                Err(e) => app.toast(e, true),
            }
        }
        Msg::MemDelete(id) => app.be.call("repeating.setStatus", serde_json::json!({"id": id, "status": "DELETED"}), |r| Message::Desk(Msg::MemDone(r, "deleted"))),
        Msg::MemRun => app.be.call("repeating.run", serde_json::json!({}), |r| Message::Desk(Msg::MemDone(r, "run"))),
        Msg::MemDone(r, what) => match r {
            Ok(v) => {
                let msg = if what == "run" { format!("{} transaction(s) entered", v.as_array().map(|a| a.iter().filter(|x| x["error"].is_null()).count()).unwrap_or(0)) } else { "Memorized transaction deleted".into() };
                let t1 = app.toast(msg, false);
                Task::batch([t1, app.load()])
            }
            Err(e) => app.toast(e, true),
        },
        Msg::StmtOne(o) => { app.desk.stmt_one = o; Task::none() }
        Msg::StmtDate(v) => { app.desk.stmt_date = v; Task::none() }
        Msg::StmtDone(r) => { app.desk.stmt_busy = false; match r { Ok(m) => app.toast(m, false), Err(e) => app.toast(e, true) } }
        Msg::StmtEmail => {
            let ids = statement_ids(app);
            if ids.is_empty() { return app.toast("No customers with an open balance", true); }
            app.desk.stmt_busy = true;
            app.be.call("statements.email", serde_json::json!({"contact_ids": ids}), |r| Message::Desk(Msg::StmtDone(r.map(|v| format!("{} statement(s) queued to email (customers without an email address are skipped)", v["queued"].as_i64().unwrap_or(0))))))
        }
        Msg::StmtPrint => {
            let ids = statement_ids(app);
            if ids.is_empty() { return app.toast("No customers with an open balance", true); }
            let Some(date) = ui::to_iso(&app.desk.stmt_date, app.date_style()).or_else(|| Some(ui::today_iso())) else { return Task::none() };
            app.desk.stmt_busy = true;
            let be = app.be.clone();
            let style = app.date_style();
            let letter = matches!(app.country().as_str(), "US" | "CA");
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let rep = statement_report(&be, &ids, &date, style)?;
                    let path = crate::pdf::output_dir().join(format!("Statements {date}.pdf"));
                    std::fs::write(&path, crate::pdf::report(&rep, "Statement", letter, &|s: &str| s.to_string())).map_err(|e| format!("Couldn't save: {e}"))?;
                    crate::pdf::open_file(&path);
                    Ok(format!("Saved {}", path.display()))
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, |r| Message::Desk(Msg::StmtDone(r)))
        }
        Msg::CalMonth(d) => { app.desk.cal_offset = if d == 0 { 0 } else { app.desk.cal_offset + d }; Task::none() }
        Msg::Employee(id) => { app.desk.emp = Some(id); Task::none() }
        Msg::Tracker(f) => { app.desk.tracker = if app.desk.tracker == f { "all" } else { f }; Task::none() }
        Msg::Detail(id, r) => {
            if app.desk.sel == Some(id) { app.desk.detail = r.ok(); }
            Task::none()
        }
    }
}

/// After a Center's list loads, select the remembered name (or the first one).
pub fn after_load(app: &mut App) -> Task<Message> {
    if !app.desktop() || !matches!(app.route, Route::Customers | Route::Vendors) { return Task::none(); }
    let list = app.data.as_ref().and_then(|d| d.as_array().cloned()).unwrap_or_default();
    let keep = app.desk.sel.filter(|s| list.iter().any(|c| c["id"].as_i64() == Some(*s)));
    match keep.or_else(|| list.first().and_then(|c| c["id"].as_i64())) {
        Some(id) if app.desk.detail.as_ref().and_then(|d| d["contact"]["id"].as_i64()) != Some(id) => update(app, Msg::Select(id)),
        _ => Task::none(),
    }
}

/// Window title as QuickBooks Desktop shows it.
pub fn title(app: &App, r: &Route) -> String {
    match r {
        Route::Dashboard => "Home".into(),
        Route::AllSales | Route::Invoices => "Income Tracker".into(),
        Route::Quotes => "Estimates".into(),
        Route::CreditNotes => "Credit Memos".into(),
        Route::PurchaseOrders => "Purchase Orders".into(),
        Route::Bills | Route::Expenses => "Bill Tracker".into(),
        Route::Customers | Route::Contacts => "Customer Center".into(),
        Route::Vendors => "Vendor Center".into(),
        Route::Contact(_) => "Customer Center".into(),
        Route::Products => "Item List".into(),
        Route::Chart => "Chart of Accounts".into(),
        Route::Banking(_) => "Bank Feeds Center".into(),
        Route::Reports => "Report Center".into(),
        Route::Report(k) => match k.as_str() {
            "profit-and-loss" => "Profit & Loss".into(), "balance-sheet" => "Balance Sheet".into(), "cash-flow-statement" => "Statement of Cash Flows".into(),
            "trial-balance" => "Trial Balance".into(), "general-ledger-detail" => "General Ledger".into(), "general-ledger" => "General Ledger Summary".into(),
            _ => {
                // The report's own title once it's loaded, else the QuickBooks name.
                let loaded = app.data.as_ref().filter(|_| app.route == *r).and_then(|d| d["title"].as_str()).map(String::from);
                let n = crate::reports::name(app.skin, k, "");
                if n.is_empty() { loaded.unwrap_or_else(|| "Report".into()) } else { n }
            }
        },
        Route::Settings => "Preferences".into(),
        Route::Reconcile => "Reconcile".into(),
        Route::Calendar => "Calendar".into(),
        Route::Statements => "Create Statements".into(),
        Route::Memorized => "Memorized Transaction List".into(),
        Route::PaySalesTax => "Pay Sales Tax".into(),
        Route::AdjustInventory => "Adjust Quantity/Value on Hand".into(),
        Route::Help => "LedgerCraft Help".into(),
        Route::Register(_) => app.data.as_ref().filter(|_| app.route == *r).and_then(|d| d["banks"].as_array().and_then(|b| b.iter().find(|x| x["id"] == d["account"]).and_then(|x| x["name"].as_str().map(String::from)))).unwrap_or_else(|| "Register".into()),
        Route::Doc { kind, .. } => match kind.as_str() {
            "ACCREC" => "Create Invoices", "QUOTE" => "Create Estimates", "ACCRECCREDIT" => "Create Credit Memos/Refunds", "SALESRECEIPT" => "Enter Sales Receipts",
            "ACCPAY" => "Enter Bills", "ACCPAYCREDIT" => "Enter Bills (Credit)", "PO" => "Create Purchase Orders", "DELAYEDCHARGE" | "DELAYEDCREDIT" => "Enter Statement Charges", "SALESORDER" => "Create Sales Orders", _ => "Transaction",
        }.into(),
        Route::ReceivePayment(_) => "Receive Payments".into(),
        Route::PayBills => "Pay Bills".into(),
        Route::Spend(_) => "Enter Credit Card Charges".into(),
        Route::Check(_) => "Write Checks".into(),
        Route::Deposit => "Make Deposits".into(),
        Route::Transfer => "Transfer Funds Between Accounts".into(),
        Route::Journal => "Make General Journal Entries".into(),
        Route::NewContact(true) => "New Customer".into(),
        Route::NewContact(false) => "New Vendor".into(),
        Route::NewProduct => "New Item".into(),
        Route::NewAccount => "Add New Account".into(),
        Route::Payroll => "Employee Center".into(),
        Route::PayRun(_) => "Pay Employees".into(),
        Route::Taxes | Route::TaxReturn(_) => "Manage Sales Tax".into(),
        Route::Projects => "Projects".into(),
        Route::Mileage => "Enter Vehicle Mileage".into(),
        Route::Placeholder(p) => p.to_string(),
        Route::Setup | Route::Login => "Company".into(),
    }
}

// ---------------- menu bar ----------------

const MENUS: [&str; 14] = ["File", "Edit", "View", "Lists", "Favorites", "Company", "Inventory", "Customers", "Vendors", "Employees", "Banking", "Reports", "Window", "Help"];

/// Width of a menu bar title (fixed so dropdowns can line up under it).
fn menu_w(name: &str) -> f32 { name.len() as f32 * 6.9 + 16.0 }

fn nav(r: Route) -> Option<Message> { Some(Message::Nav(r)) }
fn doc_new(kind: &str) -> Option<Message> { nav(Route::Doc { kind: kind.into(), id: None }) }
fn rep(key: &str) -> Option<Message> { nav(Route::Report(key.into())) }

/// Menu entries: (label, shortcut, action). "-" is a separator; a label starting with "#" is a group heading.
fn menu_items(app: &App, name: &str) -> Vec<(String, &'static str, Option<Message>)> {
    let e = |l: &str, k: &'static str, m: Option<Message>| (l.to_string(), k, m);
    let sep = || ("-".to_string(), "", None);
    match name {
        "File" => vec![e("New Company…", "", nav(Route::Setup)), e("Open or Restore Company…", "", nav(Route::Setup)), e("Restore a Backup Copy…", "", Some(Message::Setup(crate::setup::Msg::Restore))), e("Open Previous Company", "", nav(Route::Setup)), sep(),
            e("Back Up Company…", "", Some(Message::BackupCompany)), e("Print Forms", "", None), sep(), e("Exit", "Alt+F4", Some(Message::Desk(Msg::Exit)))],
        "Edit" => vec![e("Undo", "Ctrl+Z", None), sep(), e("Find…", "Ctrl+F", Some(Message::FocusSearch)), e("Search…", "F3", Some(Message::FocusSearch)), sep(), e("Preferences…", "", nav(Route::Settings))],
        "View" => vec![e(if app.desk.one_window { "✓ One Window" } else { "   One Window" }, "", Some(Message::Desk(Msg::OneWindow(true)))),
            e(if app.desk.one_window { "   Multiple Windows" } else { "✓ Multiple Windows" }, "", Some(Message::Desk(Msg::OneWindow(false)))), sep(),
            e("Open Window List", "", Some(Message::Desk(Msg::Pane(Pane::Windows)))), e("View Balances", "", Some(Message::Desk(Msg::Pane(Pane::Balances)))),
            e("My Shortcuts", "", Some(Message::Desk(Msg::Pane(Pane::Shortcuts)))), sep(), e(if app.dark { "Light Mode" } else { "Dark Mode" }, "Alt+T", Some(Message::ToggleTheme)), sep(),
            e("Switch to QuickBooks Online-style layout", "", Some(Message::SetLayout(theme::Layout::Online))), e("Switch to Xero-style layout", "", Some(Message::SetLayout(theme::Layout::Xero)))],
        "Lists" => vec![e("Chart of Accounts", "Ctrl+A", nav(Route::Chart)), e("Item List", "", nav(Route::Products)), e("Fixed Asset Item List", "", rep("fixed-asset-register")), e("Price Level List", "", None),
            e("Sales Tax Code List", "", nav(Route::Taxes)), e("Payroll Item List", "", None), e("Class List", "", None), e("Other Names List", "", None), e("Templates", "", None), e("Memorized Transaction List", "Ctrl+T", nav(Route::Memorized))],
        "Favorites" => vec![e("Customize Favorites…", "", None)],
        "Company" => vec![e("Home Page", "", nav(Route::Dashboard)), e("Company Snapshot", "", Some(Message::Desk(Msg::Insights(true)))), sep(), e("My Company", "", nav(Route::Settings)),
            e("Company Information…", "", nav(Route::Settings)), e("Set Up Users and Roles…", "", nav(Route::Settings)), sep(), e("Chart of Accounts", "Ctrl+A", nav(Route::Chart)),
            e("Make General Journal Entries…", "", nav(Route::Journal)), e("Enter Vehicle Mileage…", "", nav(Route::Mileage)), e("Calendar", "", nav(Route::Calendar)), e("Documents", "", None)],
        "Inventory" => vec![e("Inventory Center", "", nav(Route::Products)), e("Item List", "", nav(Route::Products)), sep(), e("Create Purchase Orders", "", doc_new("PO")),
            e("Purchase Orders List", "", nav(Route::PurchaseOrders)), e("Receive Items", "", Some(Message::ReceiveItems)), e("Enter Bills Against Inventory", "", nav(Route::PurchaseOrders)), e("Adjust Quantity/Value on Hand…", "", nav(Route::AdjustInventory))],
        "Customers" => vec![e("Customer Center", "Ctrl+J", nav(Route::Customers)), sep(), e("Create Estimates", "", doc_new("QUOTE")), e("Create Sales Orders", "", doc_new("SALESORDER")),
            e("Create Invoices", "Ctrl+I", doc_new("ACCREC")), e("Enter Sales Receipts", "", doc_new("SALESRECEIPT")), e("Enter Statement Charges", "", doc_new("DELAYEDCHARGE")),
            e("Create Statements…", "", nav(Route::Statements)), e("Assess Finance Charges…", "", None), sep(), e("Receive Payments", "", nav(Route::ReceivePayment(None))),
            e("Create Credit Memos/Refunds", "", doc_new("ACCRECCREDIT")), sep(), e("Income Tracker", "", nav(Route::AllSales)), e("Item List", "", nav(Route::Products))],
        "Vendors" => vec![e("Vendor Center", "", nav(Route::Vendors)), sep(), e("Enter Bills", "", doc_new("ACCPAY")), e("Pay Bills", "", nav(Route::PayBills)), e("Bill Tracker", "", nav(Route::Bills)),
            sep(), e("Sales Tax: Manage Sales Tax…", "", nav(Route::Taxes)), e("Sales Tax: Pay Sales Tax…", "", nav(Route::PaySalesTax)), sep(), e("Create Purchase Orders", "", doc_new("PO")), e("Purchase Orders List", "", nav(Route::PurchaseOrders)),
            sep(), e("Print/E-file 1099s: 1099 Summary", "", rep("contractor-1099")), e("Enter Bills for Received Items", "", None)],
        "Employees" => vec![e("Employee Center", "", nav(Route::Payroll)), e("Payroll Center", "", nav(Route::Payroll)), sep(), e("Pay Employees", "", nav(Route::Payroll)),
            e("Pay Liabilities", "", None), e("Process Payroll Forms", "", None), sep(), e("Enter Time", "", None)],
        "Banking" => vec![e("Write Checks", "Ctrl+W", nav(Route::Check(None))), e("Order Checks & Envelopes", "", None), e("Enter Credit Card Charges", "", nav(Route::Spend(None))), sep(),
            e("Use Register", "Ctrl+R", nav(Route::Register(None))), e("Make Deposits", "", nav(Route::Deposit)), e("Transfer Funds", "", nav(Route::Transfer)), e("Reconcile", "", nav(Route::Reconcile)),
            sep(), e("Bank Feeds Center", "", nav(Route::Banking(None)))],
        "Reports" => vec![e("Report Center", "", nav(Route::Reports)), e("Memorized Reports", "", None), e("Company Snapshot", "", Some(Message::Desk(Msg::Insights(true)))), sep(),
            e("#Company & Financial", "", None), e("Profit & Loss Standard", "", rep("profit-and-loss")), e("Balance Sheet Standard", "", rep("balance-sheet")), e("Statement of Cash Flows", "", rep("cash-flow-statement")),
            e("#Customers & Receivables", "", None), e("A/R Aging Summary", "", rep("aged-receivables")), e("Customer Balance Summary", "", rep("contact-transactions")),
            e("#Sales", "", None), e("Sales by Customer Summary", "", rep("sales-by-contact")), e("Sales by Item Summary", "", rep("sales-by-item")),
            e("#Vendors & Payables", "", None), e("A/P Aging Summary", "", rep("aged-payables")), e("1099 Summary", "", rep("contractor-1099")),
            e("#Banking", "", None), e("Reconciliation Discrepancy", "", rep("bank-reconciliation")),
            e("#Accountant & Taxes", "", None), e("Trial Balance", "", rep("trial-balance")), e("General Ledger", "", rep("general-ledger-detail")), e("Journal", "", rep("journal-report"))],
        "Window" => {
            let mut v = vec![e("Close All", "", Some(Message::Desk(Msg::CloseAll))), sep()];
            for (n, w) in app.desk.windows.iter().enumerate() { v.push((format!("{}{}", if n == app.desk.active { "✓ " } else { "   " }, title(app, &w.route)), "", Some(Message::Desk(Msg::Activate(n))))); }
            v
        }
        _ => vec![e("LedgerCraft Help", "F1", nav(Route::Help)), e("Keyboard Shortcuts", "", nav(Route::Help)), sep(), e("About LedgerCraft", "", nav(Route::Help))],
    }
}

fn menu_bar(app: &App) -> Element<'_, Message> {
    let k = t();
    let mut r = row![].spacing(0).align_y(Alignment::Center);
    for m in MENUS {
        let open = app.menu == Some(m);
        r = r.push(button(text(m).size(13).width(Fill).align_x(iced::alignment::Horizontal::Center)).padding([4, 0]).width(Length::Fixed(menu_w(m)))
            .style(move |_, s| {
                let hot = open || matches!(s, button::Status::Hovered);
                button::Style { background: if hot { Some(iced::Background::Color(k.hover)) } else { None }, text_color: k.text, border: iced::Border { radius: 3.0.into(), ..Default::default() }, shadow: Default::default(), snap: true }
            })
            .on_press(Message::OpenMenu(if open { None } else { Some(m) })));
    }
    container(row![r, ui::hspace(), text(app.org_name()).size(12).color(k.muted)].align_y(Alignment::Center).padding([0, 8])).height(28).center_y(28).width(Fill).style(theme::topbar).into()
}

fn dropdown<'a>(app: &'a App, name: &'static str) -> Element<'a, Message> {
    let k = t();
    let mut c = column![].spacing(0).width(300);
    for (label, key, msg) in menu_items(app, name) {
        if label == "-" { c = c.push(container(ui::hr()).padding([4, 6])); continue; }
        if let Some(h) = label.strip_prefix('#') { c = c.push(container(text(h.to_string()).size(11).font(BOLD).color(k.muted)).padding(Padding { top: 6.0, bottom: 2.0, left: 10.0, right: 10.0 })); continue; }
        let enabled = msg.is_some();
        let mut b = button(row![text(label).size(13).color(if enabled { k.text } else { k.faint }), ui::hspace(), text(key).size(12).color(k.faint)].align_y(Alignment::Center))
            .style(theme::menu_item).padding([5, 10]).width(Fill);
        if let Some(m) = msg { b = b.on_press(m); }
        c = c.push(b);
    }
    let left: f32 = 8.0 + MENUS.iter().take_while(|m| **m != name).map(|m| menu_w(m)).sum::<f32>();
    container(opaque(container(scrollable(c).style(theme::scroll)).max_height(620).padding(4).style(theme::menu))).padding(Padding { top: 28.0, left, ..Padding::ZERO }).into()
}

/// The icon bar search box (Ctrl+F / Edit ▸ Find focuses it).
pub const SEARCH_ID: &str = "desk-search";

// ---------------- left icon bar ----------------

fn icon_bar(app: &App) -> Element<'_, Message> {
    let k = t();
    let item = |ic: &'static str, label: &'static str, msg: Option<Message>, active: bool| -> Element<'_, Message> {
        let col_ = if msg.is_some() { k.nav_text } else { k.nav_muted };
        let mut b = button(row![icon(ic, 18.0, if active { k.nav_active } else { col_ }), text(label).size(13).color(col_)].spacing(10).align_y(Alignment::Center))
            .style(theme::nav_item(active)).padding([6, 10]).width(Fill);
        if let Some(m) = msg { b = b.on_press(m); }
        b.into()
    };
    let r = &app.route;
    let body: Element<Message> = match app.desk.pane {
        Pane::Shortcuts => column![
            text("My Shortcuts").size(11).font(BOLD).color(k.nav_muted),
            item("home", "Home", nav(Route::Dashboard), *r == Route::Dashboard),
            item("apps", "My Company", nav(Route::Settings), *r == Route::Settings),
            item("sales", "Income Tracker", nav(Route::AllSales), matches!(r, Route::AllSales | Route::Invoices)),
            item("expenses", "Bill Tracker", nav(Route::Bills), matches!(r, Route::Bills)),
            item("calendar", "Calendar", nav(Route::Calendar), *r == Route::Calendar),
            item("chart", "Snapshots", Some(Message::Desk(Msg::Insights(true))), false),
            item("contacts", "Customers", nav(Route::Customers), matches!(r, Route::Customers | Route::Contact(_))),
            item("users", "Vendors", nav(Route::Vendors), matches!(r, Route::Vendors)),
            item("payroll", "Employees", nav(Route::Payroll), matches!(r, Route::Payroll | Route::PayRun(_))),
            item("bank", "Bank Feeds", nav(Route::Banking(None)), matches!(r, Route::Banking(_))),
            item("docs", "Docs", None, false),
            item("reports", "Reports", nav(Route::Reports), matches!(r, Route::Reports | Route::Report(_))),
            item("check-write", "Write Checks", nav(Route::Check(None)), matches!(r, Route::Check(_))),
            item("reconcile", "Reconcile", nav(Route::Reconcile), matches!(r, Route::Reconcile)),
        ].spacing(1).into(),
        Pane::Balances => {
            let mut c = column![text("Account Balances").size(11).font(BOLD).color(k.nav_muted)].spacing(4);
            for (head, pred) in [("Bank Accounts", "BANK"), ("Accounts Receivable", "AR"), ("Accounts Payable", "AP")] {
                c = c.push(text(head).size(12).font(BOLD).color(k.nav_text));
                for a in app.lk.accounts.iter().filter(|a| a["status"] == "ACTIVE" && match pred {
                    "BANK" => a["type"] == "BANK", "AR" => a["system_account"] == "AR", _ => a["system_account"] == "AP" }) {
                    let bal = a["ytd_balance"].as_i64().unwrap_or(0) * if pred == "AP" { -1 } else { 1 };
                    c = c.push(row![text(a["name"].as_str().unwrap_or("").to_string()).size(12).color(k.nav_muted).width(Fill), text(ui::money(bal)).size(12).color(k.nav_text)].spacing(6));
                }
            }
            c.into()
        }
        Pane::Windows => {
            let mut c = column![text("Open Windows").size(11).font(BOLD).color(k.nav_muted)].spacing(1);
            for (n, w) in app.desk.windows.iter().enumerate() {
                let active = n == app.desk.active;
                c = c.push(button(text(title(app, &w.route)).size(13).color(k.nav_text)).style(theme::nav_item(active)).padding([6, 10]).width(Fill).on_press(Message::Desk(Msg::Activate(n))));
            }
            c.into()
        }
    };
    let pane_btn = |label: &'static str, p: Pane, ic: &'static str| -> Element<'_, Message> {
        let active = app.desk.pane == p;
        button(row![icon(ic, 15.0, if active { k.nav_active } else { k.nav_muted }), text(label).size(12).color(if active { k.nav_text } else { k.nav_muted })].spacing(8).align_y(Alignment::Center))
            .style(theme::nav_item(active)).padding([5, 10]).width(Fill).on_press(Message::Desk(Msg::Pane(p))).into()
    };
    let search = text_input("Search Company or Help", &app.search).id(SEARCH_ID).on_input(Message::Search).on_submit(Message::SearchGo).padding([5, 8]).size(12).style(theme::input);
    container(column![
        container(search).padding(8),
        scrollable(container(body).padding([4, 8])).style(theme::scroll).height(Fill),
        container(ui::hr()).padding([0, 8]),
        container(column![
            pane_btn("My Shortcuts", Pane::Shortcuts, "list"),
            pane_btn("View Balances", Pane::Balances, "bank"),
            button(row![icon("reports", 15.0, k.nav_muted), text("Run Favorite Reports").size(12).color(k.nav_muted)].spacing(8).align_y(Alignment::Center)).style(theme::nav_item(false)).padding([5, 10]).width(Fill).on_press(Message::Nav(Route::Reports)),
            pane_btn("Open Windows", Pane::Windows, "layout"),
        ].spacing(1)).padding(Padding { top: 4.0, bottom: 8.0, left: 8.0, right: 8.0 }),
    ]).width(208).height(Fill).style(theme::nav).into()
}

// ---------------- window tabs ----------------

fn window_tabs(app: &App) -> Element<'_, Message> {
    let k = t();
    let mut r = row![].spacing(2).align_y(Alignment::End);
    for (n, w) in app.desk.windows.iter().enumerate() {
        let active = n == app.desk.active;
        let tab = row![
            button(text(title(app, &w.route)).size(13).font(if active { BOLD } else { iced::Font::DEFAULT }).color(if active { k.text } else { k.muted })).style(theme::ghost).padding([5, 4]).on_press(Message::Desk(Msg::Activate(n))),
            button(icon("x", 12.0, k.muted)).style(theme::ghost).padding(3).on_press(Message::Desk(Msg::Close(n))),
        ].spacing(2).align_y(Alignment::Center);
        r = r.push(container(tab).padding(Padding { top: 1.0, bottom: 1.0, left: 8.0, right: 4.0 }).style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(if active { k.panel } else { k.bg2 })),
            border: iced::Border { color: if active { k.line2 } else { k.line }, width: 1.0, radius: iced::border::Radius { top_left: 5.0, top_right: 5.0, bottom_left: 0.0, bottom_right: 0.0 } },
            ..Default::default()
        }));
    }
    container(scrollable(r).direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new().width(3).scroller_width(3))).style(theme::scroll))
        .padding(Padding { top: 5.0, left: 6.0, right: 6.0, bottom: 0.0 }).width(Fill).style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.bg2)), ..Default::default() }).into()
}

/// The whole Desktop frame around the current screen.
pub fn frame<'a>(app: &'a App, content: Element<'a, Message>) -> Element<'a, Message> {
    let k = t();
    let work = column![window_tabs(app), container(ui::hr()).width(Fill), content];
    let base: Element<Message> = column![menu_bar(app), row![icon_bar(app), work]].into();
    let mut layers = stack![base].width(Fill).height(Fill);
    if let Some(m) = app.menu {
        layers = layers.push(mouse_area(container(Space::new()).width(Fill).height(Fill)).on_press(Message::CloseMenus));
        layers = layers.push(dropdown(app, m));
    }
    if !app.search_results.is_empty() && !app.search.is_empty() {
        layers = layers.push(mouse_area(container(Space::new()).width(Fill).height(Fill)).on_press(Message::CloseMenus));
        let list = column(app.search_results.iter().map(|r| {
            let entity = r["entity"].as_str().unwrap_or("").to_string();
            button(column![text(r["title"].as_str().unwrap_or("").to_string()).size(13), text(r["snippet"].as_str().unwrap_or("").replace(['[', ']'], "")).size(11).color(k.muted)].spacing(1))
                .style(theme::menu_item).padding([5, 8]).width(Fill).on_press(Message::OpenResult(entity, r["id"].as_i64().unwrap_or(0))).into()
        })).spacing(1);
        layers = layers.push(container(opaque(container(list).padding(8).width(400).style(theme::menu))).padding(Padding { top: 72.0, left: 8.0, ..Padding::ZERO }));
    }
    layers.into()
}

// ---------------- Help ----------------

/// Help ▸ LedgerCraft Help: shortcuts, where things are, and about.
pub fn help(app: &App) -> Element<'_, Message> {
    let k = t();
    let pair = |a: &'static str, b: &'static str| -> Element<'_, Message> { row![text(a).size(13).font(BOLD).width(160), text(b).size(13)].spacing(10).into() };
    let shortcuts = column![
        text("Keyboard shortcuts").size(16).font(BOLD),
        pair("Ctrl+I", "Create Invoices"), pair("Ctrl+W", "Write Checks"), pair("Ctrl+E", "Enter Bills"), pair("Ctrl+J", "Customer Center"),
        pair("Ctrl+A", "Chart of Accounts"), pair("Ctrl+R", "Use Register"), pair("Ctrl+T", "Memorized Transaction List"), pair("Ctrl+F / F3", "Search"),
        pair("Ctrl+S", "Save the transaction you're editing"), pair("Tab / Shift+Tab", "Next / previous field"), pair("F5", "Refresh"), pair("Esc", "Close menus"),
        pair("Alt+T", "Light / dark mode"), pair("F1", "This help"),
    ].spacing(6);
    let places = column![
        text("Where things are").size(16).font(BOLD),
        pair("Customers menu", "Invoices, estimates, sales receipts, statements, payments, credit memos"),
        pair("Vendors menu", "Bills, pay bills, purchase orders, sales tax, 1099s"),
        pair("Banking menu", "Write checks, deposits, transfers, register, reconcile, bank feeds"),
        pair("Lists menu", "Chart of accounts, items, memorized transactions"),
        pair("Edit ▸ Preferences", "Layout (Desktop / Online / Xero), dark mode, company, email, users"),
        pair("Window menu", "Every open window; Close All"),
        pair("File menu", "Switch company, back up, restore a backup"),
    ].spacing(6);
    let about = column![
        text("About LedgerCraft").size(16).font(BOLD),
        text(format!("Version {} — open-source accounting, 100% Rust.", env!("CARGO_PKG_VERSION"))).size(13),
        text("QuickBooks Desktop-style, QuickBooks Online-style and Xero-style layouts over the same books. Not affiliated with Intuit or Xero.").size(13).color(k.muted),
        text("Your company files live in %APPDATA%\\LedgerCraft; automatic daily backups in its backups folder.").size(13).color(k.muted),
    ].spacing(6);
    let _ = app;
    scrollable(column![
        text("LedgerCraft Help").size(22).font(BOLD),
        row![container(shortcuts).padding(16).width(Fill).style(theme::card), container(places).padding(16).width(Fill).style(theme::card)].spacing(14),
        container(about).padding(16).width(Fill).style(theme::card),
    ].spacing(14).padding(18).max_width(1150)).style(theme::scroll).height(Fill).into()
}

// ---------------- Adjust Quantity/Value on Hand ----------------

/// The usual account for stock adjustments: Cost of Goods Sold.
fn default_adj_account(app: &App) -> Option<crate::app::Opt> {
    app.lk.account_opts(|a| a["type"] == "DIRECTCOSTS").into_iter().next()
}

/// Inventory ▸ Adjust Quantity/Value on Hand.
pub fn adjust_inventory(app: &App) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let items: Vec<Value> = d.as_array().cloned().unwrap_or_default().into_iter().filter(|x| x["is_tracked"].as_i64() == Some(1)).collect();
    let accounts = app.lk.account_opts(|a| matches!(a["class"].as_str(), Some("EXPENSE")));
    let acc = app.desk.adj_account.clone().or_else(|| default_adj_account(app));
    let date = if app.desk.adj_date.is_empty() { app.d(&ui::today_iso()) } else { app.desk.adj_date.clone() };
    let cols: Vec<(&str, u16, bool)> = vec![("Item", 30, false), ("Description", 30, false), ("Current Qty", 12, true), ("New Quantity", 14, true), ("Qty Difference", 12, true)];
    let head = container(row(cols.iter().map(|(h, w, r)| {
        let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
        container(if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
    })).spacing(8).padding([6, 10])).style(theme::header_row).width(Fill);
    let mut list = column![head];
    if items.is_empty() { list = list.push(container(ui::muted("No inventory items with quantity tracking.")).padding(16)); }
    let qty = |e4: i64| { let s = format!("{:.4}", e4 as f64 / 10000.0); s.trim_end_matches('0').trim_end_matches('.').to_string() };
    for x in &items {
        let id = x["id"].as_i64().unwrap_or(0);
        let cur = x["qty_on_hand_e4"].as_i64().unwrap_or(0);
        let new = app.desk.adj.iter().find(|a| a.0 == id).map(|a| a.1.clone()).unwrap_or_default();
        let diff = crate::doc::parse_e4(&new).filter(|_| !new.trim().is_empty()).map(|n| n - cur);
        let cells: Vec<Element<Message>> = vec![
            ui::cell(x["name"].as_str().unwrap_or("").to_string()), ui::cell_muted(x["purchase_description"].as_str().or(x["sales_description"].as_str()).unwrap_or("").to_string()),
            ui::num(qty(cur)),
            text_input("", &new).on_input(move |v| Message::Desk(Msg::AdjQty(id, v))).padding([4, 6]).size(13).align_x(iced::alignment::Horizontal::Right).style(theme::cell_input).into(),
            text(diff.map(|d| format!("{}{}", if d > 0 { "+" } else { "" }, qty(d))).unwrap_or_default()).size(13).color(if diff.unwrap_or(0) < 0 { k.bad } else { k.text }).align_x(iced::alignment::Horizontal::Right).width(Fill).into(),
        ];
        list = list.push(row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(8).align_y(Alignment::Center).padding([5, 10]));
        list = list.push(ui::hr());
    }
    let busy = app.desk.adj_busy;
    container(column![
        text("Adjust Quantity/Value on Hand").size(22).font(BOLD),
        row![
            column![text("ADJUSTMENT DATE").size(11).font(BOLD).color(k.info), text_input("", &date).on_input(|v| Message::Desk(Msg::AdjDate(v))).padding([5, 7]).size(13).style(theme::input).width(130)].spacing(3),
            column![text("ADJUSTMENT ACCOUNT").size(11).font(BOLD).color(k.info), pick_list(accounts, acc, |a| Message::Desk(Msg::AdjAccount(a))).padding([5, 7]).text_size(13).style(theme::pick).width(300)].spacing(3),
            column![text("MEMO").size(11).font(BOLD).color(k.info), text_input("", &app.desk.adj_memo).on_input(|v| Message::Desk(Msg::AdjMemo(v))).padding([5, 7]).size(13).style(theme::input).width(320)].spacing(3),
        ].spacing(16).align_y(Alignment::End),
        container(scrollable(list).style(theme::scroll)).style(theme::card).width(Fill).height(Length::Fixed(420.0)).clip(true),
        row![text("Increases are valued at each item's purchase cost; decreases at its average cost.").size(12).color(k.muted), ui::hspace(),
            ui::btn_primary("Save & Close", if busy { None } else { Some(Message::Desk(Msg::AdjSave)) }), ui::btn("Cancel", Some(Message::Back))].spacing(8).align_y(Alignment::Center),
    ].spacing(12)).padding(18).max_width(1150).into()
}

// ---------------- Pay Sales Tax ----------------

/// The (pay?, amount) entry for an agency, created with the tax collected so far as the amount.
fn tax_row(app: &mut App, id: i64) -> &mut (i64, bool, String) {
    if let Some(i) = app.desk.tax_pay.iter().position(|r| r.0 == id) { return &mut app.desk.tax_pay[i]; }
    let due = app.data.as_ref().and_then(|d| d.as_array().cloned()).unwrap_or_default().iter().find(|a| a["id"].as_i64() == Some(id)).and_then(|a| a["tax"].as_i64()).unwrap_or(0);
    app.desk.tax_pay.push((id, false, ui::plain(due)));
    app.desk.tax_pay.last_mut().unwrap()
}

/// Vendors ▸ Sales Tax ▸ Pay Sales Tax.
pub fn pay_sales_tax(app: &App) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let agencies = d.as_array().cloned().unwrap_or_default();
    let banks = crate::payments::bank_only(app);
    let from = app.desk.tax_from.clone().or_else(|| banks.first().cloned());
    let date = if app.desk.tax_date.is_empty() { app.d(&ui::today_iso()) } else { app.desk.tax_date.clone() };
    let cols: Vec<(&str, u16, bool)> = vec![("Pay", 4, false), ("Agency", 26, false), ("Period", 20, false), ("Collected", 12, true), ("Amt. Paid", 14, true)];
    let head = container(row(cols.iter().map(|(h, w, r)| {
        let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
        container(if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
    })).spacing(8).padding([6, 10])).style(theme::header_row).width(Fill);
    let mut list = column![head];
    let mut total = 0;
    if agencies.is_empty() { list = list.push(container(ui::muted("No sales tax agencies yet — set them up in Manage Sales Tax.")).padding(16)); }
    for a in &agencies {
        let id = a["id"].as_i64().unwrap_or(0);
        let due = a["tax"].as_i64().unwrap_or(0);
        let (on, amt) = app.desk.tax_pay.iter().find(|r| r.0 == id).map(|r| (r.1, r.2.clone())).unwrap_or((false, ui::plain(due)));
        if on { total += ui::parse_money(&amt).unwrap_or(0); }
        let cells: Vec<Element<Message>> = vec![
            iced::widget::checkbox(on).on_toggle(move |v| Message::Desk(Msg::TaxPick(id, v))).into(),
            ui::cell(a["name"].as_str().unwrap_or("").to_string()),
            ui::cell_muted(format!("{} – {}", app.d(a["period_from"].as_str().unwrap_or("")), app.d(a["period_to"].as_str().unwrap_or("")))),
            ui::cell_money(due),
            text_input("", &amt).on_input(move |v| Message::Desk(Msg::TaxAmt(id, v))).padding([4, 6]).size(13).align_x(iced::alignment::Horizontal::Right).style(theme::cell_input).into(),
        ];
        list = list.push(row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(8).align_y(Alignment::Center).padding([5, 10]));
        list = list.push(ui::hr());
    }
    let busy = app.desk.tax_busy;
    container(column![
        text("Pay Sales Tax").size(22).font(BOLD),
        row![
            column![text("PAY FROM ACCOUNT").size(11).font(BOLD).color(k.info), pick_list(banks, from, |a| Message::Desk(Msg::TaxFrom(a))).padding([5, 7]).text_size(13).style(theme::pick).width(280)].spacing(3),
            column![text("CHECK DATE").size(11).font(BOLD).color(k.info), text_input("", &date).on_input(|v| Message::Desk(Msg::TaxDate(v))).padding([5, 7]).size(13).style(theme::input).width(130)].spacing(3),
        ].spacing(20),
        container(list).style(theme::card).width(Fill).clip(true),
        row![text("Collected is the tax charged in each agency's current filing period; enter what you're paying.").size(12).color(k.muted), ui::hspace(),
            text("Total").size(13).font(BOLD), text(ui::money(total)).size(14).font(BOLD)].spacing(12).align_y(Alignment::Center),
        row![ui::hspace(), ui::btn_primary("OK", if busy { None } else { Some(Message::Desk(Msg::TaxPay)) }), ui::btn("Cancel", Some(Message::Back))].spacing(8),
    ].spacing(12)).padding(18).max_width(1100).into()
}

// ---------------- Memorized Transaction List ----------------

/// Lists ▸ Memorized Transaction List.
pub fn memorized(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let cols: Vec<(&str, u16, bool)> = vec![("Transaction Name", 24, false), ("Type", 10, false), ("Amount", 11, true), ("Frequency", 14, false), ("Auto", 6, false), ("Next Date", 11, false), ("", 8, false)];
    let rows = d.as_array().cloned().unwrap_or_default().iter().map(|r| {
        let id = r["id"].as_i64().unwrap_or(0);
        let count = r["period_count"].as_i64().unwrap_or(1);
        let freq = match (r["period_unit"].as_str().unwrap_or(""), count) { ("WEEKLY", 1) => "Weekly".to_string(), ("WEEKLY", 2) => "Every two weeks".into(), ("MONTHLY", 1) => "Monthly".into(),
            ("MONTHLY", 3) => "Quarterly".into(), ("MONTHLY", 12) => "Annually".into(), (u, n) => format!("Every {n} {}", if u == "WEEKLY" { "weeks" } else { "months" }) };
        let kind = match r["kind"].as_str().unwrap_or("") { "ACCREC" => "Invoice", "ACCPAY" => "Bill", "MJ" => "General Journal", _ => "Transaction" };
        let status = r["status"].as_str().unwrap_or("");
        (vec![ui::cell(r["contact_name"].as_str().unwrap_or("").to_string()), ui::cell_muted(kind), ui::cell_money(r["estimated_total"].as_i64().unwrap_or(0)),
            ui::cell_muted(freq), ui::cell(if r["save_as"].as_str().unwrap_or("").starts_with("AUTHORISED") { "✓" } else { "" }),
            ui::cell(if status == "ACTIVE" { app.d(r["next_date"].as_str().unwrap_or("")) } else { status.to_lowercase() }),
            ui::link("Delete", Message::Desk(Msg::MemDelete(id)))], None)
    }).collect();
    list_window(&cols, rows, vec![
        ui::btn("Enter All Due Now", Some(Message::Desk(Msg::MemRun))),
        ui::hspace(),
        text("Memorize a transaction from the Memorize button on Create Invoices or Enter Bills.").size(12).color(t().muted).into(),
    ])
}

// ---------------- Create Statements ----------------

/// Customers the statements are for: the chosen one, or everyone with an open balance.
fn statement_ids(app: &App) -> Vec<i64> {
    if let Some(o) = &app.desk.stmt_one { return vec![o.id]; }
    app.data.as_ref().and_then(|d| d.as_array().cloned()).unwrap_or_default().iter()
        .filter(|c| c["receivable"].as_i64().unwrap_or(0) > 0).filter_map(|c| c["id"].as_i64()).collect()
}

/// Statements as a printable report: one section per customer listing their open invoices.
pub fn statement_report(be: &crate::backend::Backend, ids: &[i64], date: &str, style: ui::DateStyle) -> Result<Value, String> {
    let org = be.call_now("session.get", serde_json::json!({}))?["org"]["name"].clone();
    let mut sections = vec![];
    for id in ids {
        let c = be.call_now("contacts.get", serde_json::json!({"id": id}))?;
        let docs = be.call_now("docs.list", serde_json::json!({"type": "SALES", "contact_id": id, "tab": "awaiting", "limit": 500}))?;
        let mut rows = vec![];
        let mut total = 0;
        for d in docs["rows"].as_array().cloned().unwrap_or_default() {
            if d["date"].as_str().unwrap_or("") > date { continue; }
            let due = d["amount_due"].as_i64().unwrap_or(0) * if d["type"] == "ACCRECCREDIT" { -1 } else { 1 };
            total += due;
            rows.push(serde_json::json!({"label": format!("{}  {}  {}  due {}", ui::date(d["date"].as_str().unwrap_or(""), style), if d["type"] == "ACCRECCREDIT" { "Credit Memo" } else { "Invoice" },
                d["number"].as_str().unwrap_or(""), ui::date(d["due_date"].as_str().unwrap_or(""), style)), "values": [due]}));
        }
        let addr = c["postal_address"].as_str().unwrap_or("").replace('\n', ", ");
        sections.push(serde_json::json!({"title": if addr.is_empty() { c["name"].clone() } else { serde_json::json!(format!("{} — {}", c["name"].as_str().unwrap_or(""), addr)) },
            "rows": rows, "total": {"label": "Amount Due", "values": [total]}}));
    }
    Ok(serde_json::json!({"title": "Statement", "org": org, "subtitle": format!("Statement date {}", ui::date(date, style)), "columns": ["Amount"], "sections": sections}))
}

/// Customers ▸ Create Statements.
pub fn statements(app: &App) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let list = d.as_array().cloned().unwrap_or_default();
    let with_balance: Vec<&Value> = list.iter().filter(|c| c["receivable"].as_i64().unwrap_or(0) > 0).collect();
    let opts: Vec<crate::app::Opt> = list.iter().map(|c| crate::app::Opt { id: c["id"].as_i64().unwrap_or(0), label: c["name"].as_str().unwrap_or("").to_string() }).collect();
    let date = if app.desk.stmt_date.is_empty() { app.d(&ui::today_iso()) } else { app.desk.stmt_date.clone() };
    let all = app.desk.stmt_one.is_none();
    let radio = |on: bool, label: &'static str, m: Message| -> Element<'_, Message> {
        button(row![text(if on { "●" } else { "○" }).size(14).color(if on { k.accent } else { k.muted }), text(label).size(13)].spacing(8).align_y(Alignment::Center)).style(theme::ghost).padding([3, 4]).on_press(m).into()
    };
    let n = statement_ids(app).len();
    let busy = app.desk.stmt_busy;
    container(column![
        text("Create Statements").size(22).font(BOLD),
        row![
            container(column![
                text("SELECT STATEMENT OPTIONS").size(11).font(BOLD).color(k.info),
                row![text("Statement Date").size(13).width(140), text_input("", &date).on_input(|v| Message::Desk(Msg::StmtDate(v))).padding([5, 7]).size(13).style(theme::input).width(140)].spacing(10).align_y(Alignment::Center),
                radio(true, "Statement period: all open transactions as of the statement date", Message::Noop),
                text("SELECT TEMPLATE").size(11).font(BOLD).color(k.info),
                container(text("LedgerCraft Statement (open items)").size(13)).padding([5, 7]).width(280).style(theme::soft),
            ].spacing(10)).padding(14).width(Fill).style(theme::card),
            container(column![
                text("SELECT CUSTOMERS").size(11).font(BOLD).color(k.info),
                radio(all, "All Customers (with an open balance)", Message::Desk(Msg::StmtOne(None))),
                radio(!all, "One Customer", Message::Desk(Msg::StmtOne(app.desk.stmt_one.clone().or_else(|| opts.first().cloned())))),
                if all { Element::from(text(format!("{} customer(s) will get a statement", with_balance.len())).size(12).color(k.muted)) }
                else { pick_list(opts.clone(), app.desk.stmt_one.clone(), |o| Message::Desk(Msg::StmtOne(Some(o)))).padding([5, 7]).text_size(13).style(theme::pick).width(280).into() },
            ].spacing(10)).padding(14).width(Fill).style(theme::card),
        ].spacing(14),
        row![ui::hspace(), text(format!("{n} statement(s)")).size(13).color(k.muted),
            ui::btn("E-mail", if busy || n == 0 { None } else { Some(Message::Desk(Msg::StmtEmail)) }),
            ui::btn_primary("Print", if busy || n == 0 { None } else { Some(Message::Desk(Msg::StmtPrint)) }),
            ui::btn("Close", Some(Message::Back))].spacing(8).align_y(Alignment::Center),
    ].spacing(14)).padding(18).max_width(1100).into()
}

// ---------------- Calendar ----------------

/// Open invoices and bills with their due dates for the Calendar.
pub fn load_calendar(app: &mut App) -> Task<Message> {
    let be = app.be.clone();
    let g = app.page_gen;
    Task::perform(async move {
        tokio::task::spawn_blocking(move || -> Result<Value, String> {
            let mut rows = vec![];
            for k in ["ACCREC", "ACCPAY"] { rows.extend(be.call_now("docs.list", serde_json::json!({"type": k, "tab": "awaiting", "limit": 2000}))?["rows"].as_array().cloned().unwrap_or_default()); }
            Ok(serde_json::json!({"rows": rows}))
        }).await.unwrap_or_else(|e| Err(e.to_string()))
    }, move |r| Message::Loaded(g, r))
}

fn iso(d: chrono::NaiveDate) -> String { d.to_string() }

/// Company ▸ Calendar: a month grid of what's due, plus Upcoming and Past Due lists.
pub fn calendar(app: &App) -> Element<'_, Message> {
    use chrono::Datelike;
    let k = t();
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let rows = d["rows"].as_array().cloned().unwrap_or_default();
    let today = chrono::Local::now().date_naive();
    let first_this = chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
    let first = crate::ui::add_months_date(first_this, app.desk.cal_offset);
    let next_first = crate::ui::add_months_date(first, 1);
    // Grid starts on the Sunday on or before the 1st.
    let start = first - chrono::Duration::days(first.weekday().num_days_from_sunday() as i64);
    let due_on = |day: chrono::NaiveDate| -> Vec<&Value> {
        let key = iso(day);
        rows.iter().filter(|r| r["due_date"].as_str() == Some(key.as_str())).collect()
    };
    let mut grid = column![row(["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"].iter().map(|h| text(*h).size(11).font(BOLD).color(k.muted).width(Fill).into())).spacing(4)].spacing(4);
    let mut day = start;
    for _ in 0..6 {
        let mut week = row![].spacing(4);
        for _ in 0..7 {
            let inside = day >= first && day < next_first;
            let is_today = day == today;
            let items = due_on(day);
            let (inv, bills): (Vec<&Value>, Vec<&Value>) = items.iter().partition(|r| r["type"] == "ACCREC");
            let mut cell = column![text(day.day().to_string()).size(12).font(if is_today { BOLD } else { iced::Font::DEFAULT }).color(if inside { k.text } else { k.faint })].spacing(2);
            if !inv.is_empty() { cell = cell.push(text(format!("Invoices due ({})", inv.len())).size(11).color(k.info)); }
            if !bills.is_empty() { cell = cell.push(text(format!("Bills due ({})", bills.len())).size(11).color(k.warn)); }
            week = week.push(container(cell).padding(6).width(Fill).height(Length::Fixed(84.0)).style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(if is_today { k.accent_soft } else if inside { k.panel } else { k.bg2 })),
                border: iced::Border { color: if is_today { k.accent } else { k.line }, width: 1.0, radius: 3.0.into() }, ..Default::default() }));
            day += chrono::Duration::days(1);
        }
        grid = grid.push(week);
        if day >= next_first { break; }
    }
    let today_s = iso(today);
    let week_s = iso(today + chrono::Duration::days(7));
    let entry = |r: &Value| -> Element<'_, Message> {
        let kind = r["type"].as_str().unwrap_or("").to_string();
        button(column![
            text(format!("{} {} · {}", if kind == "ACCREC" { "Invoice" } else { "Bill" }, r["number"].as_str().unwrap_or(""), r["contact_name"].as_str().unwrap_or(""))).size(12),
            text(format!("Due {} · {}", app.d(r["due_date"].as_str().unwrap_or("")), ui::money(r["amount_due"].as_i64().unwrap_or(0)))).size(11).color(k.muted),
        ].spacing(1)).style(theme::ghost).padding([3, 4]).width(Fill).on_press(Message::Nav(Route::Doc { kind, id: r["id"].as_i64() })).into()
    };
    let mut upcoming = column![text("UPCOMING: NEXT 7 DAYS").size(11).font(BOLD).color(k.info)].spacing(2);
    let mut past = column![text("DUE: PAST DUE").size(11).font(BOLD).color(k.bad)].spacing(2);
    let mut sorted = rows.clone();
    sorted.sort_by(|a, b| a["due_date"].as_str().cmp(&b["due_date"].as_str()));
    for r in &sorted {
        let due = r["due_date"].as_str().unwrap_or("");
        if due >= today_s.as_str() && due <= week_s.as_str() { upcoming = upcoming.push(entry(r)); }
        if !due.is_empty() && due < today_s.as_str() { past = past.push(entry(r)); }
    }
    let side = container(scrollable(column![upcoming, ui::hr(), past].spacing(10)).style(theme::scroll).height(Fill)).padding(10).width(300).height(Fill).style(theme::card);
    let month_name = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"][first.month0() as usize];
    let head = row![
        ui::btn("<", Some(Message::Desk(Msg::CalMonth(-1)))), text(format!("{month_name} {}", first.year())).size(18).font(BOLD), ui::btn(">", Some(Message::Desk(Msg::CalMonth(1)))),
        ui::btn("Today", Some(Message::Desk(Msg::CalMonth(0)))), ui::hspace(),
        text("Invoices due").size(12).color(k.info), text("Bills due").size(12).color(k.warn),
    ].spacing(10).align_y(Alignment::Center);
    column![head, row![scrollable(grid).style(theme::scroll).height(Fill).width(Fill), side].spacing(12).height(Fill)].spacing(10).padding(12).height(Fill).into()
}

// ---------------- Employee Center ----------------

/// Employees ▸ Employee Center: employee list, Employee Information and the payroll runs.
pub fn employee_center(app: &App) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let emps = d["employees"].as_array().cloned().unwrap_or_default();
    let sel = app.desk.emp.filter(|id| emps.iter().any(|e| e["id"].as_i64() == Some(*id))).or_else(|| emps.first().and_then(|e| e["id"].as_i64()));
    let name = |e: &Value| format!("{} {}", e["first_name"].as_str().unwrap_or(""), e["last_name"].as_str().unwrap_or(""));
    let busy = app.people.busy;
    let toolbar = row![
        ui::btn("New Employee", None),
        ui::btn("Manage Employee Information", None),
        ui::btn_primary("Pay Employees", if busy { None } else { Some(Message::People(crate::people::Msg::RunPayroll)) }),
        ui::hspace(),
        ui::btn("Payroll Summary", rep("payroll-activity")),
    ].spacing(6).align_y(Alignment::Center);
    let mut names = column![row![text("NAME").size(11).font(BOLD).color(k.muted).width(Fill), text("STATUS").size(11).font(BOLD).color(k.muted)].padding([6, 10])].spacing(0);
    for e in &emps {
        let id = e["id"].as_i64().unwrap_or(0);
        let on = sel == Some(id);
        names = names.push(button(row![text(name(e)).size(13).width(Fill), text(if e["status"] == "ACTIVE" { "Active" } else { "Inactive" }).size(12).color(k.muted)])
            .padding([4, 10]).width(Fill)
            .style(move |_, s| button::Style { background: if on { Some(iced::Background::Color(k.accent_soft)) } else if matches!(s, button::Status::Hovered) { Some(iced::Background::Color(k.hover)) } else { None },
                text_color: k.text, border: iced::Border::default(), shadow: Default::default(), snap: true })
            .on_press(Message::Desk(Msg::Employee(id))));
    }
    let left = container(column![
        row![text("Employees").size(13).font(BOLD), ui::hspace(), text("Active Employees").size(12).color(k.muted)].padding([8, 10]),
        scrollable(names).style(theme::scroll).height(Fill),
    ].spacing(6)).width(300).height(Fill).style(theme::card);
    let info: Element<Message> = match sel.and_then(|id| emps.iter().find(|e| e["id"].as_i64() == Some(id))) {
        None => container(ui::muted("No employees yet — add them in the Payroll settings of the web version for now.")).padding(16).width(Fill).style(theme::card).into(),
        Some(e) => {
            let s = |key: &str| e[key].as_str().unwrap_or("").to_string();
            let row_ = |l: &'static str, v: String| -> Element<'_, Message> { row![text(l).size(12).color(k.info).width(120), text(if v.is_empty() { "—".into() } else { v }).size(13)].spacing(6).into() };
            let rate = if e["pay_basis"] == "HOURLY" { format!("{} per hour", ui::money(e["hourly_rate_e2"].as_i64().unwrap_or(0) / 100)) } else { format!("{} per year", ui::money(e["annual_salary"].as_i64().unwrap_or(0))) };
            container(column![
                text("Employee Information").size(15).font(BOLD),
                row![
                    column![row_("Full Name", name(e)), row_("Main Email", s("email")), row_("Address", s("address").replace('\n', ", "))].spacing(6).width(Fill),
                    column![row_("Hire Date", app.d(&s("start_date"))), row_("Employment", match s("employment_type").as_str() { "FULLTIME" => "Full-time", "PARTTIME" => "Part-time", "CASUAL" => "Casual", "CONTRACTOR" => "Contractor", _ => "—" }.to_string()), row_("Pay Rate", rate), row_("Pay Schedule", s("calendar_name"))].spacing(6).width(Fill),
                ].spacing(20),
            ].spacing(10)).padding(14).width(Fill).style(theme::card).into()
        }
    };
    let runs: Vec<(Vec<Element<Message>>, Option<Message>)> = d["runs"].as_array().cloned().unwrap_or_default().iter().map(|r| (vec![
        ui::cell("Paycheck run"), ui::cell(r["number"].as_str().unwrap_or("").to_string()), ui::cell(app.d(r["payment_date"].as_str().unwrap_or(""))),
        ui::cell_muted(format!("{} – {}", app.d(r["period_start"].as_str().unwrap_or("")), app.d(r["period_end"].as_str().unwrap_or("")))),
        ui::cell_money(r["wages"].as_i64().unwrap_or(0)), ui::cell_money(r["net_pay"].as_i64().unwrap_or(0)), ui::cell_muted(if r["status"] == "POSTED" { "Paid" } else { "Draft" }),
    ], r["id"].as_i64().map(|id| Message::Nav(Route::PayRun(id))))).collect();
    let right = column![
        info,
        text("Transactions").size(14).font(BOLD),
        ui::table(&[ui::col("Type", 12), ui::col("Num", 8), ui::col("Date", 11), ui::col("Pay Period", 20), ui::coln("Gross", 12), ui::coln("Net", 12), ui::col("Status", 8)], runs, "No paychecks yet."),
        text(d["settings"]["disclaimer"].as_str().unwrap_or("Payroll tax tables are approximations — verify before paying anyone.").to_string()).size(12).color(k.muted),
    ].spacing(10).width(Fill);
    column![toolbar, row![left, scrollable(right).style(theme::scroll).height(Fill)].spacing(10).height(Fill)].spacing(10).padding(12).height(Fill).into()
}

// ---------------- Income / Bill Tracker ----------------

/// Everything the trackers show: sales (or purchases) with their open estimates (or POs).
pub fn load_tracker(app: &mut App, sales: bool) -> Task<Message> {
    let be = app.be.clone();
    let g = app.page_gen;
    Task::perform(async move {
        tokio::task::spawn_blocking(move || -> Result<Value, String> {
            let kinds: &[&str] = if sales { &["SALES", "QUOTE", "SALESORDER", "DELAYED"] } else { &["ACCPAY", "ACCPAYCREDIT", "PO"] };
            let mut rows = vec![];
            for k in kinds { rows.extend(be.call_now("docs.list", serde_json::json!({"type": k, "limit": 1000}))?["rows"].as_array().cloned().unwrap_or_default()); }
            rows.retain(|r| !matches!(r["status"].as_str(), Some("DELETED")));
            rows.sort_by(|a, b| b["date"].as_str().cmp(&a["date"].as_str()));
            Ok(serde_json::json!({"rows": rows}))
        }).await.unwrap_or_else(|e| Err(e.to_string()))
    }, move |r| Message::Loaded(g, r))
}

/// Which tracker box a row falls in.
fn tracker_bucket(r: &Value, since: &str) -> &'static str {
    let kind = r["type"].as_str().unwrap_or("");
    let st = r["status"].as_str().unwrap_or("");
    match (kind, st) {
        ("QUOTE", "SENT") if r["subtype"] == "SALESORDER" => "orders",
        ("QUOTE", "SENT" | "DRAFT" | "ACCEPTED") if r["subtype"].as_str().unwrap_or("").is_empty() => "estimates",
        ("PO", "AUTHORISED" | "DRAFT" | "SUBMITTED") => "estimates",
        ("ACCREC" | "ACCPAY", "AUTHORISED") if r["overdue"].as_i64() == Some(1) => "overdue",
        ("ACCREC" | "ACCPAY", "AUTHORISED" | "DRAFT" | "SUBMITTED") => "open",
        ("ACCREC" | "ACCPAY", "PAID") if r["paid_date"].as_str().unwrap_or("") >= since => "paid",
        _ => "",
    }
}

/// Customers ▸ Income Tracker / Vendors ▸ Bill Tracker.
pub fn tracker(app: &App, sales: bool) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let rows = d["rows"].as_array().cloned().unwrap_or_default();
    let since = ui::add_days_iso(&ui::today_iso(), -30);
    let sum = |b: &str| -> (usize, i64) {
        rows.iter().filter(|r| in_box(r, &since, b)).fold((0, 0), |(n, s), r| (n + 1, s + if matches!(b, "paid" | "estimates" | "orders") { r["total"].as_i64().unwrap_or(0) } else { r["amount_due"].as_i64().unwrap_or(0) }))
    };
    let filter = if app.desk.tracker.is_empty() { "all" } else { app.desk.tracker };
    let boxes: Vec<(&'static str, &'static str, iced::Color)> = if sales {
        vec![("estimates", "ESTIMATES", hexc(0x2f6fb5)), ("orders", "SALES ORDERS", hexc(0x1f8a9a)), ("open", "OPEN INVOICES", hexc(0x7a52b0)), ("overdue", "OVERDUE", hexc(0xd7502b)), ("paid", "PAID LAST 30 DAYS", hexc(0x3c9a3e))]
    } else {
        vec![("estimates", "PURCHASE ORDERS", hexc(0x2f6fb5)), ("open", "OPEN BILLS", hexc(0x7a52b0)), ("overdue", "OVERDUE", hexc(0xd7502b)), ("paid", "PAID LAST 30 DAYS", hexc(0x3c9a3e))]
    };
    let bar = row(boxes.into_iter().map(|(key, lbl, c)| {
        let (n, amt) = sum(key);
        let on = filter == key;
        button(column![text(ui::money(amt)).size(20).font(BOLD).color(iced::Color::WHITE), text(format!("{n} {lbl}")).size(11).font(BOLD).color(iced::Color::WHITE)].spacing(2))
            .padding([10, 14]).width(Fill)
            .style(move |_, s| button::Style { background: Some(iced::Background::Color(if matches!(s, button::Status::Hovered) || on { iced::Color { a: 0.85, ..c } } else { c })),
                text_color: iced::Color::WHITE, border: iced::Border { color: if on { k.text } else { c }, width: if on { 2.0 } else { 0.0 }, radius: 3.0.into() }, shadow: Default::default(), snap: true })
            .on_press(Message::Desk(Msg::Tracker(key))).into()
    })).spacing(4);
    let q = app.list_q.to_lowercase();
    let shown: Vec<&Value> = rows.iter().filter(|r| filter == "all" || in_box(r, &since, filter))
        .filter(|r| q.is_empty() || r["contact_name"].as_str().unwrap_or("").to_lowercase().contains(&q) || r["number"].as_str().unwrap_or("").to_lowercase().contains(&q)).collect();
    let cols: Vec<(&str, u16, bool)> = vec![(if sales { "Customer:Job" } else { "Vendor" }, 20, false), ("Type", 12, false), ("Number", 10, false), ("Date", 10, false), ("Due Date", 10, false),
        ("Amount", 11, true), ("Open Balance", 11, true), ("Status", 10, false), ("Action", 12, false)];
    let rows_el: Vec<(Vec<Element<Message>>, Option<Message>)> = shown.iter().map(|r| {
        let kind = r["type"].as_str().unwrap_or("");
        let sub = r["subtype"].as_str().unwrap_or("");
        let route_kind = if crate::doc::quote_variant(sub) { sub.to_string() } else { kind.to_string() };
        let status = crate::vocab::status(app.skin, if crate::doc::quote_variant(sub) { sub } else { kind }, r["status"].as_str().unwrap_or(""), r["overdue"].as_i64() == Some(1));
        let open = r["amount_due"].as_i64().unwrap_or(0);
        let cid = r["contact_id"].as_i64();
        let action: Element<Message> = match kind {
            "ACCREC" if open > 0 && r["status"] == "AUTHORISED" => ui::link("Receive Payment", Message::Nav(Route::ReceivePayment(cid))),
            "ACCPAY" if open > 0 && r["status"] == "AUTHORISED" => ui::link("Pay Bill", Message::Nav(Route::PayBills)),
            "QUOTE" if bucket_is(r, &since, "estimates") || bucket_is(r, &since, "orders") => ui::link("Create Invoice", Message::Nav(Route::Doc { kind: route_kind.clone(), id: r["id"].as_i64() })),
            _ => Space::new().into(),
        };
        (vec![ui::cell(r["contact_name"].as_str().unwrap_or("").to_string()), ui::cell_muted(type_name(kind, sub)), ui::cell(r["number"].as_str().unwrap_or("").to_string()),
            ui::cell(app.d(r["date"].as_str().unwrap_or(""))), ui::cell_muted(app.d(r["due_date"].as_str().unwrap_or(""))),
            ui::cell_money(r["total"].as_i64().unwrap_or(0)), ui::cell_money(if matches!(kind, "ACCREC" | "ACCPAY" | "ACCRECCREDIT" | "ACCPAYCREDIT") { open } else { 0 }),
            text(status).size(13).color(crate::lists::status_color(status)).into(), action],
         Some(Message::Nav(Route::Doc { kind: route_kind, id: r["id"].as_i64() })))
    }).collect();
    let filters = row![
        text(if sales { "CUSTOMER:JOB" } else { "VENDOR" }).size(11).font(BOLD).color(k.info),
        text_input("All", &app.list_q).on_input(Message::ListQuery).padding([4, 7]).size(13).style(theme::input).width(220),
        text("STATUS").size(11).font(BOLD).color(k.info),
        text(match filter { "estimates" => if sales { "Estimates" } else { "Purchase Orders" }, "orders" => "Sales Orders", "open" => "Open", "overdue" => "Overdue", "paid" => "Paid", _ => "All" }).size(13),
        ui::hspace(),
        if filter != "all" { ui::btn("Clear Filter", Some(Message::Desk(Msg::Tracker(filter)))) } else { Space::new().into() },
    ].spacing(10).align_y(Alignment::Center);
    let manage = if sales {
        row![ui::btn("Create Invoices", doc_new("ACCREC")), ui::btn("Create Estimates", doc_new("QUOTE")), ui::btn("Receive Payments", nav(Route::ReceivePayment(None))), ui::btn("Enter Sales Receipts", doc_new("SALESRECEIPT"))]
    } else {
        row![ui::btn("Enter Bills", doc_new("ACCPAY")), ui::btn("Pay Bills", nav(Route::PayBills)), ui::btn("Create Purchase Orders", doc_new("PO")), ui::btn("Write Checks", nav(Route::Check(None)))]
    }.spacing(8);
    let cols_ui: Vec<ui::Col> = cols.iter().map(|(h, w, r)| ui::Col { title: *h, width: *w, right: *r }).collect();
    scrollable(column![bar, filters, ui::table(&cols_ui, rows_el, "Nothing to show."), row![text("Manage Transactions").size(12).color(k.muted), manage].spacing(12).align_y(Alignment::Center)]
        .spacing(10).padding(12)).style(theme::scroll).height(Fill).into()
}

fn bucket_is(r: &Value, since: &str, b: &str) -> bool { tracker_bucket(r, since) == b }
/// Tracker box membership: Open includes Overdue (as in QuickBooks Desktop).
fn in_box(r: &Value, since: &str, b: &str) -> bool { let x = tracker_bucket(r, since); x == b || (b == "open" && x == "overdue") }
fn hexc(v: u32) -> iced::Color { iced::Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8) }

// ---------------- Report Center ----------------

const REPORT_CATS: [&str; 11] = ["Company & Financial", "Customers & Receivables", "Sales", "Jobs, Time & Mileage", "Vendors & Payables", "Purchases", "Inventory",
    "Employees & Payroll", "Banking", "Accountant & Taxes", "Budgets & Forecasts"];

/// The Report Center category a report sits in (as in QuickBooks Desktop).
fn report_cat(key: &str) -> &'static str {
    match key {
        "profit-and-loss" | "balance-sheet" | "cash-flow-statement" | "executive-summary" | "cash-summary" | "income-by-contact" => "Company & Financial",
        "aged-receivables" | "receivable-invoice-detail" | "contact-transactions" => "Customers & Receivables",
        "sales-by-contact" | "sales-by-item" => "Sales",
        "project-profitability" | "expense-claims" => "Jobs, Time & Mileage",
        "aged-payables" | "payable-invoice-detail" | "contractor-1099" | "sales-tax-by-agency" | "tax-reconciliation" => "Vendors & Payables",
        "purchases-by-contact" | "purchases-by-item" => "Purchases",
        "inventory-summary" | "fixed-asset-register" | "fixed-asset-reconciliation" | "depreciation-schedule" => "Inventory",
        "payroll-activity" | "leave-balances" => "Employees & Payroll",
        "bank-summary" | "bank-reconciliation" => "Banking",
        "budget-summary" | "budget-variance" => "Budgets & Forecasts",
        _ => "Accountant & Taxes",
    }
}

/// Reports ▸ Report Center.
pub fn report_center(app: &App) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let us = app.country() == "US";
    let cat = app.desk.report_cat.unwrap_or("Company & Financial");
    let mut left = column![].spacing(1);
    for c in REPORT_CATS {
        let on = c == cat;
        left = left.push(button(text(c).size(13).font(if on { BOLD } else { iced::Font::DEFAULT })).style(theme::nav_item(on)).padding([6, 10]).width(Fill).on_press(Message::Desk(Msg::ReportCat(c))));
    }
    let mut right = column![text(cat).size(18).font(BOLD)].spacing(8);
    let mut any = false;
    for r in d.as_array().cloned().unwrap_or_default() {
        let key = r["key"].as_str().unwrap_or("").to_string();
        if report_cat(&key) != cat || (!us && matches!(key.as_str(), "contractor-1099" | "sales-tax-by-agency")) || key == "bank-reconciliation" { continue; }
        any = true;
        let title = title(app, &Route::Report(key.clone()));
        let title = if title == "Report" { r["title"].as_str().unwrap_or("").to_string() } else { title };
        right = right.push(container(row![
            column![text(title).size(14).font(BOLD), text(r["description"].as_str().unwrap_or("").to_string()).size(12).color(k.muted)].spacing(2).width(Fill),
            ui::btn("Run", Some(Message::Nav(Route::Report(key)))),
        ].spacing(10).align_y(Alignment::Center)).padding([8, 12]).width(Fill).style(theme::card));
    }
    if !any { right = right.push(ui::muted("No reports in this category yet.")); }
    let tabs = row![text("Standard").size(13).font(BOLD), text("Memorized").size(13).color(k.faint), text("Favorites").size(13).color(k.faint), text("Recent").size(13).color(k.faint), text("Contributed").size(13).color(k.faint)].spacing(20);
    column![
        tabs,
        row![
            container(scrollable(left).style(theme::scroll)).width(230).height(Fill).padding(6).style(theme::card),
            scrollable(right.width(Fill)).style(theme::scroll).height(Fill),
        ].spacing(12).height(Fill),
    ].spacing(10).padding(12).height(Fill).into()
}

// ---------------- Lists ----------------

/// A Desktop list window: header row, rows, and the button bar along the bottom.
fn list_window<'a>(cols: &[(&str, u16, bool)], rows: Vec<(Vec<Element<'a, Message>>, Option<Message>)>, bottom: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let k = t();
    let head = container(row(cols.iter().map(|(h, w, r)| {
        let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
        container(if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
    })).spacing(8).padding([6, 10])).style(theme::header_row).width(Fill);
    let mut list = column![].spacing(0);
    for (n, (cells, msg)) in rows.into_iter().enumerate() {
        let shade = n % 2 == 1;
        let r = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(8).align_y(Alignment::Center).padding([4, 10]);
        list = list.push(button(r).padding(0).width(Fill).on_press_maybe(msg)
            .style(move |_, s| button::Style { background: Some(iced::Background::Color(if matches!(s, button::Status::Hovered) { k.hover } else if shade { k.panel2 } else { k.panel })),
                text_color: k.text, border: iced::Border::default(), shadow: Default::default(), snap: true }));
    }
    column![
        container(column![head, scrollable(list).style(theme::scroll).height(Fill)]).style(theme::card).width(Fill).height(Fill).clip(true),
        row(bottom).spacing(8).align_y(Alignment::Center),
    ].spacing(8).padding(12).height(Fill).into()
}

/// Estimates, Credit Memos and Purchase Orders lists.
pub fn doc_list(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let (kind, who, new_label) = match app.route { Route::Quotes => ("QUOTE", "Customer:Job", "New Estimate"), Route::CreditNotes => ("ACCRECCREDIT", "Customer:Job", "New Credit Memo"), _ => ("PO", "Vendor", "New Purchase Order") };
    let cols: Vec<(&str, u16, bool)> = vec![("Num", 10, false), ("Date", 11, false), (who, 26, false), ("Memo", 22, false), ("Amount", 12, true), ("Status", 12, false)];
    let rows = d["rows"].as_array().cloned().unwrap_or_default().iter().map(|r| {
        let st = crate::vocab::status(app.skin, kind, r["status"].as_str().unwrap_or(""), r["overdue"].as_i64() == Some(1));
        (vec![ui::cell(r["number"].as_str().unwrap_or("").to_string()), ui::cell(app.d(r["date"].as_str().unwrap_or(""))), ui::cell(r["contact_name"].as_str().unwrap_or("").to_string()),
            ui::cell_muted(r["reference"].as_str().unwrap_or("").to_string()), ui::cell_money(r["total"].as_i64().unwrap_or(0)), text(st).size(13).color(crate::lists::status_color(st)).into()],
         Some(Message::Nav(Route::Doc { kind: kind.into(), id: r["id"].as_i64() })))
    }).collect();
    let mut bottom = vec![ui::btn(new_label, doc_new(kind))];
    if kind == "PO" { bottom.push(ui::btn("Enter Bills", doc_new("ACCPAY"))); } else { bottom.push(ui::btn("Create Invoices", doc_new("ACCREC"))); }
    bottom.push(ui::hspace());
    bottom.push(text_input("Search", &app.list_q).on_input(Message::ListQuery).padding([5, 7]).size(13).style(theme::input).width(220).into());
    list_window(&cols, rows, bottom)
}

/// Lists ▸ Chart of Accounts.
pub fn chart(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let mut accts: Vec<Value> = d.as_array().cloned().unwrap_or_default().into_iter().filter(|a| a["status"] == "ACTIVE").collect();
    let s = |a: &Value, k: &str| a[k].as_str().unwrap_or("").to_string();
    accts.sort_by_key(|a| (crate::coa::desktop_type(a["type"].as_str().unwrap_or(""), a["bank_type"].as_str().unwrap_or(""), a["system_account"].as_str().unwrap_or("")).1, s(a, "code")));
    let cols: Vec<(&str, u16, bool)> = vec![("Name", 40), ("Type", 22), ("Balance Total", 16)].into_iter().map(|(h, w)| (h, w, h == "Balance Total")).collect();
    let rows = accts.iter().map(|a| {
        let kind = a["type"].as_str().unwrap_or("");
        let (ty, _) = crate::coa::desktop_type(kind, a["bank_type"].as_str().unwrap_or(""), a["system_account"].as_str().unwrap_or(""));
        // Desktop shows balances only for balance sheet accounts.
        let bs = matches!(a["class"].as_str(), Some("ASSET" | "LIABILITY" | "EQUITY"));
        let sign = if matches!(a["class"].as_str(), Some("LIABILITY" | "EQUITY")) { -1 } else { 1 };
        let bal: Element<Message> = if bs { ui::cell_money(sign * a["ytd_balance"].as_i64().unwrap_or(0)) } else { Space::new().into() };
        let msg = if kind == "BANK" { a["id"].as_i64().map(|id| Message::Nav(Route::Register(Some(id)))) } else { None };
        (vec![ui::cell(format!("{} · {}", s(a, "code"), s(a, "name"))), ui::cell_muted(ty), bal], msg)
    }).collect();
    list_window(&cols, rows, vec![
        ui::btn("Account: New", nav(Route::NewAccount)),
        ui::btn("Activities: Write Checks", nav(Route::Check(None))), ui::btn("Make Deposits", nav(Route::Deposit)), ui::btn("Use Register", nav(Route::Register(None))),
        ui::btn("Make General Journal Entries", nav(Route::Journal)), ui::btn("Reconcile", nav(Route::Reconcile)),
        ui::hspace(), ui::btn("Reports: Trial Balance", rep("trial-balance")),
    ])
}

/// Lists ▸ Item List.
pub fn item_list(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let items = d.as_array().cloned().unwrap_or_default();
    let acct = |id: Option<i64>| id.and_then(|id| app.lk.accounts.iter().find(|a| a["id"].as_i64() == Some(id))).map(|a| format!("{} · {}", a["code"].as_str().unwrap_or(""), a["name"].as_str().unwrap_or(""))).unwrap_or_default();
    let cols: Vec<(&str, u16, bool)> = vec![("Name", 20, false), ("Description", 28, false), ("Type", 12, false), ("Account", 22, false), ("On Hand", 8, true), ("Price", 10, true)];
    let rows = items.iter().map(|x| {
        let tracked = x["is_tracked"].as_i64() == Some(1);
        let ty = if tracked { "Inventory Part" } else if x["is_purchased"].as_i64() == Some(1) && x["is_sold"].as_i64() == Some(1) { "Non-inventory Part" } else { "Service" };
        (vec![
            ui::cell(x["name"].as_str().unwrap_or("").to_string()),
            ui::cell_muted(x["sales_description"].as_str().unwrap_or("").to_string()),
            ui::cell_muted(ty),
            ui::cell_muted(acct(x["sale_account_id"].as_i64())),
            if tracked { ui::num(format!("{}", x["qty_on_hand_e4"].as_i64().unwrap_or(0) as f64 / 10000.0)) } else { Space::new().into() },
            ui::num(ui::money(x["sale_unit_e2"].as_i64().unwrap_or(0) / 100)),
        ], None)
    }).collect();
    list_window(&cols, rows, vec![
        ui::btn("Item: New", nav(Route::NewProduct)),
        ui::btn("Activities: Create Invoices", doc_new("ACCREC")), ui::btn("Enter Sales Receipts", doc_new("SALESRECEIPT")), ui::btn("Create Purchase Orders", doc_new("PO")),
        ui::hspace(), ui::btn("Reports: Sales by Item", rep("sales-by-item")),
    ])
}

// ---------------- Register ----------------

/// A bank account register: every transaction with a running balance, newest first.
pub fn register(app: &App) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return container(ui::muted("Loading…")).padding(20).into() };
    let banks: Vec<crate::app::Opt> = d["banks"].as_array().cloned().unwrap_or_default().iter()
        .map(|b| crate::app::Opt { id: b["id"].as_i64().unwrap_or(0), label: b["name"].as_str().unwrap_or("").to_string() }).collect();
    let sel = banks.iter().find(|b| Some(b.id) == d["account"].as_i64()).cloned();
    let cols: Vec<(&str, u16, bool)> = vec![("Date", 10, false), ("Number", 9, false), ("Payee", 20, false), ("Memo", 22, false), ("Payment", 11, true), ("✓", 3, false), ("Deposit", 11, true), ("Balance", 12, true)];
    let head = container(row(cols.iter().map(|(h, w, r)| {
        let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
        container(if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
    })).spacing(6).padding([6, 10])).style(theme::header_row).width(Fill);
    let mut list = column![].spacing(0);
    // Oldest first, as in QuickBooks Desktop, with the running balance after each line.
    let mut rows: Vec<Value> = d["rows"].as_array().cloned().unwrap_or_default();
    rows.reverse();
    let mut bal = d["balance"].as_i64().unwrap_or(0) - rows.iter().map(|r| r["amount"].as_i64().unwrap_or(0)).sum::<i64>();
    for (n, r) in rows.iter().enumerate() {
        let amt = r["amount"].as_i64().unwrap_or(0);
        bal += amt;
        let desc = r["description"].as_str().unwrap_or("");
        let number = if let Some(no) = desc.strip_prefix("Check ") { no.to_string() } else { r["reference"].as_str().unwrap_or("").to_string() };
        let kind = match r["kind"].as_str().unwrap_or("") { "payment" if amt > 0 => "PMT", "payment" => "BILLPMT", "deposit" => "DEP", "transfer_from" | "transfer_to" => "TRANSFR", _ if desc.starts_with("Check") => "CHK", _ if amt > 0 => "DEP", _ => "EXP" };
        let cells: Vec<Element<Message>> = vec![
            column![ui::cell(app.d(r["date"].as_str().unwrap_or(""))), text(kind).size(10).color(k.faint)].spacing(0).into(),
            ui::cell_muted(number),
            ui::cell(r["contact"].as_str().unwrap_or("").to_string()),
            ui::cell_muted(desc.to_string()),
            if amt < 0 { ui::cell_money(-amt) } else { Space::new().into() },
            text(if r["reconciled"].as_i64() == Some(1) { "✓" } else { "" }).size(13).color(k.ok).into(),
            if amt > 0 { ui::cell_money(amt) } else { Space::new().into() },
            text(ui::money(bal)).size(13).color(if bal < 0 { k.bad } else { k.text }).align_x(iced::alignment::Horizontal::Right).width(Fill).into(),
        ];
        let msg = r["document_id"].as_i64().map(|id| Message::Nav(Route::Doc { kind: if amt > 0 { "ACCREC".into() } else { "ACCPAY".into() }, id: Some(id) }));
        let rr = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(6).align_y(Alignment::Center).padding([4, 10]);
        let shade = n % 2 == 1;
        list = list.push(button(rr).padding(0).width(Fill).on_press_maybe(msg)
            .style(move |_, s| button::Style { background: Some(iced::Background::Color(if matches!(s, button::Status::Hovered) { k.hover } else if shade { k.panel2 } else { k.panel })),
                text_color: k.text, border: iced::Border::default(), shadow: Default::default(), snap: true }));
    }
    let top = row![
        text("Go to").size(13).color(k.muted),
        pick_list(banks, sel, |b: crate::app::Opt| Message::Nav(Route::Register(Some(b.id)))).padding([5, 7]).text_size(13).style(theme::pick).width(300),
        ui::hspace(),
        ui::btn("Write Checks", nav(Route::Check(d["account"].as_i64()))),
        ui::btn("Make Deposits", nav(Route::Deposit)),
        ui::btn("Reconcile", nav(Route::Reconcile)),
    ].spacing(10).align_y(Alignment::Center);
    let ending = row![ui::hspace(), text("ENDING BALANCE").size(12).font(BOLD).color(k.info), text(ui::money(d["balance"].as_i64().unwrap_or(0))).size(15).font(BOLD)].spacing(14).align_y(Alignment::Center);
    // The blank row for typing a new transaction straight into the register.
    let r = &app.desk.reg;
    let accounts = app.lk.account_opts(|a| a["type"] != "BANK");
    let inp = |v: &str, ph: &str, f: RegField, right: bool| -> Element<'_, Message> {
        let ti = text_input(ph, v).on_input(move |s| Message::Desk(Msg::Reg(f.clone(), s))).on_submit(Message::Desk(Msg::RegRecord)).padding([4, 6]).size(13).style(theme::cell_input);
        if right { ti.align_x(iced::alignment::Horizontal::Right).into() } else { ti.into() }
    };
    let entry_cells: Vec<Element<Message>> = vec![
        inp(&r.date, &app.d(&ui::today_iso()), RegField::Date, false),
        inp(&r.number, "Number", RegField::Number, false),
        column![inp(&r.payee, "Payee", RegField::Payee, false), pick_list(accounts, r.account.clone(), |a| Message::Desk(Msg::RegAccount(a))).placeholder("Account").padding([3, 6]).text_size(12).style(theme::pick).width(Fill)].spacing(2).into(),
        inp(&r.memo, "Memo", RegField::Memo, false),
        inp(&r.payment, "", RegField::Payment, true),
        Space::new().into(),
        inp(&r.deposit, "", RegField::Deposit, true),
        ui::btn_primary("Record", if r.busy { None } else { Some(Message::Desk(Msg::RegRecord)) }),
    ];
    let entry = container(row(entry_cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(6).align_y(Alignment::Center).padding([6, 10]))
        .style(theme::accent_soft).width(Fill);
    column![
        top,
        container(column![head, scrollable(list).anchor_bottom().style(theme::scroll).height(Fill), entry]).style(theme::card).width(Fill).height(Fill).clip(true),
        ending,
    ].spacing(10).padding(12).height(Fill).into()
}

// ---------------- Customer / Vendor Center ----------------

const CUSTOMER_TX: [&str; 6] = ["Estimates", "Sales Orders", "Invoices", "Statement Charges", "Sales Receipts", "Credit Memos"];
const VENDOR_TX: [&str; 3] = ["Purchase Orders", "Bills", "Vendor Credits"];

/// Engine list type for a Transactions-tab entry, and whether to keep only (or drop) sales receipts.
fn tx_kind(t: &str) -> (&'static str, Option<bool>) {
    match t {
        "Estimates" => ("QUOTE", None), "Sales Orders" => ("SALESORDER", None), "Statement Charges" => ("DELAYED", None), "Sales Receipts" => ("ACCREC", Some(true)), "Invoices" => ("ACCREC", Some(false)),
        "Credit Memos" => ("ACCRECCREDIT", None), "Purchase Orders" => ("PO", None), "Bills" => ("ACCPAY", None), _ => ("ACCPAYCREDIT", None),
    }
}

/// The Center's Transactions tab: every transaction of the chosen type.
fn center_transactions(app: &App, customers: bool) -> Element<'_, Message> {
    let types: &[&'static str] = if customers { &CUSTOMER_TX } else { &VENDOR_TX };
    let sel = app.desk.tx_type.filter(|t| types.contains(t));
    let mut list = column![].spacing(1);
    for ty in types {
        let on = sel == Some(*ty);
        list = list.push(button(text(*ty).size(13).font(if on { BOLD } else { iced::Font::DEFAULT })).style(theme::nav_item(on)).padding([5, 10]).width(Fill).on_press(Message::Desk(Msg::TxType(Some(ty)))));
    }
    let left = container(column![tabs_row(customers, true), list].spacing(8)).padding(6).width(340).height(Fill).style(theme::card);
    let right: Element<Message> = match sel {
        None => container(ui::muted("Choose a transaction type on the left.")).padding(16).width(Fill).style(theme::card).into(),
        Some(ty) => {
            let rows: Vec<(Vec<Element<Message>>, Option<Message>)> = app.desk.tx_rows.iter().map(|r| {
                let kind = r["type"].as_str().unwrap_or("");
                let sub = r["subtype"].as_str().unwrap_or("");
                let route_kind = if crate::doc::quote_variant(sub) { sub.to_string() } else { kind.to_string() };
                let st = crate::vocab::status(app.skin, if crate::doc::quote_variant(sub) { sub } else { kind }, r["status"].as_str().unwrap_or(""), r["overdue"].as_i64() == Some(1));
                (vec![ui::cell(r["contact_name"].as_str().unwrap_or("").to_string()), ui::cell(r["number"].as_str().unwrap_or("").to_string()), ui::cell(app.d(r["date"].as_str().unwrap_or(""))),
                    ui::cell_muted(app.d(r["due_date"].as_str().unwrap_or(""))), ui::cell_money(r["total"].as_i64().unwrap_or(0)), ui::cell_money(r["amount_due"].as_i64().unwrap_or(0)),
                    text(st).size(13).color(crate::lists::status_color(st)).into()],
                 Some(Message::Nav(Route::Doc { kind: route_kind, id: r["id"].as_i64() })))
            }).collect();
            column![
                text(ty).size(15).font(BOLD),
                ui::table(&[ui::col(if customers { "Customer:Job" } else { "Vendor" }, 20), ui::col("Num", 10), ui::col("Date", 11), ui::col("Due Date", 11), ui::coln("Amount", 12), ui::coln("Open Balance", 12), ui::col("Status", 10)],
                    rows, "None yet."),
            ].spacing(8).width(Fill).into()
        }
    };
    row![left, scrollable(right).style(theme::scroll).height(Fill)].spacing(10).height(Fill).into()
}

/// "Customers & Jobs | Transactions" (or "Vendors | Transactions") tabs at the top of a Center's left pane.
fn tabs_row(customers: bool, tx: bool) -> Element<'static, Message> {
    let k = t();
    let tab = move |label: &'static str, on: bool, m: Message| -> Element<'static, Message> {
        column![
            button(text(label).size(13).font(if on { BOLD } else { iced::Font::DEFAULT })).style(theme::tab(on)).padding([5, 10]).on_press(m),
            container(Space::new()).width(Fill).height(2).style(move |_: &iced::Theme| container::Style { background: if on { Some(iced::Background::Color(k.accent)) } else { None }, ..Default::default() }),
        ].width(iced::Shrink).into()
    };
    let first = if customers { "Customers & Jobs" } else { "Vendors" };
    let default_type = if customers { "Invoices" } else { "Bills" };
    row![tab(first, !tx, Message::Desk(Msg::TxType(None))), tab("Transactions", tx, Message::Desk(Msg::TxType(Some(default_type))))].spacing(4).into()
}

fn type_name(kind: &str, sub: &str) -> &'static str {
    match (kind, sub) {
        (_, "RECEIPT") => "Sales Receipt", (_, "DELAYEDCHARGE") => "Statement Charge", (_, "DELAYEDCREDIT") => "Statement Credit", (_, "SALESORDER") => "Sales Order",
        ("ACCREC", _) => "Invoice", ("ACCRECCREDIT", _) => "Credit Memo", ("QUOTE", _) => "Estimate", ("ACCPAY", _) => "Bill", ("ACCPAYCREDIT", _) => "Credit", ("PO", _) => "Purchase Order", _ => "Transaction",
    }
}

pub fn center(app: &App, customers: bool) -> Element<'_, Message> {
    let k = t();
    let list = app.data.as_ref().and_then(|d| d.as_array().cloned()).unwrap_or_default();
    let tool = |label: &'static str, msg: Option<Message>| -> Element<'_, Message> { ui::btn(label, msg) };
    let toolbar = if customers {
        row![tool("New Customer & Job", nav(Route::NewContact(true))), tool("Create Invoices", doc_new("ACCREC")), tool("Receive Payments", nav(Route::ReceivePayment(app.desk.sel))),
            tool("Create Estimates", doc_new("QUOTE")), tool("Enter Sales Receipts", doc_new("SALESRECEIPT")), ui::hspace(), tool("Income Tracker", nav(Route::AllSales))]
    } else {
        row![tool("New Vendor", nav(Route::NewContact(false))), tool("Enter Bills", doc_new("ACCPAY")), tool("Pay Bills", nav(Route::PayBills)),
            tool("Write Checks", nav(Route::Check(None))), tool("Create Purchase Orders", doc_new("PO")), ui::hspace(), tool("Bill Tracker", nav(Route::Bills))]
    }.spacing(6).align_y(Alignment::Center);

    // ---- left: names with balances ----
    let mut names = column![row![
        text("NAME").size(11).font(BOLD).color(k.muted).width(Fill),
        text("BALANCE TOTAL").size(11).font(BOLD).color(k.muted),
    ].padding([6, 10])].spacing(0);
    for c in &list {
        let id = c["id"].as_i64().unwrap_or(0);
        let bal = if customers { c["receivable"].as_i64().unwrap_or(0) } else { c["payable"].as_i64().unwrap_or(0) };
        let on = app.desk.sel == Some(id);
        names = names.push(button(row![text(c["name"].as_str().unwrap_or("").to_string()).size(13).width(Fill), text(ui::money(bal)).size(13)].spacing(8))
            .padding([4, 10]).width(Fill)
            .style(move |_, s| button::Style { background: if on { Some(iced::Background::Color(k.accent_soft)) } else if matches!(s, button::Status::Hovered) { Some(iced::Background::Color(k.hover)) } else { None },
                text_color: k.text, border: iced::Border::default(), shadow: Default::default(), snap: true })
            .on_press(Message::Desk(Msg::Select(id))));
    }
    if app.desk.tx_type.is_some() {
        return column![toolbar, center_transactions(app, customers)].spacing(10).padding(12).height(Fill).into();
    }
    let left = container(column![
        tabs_row(customers, false),
        row![text(if customers { "Active Customers" } else { "Active Vendors" }).size(12).color(k.muted)].padding([4, 10]),
        container(text_input("Search", &app.list_q).on_input(Message::ListQuery).padding([5, 8]).size(13).style(theme::input)).padding([0, 10]),
        scrollable(names).style(theme::scroll).height(Fill),
    ].spacing(6)).width(340).height(Fill).style(theme::card);

    // ---- right: information + transactions ----
    let right: Element<Message> = match &app.desk.detail {
        None => container(ui::muted(if list.is_empty() { "No names yet — use the New button above." } else { "Loading…" })).padding(20).width(Fill).height(Fill).style(theme::card).into(),
        Some(d) => {
            let c = &d["contact"];
            let id = c["id"].as_i64().unwrap_or(0);
            let s = |key: &str| c[key].as_str().unwrap_or("").to_string();
            let info_row = |label: &'static str, v: String| -> Element<'_, Message> { row![text(label).size(12).color(k.info).width(110), text(if v.is_empty() { "—".into() } else { v }).size(13)].spacing(6).into() };
            let full = format!("{} {}", s("first_name"), s("last_name")).trim().to_string();
            let info = container(column![
                row![text(if customers { "Customer Information" } else { "Vendor Information" }).size(15).font(BOLD), ui::hspace(),
                    ui::link("QuickReport", Message::Nav(Route::Report("contact-transactions".into()))),
                    ui::link("Open Balance", Message::Nav(Route::Report(if customers { "aged-receivables" } else { "aged-payables" }.into())))].spacing(14).align_y(Alignment::Center),
                row![
                    column![info_row("Company Name", s("name")), info_row("Full Name", full), info_row(if customers { "Bill To" } else { "Billed From" }, s("postal_address").replace('\n', ", "))].spacing(6).width(Fill),
                    column![info_row("Main Phone", s("phone")), info_row("Main Email", s("email")), info_row("Account No.", s("account_number"))].spacing(6).width(Fill),
                ].spacing(20),
            ].spacing(10)).padding(14).width(Fill).style(theme::card);
            let rows: Vec<(Vec<Element<Message>>, Option<Message>)> = d["rows"].as_array().cloned().unwrap_or_default().iter().map(|r| {
                let kind = r["type"].as_str().unwrap_or("");
                let sub = r["subtype"].as_str().unwrap_or("");
                let route_kind = if crate::doc::quote_variant(sub) { sub.to_string() } else { kind.to_string() };
                (vec![ui::cell(type_name(kind, sub)), ui::cell_muted(r["number"].as_str().unwrap_or("").to_string()), ui::cell(app.d(r["date"].as_str().unwrap_or(""))),
                    ui::cell_muted(crate::vocab::status(app.skin, if crate::doc::quote_variant(sub) { sub } else { kind }, r["status"].as_str().unwrap_or(""), r["overdue"].as_i64() == Some(1))),
                    ui::cell_money(r["amount_due"].as_i64().unwrap_or(0)), ui::cell_money(r["total"].as_i64().unwrap_or(0))],
                 Some(Message::Nav(Route::Doc { kind: route_kind, id: r["id"].as_i64() })))
            }).collect();
            let actions = if customers { row![ui::btn("Create Invoice", Some(Message::NewDocFor("ACCREC".into(), id))), ui::btn("Receive Payment", nav(Route::ReceivePayment(Some(id))))] }
                else { row![ui::btn("Enter Bill", Some(Message::NewDocFor("ACCPAY".into(), id))), ui::btn("Pay Bills", nav(Route::PayBills))] };
            column![
                info,
                row![text("Transactions").size(14).font(BOLD), ui::hspace(), actions.spacing(6)].align_y(Alignment::Center),
                ui::table(&[ui::col("Type", 16), ui::col("Num", 10), ui::col("Date", 12), ui::col("Status", 12), ui::coln("Open Balance", 14), ui::coln("Amount", 14)], rows, "No transactions for this name yet."),
            ].spacing(10).width(Fill).into()
        }
    };
    let right = scrollable(right).style(theme::scroll).height(Fill).width(Fill);
    column![toolbar, row![left, right].spacing(10).height(Fill)].spacing(10).padding(12).height(Fill).into()
}

// ---------------- Home page ----------------

/// A clickable workflow icon on the Home page.
fn node<'a>(ic: &'static str, label: &'static str, msg: Option<Message>) -> Element<'a, Message> {
    let k = t();
    let on = msg.is_some();
    let c = column![
        container(icon(ic, 30.0, if on { k.info } else { k.faint })).padding(8).style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(k.panel2)), border: iced::Border { color: k.line, width: 1.0, radius: 8.0.into() }, ..Default::default() }),
        text(label).size(12).color(if on { k.text } else { k.faint }).align_x(iced::alignment::Horizontal::Center).width(Fill),
    ].spacing(4).align_x(Alignment::Center).width(96);
    match msg {
        Some(m) => button(c).style(theme::ghost).padding(4).on_press(m).into(),
        None => container(c).padding(4).into(),
    }
}
fn arrow<'a>() -> Element<'a, Message> { container(icon("arrow", 22.0, t().faint)).padding(Padding { top: 18.0, ..Padding::ZERO }).into() }
fn gap<'a>() -> Element<'a, Message> { Space::new().width(30).into() }

fn band<'a>(name: &'static str, rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let k = t();
    let head = container(text(name).size(12).font(BOLD).color(k.accent_ink)).padding([5, 10]).width(Fill)
        .style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.info)), border: iced::Border { radius: iced::border::Radius { top_left: 6.0, top_right: 6.0, bottom_left: 0.0, bottom_right: 0.0 }, ..Default::default() }, ..Default::default() });
    container(column![head, container(column(rows).spacing(6)).padding([10, 14])]).width(Fill).style(theme::card).into()
}

pub fn home(app: &App) -> Element<'_, Message> {
    let k = t();
    let tab = |label: &'static str, on: bool, v: bool| -> Element<'_, Message> {
        column![
            button(text(label).size(14).font(if on { BOLD } else { iced::Font::DEFAULT })).style(theme::tab(on)).padding([6, 12]).on_press(Message::Desk(Msg::Insights(v))),
            container(Space::new()).width(Fill).height(2).style(move |_: &iced::Theme| container::Style { background: if on { Some(iced::Background::Color(k.accent)) } else { None }, ..Default::default() }),
        ].width(iced::Shrink).into()
    };
    let tabs = row![tab("Home Page", !app.desk.insights, false), tab("Insights", app.desk.insights, true)].spacing(4);
    if app.desk.insights {
        return column![container(tabs).padding(Padding { top: 10.0, left: 20.0, ..Padding::ZERO }), crate::dashboard::view(app)].into();
    }
    let vendors = band("VENDORS", vec![
        row![node("box", "Purchase Orders", doc_new("PO")), arrow(), node("truck", "Receive Inventory", Some(Message::ReceiveItems)), arrow(), node("expenses", "Enter Bills Against Inventory", nav(Route::PurchaseOrders)), gap(), node("taxes", "Manage Sales Tax", nav(Route::Taxes))].align_y(Alignment::Start).into(),
        row![node("expenses", "Enter Bills", doc_new("ACCPAY")), arrow(), node("check-write", "Pay Bills", nav(Route::PayBills)), gap(), node("file", "1099 Summary", rep("contractor-1099"))].align_y(Alignment::Start).into(),
    ]);
    let customers = band("CUSTOMERS", vec![
        row![node("file", "Estimates", doc_new("QUOTE")), arrow(), node("sales", "Create Invoices", doc_new("ACCREC")), arrow(), node("receive", "Receive Payments", nav(Route::ReceivePayment(None))), arrow(), node("deposit", "Record Deposits", nav(Route::Deposit))].align_y(Alignment::Start).into(),
        row![node("receive", "Create Sales Receipts", doc_new("SALESRECEIPT")), gap(), node("list", "Statement Charges", doc_new("DELAYEDCHARGE")), arrow(), node("docs", "Statements", nav(Route::Statements)), gap(), node("refresh", "Refunds & Credits", doc_new("ACCRECCREDIT"))].align_y(Alignment::Start).into(),
    ]);
    let employees = band("EMPLOYEES", vec![
        row![node("payroll", "Payroll Center", nav(Route::Payroll)), gap(), node("check-write", "Pay Employees", nav(Route::Payroll)), arrow(), node("taxes", "Pay Liabilities", None), arrow(), node("docs", "Process Payroll Forms", None)].align_y(Alignment::Start).into(),
    ]);
    let company = band("COMPANY", vec![
        row![node("accounting", "Chart of Accounts", nav(Route::Chart)), node("box", "Items & Services", nav(Route::Products))].into(),
        row![node("calendar", "Calendar", nav(Route::Calendar)), node("mileage", "Vehicle Mileage", nav(Route::Mileage))].into(),
    ]);
    let banking = band("BANKING", vec![
        row![node("deposit", "Record Deposits", nav(Route::Deposit)), node("reconcile", "Reconcile", nav(Route::Reconcile))].into(),
        row![node("check-write", "Write Checks", nav(Route::Check(None))), node("register", "Check Register", nav(Route::Register(None)))].into(),
        row![node("card", "Enter Credit Card Charges", nav(Route::Spend(None))), node("swap", "Transfer Funds", nav(Route::Transfer))].into(),
    ]);
    let body = row![
        column![vendors, customers, employees].spacing(12).width(Length::FillPortion(3)),
        column![company, banking].spacing(12).width(Length::FillPortion(1)),
    ].spacing(12);
    scrollable(column![tabs, body].spacing(12).padding(Padding { top: 10.0, left: 20.0, right: 20.0, bottom: 24.0 }).max_width(1400)).style(theme::scroll).height(Fill).into()
}
