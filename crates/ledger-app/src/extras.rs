//! Screens for features the engine already had but the native app didn't show: fixed assets,
//! the audit log and currencies (more join them). Built from the shared page/table pieces so
//! they look at home in every layout; each layout reaches them through the feature registry.

use crate::app::{App, Message, Opt, Route};
use crate::backend::Reply;
use crate::theme::{self, t};
use crate::ui::{self, BOLD};
use iced::widget::{column, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length, Task};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct State {
    /// Fixed assets: tab (DRAFT / REGISTERED / DISPOSED), the new-asset form, depreciation date,
    /// and the asset being disposed (id, date, sale price, proceeds account, gain/loss account).
    pub fa_tab: Option<&'static str>,
    pub fa_new: Option<BTreeMap<&'static str, String>>,
    pub fa_type: Option<Opt>,
    pub fa_method: Option<&'static str>,
    pub fa_dep_date: String,
    pub fa_dispose: Option<(i64, String, String, Option<Opt>, Option<Opt>)>,
    /// Find & recode: filters (account, tax rate, contact, from, to, text), the lines ticked,
    /// and what to change them to.
    pub rc_account: Option<Opt>,
    pub rc_tax: Option<Opt>,
    pub rc_contact: Option<Opt>,
    pub rc_text: BTreeMap<&'static str, String>,
    pub rc_sel: std::collections::BTreeSet<i64>,
    pub rc_to_account: Option<Opt>,
    pub rc_to_tax: Option<Opt>,
    /// Price rules: the new rule being entered (name, min qty, percent, start, end) and its item / level.
    pub pr_new: Option<[String; 5]>,
    pub pr_item: Option<Opt>,
    pub pr_level: Option<Opt>,
    /// Locations: the new location (name, address), and the stock transfer being entered
    /// (item, from, to, quantity, date).
    pub loc_new: Option<(String, String)>,
    /// Custom fields: the definition being added (record type, name, kind), and values being
    /// typed on a record (field id → text).
    pub cf_new: Option<(&'static str, String, &'static str)>,
    pub cf_vals: BTreeMap<i64, String>,
    /// Landed costs: the cost bill, the receipt bills ticked, and how to spread it.
    pub lc_bill: Option<Opt>,
    pub lc_receipts: std::collections::BTreeSet<i64>,
    pub lc_qty: bool,
    /// Consolidation: companies ticked, dates and intercompany account codes, and the result.
    pub co_files: std::collections::BTreeSet<String>,
    pub co_from: String,
    pub co_to: String,
    pub co_elim: String,
    pub co_result: Option<Value>,
    /// Item receipts: the receipt being entered (vendor, date, reference, location, and lines of
    /// item, quantity, unit cost, lot/serials).
    /// Approval rules: the rule being added (document type, amount, approver role).
    pub ap_new: Option<(&'static str, String, &'static str)>,
    pub ir_vendor: Option<Opt>,
    pub ir_date: String,
    pub ir_ref: String,
    pub ir_loc: Option<Opt>,
    pub ir_lines: Vec<(Option<Opt>, String, String, String)>,
    /// Revenue recognition: the invoice, months and start date for a new schedule, and the run-to date.
    pub rr_invoice: Option<Opt>,
    pub rr_months: String,
    pub rr_start: String,
    pub rr_to: String,
    /// Lots: the item whose tracking is being set, and expiry dates being typed (by item|lot).
    pub lot_item: Option<Opt>,
    pub lot_exp: BTreeMap<String, String>,
    pub tr_item: Option<Opt>,
    pub tr_from: Option<Opt>,
    pub tr_to: Option<Opt>,
    pub tr_qty: String,
    pub tr_date: String,
    /// Payment reminders: a new level's days after the due date, as typed.
    pub dun_days: String,
    /// Contact page: the credit limit being typed (None = not being edited).
    pub credit_text: Option<String>,
    /// Contact page: the note being written in History & notes.
    pub contact_note: String,
    /// Audit log: what kind of record to show.
    pub audit_entity: Option<&'static str>,
    /// Currencies: the rate being entered (code, date, rate).
    pub fx_rate: Option<(String, String, String)>,
    pub fx_reval_date: String,
    pub busy: bool,
    /// Budgets: the budget shown, the financial year (its first month, YYYY-MM), cells typed
    /// (account id, period) → amount, and the name of a new budget being created.
    pub bud_sel: Option<i64>,
    pub bud_start: Option<String>,
    pub bud_cells: BTreeMap<(i64, String), String>,
    pub bud_new: Option<String>,
    /// Expense claims: status tab, and the claim being entered (mileage?, fields, account, tax).
    pub cl_tab: Option<&'static str>,
    pub cl_new: Option<(bool, BTreeMap<&'static str, String>)>,
    pub cl_account: Option<Opt>,
    pub cl_tax: Option<Opt>,
    /// Bank rules: the rule being added — name, type, bank account, condition (field, op, value),
    /// allocation account, transfer account, contact.
    pub br_new: Option<BTreeMap<&'static str, String>>,
    pub br_bank: Option<Opt>,
    pub br_account: Option<Opt>,
    pub br_contact: Option<Opt>,
    /// Import: what's being imported, the chosen file (name, text), the dry-run preview, and the
    /// opening-balance date.
    pub ie_what: Option<&'static str>,
    pub ie_file: Option<(String, String)>,
    pub ie_preview: Option<Value>,
    pub ie_date: String,
    /// Custom reports: the report being built — name, monthly columns, and its rows.
    pub cr_new: Option<(String, bool, Vec<CrRow>)>,
    /// Payroll settings: new pay schedule (name, frequency, first period start) and leave type (name, hours a year).
    pub ps_cal: Option<(String, &'static str, String)>,
    pub ps_leave: Option<(String, String)>,
    /// Projects: the project open below the list (its projects.get), and the forms on it.
    pub pj_sel: Option<i64>,
    pub pj_detail: Option<Value>,
    pub pj_new: Option<(String, Option<Opt>)>,
    pub pj_task: Option<(String, &'static str, String)>,
    pub pj_time: Option<(Option<Opt>, String, String, String)>,
    pub pj_cost: Option<(String, String, String, bool)>,
}

/// A custom report row: a heading, a total of account types, or a formula over earlier rows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrRow { pub kind: &'static str, pub label: String, pub types: Vec<&'static str>, pub expr: String, pub sign: i64 }

#[derive(Debug, Clone)]
pub enum Msg {
    FaTab(&'static str), FaNew, FaField(&'static str, String), FaType(Opt), FaMethod(&'static str), FaSave, FaCancel, FaRegister(i64),
    FaDepDate(String), FaRun, FaDisposeOpen(i64), FaDisposeField(u8, String), FaDisposeProceeds(Opt), FaDisposeGain(Opt), FaDispose,
    AuditEntity(&'static str),
    /// Contact History & notes: the note text, then add it to the contact.
    ContactNote(String), ContactNoteAdd(i64),
    /// Contact credit: the limit as typed, on/off hold, save the limit.
    CreditText(String), CreditHold(i64, bool), CreditSave(i64),
    /// Payment reminders: on/off, a new level's days, add, switch a level on/off, delete, send due reminders now.
    PrNew(bool), PrField(usize, String), PrItem(Opt), PrLevel(Opt), PrSave, PrDelete(i64),
    LotItem(Opt), LotMode(&'static str), LotExpiry(i64, String, String), LotExpirySave(i64, String),
    RtRelease(i64), RtReleased(Reply),
    ApNew(bool), ApType(&'static str), ApMin(String), ApRole(&'static str), ApSave, ApDelete(i64),
    IrVendor(Opt), IrDate(String), IrRef(String), IrLoc(Opt), IrItem(usize, Opt), IrQty(usize, String), IrCost(usize, String), IrLot(usize, String), IrAddLine, IrReceive, IrBill(i64), IrBilled(Reply), IrVoid(i64),
    CoFile(String), CoFrom(String), CoTo(String), CoElim(String), CoRun, CoDone(Reply),
    LcBill(Opt), LcReceipt(i64), LcByQty(bool), LcApply,
    CfNew(Option<&'static str>), CfName(String), CfKind(&'static str), CfSave, CfDelete(i64), CfValue(i64, String), CfSaveValues(&'static str, i64),
    RrInvoice(Opt), RrMonths(String), RrStart(String), RrTo(String), RrCreate, RrRun,
    LocNew(bool), LocField(u8, String), LocSave, LocDelete(i64), TrItem(Opt), TrFrom(Opt), TrTo(Opt), TrQty(String), TrDate(String), TrMove,
    DunOn(bool), DunDays(String), DunAdd, DunActive(i64, i64, bool), DunDelete(i64), DunRun,
    /// Find & recode.
    RcAccount(Opt), RcTax(Opt), RcContact(Opt), RcText(&'static str, String), RcFind, RcClear, RcToggle(i64), RcAll(bool), RcToAccount(Opt), RcToTax(Opt), RcApply,
    FxEnable(String, bool), FxRateOpen(String), FxRateField(u8, String), FxRateSave, FxRevalDate(String), FxRevalue, FxFetch,
    BudPick(i64), BudYear(i32), BudCell(i64, String, String), BudCopyAcross(i64), BudNew(Option<String>), BudCreate, BudSave, BudCreated(Reply),
    ClTab(&'static str), ClNew(bool), ClField(&'static str, String), ClAccount(Opt), ClTax(Opt), ClSave(bool), ClStatus(i64, &'static str), ClCancel,
    BrNew(bool), BrField(&'static str, String), BrBank(Opt), BrAccount(Opt), BrContact(Opt), BrSave, BrDelete(i64),
    IeWhat(&'static str), IePick, IePicked(Result<Option<(String, String)>, String>), IePreview(Reply), IeRun, IeDate(String), IeExport(&'static str), IeExported(Result<String, String>),
    CrNew(bool), CrName(String), CrMonthly(bool), CrAdd(&'static str), CrLabel(usize, String), CrType(usize, &'static str), CrExpr(usize, String), CrSign(usize, bool), CrRemove(usize), CrSave, CrDelete(i64),
    PsCalNew(bool), PsCalField(u8, String), PsCalFreq(&'static str), PsCalSave, PsLeaveNew(bool), PsLeaveField(u8, String), PsLeaveSave,
    PayslipsPdf(i64), PayrunBankFile(i64), BatchBankFile(i64), Saved(Result<String, String>, &'static str),
    PjSelect(i64), PjLoaded(Reply), PjNew(bool), PjNewName(String), PjNewCustomer(Opt), PjCreate,
    PjTaskOpen(bool), PjTaskField(u8, String), PjTaskKind(&'static str), PjTaskSave,
    PjTimeOpen(bool), PjTimeTask(Opt), PjTimeField(u8, String), PjTimeSave,
    PjCostOpen(bool), PjCostField(u8, String), PjCostCharge(bool), PjCostSave, PjStatus(&'static str), PjDone(Reply, &'static str),
    Done(Reply, &'static str),
}

/// Depreciation methods: (code, how each product's screens name it).
pub const METHODS: [(&str, &str); 6] = [("STRAIGHTLINE", "Straight line"), ("DIMINISHING100", "Diminishing value (100%)"), ("DIMINISHING150", "Diminishing value (150%)"),
    ("DIMINISHING200", "Diminishing value (200%)"), ("FULL", "Full depreciation at purchase"), ("NONE", "No depreciation")];

/// Audit log filters: (label, entity name in the log).
pub const AUDIT_ENTITIES: [(&str, &str); 9] = [("Everything", ""), ("Invoices, bills and quotes", "document"), ("Payments", "payment"), ("Contacts", "contact"),
    ("Accounts", "account"), ("Items", "item"), ("Bank transactions", "bank_transaction"), ("Journals", "manual_journal"), ("Settings", "setting")];

pub fn load(app: &mut App) -> Task<Message> {
    let g = app.page_gen;
    let call = |app: &App, m: &str, p: Value| app.be.call(m, p, move |r| Message::Loaded(g, r));
    match app.route {
        Route::FixedAssets => call(app, "assets.list", json!({"status": "ALL"})),
        Route::AuditLog => {
            let entity = AUDIT_ENTITIES.iter().find(|e| Some(e.0) == app.extras.audit_entity).map(|e| e.1).filter(|e| !e.is_empty());
            call(app, "audit.list", json!({"limit": 500, "entity": entity}))
        }
        Route::Currencies => call(app, "currencies.list", json!({})),
        Route::ExpenseClaims => call(app, "receipts.list", json!({})),
        Route::BankRules => call(app, "bankRules.list", json!({})),
        Route::CustomReports => call(app, "layouts.list", json!({})),
        Route::PayrollSettings => call(app, "payroll.settings", json!({})),
        Route::ImportExport => { app.data = Some(json!({})); Task::none() }
        Route::BatchPayments => call(app, "batch.list", json!({})),
        Route::Locations => call(app, "locations.stock", json!({})),
        Route::Lots => call(app, "lots.list", json!({})),
        Route::CustomFields => call(app, "customFields.list", json!({})),
        Route::Consolidation => call(app, "orgs.list", json!({})),
        Route::ItemReceipts => call(app, "itemReceipts.list", json!({})),
        Route::ApprovalRules => call(app, "approvals.list", json!({})),
        Route::Retainage => call(app, "retainage.list", json!({})),
        Route::LandedCosts => {
            let be = app.be.clone();
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<Value, String> {
                    let applied = be.call_now("landed.list", json!({}))?;
                    let bills: Vec<Value> = be.call_now("docs.list", json!({"type": "ACCPAY", "limit": 500}))?["rows"].as_array().cloned().unwrap_or_default()
                        .into_iter().filter(|d| d["type"] == "ACCPAY" && matches!(d["status"].as_str(), Some("AUTHORISED" | "PAID"))).collect();
                    // Bills that brought stock in.
                    let mut with_stock = vec![];
                    for b in &bills {
                        let d = be.call_now("docs.get", json!({"id": b["id"]}))?;
                        if d["lines"].as_array().is_some_and(|ls| ls.iter().any(|l| l["item_id"].is_i64())) { with_stock.push(b["id"].clone()); }
                    }
                    Ok(json!({"applied": applied, "bills": bills, "stock_bills": with_stock}))
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, move |r| Message::Loaded(g, r))
        }
        Route::RevenueRecognition => {
            let be = app.be.clone();
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<Value, String> {
                    let schedules = be.call_now("revrec.list", json!({}))?;
                    let invoices: Vec<Value> = be.call_now("docs.list", json!({"type": "ACCREC", "limit": 500}))?["rows"].as_array().cloned().unwrap_or_default()
                        .into_iter().filter(|d| d["type"] == "ACCREC" && matches!(d["status"].as_str(), Some("AUTHORISED" | "PAID"))).collect();
                    Ok(json!({"schedules": schedules, "invoices": invoices}))
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, move |r| Message::Loaded(g, r))
        }
        Route::PaymentReminders => {
            let be = app.be.clone();
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<Value, String> {
                    let rules = be.call_now("reminders.list", json!({}))?;
                    let on = be.call_now("settings.get", json!({}))?["invoice_reminders_enabled"] == "1";
                    Ok(json!({"rules": rules, "on": on}))
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, move |r| Message::Loaded(g, r))
        }
        Route::Recode => {
            let x = &app.extras;
            let style = app.date_style();
            let date = |k: &str| x.rc_text.get(k).and_then(|v| ui::to_iso(v, style));
            call(app, "recode.find", json!({"account_id": x.rc_account.as_ref().map(|o| o.id), "tax_rate_id": x.rc_tax.as_ref().map(|o| o.id),
                "contact_id": x.rc_contact.as_ref().map(|o| o.id), "from": date("from"), "to": date("to"), "q": x.rc_text.get("q").cloned().unwrap_or_default()}))
        }
        Route::Budgets => {
            let be = app.be.clone();
            let sel = app.extras.bud_sel;
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<Value, String> {
                    let list = be.call_now("budgets.list", json!({}))?;
                    let id = sel.or_else(|| list.as_array().and_then(|a| a.first()).and_then(|b| b["id"].as_i64()));
                    let budget = match id { Some(id) => be.call_now("budgets.get", json!({"id": id}))?, None => Value::Null };
                    Ok(json!({"list": list, "budget": budget}))
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, move |r| Message::Loaded(g, r))
        }
        _ => Task::none(),
    }
}

fn opt_of(app: &App, id: Option<i64>) -> Option<Opt> { id.and_then(|id| app.lk.account_opts(|_| true).into_iter().find(|o| o.id == id)) }

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    let style = app.date_style();
    match m {
        // ---- find & recode ----
        Msg::RcAccount(o) => { app.extras.rc_account = Some(o); Task::none() }
        Msg::RcTax(o) => { app.extras.rc_tax = Some(o); Task::none() }
        Msg::RcContact(o) => { app.extras.rc_contact = Some(o); Task::none() }
        Msg::RcText(k, v) => {
            let v = if k == "q" { v } else { let old = app.extras.rc_text.get(k).cloned().unwrap_or_default(); ui::date_keys(&old, &v, style) };
            app.extras.rc_text.insert(k, v); Task::none()
        }
        Msg::RcFind => { app.extras.rc_sel.clear(); load(app) }
        Msg::RcClear => { app.extras.rc_account = None; app.extras.rc_tax = None; app.extras.rc_contact = None; app.extras.rc_text.clear(); app.extras.rc_sel.clear(); load(app) }
        Msg::RcToggle(id) => { if !app.extras.rc_sel.remove(&id) { app.extras.rc_sel.insert(id); } Task::none() }
        Msg::RcAll(on) => {
            app.extras.rc_sel = if on { app.data.as_ref().and_then(|d| d.as_array()).map(|a| a.iter().filter_map(|l| l["line_id"].as_i64()).collect()).unwrap_or_default() } else { Default::default() };
            Task::none()
        }
        Msg::RcToAccount(o) => { app.extras.rc_to_account = Some(o); Task::none() }
        Msg::RcToTax(o) => { app.extras.rc_to_tax = Some(o); Task::none() }
        Msg::RcApply => {
            let x = &app.extras;
            if x.rc_sel.is_empty() { return app.toast("Tick the lines to recode", true); }
            if x.rc_to_account.is_none() && x.rc_to_tax.is_none() { return app.toast("Choose a new account or tax rate", true); }
            let p = json!({"line_ids": x.rc_sel.iter().collect::<Vec<_>>(), "account_id": x.rc_to_account.as_ref().map(|o| o.id), "tax_rate_id": x.rc_to_tax.as_ref().map(|o| o.id)});
            app.extras.busy = true;
            app.be.call("recode.apply", p, |r| Message::Extras(Msg::Done(r.map(|v| { let n = v["transactions"].as_i64().unwrap_or(0); json!({"n": n}) }), "Recoded — the transactions were reposted")))
        }
        // ---- retainage ----
        Msg::RtRelease(contact) => app.be.call("retainage.release", json!({"contact_id": contact}), |r| Message::Extras(Msg::RtReleased(r))),
        Msg::RtReleased(r) => match r {
            Ok(v) => { let t1 = app.toast("Retainage invoice created; check it and approve", false); let t2 = app.go(Route::Doc { kind: "ACCREC".into(), id: v["id"].as_i64() }); Task::batch([t1, t2]) }
            Err(e) => app.toast(e, true),
        },
        // ---- approval rules ----
        Msg::ApNew(on) => { app.extras.ap_new = on.then(|| ("ACCPAY", String::new(), "ADVISER")); Task::none() }
        Msg::ApType(v) => { if let Some(n) = app.extras.ap_new.as_mut() { n.0 = v; } Task::none() }
        Msg::ApMin(v) => { if let Some(n) = app.extras.ap_new.as_mut() { n.1 = v; } Task::none() }
        Msg::ApRole(v) => { if let Some(n) = app.extras.ap_new.as_mut() { n.2 = v; } Task::none() }
        Msg::ApSave => {
            let Some((t, min, role)) = app.extras.ap_new.clone() else { return Task::none() };
            let Some(min) = (if min.trim().is_empty() { Some(0) } else { ui::parse_money(&min) }) else { return app.toast("Enter the amount, e.g. 1000", true) };
            app.extras.ap_new = None;
            app.be.call("approvals.save", json!({"doc_type": t, "min_total": min, "role": role}), |r| Message::Extras(Msg::Done(r, "Approval rule added")))
        }
        Msg::ApDelete(id) => app.be.call("approvals.delete", json!({"id": id}), |r| Message::Extras(Msg::Done(r, "Approval rule removed"))),
        // ---- item receipts ----
        Msg::IrVendor(o) => { app.extras.ir_vendor = Some(o); Task::none() }
        Msg::IrDate(v) => { let old = if app.extras.ir_date.is_empty() { app.d(&ui::today_iso()) } else { app.extras.ir_date.clone() }; app.extras.ir_date = ui::date_keys(&old, &v, style); Task::none() }
        Msg::IrRef(v) => { app.extras.ir_ref = v; Task::none() }
        Msg::IrLoc(o) => { app.extras.ir_loc = (o.id != 0).then_some(o); Task::none() }
        Msg::IrItem(i, o) => {
            let cost = app.lk.item(o.id).and_then(|it| it["purchase_unit_e2"].as_i64()).map(|c| format!("{:.2}", c as f64 / 10_000.0)).unwrap_or_default();
            while app.extras.ir_lines.len() <= i { app.extras.ir_lines.push(Default::default()); }
            let l = &mut app.extras.ir_lines[i];
            l.0 = Some(o);
            if l.2.is_empty() { l.2 = cost; }
            if l.1.is_empty() { l.1 = "1".into(); }
            Task::none()
        }
        Msg::IrQty(i, v) => { while app.extras.ir_lines.len() <= i { app.extras.ir_lines.push(Default::default()); } app.extras.ir_lines[i].1 = v; Task::none() }
        Msg::IrCost(i, v) => { while app.extras.ir_lines.len() <= i { app.extras.ir_lines.push(Default::default()); } app.extras.ir_lines[i].2 = v; Task::none() }
        Msg::IrLot(i, v) => { while app.extras.ir_lines.len() <= i { app.extras.ir_lines.push(Default::default()); } app.extras.ir_lines[i].3 = v; Task::none() }
        Msg::IrAddLine => { let n = app.extras.ir_lines.len().max(1); app.extras.ir_lines.resize(n + 1, Default::default()); Task::none() }
        Msg::IrReceive => {
            let x = &app.extras;
            let Some(vendor) = x.ir_vendor.as_ref().map(|o| o.id) else { return app.toast("Choose the vendor", true) };
            let shown = if x.ir_date.is_empty() { app.d(&ui::today_iso()) } else { x.ir_date.clone() };
            let Some(date) = ui::to_iso(&shown, style) else { return app.toast("Check the date", true) };
            let lines: Vec<Value> = x.ir_lines.iter().filter_map(|l| l.0.as_ref().map(|o| json!({"item_id": o.id, "quantity": l.1.replace(',', ""), "unit_price": l.2.replace(',', ""), "lot": l.3}))).collect();
            let p = json!({"contact_id": vendor, "date": date, "reference": x.ir_ref, "location_id": x.ir_loc.as_ref().map(|o| o.id), "lines": lines});
            app.be.call("itemReceipts.create", p, |r| Message::Extras(Msg::Done(r, "Items received; enter the bill when it arrives")))
        }
        Msg::IrBill(id) => app.be.call("itemReceipts.bill", json!({"id": id}), |r| Message::Extras(Msg::IrBilled(r))),
        Msg::IrBilled(r) => match r {
            Ok(v) => { let t1 = app.toast("Bill created from the receipt; check it and approve", false); let t2 = app.go(Route::Doc { kind: "ACCPAY".into(), id: v["id"].as_i64() }); Task::batch([t1, t2]) }
            Err(e) => app.toast(e, true),
        },
        Msg::IrVoid(id) => app.be.call("itemReceipts.void", json!({"id": id}), |r| Message::Extras(Msg::Done(r, "Receipt voided"))),
        // ---- consolidation ----
        Msg::CoFile(f) => { if !app.extras.co_files.remove(&f) { app.extras.co_files.insert(f); } Task::none() }
        Msg::CoFrom(v) => { let old = app.extras.co_from.clone(); app.extras.co_from = ui::date_keys(&old, &v, style); Task::none() }
        Msg::CoTo(v) => { let old = app.extras.co_to.clone(); app.extras.co_to = ui::date_keys(&old, &v, style); Task::none() }
        Msg::CoElim(v) => { app.extras.co_elim = v; Task::none() }
        Msg::CoRun => {
            let x = &app.extras;
            if x.co_files.len() < 2 { return app.toast("Tick at least two companies", true); }
            let today = ui::today_iso();
            let from = if x.co_from.is_empty() { Some(format!("{}-01-01", &today[..4])) } else { ui::to_iso(&x.co_from, style) };
            let to = if x.co_to.is_empty() { Some(today) } else { ui::to_iso(&x.co_to, style) };
            let (Some(from), Some(to)) = (from, to) else { return app.toast("Check the dates", true) };
            let elim: Vec<String> = x.co_elim.split(|c: char| c == ',' || c.is_whitespace()).map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
            app.be.call("consolidate", json!({"files": x.co_files.iter().collect::<Vec<_>>(), "from": from, "to": to, "eliminate": elim}), |r| Message::Extras(Msg::CoDone(r)))
        }
        Msg::CoDone(r) => match r { Ok(v) => { app.extras.co_result = Some(v); Task::none() } Err(e) => app.toast(e, true) },
        // ---- landed costs ----
        Msg::LcBill(o) => { app.extras.lc_bill = Some(o); Task::none() }
        Msg::LcReceipt(id) => { if !app.extras.lc_receipts.remove(&id) { app.extras.lc_receipts.insert(id); } Task::none() }
        Msg::LcByQty(v) => { app.extras.lc_qty = v; Task::none() }
        Msg::LcApply => {
            let Some(bill) = app.extras.lc_bill.as_ref().map(|o| o.id) else { return app.toast("Choose the freight, duty or insurance bill", true) };
            if app.extras.lc_receipts.is_empty() { return app.toast("Tick the bills for the stock it belongs to", true) }
            let p = json!({"bill_id": bill, "receipts": app.extras.lc_receipts.iter().collect::<Vec<_>>(), "method": if app.extras.lc_qty { "QTY" } else { "VALUE" }});
            app.extras.lc_bill = None; app.extras.lc_receipts.clear();
            app.be.call("landed.apply", p, |r| Message::Extras(Msg::Done(r, "Landed cost added to the stock")))
        }
        // ---- custom fields ----
        Msg::CfNew(e) => { app.extras.cf_new = e.map(|e| (e, String::new(), "TEXT")); Task::none() }
        Msg::CfName(v) => { if let Some(n) = app.extras.cf_new.as_mut() { n.1 = v; } Task::none() }
        Msg::CfKind(v) => { if let Some(n) = app.extras.cf_new.as_mut() { n.2 = v; } Task::none() }
        Msg::CfSave => {
            let Some((entity, name, kind)) = app.extras.cf_new.take() else { return Task::none() };
            app.be.call("customFields.save", json!({"entity": entity, "name": name, "kind": kind}), |r| Message::Extras(Msg::Done(r, "Custom field added")))
        }
        Msg::CfDelete(id) => app.be.call("customFields.delete", json!({"id": id}), |r| Message::Extras(Msg::Done(r, "Custom field removed"))),
        Msg::CfValue(id, v) => { app.extras.cf_vals.insert(id, v); Task::none() }
        Msg::CfSaveValues(entity, id) => {
            let vals: serde_json::Map<String, Value> = std::mem::take(&mut app.extras.cf_vals).into_iter().map(|(k, v)| {
                // Dates are typed in the company's date style; the engine keeps them as YYYY-MM-DD.
                let v = ui::to_iso(&v, style).filter(|_| v.contains('/') || v.contains(' ')).unwrap_or(v);
                (k.to_string(), json!(v))
            }).collect();
            app.be.call("customFields.setValues", json!({"entity": entity, "entity_id": id, "values": vals}), |r| Message::Extras(Msg::Done(r, "Custom fields saved")))
        }
        // ---- revenue recognition ----
        Msg::RrInvoice(o) => { app.extras.rr_invoice = Some(o); Task::none() }
        Msg::RrMonths(v) => { app.extras.rr_months = v.chars().filter(|c| c.is_ascii_digit()).collect(); Task::none() }
        Msg::RrStart(v) => { let old = app.extras.rr_start.clone(); app.extras.rr_start = ui::date_keys(&old, &v, style); Task::none() }
        Msg::RrTo(v) => { let old = if app.extras.rr_to.is_empty() { app.d(&ui::today_iso()) } else { app.extras.rr_to.clone() }; app.extras.rr_to = ui::date_keys(&old, &v, style); Task::none() }
        Msg::RrCreate => {
            let x = &app.extras;
            let Some(inv) = x.rr_invoice.as_ref().map(|o| o.id) else { return app.toast("Choose the invoice", true) };
            let Ok(months) = x.rr_months.parse::<i64>() else { return app.toast("Enter how many months to spread it over", true) };
            let Some(start) = ui::to_iso(&x.rr_start, style) else { return app.toast("Enter the date of the first month", true) };
            app.extras.rr_invoice = None; app.extras.rr_months.clear(); app.extras.rr_start.clear();
            app.be.call("revrec.create", json!({"document_id": inv, "months": months, "start": start}), |r| Message::Extras(Msg::Done(r, "Revenue deferred — recognise it month by month below")))
        }
        Msg::RrRun => {
            let shown = if app.extras.rr_to.is_empty() { app.d(&ui::today_iso()) } else { app.extras.rr_to.clone() };
            let Some(to) = ui::to_iso(&shown, style) else { return app.toast("Check the date", true) };
            app.be.call("revrec.run", json!({"to": to}), |r| Message::Extras(Msg::Done(r, "Revenue recognised up to that date")))
        }
        // ---- lots ----
        Msg::LotItem(o) => { app.extras.lot_item = Some(o); Task::none() }
        Msg::LotMode(mode) => {
            let Some(item) = app.extras.lot_item.as_ref().map(|o| o.id) else { return app.toast("Choose the item first", true) };
            app.be.call("items.setLotTracking", json!({"item_id": item, "mode": mode}), move |r| Message::Extras(Msg::Done(r, match mode { "LOT" => "Tracked by lot number", "SERIAL" => "Tracked by serial number", _ => "No longer tracked by lot or serial" })))
        }
        Msg::LotExpiry(item, lot, v) => {
            let key = format!("{item}|{lot}");
            let old = app.extras.lot_exp.get(&key).cloned().unwrap_or_default();
            app.extras.lot_exp.insert(key, ui::date_keys(&old, &v, style));
            Task::none()
        }
        Msg::LotExpirySave(item, lot) => {
            let shown = app.extras.lot_exp.remove(&format!("{item}|{lot}")).unwrap_or_default();
            let expiry = if shown.trim().is_empty() { None } else { match ui::to_iso(&shown, style) { Some(d) => Some(d), None => return app.toast("Check the expiry date", true) } };
            app.be.call("lots.setExpiry", json!({"item_id": item, "lot": lot, "expiry": expiry}), |r| Message::Extras(Msg::Done(r, "Expiry date saved")))
        }
        // ---- locations ----
        Msg::LocNew(on) => { app.extras.loc_new = on.then(Default::default); Task::none() }
        Msg::LocField(n, v) => { if let Some(f) = app.extras.loc_new.as_mut() { if n == 0 { f.0 = v } else { f.1 = v } } Task::none() }
        Msg::LocSave => {
            let Some((name, address)) = app.extras.loc_new.take() else { return Task::none() };
            app.be.call("locations.save", json!({"name": name, "address": address}), |r| Message::Extras(Msg::Done(r, "Location added")))
        }
        Msg::LocDelete(id) => app.be.call("locations.delete", json!({"id": id}), |r| Message::Extras(Msg::Done(r, "Location removed"))),
        Msg::TrItem(o) => { app.extras.tr_item = Some(o); Task::none() }
        Msg::TrFrom(o) => { app.extras.tr_from = Some(o); Task::none() }
        Msg::TrTo(o) => { app.extras.tr_to = Some(o); Task::none() }
        Msg::TrQty(v) => { app.extras.tr_qty = v; Task::none() }
        Msg::TrDate(v) => { let old = if app.extras.tr_date.is_empty() { app.d(&ui::today_iso()) } else { app.extras.tr_date.clone() }; app.extras.tr_date = ui::date_keys(&old, &v, style); Task::none() }
        Msg::TrMove => {
            let x = &app.extras;
            let Some(item) = x.tr_item.as_ref().map(|o| o.id) else { return app.toast("Choose the item to move", true) };
            let (Some(from), Some(to)) = (x.tr_from.as_ref(), x.tr_to.as_ref()) else { return app.toast("Choose where it's moving from and to", true) };
            let Some(q) = x.tr_qty.trim().parse::<f64>().ok().filter(|q| *q > 0.0) else { return app.toast("Enter the quantity to move", true) };
            let shown = if x.tr_date.is_empty() { app.d(&ui::today_iso()) } else { x.tr_date.clone() };
            let Some(date) = ui::to_iso(&shown, style) else { return app.toast("Check the date", true) };
            let loc = |o: &Opt| if o.id == 0 { None } else { Some(o.id) };
            let p = json!({"item_id": item, "from": loc(from), "to": loc(to), "qty_e4": (q * 10_000.0).round() as i64, "date": date});
            app.extras.tr_qty.clear();
            app.be.call("locations.transfer", p, |r| Message::Extras(Msg::Done(r, "Stock moved")))
        }
        // ---- price rules ----
        Msg::PrNew(on) => { app.extras.pr_new = on.then(|| [String::new(), "1".into(), String::new(), String::new(), String::new()]); app.extras.pr_item = None; app.extras.pr_level = None; Task::none() }
        Msg::PrField(n, v) => {
            if let Some(f) = app.extras.pr_new.as_mut() { f[n] = if n >= 3 { ui::date_keys(&f[n], &v, style) } else { v }; }
            Task::none()
        }
        Msg::PrItem(o) => { app.extras.pr_item = Some(o); Task::none() }
        Msg::PrLevel(o) => { app.extras.pr_level = Some(o); Task::none() }
        Msg::PrSave => {
            let Some(f) = app.extras.pr_new.clone() else { return Task::none() };
            let Some(pct) = f[2].trim().trim_end_matches('%').parse::<f64>().ok() else { return app.toast("Enter the percentage, e.g. -10 for 10% off", true) };
            let qty = f[1].trim().parse::<f64>().unwrap_or(0.0);
            let date = |s: &str| if s.trim().is_empty() { Some(None) } else { ui::to_iso(s, style).map(Some) };
            let (Some(start), Some(end)) = (date(&f[3]), date(&f[4])) else { return app.toast("Check the dates", true) };
            app.extras.busy = true;
            app.extras.pr_new = None;
            app.be.call("priceRules.save", json!({"name": f[0], "item_id": app.extras.pr_item.as_ref().map(|o| o.id), "price_level_id": app.extras.pr_level.as_ref().map(|o| o.id),
                "min_qty_e4": (qty * 10_000.0).round() as i64, "percent_bp": (pct * 100.0).round() as i64, "start_date": start, "end_date": end}), |r| Message::Extras(Msg::Done(r, "Price rule saved")))
        }
        Msg::PrDelete(id) => app.be.call("priceRules.delete", json!({"id": id}), |r| Message::Extras(Msg::Done(r, "Price rule removed"))),
        // ---- payment reminders ----
        Msg::DunOn(on) => app.be.call("settings.update", json!({"invoice_reminders_enabled": if on { "1" } else { "0" }}), move |r| Message::Extras(Msg::Done(r, if on { "Payment reminders are on" } else { "Payment reminders are off" }))),
        Msg::DunDays(s) => { app.extras.dun_days = s.chars().filter(|c| c.is_ascii_digit() || *c == '-').collect(); Task::none() }
        Msg::DunAdd => {
            let Ok(days) = app.extras.dun_days.trim().parse::<i64>() else { return app.toast("Enter how many days after the due date", true) };
            app.extras.dun_days.clear();
            app.be.call("reminders.save", json!({"days": days}), |r| Message::Extras(Msg::Done(r, "Reminder level added")))
        }
        Msg::DunActive(id, days, on) => app.be.call("reminders.save", json!({"id": id, "days": days, "active": on}), |r| Message::Extras(Msg::Done(r, ""))),
        Msg::DunDelete(id) => app.be.call("reminders.delete", json!({"id": id}), |r| Message::Extras(Msg::Done(r, "Reminder level removed"))),
        Msg::DunRun => app.be.call("reminders.run", json!({}), |r| Message::Extras(Msg::Done(r.map(|v| v), "Reminders due today were queued in the Outbox"))),
        // ---- contact credit ----
        Msg::CreditText(s) => { app.extras.credit_text = Some(s); Task::none() }
        Msg::CreditHold(id, on) => {
            let limit = app.data.as_ref().and_then(|d| d["contact"]["credit_limit"].as_i64());
            app.be.call("contacts.setCredit", json!({"id": id, "credit_limit": limit, "hold": on}), move |r| Message::Extras(Msg::Done(r, if on { "On credit hold" } else { "Off credit hold" })))
        }
        Msg::CreditSave(id) => {
            let txt = app.extras.credit_text.take().unwrap_or_default();
            let limit = if txt.trim().is_empty() { None } else { match ui::parse_money(&txt) { Some(v) => Some(v), None => return app.toast("Enter the credit limit as an amount, e.g. 5000", true) } };
            let hold = app.data.as_ref().is_some_and(|d| d["contact"]["credit_hold"] == 1);
            app.be.call("contacts.setCredit", json!({"id": id, "credit_limit": limit, "hold": hold}), |r| Message::Extras(Msg::Done(r, "Credit limit saved")))
        }
        // ---- contact notes ----
        Msg::ContactNote(s) => { app.extras.contact_note = s; Task::none() }
        Msg::ContactNoteAdd(id) => {
            let text = app.extras.contact_note.trim().to_string();
            if text.is_empty() { return Task::none(); }
            app.extras.contact_note.clear();
            app.be.call("notes.add", json!({"entity": "contact", "id": id, "text": text}), |r| Message::Extras(Msg::Done(r, "Note added")))
        }
        // ---- fixed assets ----
        Msg::FaTab(t) => { app.extras.fa_tab = Some(t); Task::none() }
        Msg::FaNew => {
            let mut f = BTreeMap::new();
            f.insert("purchase_date", app.d(&ui::today_iso()));
            app.extras.fa_new = Some(f);
            let types = app.data.as_ref().and_then(|d| d["types"].as_array().cloned()).unwrap_or_default();
            app.extras.fa_type = types.first().map(|t| Opt { id: t["id"].as_i64().unwrap_or(0), label: t["name"].as_str().unwrap_or("").to_string() });
            app.extras.fa_method = None;
            Task::none()
        }
        Msg::FaField(k, v) => {
            if let Some(f) = app.extras.fa_new.as_mut() {
                let v = if k.ends_with("_date") { let old = f.get(k).cloned().unwrap_or_default(); ui::date_keys(&old, &v, style) } else { v };
                f.insert(k, v);
            }
            Task::none()
        }
        Msg::FaType(o) => { app.extras.fa_type = Some(o); Task::none() }
        Msg::FaMethod(m) => { app.extras.fa_method = Some(m); Task::none() }
        Msg::FaCancel => { app.extras.fa_new = None; app.extras.fa_dispose = None; Task::none() }
        Msg::FaSave => {
            let Some(f) = app.extras.fa_new.clone() else { return Task::none() };
            let g = |k: &str| f.get(k).cloned().unwrap_or_default().trim().to_string();
            if g("name").is_empty() { return app.toast("Enter the asset name", true); }
            let Some(date) = ui::to_iso(&g("purchase_date"), style) else { return app.toast("Check the purchase date", true) };
            if ui::parse_money(&g("purchase_price")).unwrap_or(0) <= 0 { return app.toast("Enter the purchase price", true); }
            let asset = json!({"name": g("name"), "description": g("description"), "asset_type_id": app.extras.fa_type.as_ref().map(|t| t.id), "purchase_date": date,
                "purchase_price": g("purchase_price").replace(',', ""), "serial_number": g("serial_number"), "method": app.extras.fa_method,
                "life_years": Some(g("life_years")).filter(|x| !x.is_empty()), "rate": Some(g("rate")).filter(|x| !x.is_empty()),
                "residual_value": Some(g("residual_value")).filter(|x| !x.is_empty())});
            app.extras.busy = true;
            app.be.call("assets.save", json!({"asset": asset}), |r| Message::Extras(Msg::Done(r, "Asset saved as a draft — Register it to start depreciating")))
        }
        Msg::FaRegister(id) => { app.extras.busy = true; app.be.call("assets.register", json!({"id": id}), |r| Message::Extras(Msg::Done(r, "Asset registered"))) }
        Msg::FaDepDate(v) => { let old = if app.extras.fa_dep_date.is_empty() { app.d(&ui::today_iso()) } else { app.extras.fa_dep_date.clone() }; app.extras.fa_dep_date = ui::date_keys(&old, &v, style); Task::none() }
        Msg::FaRun => {
            let shown = if app.extras.fa_dep_date.is_empty() { app.d(&ui::today_iso()) } else { app.extras.fa_dep_date.clone() };
            let Some(date) = ui::to_iso(&shown, style) else { return app.toast("Check the depreciation date", true) };
            app.extras.busy = true;
            app.be.call("assets.runDepreciation", json!({"date": date}), |r| Message::Extras(Msg::Done(r, "Depreciation posted")))
        }
        Msg::FaDisposeOpen(id) => {
            let accts = app.lk.accounts.clone();
            let find = |pred: &dyn Fn(&Value) -> bool| accts.iter().find(|a| a["status"] == "ACTIVE" && pred(a)).and_then(|a| a["id"].as_i64());
            let gain = find(&|a| a["name"].as_str().unwrap_or("").to_lowercase().contains("disposal")).or_else(|| find(&|a| a["type"] == "OTHERINCOME"));
            let bank = crate::payments::bank_only(app).into_iter().next();
            app.extras.fa_dispose = Some((id, app.d(&ui::today_iso()), String::new(), bank, opt_of(app, gain)));
            Task::none()
        }
        Msg::FaDisposeField(f, v) => { if let Some(d) = app.extras.fa_dispose.as_mut() { if f == 0 { d.1 = ui::date_keys(&d.1, &v, style) } else { d.2 = v } } Task::none() }
        Msg::FaDisposeProceeds(o) => { if let Some(d) = app.extras.fa_dispose.as_mut() { d.3 = Some(o); } Task::none() }
        Msg::FaDisposeGain(o) => { if let Some(d) = app.extras.fa_dispose.as_mut() { d.4 = Some(o); } Task::none() }
        Msg::FaDispose => {
            let Some((id, date, price, proceeds, gain)) = app.extras.fa_dispose.clone() else { return Task::none() };
            let Some(date) = ui::to_iso(&date, style) else { return app.toast("Check the disposal date", true) };
            let Some(gain) = gain else { return app.toast("Choose the gain/loss account", true) };
            let price = if price.trim().is_empty() { "0".to_string() } else { price.replace(',', "") };
            app.extras.busy = true;
            app.be.call("assets.dispose", json!({"id": id, "date": date, "sale_price": price, "proceeds_account_id": proceeds.map(|p| p.id), "gain_loss_account_id": gain.id}),
                |r| Message::Extras(Msg::Done(r, "Asset disposed")))
        }
        // ---- audit log ----
        Msg::AuditEntity(e) => { app.extras.audit_entity = Some(e); app.load() }
        // ---- currencies ----
        Msg::FxEnable(code, on) => app.be.call("currencies.enable", json!({"code": code, "enabled": on}), |r| Message::Extras(Msg::Done(r, ""))),
        Msg::FxRateOpen(code) => { app.extras.fx_rate = Some((code, app.d(&ui::today_iso()), String::new())); Task::none() }
        Msg::FxRateField(f, v) => { if let Some(r) = app.extras.fx_rate.as_mut() { if f == 0 { r.1 = ui::date_keys(&r.1, &v, style) } else { r.2 = v } } Task::none() }
        Msg::FxRateSave => {
            let Some((code, date, rate)) = app.extras.fx_rate.clone() else { return Task::none() };
            let Some(date) = ui::to_iso(&date, style) else { return app.toast("Check the date", true) };
            if rate.trim().parse::<f64>().map_or(true, |r| r <= 0.0) { return app.toast("Enter the rate (units of this currency per 1 of your home currency)", true); }
            app.extras.fx_rate = None;
            app.be.call("rates.set", json!({"code": code, "date": date, "rate": rate.trim()}), |r| Message::Extras(Msg::Done(r, "Rate saved")))
        }
        Msg::FxRevalDate(v) => { let old = if app.extras.fx_reval_date.is_empty() { app.d(&ui::today_iso()) } else { app.extras.fx_reval_date.clone() }; app.extras.fx_reval_date = ui::date_keys(&old, &v, style); Task::none() }
        Msg::FxRevalue => {
            let shown = if app.extras.fx_reval_date.is_empty() { app.d(&ui::today_iso()) } else { app.extras.fx_reval_date.clone() };
            let Some(date) = ui::to_iso(&shown, style) else { return app.toast("Check the revaluation date", true) };
            app.be.call("fx.revalue", json!({"date": date}), |r| Message::Extras(Msg::Done(r, "Foreign currency balances revalued")))
        }
        Msg::FxFetch => app.be.call("rates.fetch", json!({}), |r| Message::Extras(Msg::Done(r, "Exchange rates updated from the European Central Bank"))),
        // ---- budgets ----
        Msg::BudPick(id) => { app.extras.bud_sel = Some(id); app.extras.bud_cells.clear(); app.load() }
        Msg::BudYear(step) => {
            let start = budget_start(app);
            let y: i32 = start[..4].parse().unwrap_or(2026);
            app.extras.bud_start = Some(format!("{}{}", y + step, &start[4..]));
            Task::none()
        }
        Msg::BudCell(acct, period, v) => { app.extras.bud_cells.insert((acct, period), v); Task::none() }
        Msg::BudCopyAcross(acct) => {
            let months = budget_months(app);
            let first = cell_value(app, acct, &months[0]);
            for m in &months[1..] { app.extras.bud_cells.insert((acct, m.clone()), first.clone()); }
            Task::none()
        }
        Msg::BudNew(v) => { app.extras.bud_new = v; Task::none() }
        Msg::BudCreate => {
            let name = app.extras.bud_new.clone().unwrap_or_default().trim().to_string();
            if name.is_empty() { return app.toast("Enter a name for the budget", true); }
            app.be.call("budgets.save", json!({"name": name, "lines": []}), |r| Message::Extras(Msg::BudCreated(r)))
        }
        Msg::BudCreated(r) => match r {
            Ok(v) => { app.extras.bud_new = None; app.extras.bud_sel = v["id"].as_i64(); app.extras.bud_cells.clear(); let t1 = app.toast("Budget created", false); Task::batch([t1, app.load()]) }
            Err(e) => app.toast(e, true),
        },
        Msg::BudSave => {
            let Some(b) = app.data.as_ref().map(|d| d["budget"].clone()).filter(|b| !b.is_null()) else { return app.toast("Create a budget first", true) };
            let lines: Vec<Value> = app.extras.bud_cells.iter().map(|((a, p), v)| json!({"account_id": a, "period": p, "amount": if v.trim().is_empty() { "0".to_string() } else { v.replace(',', "") }})).collect();
            if lines.is_empty() { return app.toast("Nothing has changed", true); }
            if let Some(((_, p), v)) = app.extras.bud_cells.iter().find(|(_, v)| !v.trim().is_empty() && ui::parse_money(v).is_none()) { return app.toast(format!("{p}: \"{v}\" isn't an amount"), true); }
            app.extras.busy = true;
            app.be.call("budgets.save", json!({"id": b["id"], "name": b["name"], "tracking_option_id": b["tracking_option_id"], "lines": lines}), |r| Message::Extras(Msg::Done(r, "Budget saved")))
        }
        // ---- expense claims ----
        Msg::ClTab(t) => { app.extras.cl_tab = Some(t); Task::none() }
        Msg::ClNew(mileage) => {
            let mut f = BTreeMap::new();
            f.insert("date", app.d(&ui::today_iso()));
            if mileage { f.insert("rate", if app.country() == "US" { "0.67".into() } else { "0.88".into() }); }
            app.extras.cl_new = Some((mileage, f));
            Task::none()
        }
        Msg::ClField(k, v) => { if let Some((_, f)) = app.extras.cl_new.as_mut() { let v = if k == "date" { ui::date_keys(&f.get(k).cloned().unwrap_or_default(), &v, style) } else { v }; f.insert(k, v); } Task::none() }
        Msg::ClAccount(o) => { app.extras.cl_account = Some(o); Task::none() }
        Msg::ClTax(o) => { app.extras.cl_tax = Some(o); Task::none() }
        Msg::ClCancel => { app.extras.cl_new = None; Task::none() }
        Msg::ClSave(submit) => {
            let Some((mileage, f)) = app.extras.cl_new.clone() else { return Task::none() };
            let g = |k: &str| f.get(k).cloned().unwrap_or_default().trim().to_string();
            let Some(date) = ui::to_iso(&g("date"), style) else { return app.toast("Check the date", true) };
            let receipt = if mileage {
                if g("distance").parse::<f64>().map_or(true, |x| x <= 0.0) { return app.toast("Enter the distance travelled", true); }
                json!({"kind": "MILEAGE", "date": date, "reference": g("purpose"), "supplier": "", "distance": g("distance"), "rate": g("rate"), "lines": [{"account_id": app.extras.cl_account.as_ref().map(|a| a.id), "description": g("purpose")}]})
            } else {
                if ui::parse_money(&g("amount")).unwrap_or(0) <= 0 { return app.toast("Enter the amount spent", true); }
                let Some(acct) = app.extras.cl_account.clone() else { return app.toast("Choose what it was for (the account)", true) };
                json!({"kind": "RECEIPT", "date": date, "supplier": g("supplier"), "reference": g("description"), "line_amount_type": "INCLUSIVE",
                    "lines": [{"description": g("description"), "quantity": "1", "unit_price": g("amount").replace(',', ""), "account_id": acct.id, "tax_rate_id": app.extras.cl_tax.as_ref().map(|t| t.id)}]})
            };
            let be = app.be.clone();
            app.extras.busy = true;
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<Value, String> {
                    let id = be.call_now("receipts.save", json!({"receipt": receipt}))?["id"].clone();
                    if submit { be.call_now("receipts.setStatus", json!({"id": id, "status": "SUBMITTED"}))?; }
                    Ok(json!({"id": id}))
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, move |r| Message::Extras(Msg::Done(r, if submit { "Claim submitted for approval" } else { "Claim saved as a draft" })))
        }
        Msg::ClStatus(id, st) => {
            let what = match st { "SUBMITTED" => "Claim submitted", "AUTHORISED" => "Claim approved — a bill to repay the claimant was created", "DECLINED" => "Claim declined", _ => "Claim updated" };
            app.be.call("receipts.setStatus", json!({"id": id, "status": st}), move |r| Message::Extras(Msg::Done(r, what)))
        }
        // ---- bank rules ----
        Msg::BrNew(on) => {
            app.extras.br_new = on.then(|| { let mut f = BTreeMap::new(); f.insert("type", "SPEND".to_string()); f.insert("field", "any".to_string()); f.insert("op", "contains".to_string()); f });
            app.extras.br_bank = None; app.extras.br_account = None; app.extras.br_contact = None;
            Task::none()
        }
        Msg::BrField(k, v) => { if let Some(f) = app.extras.br_new.as_mut() { f.insert(k, v); } Task::none() }
        Msg::BrBank(o) => { app.extras.br_bank = Some(o); Task::none() }
        Msg::BrAccount(o) => { app.extras.br_account = Some(o); Task::none() }
        Msg::BrContact(o) => { app.extras.br_contact = Some(o); Task::none() }
        Msg::BrSave => {
            let Some(f) = app.extras.br_new.clone() else { return Task::none() };
            let g = |k: &str| f.get(k).cloned().unwrap_or_default().trim().to_string();
            if g("value").is_empty() { return app.toast("Enter what to look for", true); }
            let transfer = g("type") == "TRANSFER";
            let Some(acct) = app.extras.br_account.clone() else { return app.toast(if transfer { "Choose the account to transfer to" } else { "Choose the account to code it to" }, true) };
            let name = if g("name").is_empty() { format!("{} {}", if transfer { "Transfer" } else { "Code" }, g("value")) } else { g("name") };
            let rule = json!({"name": name, "type": g("type"), "account_id": app.extras.br_bank.as_ref().map(|b| b.id), "match_mode": "ALL",
                "conditions": [{"field": g("field"), "op": g("op"), "value": g("value")}], "contact_id": app.extras.br_contact.as_ref().map(|c| c.id), "reference_field": "",
                "fixed_lines": [], "percent_lines": if transfer { json!([]) } else { json!([{"account_id": acct.id, "percent": "100", "description": ""}]) },
                "transfer_account_id": if transfer { Some(acct.id) } else { None }, "priority": 0, "active": true});
            app.be.call("bankRules.save", json!({"rule": rule}), |r| Message::Extras(Msg::Done(r, "Bank rule saved")))
        }
        Msg::BrDelete(id) => app.be.call("bankRules.delete", json!({"id": id}), |r| Message::Extras(Msg::Done(r, "Bank rule deleted"))),
        // ---- import / export ----
        Msg::IeWhat(w) => { app.extras.ie_what = Some(w); app.extras.ie_file = None; app.extras.ie_preview = None; Task::none() }
        Msg::IeDate(v) => { let old = if app.extras.ie_date.is_empty() { app.d(&ui::today_iso()) } else { app.extras.ie_date.clone() }; app.extras.ie_date = ui::date_keys(&old, &v, style); Task::none() }
        Msg::IePick => Task::perform(async move {
            let Some(file) = rfd::AsyncFileDialog::new().set_title("Choose a CSV file").add_filter("CSV", &["csv", "txt"]).pick_file().await else { return Ok(None) };
            let name = file.file_name();
            let bytes = file.read().await;
            Ok(Some((name, String::from_utf8_lossy(&bytes).into_owned())))
        }, |r| Message::Extras(Msg::IePicked(r))),
        Msg::IePicked(r) => match r {
            Ok(Some((name, text))) => {
                app.extras.ie_file = Some((name, text.clone()));
                app.extras.ie_preview = None;
                let (what, date) = ie_params(app);
                app.be.call("import.run", json!({"what": what, "text": text, "dry_run": true, "date": date}), |r| Message::Extras(Msg::IePreview(r)))
            }
            Ok(None) => Task::none(),
            Err(e) => app.toast(e, true),
        },
        Msg::IePreview(r) => match r { Ok(v) => { app.extras.ie_preview = Some(v); Task::none() } Err(e) => { app.extras.ie_file = None; app.toast(e, true) } },
        Msg::IeRun => {
            let Some((_, text)) = app.extras.ie_file.clone() else { return app.toast("Choose a file first", true) };
            let (what, date) = ie_params(app);
            app.extras.busy = true;
            app.be.call("import.run", json!({"what": what, "text": text, "dry_run": false, "date": date}), |r| Message::Extras(Msg::Done(r, "Imported")))
        }
        Msg::IeExport(what) => {
            let st = app.be.st.clone();
            let open = !cfg!(test);
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let csv = { let g = st.org.lock().unwrap(); g.as_ref().ok_or("no company open")?.ledger.export_csv(what).map_err(|e| e.to_string())? };
                    let path = crate::pdf::output_dir().join([what, ".csv"].concat());
                    std::fs::write(&path, csv).map_err(|e| e.to_string())?;
                    if open { crate::pdf::open_file(&path); }
                    Ok(path.display().to_string())
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, |r| Message::Extras(Msg::IeExported(r)))
        }
        Msg::IeExported(r) => match r { Ok(p) => app.toast(format!("Exported to {p}"), false), Err(e) => app.toast(e, true) },
        // ---- custom reports ----
        Msg::CrNew(on) => {
            // Start from a useful shape: income, expenses and a net line.
            app.extras.cr_new = on.then(|| ("".to_string(), true, vec![
                CrRow { kind: "types", label: "Income".into(), types: vec!["REVENUE"], sign: -1, ..Default::default() },
                CrRow { kind: "types", label: "Expenses".into(), types: vec!["EXPENSE"], sign: 1, ..Default::default() },
                CrRow { kind: "formula", label: "Net profit".into(), expr: "r1 - r2".into(), sign: 1, ..Default::default() }]));
            Task::none()
        }
        Msg::CrName(v) => { if let Some(c) = app.extras.cr_new.as_mut() { c.0 = v; } Task::none() }
        Msg::CrMonthly(v) => { if let Some(c) = app.extras.cr_new.as_mut() { c.1 = v; } Task::none() }
        Msg::CrAdd(kind) => { if let Some(c) = app.extras.cr_new.as_mut() { c.2.push(CrRow { kind, sign: 1, types: if kind == "types" { vec!["EXPENSE"] } else { vec![] }, ..Default::default() }); } Task::none() }
        Msg::CrLabel(n, v) => { if let Some(r) = app.extras.cr_new.as_mut().and_then(|c| c.2.get_mut(n)) { r.label = v; } Task::none() }
        Msg::CrType(n, t) => { if let Some(r) = app.extras.cr_new.as_mut().and_then(|c| c.2.get_mut(n)) { r.types = vec![t]; r.sign = if matches!(t, "REVENUE" | "LIABILITY" | "EQUITY") { -1 } else { 1 }; } Task::none() }
        Msg::CrExpr(n, v) => { if let Some(r) = app.extras.cr_new.as_mut().and_then(|c| c.2.get_mut(n)) { r.expr = v; } Task::none() }
        Msg::CrSign(n, flip) => { if let Some(r) = app.extras.cr_new.as_mut().and_then(|c| c.2.get_mut(n)) { r.sign = if flip { -1 } else { 1 }; } Task::none() }
        Msg::CrRemove(n) => { if let Some(c) = app.extras.cr_new.as_mut() { if n < c.2.len() { c.2.remove(n); } } Task::none() }
        Msg::CrSave => {
            let Some((name, monthly, rows)) = app.extras.cr_new.clone() else { return Task::none() };
            if name.trim().is_empty() { return app.toast("Give the report a name", true); }
            if rows.is_empty() { return app.toast("Add at least one row", true); }
            let rows: Vec<Value> = rows.iter().map(|r| match r.kind {
                "heading" => json!({"type": "heading", "label": r.label}),
                "formula" => json!({"type": "formula", "label": r.label, "expr": r.expr, "bold": true}),
                _ => json!({"type": "types", "label": r.label, "types": r.types, "sign": r.sign}),
            }).collect();
            app.be.call("layouts.save", json!({"name": name.trim(), "kind": "CUSTOM", "definition": {"rows": rows, "monthly": monthly}}), |r| Message::Extras(Msg::Done(r, "Custom report saved")))
        }
        Msg::CrDelete(id) => app.be.call("layouts.delete", json!({"id": id}), |r| Message::Extras(Msg::Done(r, "Custom report deleted"))),
        // ---- payroll settings ----
        Msg::PsCalNew(on) => { app.extras.ps_cal = on.then(|| (String::new(), "FORTNIGHTLY", app.d(&ui::today_iso()))); Task::none() }
        Msg::PsCalField(f, v) => { if let Some(c) = app.extras.ps_cal.as_mut() { if f == 0 { c.0 = v } else { c.2 = ui::date_keys(&c.2, &v, style) } } Task::none() }
        Msg::PsCalFreq(fq) => { if let Some(c) = app.extras.ps_cal.as_mut() { c.1 = fq; } Task::none() }
        Msg::PsCalSave => {
            let Some((name, freq, start)) = app.extras.ps_cal.clone() else { return Task::none() };
            if name.trim().is_empty() { return app.toast("Name the pay schedule", true); }
            let Some(start) = ui::to_iso(&start, style) else { return app.toast("Check the first pay period's start date", true) };
            app.be.call("payroll.saveCalendar", json!({"name": name.trim(), "frequency": freq, "start_date": start}), |r| Message::Extras(Msg::Done(r, "Pay schedule added")))
        }
        Msg::PsLeaveNew(on) => { app.extras.ps_leave = on.then(|| (String::new(), String::new())); Task::none() }
        Msg::PsLeaveField(f, v) => { if let Some(l) = app.extras.ps_leave.as_mut() { if f == 0 { l.0 = v } else { l.1 = v } } Task::none() }
        Msg::PsLeaveSave => {
            let Some((name, hours)) = app.extras.ps_leave.clone() else { return Task::none() };
            if name.trim().is_empty() { return app.toast("Name the leave type", true); }
            if !hours.trim().is_empty() && hours.trim().parse::<f64>().is_err() { return app.toast("Hours a year should be a number", true); }
            app.be.call("payroll.saveLeaveType", json!({"name": name.trim(), "hours": hours.trim()}), |r| Message::Extras(Msg::Done(r, "Leave type added")))
        }
        // ---- pay run files ----
        Msg::PayslipsPdf(id) => {
            let be = app.be.clone();
            let style = app.date_style();
            let open = !cfg!(test);
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let run = be.call_now("payroll.payRun", json!({"id": id}))?;
                    let st = be.call_now("settings.get", json!({}))?;
                    let ret = be.call_now("payroll.settings", json!({}))?["retirement_label"].as_str().unwrap_or("Retirement").to_string();
                    let bytes = crate::pdf::payslips(&run, &st, &|x| ui::date(x, style), &ret);
                    let path = crate::pdf::output_dir().join(["Payslips ", run["number"].as_str().unwrap_or("pay run"), ".pdf"].concat());
                    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
                    if open { crate::pdf::open_file(&path); }
                    Ok(path.display().to_string())
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, |r| Message::Extras(Msg::Saved(r, "Payslips saved to")))
        }
        Msg::PayrunBankFile(id) => {
            let st = app.be.st.clone();
            let format = match app.country().as_str() { "GB" | "UK" => "BACS", "US" => "NACHA", _ => "ABA" };
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let (name, text) = { let g = st.org.lock().unwrap(); g.as_ref().ok_or("no company open")?.ledger.payrun_file(id, format).map_err(|e| e.to_string())? };
                    let path = crate::pdf::output_dir().join(name);
                    std::fs::write(&path, text).map_err(|e| e.to_string())?;
                    Ok(path.display().to_string())
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, |r| Message::Extras(Msg::Saved(r, "Bank file saved to — upload it to your bank:")))
        }
        Msg::BatchBankFile(id) => {
            let st = app.be.st.clone();
            let format = match app.country().as_str() { "GB" | "UK" => "BACS", "US" => "NACHA", _ => "ABA" };
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let (name, text) = { let g = st.org.lock().unwrap(); g.as_ref().ok_or("no company open")?.ledger.batch_file(id, format).map_err(|e| e.to_string())? };
                    let path = crate::pdf::output_dir().join(name);
                    std::fs::write(&path, text).map_err(|e| e.to_string())?;
                    Ok(path.display().to_string())
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, |r| Message::Extras(Msg::Saved(r, "Bank file saved to — upload it to your bank:")))
        }
        Msg::Saved(r, what) => match r { Ok(p) => app.toast(format!("{what} {p}"), false), Err(e) => app.toast(e, true) },
        // ---- projects ----
        Msg::PjSelect(id) => { app.extras.pj_sel = Some(id); app.be.call("projects.get", json!({"id": id}), |r| Message::Extras(Msg::PjLoaded(r))) }
        Msg::PjLoaded(r) => match r { Ok(v) => { app.extras.pj_detail = Some(v); Task::none() } Err(e) => app.toast(e, true) },
        Msg::PjNew(on) => { app.extras.pj_new = on.then(|| (String::new(), None)); Task::none() }
        Msg::PjNewName(v) => { if let Some(n) = app.extras.pj_new.as_mut() { n.0 = v; } Task::none() }
        Msg::PjNewCustomer(o) => { if let Some(n) = app.extras.pj_new.as_mut() { n.1 = Some(o); } Task::none() }
        Msg::PjCreate => {
            let Some((name, cust)) = app.extras.pj_new.clone() else { return Task::none() };
            if name.trim().is_empty() { return app.toast("Name the project", true); }
            app.extras.pj_new = None;
            app.be.call("projects.save", json!({"project": {"name": name.trim(), "contact_id": cust.map(|c| c.id)}}), |r| Message::Extras(Msg::Done(r, "Project created")))
        }
        Msg::PjTaskOpen(on) => { app.extras.pj_task = on.then(|| (String::new(), "TIME", String::new())); Task::none() }
        Msg::PjTaskField(f, v) => { if let Some(t) = app.extras.pj_task.as_mut() { if f == 0 { t.0 = v } else { t.2 = v } } Task::none() }
        Msg::PjTaskKind(k) => { if let Some(t) = app.extras.pj_task.as_mut() { t.1 = k; } Task::none() }
        Msg::PjTaskSave => {
            let (Some(pid), Some((name, kind, rate))) = (app.extras.pj_sel, app.extras.pj_task.clone()) else { return Task::none() };
            if name.trim().is_empty() { return app.toast("Name the task", true); }
            if ui::parse_money(&rate).is_none() { return app.toast(if kind == "FIXED" { "Enter the fixed price" } else { "Enter the hourly rate" }, true); }
            app.extras.pj_task = None;
            app.be.call("projects.saveTask", json!({"project_id": pid, "task": {"name": name.trim(), "charge_type": kind, "rate": rate.replace(',', "")}}), |r| Message::Extras(Msg::PjDone(r, "Task added")))
        }
        Msg::PjTimeOpen(on) => { app.extras.pj_time = on.then(|| (None, app.d(&ui::today_iso()), String::new(), String::new())); Task::none() }
        Msg::PjTimeTask(o) => { if let Some(t) = app.extras.pj_time.as_mut() { t.0 = Some(o); } Task::none() }
        Msg::PjTimeField(f, v) => { if let Some(t) = app.extras.pj_time.as_mut() { match f { 0 => t.1 = ui::date_keys(&t.1, &v, style), 1 => t.2 = v, _ => t.3 = v } } Task::none() }
        Msg::PjTimeSave => {
            let (Some(pid), Some((task, date, dur, notes))) = (app.extras.pj_sel, app.extras.pj_time.clone()) else { return Task::none() };
            let Some(task) = task else { return app.toast("Choose the task", true) };
            let Some(minutes) = crate::desktop::parse_duration(&dur) else { return app.toast("Enter the time as h:mm (1:30) or hours (1.5)", true) };
            let Some(date) = ui::to_iso(&date, style) else { return app.toast("Check the date", true) };
            app.extras.pj_time = None;
            app.be.call("projects.addTime", json!({"project_id": pid, "task_id": task.id, "date": date, "minutes": minutes, "description": notes.trim()}), |r| Message::Extras(Msg::PjDone(r, "Time added")))
        }
        Msg::PjCostOpen(on) => { app.extras.pj_cost = on.then(|| (app.d(&ui::today_iso()), String::new(), String::new(), true)); Task::none() }
        Msg::PjCostField(f, v) => { if let Some(c) = app.extras.pj_cost.as_mut() { match f { 0 => c.0 = ui::date_keys(&c.0, &v, style), 1 => c.1 = v, _ => c.2 = v } } Task::none() }
        Msg::PjCostCharge(on) => { if let Some(c) = app.extras.pj_cost.as_mut() { c.3 = on; } Task::none() }
        Msg::PjCostSave => {
            let (Some(pid), Some((date, desc, amount, charge))) = (app.extras.pj_sel, app.extras.pj_cost.clone()) else { return Task::none() };
            if ui::parse_money(&amount).unwrap_or(0) <= 0 { return app.toast("Enter the amount", true); }
            let Some(date) = ui::to_iso(&date, style) else { return app.toast("Check the date", true) };
            app.extras.pj_cost = None;
            app.be.call("projects.addCost", json!({"project_id": pid, "cost": {"date": date, "description": desc.trim(), "amount": amount.replace(',', ""), "chargeable": charge}}), |r| Message::Extras(Msg::PjDone(r, "Expense added")))
        }
        Msg::PjStatus(st) => {
            let Some(p) = app.extras.pj_detail.clone() else { return Task::none() };
            app.be.call("projects.save", json!({"id": p["id"], "project": {"name": p["name"], "contact_id": p["contact_id"], "deadline": p["deadline"], "status": st}}),
                |r| Message::Extras(Msg::PjDone(r, "Project updated")))
        }
        Msg::PjDone(r, what) => match r {
            Ok(_) => {
                let t1 = app.toast(what, false);
                let t2 = match app.extras.pj_sel { Some(id) => app.be.call("projects.get", json!({"id": id}), |r| Message::Extras(Msg::PjLoaded(r))), None => Task::none() };
                Task::batch([t1, t2, app.load()])
            }
            Err(e) => app.toast(e, true),
        },
        // ---- common ----
        Msg::Done(r, what) => {
            app.extras.busy = false;
            match r {
                Ok(_) => {
                    if app.route == Route::ItemReceipts { app.extras.ir_vendor = None; app.extras.ir_ref.clear(); app.extras.ir_lines.clear(); }
                    app.extras.fa_new = None; app.extras.fa_dispose = None; app.extras.bud_cells.clear(); app.extras.cl_new = None; app.extras.br_new = None; app.extras.cr_new = None; app.extras.ps_cal = None; app.extras.ps_leave = None;
                    if app.route == Route::ImportExport { app.extras.ie_file = None; app.extras.ie_preview = None; }
                    let t1 = if what.is_empty() { Task::none() } else { app.toast(what, false) };
                    // An open employee form shows what was just saved.
                    let t2 = match app.desk.emp_form.as_ref().and_then(|f| f.0) { Some(id) => Task::done(Message::Desk(crate::desktop::Msg::EmpEdit(id))), None => Task::none() };
                    Task::batch([t1, app.load(), app.load_lookups(), t2])
                }
                Err(e) => app.toast(e, true),
            }
        }
    }
}

fn loading<'a>() -> Element<'a, Message> { container(ui::muted("Loading…")).padding(20).into() }
fn lbl<'a>(s: &'static str) -> Element<'a, Message> { text(s).size(11).font(BOLD).color(t().info).into() }
fn input<'a>(label: &'static str, v: &str, w: f32, f: impl Fn(String) -> Message + 'a) -> Element<'a, Message> {
    column![lbl(label), text_input("", v).on_input(f).padding([5, 7]).size(13).style(theme::input).width(Length::Fixed(w))].spacing(3).into()
}
fn tabs<'a>(items: &[(&'static str, &'static str)], cur: &'static str, f: fn(&'static str) -> Message) -> Element<'a, Message> {
    let mut r = row![].spacing(4);
    for (code, label) in items { let on = *code == cur; r = r.push(iced::widget::button(text(*label).size(13).font(if on { BOLD } else { iced::Font::DEFAULT })).style(theme::tab(on)).padding([6, 12]).on_press(f(code))); }
    r.into()
}

// ---------------- Fixed assets ----------------

pub fn fixed_assets(app: &App) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return loading() };
    let assets = d["assets"].as_array().cloned().unwrap_or_default();
    let tab = app.extras.fa_tab.unwrap_or("REGISTERED");
    let count = |st: &str| assets.iter().filter(|a| a["status"] == st).count();
    let tab_items = [("DRAFT", "Draft"), ("REGISTERED", "Registered"), ("DISPOSED", "Sold & disposed")];
    let labels: Vec<(&'static str, &'static str)> = tab_items.iter().map(|(c, l)| (*c, *l)).collect();
    let mut body = column![row![tabs(&labels, tab, |c| Message::Extras(Msg::FaTab(c))), ui::hspace(),
        text(format!("{} draft · {} registered · {} disposed", count("DRAFT"), count("REGISTERED"), count("DISPOSED"))).size(12).color(k.muted)].align_y(Alignment::Center)].spacing(12);
    // New asset form.
    if let Some(f) = &app.extras.fa_new {
        let v = |key: &'static str| f.get(key).cloned().unwrap_or_default();
        let types: Vec<Opt> = d["types"].as_array().cloned().unwrap_or_default().iter().map(|t| Opt { id: t["id"].as_i64().unwrap_or(0), label: t["name"].as_str().unwrap_or("").to_string() }).collect();
        let methods: Vec<&'static str> = METHODS.iter().map(|m| m.1).collect();
        let cur_method = app.extras.fa_method.and_then(|m| METHODS.iter().find(|x| x.0 == m)).map(|x| x.1);
        body = body.push(ui::card(column![
            ui::h2("New asset"),
            row![input("ASSET NAME", &v("name"), 280.0, |x| Message::Extras(Msg::FaField("name", x))),
                column![lbl("ASSET TYPE"), pick_list(types, app.extras.fa_type.clone(), |o| Message::Extras(Msg::FaType(o))).padding([5, 7]).text_size(13).style(theme::pick).width(220)].spacing(3),
                input("SERIAL NUMBER", &v("serial_number"), 160.0, |x| Message::Extras(Msg::FaField("serial_number", x)))].spacing(14),
            row![input("PURCHASE DATE", &v("purchase_date"), 140.0, |x| Message::Extras(Msg::FaField("purchase_date", x))),
                input("PURCHASE PRICE", &v("purchase_price"), 140.0, |x| Message::Extras(Msg::FaField("purchase_price", x))),
                input("RESIDUAL VALUE", &v("residual_value"), 130.0, |x| Message::Extras(Msg::FaField("residual_value", x)))].spacing(14),
            row![column![lbl("DEPRECIATION METHOD"), pick_list(methods, cur_method, |l: &'static str| Message::Extras(Msg::FaMethod(METHODS.iter().find(|m| m.1 == l).map(|m| m.0).unwrap_or("STRAIGHTLINE"))))
                    .placeholder("From the asset type").padding([5, 7]).text_size(13).style(theme::pick).width(260)].spacing(3),
                input("EFFECTIVE LIFE (YEARS)", &v("life_years"), 160.0, |x| Message::Extras(Msg::FaField("life_years", x))),
                input("OR RATE (% A YEAR)", &v("rate"), 140.0, |x| Message::Extras(Msg::FaField("rate", x)))].spacing(14),
            input("DESCRIPTION", &v("description"), 560.0, |x| Message::Extras(Msg::FaField("description", x))),
            row![ui::hspace(), ui::btn_primary("Save", if app.extras.busy { None } else { Some(Message::Extras(Msg::FaSave)) }), ui::btn("Cancel", Some(Message::Extras(Msg::FaCancel)))].spacing(8),
        ].spacing(12)));
    }
    // Disposal form.
    if let Some((id, date, price, proceeds, gain)) = &app.extras.fa_dispose {
        let name = assets.iter().find(|a| a["id"].as_i64() == Some(*id)).and_then(|a| a["name"].as_str()).unwrap_or("").to_string();
        let gain_opts = app.lk.account_opts(|a| matches!(a["type"].as_str(), Some("OTHERINCOME" | "REVENUE" | "EXPENSE" | "DIRECTCOSTS")));
        body = body.push(ui::card(column![
            ui::h2(format!("Dispose of {name}")),
            row![input("DATE OF DISPOSAL", date, 140.0, |x| Message::Extras(Msg::FaDisposeField(0, x))), input("SALE PRICE", price, 140.0, |x| Message::Extras(Msg::FaDisposeField(1, x))),
                column![lbl("PROCEEDS PAID INTO"), pick_list(crate::payments::bank_only(app), proceeds.clone(), |o| Message::Extras(Msg::FaDisposeProceeds(o))).padding([5, 7]).text_size(13).style(theme::pick).width(220)].spacing(3),
                column![lbl("GAIN / LOSS ACCOUNT"), pick_list(gain_opts, gain.clone(), |o| Message::Extras(Msg::FaDisposeGain(o))).padding([5, 7]).text_size(13).style(theme::pick).width(240)].spacing(3)].spacing(14),
            row![ui::muted("Depreciation is run to the disposal date first; the book value is written off and any gain or loss posted."), ui::hspace(),
                ui::btn_primary("Dispose", if app.extras.busy { None } else { Some(Message::Extras(Msg::FaDispose)) }), ui::btn("Cancel", Some(Message::Extras(Msg::FaCancel)))].spacing(8).align_y(Alignment::Center),
        ].spacing(12)));
    }
    let rows = assets.iter().filter(|a| a["status"] == tab).map(|a| {
        let id = a["id"].as_i64().unwrap_or(0);
        let action: Element<Message> = match tab {
            "DRAFT" => ui::link("Register", Message::Extras(Msg::FaRegister(id))),
            "REGISTERED" => ui::link("Dispose…", Message::Extras(Msg::FaDisposeOpen(id))),
            _ => ui::cell_muted(a["disposal_date"].as_str().map(|x| app.d(x)).unwrap_or_default()),
        };
        (vec![ui::cell(a["number"].as_str().unwrap_or("").to_string()), ui::cell(a["name"].as_str().unwrap_or("").to_string()), ui::cell_muted(a["type_name"].as_str().unwrap_or("").to_string()),
            ui::cell_muted(app.d(a["purchase_date"].as_str().unwrap_or(""))), ui::cell_money(a["purchase_price"].as_i64().unwrap_or(0)), ui::cell_money(a["book_value"].as_i64().unwrap_or(0)), action], None)
    }).collect();
    body = body.push(ui::table(&[ui::col("Number", 10), ui::col("Asset", 26), ui::col("Type", 18), ui::col("Purchased", 12), ui::coln("Cost", 13), ui::coln("Book value", 13), ui::col("", 10)],
        rows, "No assets here."));
    let runs = d["runs"].as_array().cloned().unwrap_or_default();
    let last = runs.first().and_then(|r| r["period_end"].as_str()).map(|x| format!("Depreciation last run to {}", app.d(x))).unwrap_or_else(|| "Depreciation hasn't been run yet".into());
    let dep_date = if app.extras.fa_dep_date.is_empty() { app.d(&ui::today_iso()) } else { app.extras.fa_dep_date.clone() };
    body = body.push(ui::card(row![column![ui::h2("Run depreciation"), ui::muted(last)].spacing(4), ui::hspace(),
        input("DEPRECIATE TO", &dep_date, 140.0, |x| Message::Extras(Msg::FaDepDate(x))),
        container(ui::btn_primary("Run depreciation", if app.extras.busy { None } else { Some(Message::Extras(Msg::FaRun)) })).padding(iced::Padding { top: 16.0, ..Default::default() })].spacing(14).align_y(Alignment::Center)));
    ui::page(ui::h1("Fixed assets"), vec![ui::btn_primary("New asset", Some(Message::Extras(Msg::FaNew))), ui::btn("Asset register report", Some(Message::Nav(Route::Report("fixed-asset-register".into()))))],
        iced::widget::scrollable(body.max_width(1150)).style(theme::scroll).into())
}

// ---------------- Audit log ----------------

/// A logged record as "field: value · field: value" (the log stores JSON, sometimes as a string).
pub fn describe(v: &Value) -> String {
    let v = match v { Value::String(s) => serde_json::from_str::<Value>(s).unwrap_or(Value::String(s.clone())), other => other.clone() };
    match &v {
        Value::Object(m) => m.iter().filter(|(_, x)| !x.is_null() && !x.is_object() && !x.is_array()).map(|(k, x)| {
            let val = match x { Value::String(s) => s.clone(), Value::Number(n) if k.contains("total") || k.contains("amount") => n.as_i64().map(ui::money).unwrap_or_else(|| n.to_string()), other => other.to_string() };
            format!("{}: {val}", k.replace('_', " "))
        }).collect::<Vec<_>>().join(" · "),
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn audit_log(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading() };
    let rows = d.as_array().cloned().unwrap_or_default().iter().map(|e| {
        let what = format!("{} {}", e["entity"].as_str().unwrap_or("").replace('_', " "), e["entity_id"].as_i64().map(|i| format!("#{i}")).unwrap_or_default());
        let detail: String = describe(if !e["after"].is_null() { &e["after"] } else { &e["before"] }).chars().take(120).collect();
        let at = e["at"].as_str().unwrap_or("");
        (vec![ui::cell_muted(format!("{} {}", app.d(at.get(..10).unwrap_or(at)), at.get(11..16).unwrap_or(""))), ui::cell(e["user"].as_str().unwrap_or("System").to_string()),
            ui::cell(e["action"].as_str().unwrap_or("").to_lowercase()), ui::cell(what), ui::cell_muted(detail)], None)
    }).collect();
    let cur = app.extras.audit_entity.unwrap_or("Everything");
    let filter = row![text("Show").size(13), pick_list(AUDIT_ENTITIES.iter().map(|e| e.0).collect::<Vec<_>>(), Some(cur), |e| Message::Extras(Msg::AuditEntity(e)))
        .padding([5, 7]).text_size(13).style(theme::pick).width(240)].spacing(10).align_y(Alignment::Center);
    ui::page(ui::h1(if app.skin == crate::theme::Skin::Xero { "History and notes" } else { "Audit log" }), vec![], column![
        filter,
        ui::table(&[ui::col("Date", 14), ui::col("User", 12), ui::col("Event", 10), ui::col("What", 16), ui::col("Details", 48)], rows, "Nothing has been recorded yet."),
        ui::muted("Every change is recorded with who made it and when; the log can't be edited."),
    ].spacing(12).into())
}

// ---------------- Budgets ----------------

/// First month (YYYY-MM) of the financial year shown: the chosen one, else the current one.
fn budget_start(app: &App) -> String {
    if let Some(s) = &app.extras.bud_start { return s.clone(); }
    let end_month: u32 = app.setting("fy_end_month").parse().ok().filter(|m| (1..=12).contains(m)).unwrap_or(12);
    let start_month = end_month % 12 + 1;
    let today = ui::today_iso();
    let (y, m): (i32, u32) = (today[..4].parse().unwrap_or(2026), today[5..7].parse().unwrap_or(1));
    let year = if m >= start_month { y } else { y - 1 };
    format!("{year}-{start_month:02}")
}

/// The 12 months (YYYY-MM) of the year shown.
fn budget_months(app: &App) -> Vec<String> {
    let start = budget_start(app);
    let (y, m): (i32, u32) = (start[..4].parse().unwrap_or(2026), start[5..7].parse().unwrap_or(1));
    (0..12).map(|i| { let t = (m - 1 + i) as i32; format!("{}-{:02}", y + t / 12, t % 12 + 1) }).collect()
}

/// A cell's amount as shown: what was typed, else what's saved.
fn cell_value(app: &App, acct: i64, period: &str) -> String {
    if let Some(v) = app.extras.bud_cells.get(&(acct, period.to_string())) { return v.clone(); }
    app.data.as_ref().and_then(|d| d["budget"]["lines"].as_array().cloned()).unwrap_or_default().iter()
        .find(|l| l["account_id"].as_i64() == Some(acct) && l["period"] == period).and_then(|l| l["amount"].as_i64()).filter(|a| *a != 0).map(ui::plain).unwrap_or_default()
}

/// Planning & Budgeting: income and expense accounts × the 12 months of a financial year.
pub fn budgets(app: &App) -> Element<'_, Message> {
    let k = t();
    let Some(d) = app.data.as_ref().filter(|d| d["list"].is_array()) else { return loading() };
    let list: Vec<Opt> = d["list"].as_array().cloned().unwrap_or_default().iter().map(|b| Opt { id: b["id"].as_i64().unwrap_or(0), label: b["name"].as_str().unwrap_or("").to_string() }).collect();
    let cur = d["budget"]["id"].as_i64().and_then(|id| list.iter().find(|o| o.id == id).cloned());
    let months = budget_months(app);
    let mon = |p: &str| { let m: usize = p[5..7].parse().unwrap_or(1); ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][m - 1].to_string() + " " + &p[2..4] };
    let year_label = format!("{} – {}", mon(&months[0]), mon(&months[11]));
    let mut top = row![
        text("Budget").size(13), pick_list(list.clone(), cur.clone(), |o| Message::Extras(Msg::BudPick(o.id))).placeholder("No budgets yet").padding([5, 7]).text_size(13).style(theme::pick).width(240),
        ui::btn("‹", Some(Message::Extras(Msg::BudYear(-1)))), text(year_label).size(13).font(BOLD), ui::btn("›", Some(Message::Extras(Msg::BudYear(1)))),
        ui::hspace(),
    ].spacing(10).align_y(Alignment::Center);
    top = match &app.extras.bud_new {
        Some(n) => top.push(text_input("New budget name", n).on_input(|v| Message::Extras(Msg::BudNew(Some(v)))).on_submit(Message::Extras(Msg::BudCreate)).padding([5, 7]).size(13).style(theme::input).width(220))
            .push(ui::btn_primary("Create", Some(Message::Extras(Msg::BudCreate)))).push(ui::btn("Cancel", Some(Message::Extras(Msg::BudNew(None))))),
        None => top.push(ui::btn("New budget", Some(Message::Extras(Msg::BudNew(Some(String::new())))))),
    };
    if cur.is_none() {
        return ui::page(ui::h1("Budgets"), vec![], column![top, ui::card(ui::muted("Create a budget to plan income and expenses month by month, then compare with Budget vs. Actuals."))].spacing(12).into());
    }
    // Header row: account, 12 months, total.
    let w_acct = 220.0; let w_cell = 78.0;
    let mut head = row![text("ACCOUNT").size(11).font(BOLD).color(k.muted).width(Length::Fixed(w_acct))].spacing(4);
    for m in &months { head = head.push(text(mon(m).to_uppercase()).size(11).font(BOLD).color(k.muted).width(Length::Fixed(w_cell)).align_x(iced::alignment::Horizontal::Right)); }
    head = head.push(text("TOTAL").size(11).font(BOLD).color(k.muted).width(Length::Fixed(96.0)).align_x(iced::alignment::Horizontal::Right));
    let mut grid = column![container(head).padding([6, 8]).style(theme::header_row)].spacing(0);
    let pl = app.lk.accounts.iter().filter(|a| a["status"] == "ACTIVE" && matches!(a["class"].as_str(), Some("REVENUE" | "EXPENSE"))).cloned().collect::<Vec<_>>();
    let (mut tin, mut tex) = (0i64, 0i64);
    for (section, class) in [("Income", "REVENUE"), ("Expenses", "EXPENSE")] {
        grid = grid.push(container(text(section).size(13).font(BOLD)).padding([8, 8]));
        for a in pl.iter().filter(|a| a["class"] == class) {
            let id = a["id"].as_i64().unwrap_or(0);
            let mut r = row![row![iced::widget::button(text("→").size(11)).style(theme::ghost).padding([0, 4]).on_press(Message::Extras(Msg::BudCopyAcross(id))),
                text(format!("{} {}", a["code"].as_str().unwrap_or(""), a["name"].as_str().unwrap_or(""))).size(12)].spacing(4).align_y(Alignment::Center).width(Length::Fixed(w_acct))].spacing(4).align_y(Alignment::Center);
            let mut total = 0i64;
            for m in &months {
                let v = cell_value(app, id, m);
                total += ui::parse_money(&v).unwrap_or(0);
                let (acct, period) = (id, m.clone());
                r = r.push(text_input("", &v).on_input(move |x| Message::Extras(Msg::BudCell(acct, period.clone(), x))).padding([3, 4]).size(12)
                    .align_x(iced::alignment::Horizontal::Right).style(theme::cell_input).width(Length::Fixed(w_cell)));
            }
            if class == "REVENUE" { tin += total } else { tex += total }
            r = r.push(text(if total == 0 { String::new() } else { ui::money(total) }).size(12).font(BOLD).width(Length::Fixed(96.0)).align_x(iced::alignment::Horizontal::Right));
            grid = grid.push(container(r).padding([1, 8]));
        }
    }
    let summary = row![text(format!("Income {}", ui::money(tin))).size(13), text(format!("Expenses {}", ui::money(tex))).size(13),
        text(format!("Net {}", ui::money(tin - tex))).size(13).font(BOLD).color(if tin >= tex { k.ok } else { k.bad }), ui::hspace(),
        ui::btn("Budget vs. Actuals", Some(Message::Nav(Route::Report("budget-variance".into())))),
        ui::btn_primary("Save", if app.extras.busy { None } else { Some(Message::Extras(Msg::BudSave)) })].spacing(16).align_y(Alignment::Center);
    ui::page(ui::h1("Budgets"), vec![], column![
        top,
        container(iced::widget::scrollable(grid).direction(iced::widget::scrollable::Direction::Both { vertical: Default::default(), horizontal: Default::default() }).style(theme::scroll).height(Length::Fill))
            .style(theme::card).height(Length::Fill).clip(true),
        summary,
        ui::muted("Tip: → copies a row's first month across the year."),
    ].spacing(12).into())
}

// ---------------- Expense claims ----------------

/// Expense claims (receipts and mileage); `mileage_only` for the Mileage screen.
pub fn claims(app: &App, mileage_only: bool) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return loading() };
    let all: Vec<Value> = d.as_array().cloned().unwrap_or_default().into_iter().filter(|r| !mileage_only || r["kind"] == "MILEAGE").collect();
    let in_tab_of = |tab: &str, r: &Value| match tab { "AUTHORISED" => matches!(r["status"].as_str(), Some("AUTHORISED")) && r["bill_status"] != "PAID", "PAID" => r["status"] == "PAID" || r["bill_status"] == "PAID", s => r["status"] == s };
    // Open on the first tab with claims in it, unless one was chosen.
    let tab = app.extras.cl_tab.unwrap_or_else(|| ["SUBMITTED", "DRAFT", "AUTHORISED", "DECLINED", "PAID"].into_iter().find(|t| all.iter().any(|r| in_tab_of(t, r))).unwrap_or("DRAFT"));
    let in_tab = |r: &Value| match tab { "AUTHORISED" => matches!(r["status"].as_str(), Some("AUTHORISED")) && r["bill_status"] != "PAID", "PAID" => r["status"] == "PAID" || r["bill_status"] == "PAID", s => r["status"] == s };
    let tab_items: Vec<(&'static str, &'static str)> = vec![("DRAFT", "Draft"), ("SUBMITTED", "Awaiting approval"), ("AUTHORISED", "Awaiting payment"), ("PAID", "Paid"), ("DECLINED", "Declined")];
    let unit = if app.country() == "US" { "mi" } else { "km" };
    let mut body = column![tabs(&tab_items, tab, |c| Message::Extras(Msg::ClTab(c)))].spacing(12);
    if let Some((mileage, f)) = &app.extras.cl_new {
        let v = |key: &'static str| f.get(key).cloned().unwrap_or_default();
        let expense_accts = app.lk.account_opts(|a| a["class"] == "EXPENSE");
        let form: Element<Message> = if *mileage {
            column![ui::h2("New mileage claim"),
                row![input("DATE", &v("date"), 140.0, |x| Message::Extras(Msg::ClField("date", x))), input("PURPOSE OF TRIP", &v("purpose"), 320.0, |x| Message::Extras(Msg::ClField("purpose", x))),
                    input(if unit == "mi" { "MILES" } else { "KILOMETRES" }, &v("distance"), 120.0, |x| Message::Extras(Msg::ClField("distance", x))),
                    input(if unit == "mi" { "RATE PER MILE" } else { "RATE PER KM" }, &v("rate"), 120.0, |x| Message::Extras(Msg::ClField("rate", x)))].spacing(14)].spacing(12).into()
        } else {
            column![ui::h2("New expense claim"),
                row![input("DATE", &v("date"), 140.0, |x| Message::Extras(Msg::ClField("date", x))), input("SPENT AT", &v("supplier"), 240.0, |x| Message::Extras(Msg::ClField("supplier", x))),
                    input("DESCRIPTION", &v("description"), 300.0, |x| Message::Extras(Msg::ClField("description", x)))].spacing(14),
                row![column![lbl("ACCOUNT"), pick_list(expense_accts, app.extras.cl_account.clone(), |o| Message::Extras(Msg::ClAccount(o))).placeholder("What was it for?").padding([5, 7]).text_size(13).style(theme::pick).width(280)].spacing(3),
                    input("AMOUNT (INCL. TAX)", &v("amount"), 140.0, |x| Message::Extras(Msg::ClField("amount", x))),
                    column![lbl("TAX"), pick_list(app.lk.tax_opts(), app.extras.cl_tax.clone(), |o| Message::Extras(Msg::ClTax(o))).placeholder("Tax rate").padding([5, 7]).text_size(13).style(theme::pick).width(200)].spacing(3)].spacing(14)].spacing(12).into()
        };
        body = body.push(ui::card(column![form, row![ui::hspace(), ui::btn("Save draft", if app.extras.busy { None } else { Some(Message::Extras(Msg::ClSave(false))) }),
            ui::btn_primary("Submit for approval", if app.extras.busy { None } else { Some(Message::Extras(Msg::ClSave(true))) }), ui::btn("Cancel", Some(Message::Extras(Msg::ClCancel)))].spacing(8)].spacing(12)));
    }
    let rows = all.iter().filter(|r| in_tab(r)).map(|r| {
        let id = r["id"].as_i64().unwrap_or(0);
        let what = if r["kind"] == "MILEAGE" { format!("{} · {:.1} {unit}", r["reference"].as_str().unwrap_or("Trip"), r["distance_e4"].as_i64().unwrap_or(0) as f64 / 10_000.0) }
            else { format!("{}{}", r["supplier"].as_str().filter(|s| !s.is_empty()).map(|s| format!("{s} — ")).unwrap_or_default(), r["reference"].as_str().unwrap_or("")) };
        let actions: Element<Message> = match r["status"].as_str().unwrap_or("") {
            "DRAFT" => row![ui::link("Submit", Message::Extras(Msg::ClStatus(id, "SUBMITTED"))), ui::link("Approve", Message::Extras(Msg::ClStatus(id, "AUTHORISED")))].spacing(12).into(),
            "SUBMITTED" => row![ui::link("Approve", Message::Extras(Msg::ClStatus(id, "AUTHORISED"))), ui::link("Decline", Message::Extras(Msg::ClStatus(id, "DECLINED")))].spacing(12).into(),
            "DECLINED" => ui::link("Resubmit", Message::Extras(Msg::ClStatus(id, "SUBMITTED"))),
            _ if r["document_id"].as_i64().is_some() => ui::link("View bill", Message::Nav(Route::Doc { kind: "ACCPAY".into(), id: r["document_id"].as_i64() })),
            _ => ui::cell_muted(""),
        };
        (vec![ui::cell_muted(app.d(r["date"].as_str().unwrap_or(""))), ui::cell(r["claimant"].as_str().unwrap_or("").to_string()), ui::cell(if r["kind"] == "MILEAGE" { "Mileage" } else { "Receipt" }),
            ui::cell(what), ui::cell_money(r["total"].as_i64().unwrap_or(0)), actions], None)
    }).collect();
    body = body.push(ui::table(&[ui::col("Date", 11), ui::col("Claimant", 16), ui::col("Type", 9), ui::col("Description", 34), ui::coln("Amount", 12), ui::col("", 18)], rows, "No claims here."));
    body = body.push(text("Approving a claim creates a bill to repay the person who paid; pay it like any other bill.").size(12).color(k.muted));
    let mut actions = vec![ui::btn_primary(if mileage_only { "New trip" } else { "New receipt" }, Some(Message::Extras(Msg::ClNew(mileage_only))))];
    if !mileage_only { actions.push(ui::btn("New mileage claim", Some(Message::Extras(Msg::ClNew(true))))); }
    ui::page(ui::h1(if mileage_only { "Mileage" } else { "Expense claims" }), actions, iced::widget::scrollable(body.max_width(1150)).style(theme::scroll).into())
}

// ---------------- Bank rules ----------------

pub const RULE_TYPES: [(&str, &str); 3] = [("SPEND", "Money out (spend)"), ("RECEIVE", "Money in (receive)"), ("TRANSFER", "Transfer")];
pub const RULE_FIELDS: [(&str, &str); 5] = [("any", "Any text field"), ("payee", "Payee"), ("description", "Description"), ("reference", "Reference"), ("amount", "Amount")];
pub const RULE_OPS: [(&str, &str); 7] = [("contains", "contains"), ("not_contains", "doesn't contain"), ("equals", "is"), ("starts_with", "starts with"), ("gt", "is more than"), ("lt", "is less than"), ("gte", "is at least")];

fn label_of(list: &[(&'static str, &'static str)], code: &str) -> &'static str { list.iter().find(|x| x.0 == code).map(|x| x.1).unwrap_or("") }
fn code_of(list: &'static [(&'static str, &'static str)], label: &str) -> &'static str { list.iter().find(|x| x.1 == label).map(|x| x.0).unwrap_or("") }

pub fn bank_rules(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading() };
    let mut body = column![].spacing(12);
    if let Some(f) = &app.extras.br_new {
        let v = |key: &'static str| f.get(key).cloned().unwrap_or_default();
        let ty = v("type");
        let pick_str = |label: &'static str, list: &'static [(&'static str, &'static str)], cur: String, key: &'static str, w: f32| -> Element<'_, Message> {
            column![lbl(label), pick_list(list.iter().map(|x| x.1).collect::<Vec<_>>(), Some(label_of(list, &cur)).filter(|s| !s.is_empty()),
                move |l: &'static str| Message::Extras(Msg::BrField(key, code_of(list, l).to_string()))).padding([5, 7]).text_size(13).style(theme::pick).width(Length::Fixed(w))].spacing(3).into()
        };
        let accounts = if ty == "TRANSFER" { app.lk.bank_opts() } else { app.lk.account_opts(|a| a["type"] != "BANK") };
        body = body.push(ui::card(column![
            ui::h2("New bank rule"),
            row![input("RULE NAME", &v("name"), 260.0, |x| Message::Extras(Msg::BrField("name", x))), pick_str("WHEN THERE'S", &RULE_TYPES, ty.clone(), "type", 200.0),
                column![lbl("IN BANK ACCOUNT"), pick_list(app.lk.bank_opts(), app.extras.br_bank.clone(), |o| Message::Extras(Msg::BrBank(o))).placeholder("Any bank account").padding([5, 7]).text_size(13).style(theme::pick).width(220)].spacing(3)].spacing(14),
            row![pick_str("AND THE", &RULE_FIELDS, v("field"), "field", 170.0), pick_str("", &RULE_OPS, v("op"), "op", 160.0), input("VALUE", &v("value"), 240.0, |x| Message::Extras(Msg::BrField("value", x)))].spacing(10).align_y(Alignment::End),
            row![column![lbl(if ty == "TRANSFER" { "TRANSFER TO" } else { "CODE IT TO" }), pick_list(accounts, app.extras.br_account.clone(), |o| Message::Extras(Msg::BrAccount(o))).placeholder("Account").padding([5, 7]).text_size(13).style(theme::pick).width(280)].spacing(3),
                column![lbl("CONTACT (OPTIONAL)"), pick_list(app.lk.contact_opts(ty != "SPEND"), app.extras.br_contact.clone(), |o| Message::Extras(Msg::BrContact(o))).placeholder("Contact").padding([5, 7]).text_size(13).style(theme::pick).width(240)].spacing(3)].spacing(14),
            row![ui::hspace(), ui::btn_primary("Save rule", Some(Message::Extras(Msg::BrSave))), ui::btn("Cancel", Some(Message::Extras(Msg::BrNew(false))))].spacing(8),
        ].spacing(12)));
    }
    let rows = d.as_array().cloned().unwrap_or_default().iter().map(|r| {
        let id = r["id"].as_i64().unwrap_or(0);
        let conds = r["conditions"].as_array().cloned().unwrap_or_default().iter().map(|c| format!("{} {} \"{}\"", label_of(&RULE_FIELDS, c["field"].as_str().unwrap_or("")).to_lowercase(),
            label_of(&RULE_OPS, c["op"].as_str().unwrap_or("contains")), c["value"].as_str().unwrap_or(""))).collect::<Vec<_>>().join(if r["match_mode"] == "ANY" { " or " } else { " and " });
        let to = if r["type"] == "TRANSFER" { app.lk.accounts.iter().find(|a| a["id"] == r["transfer_account_id"]).and_then(|a| a["name"].as_str()).map(|n| format!("Transfer to {n}")).unwrap_or_default() }
            else { r["percent_lines"].as_array().into_iter().chain(r["fixed_lines"].as_array()).flatten().filter_map(|l| app.lk.accounts.iter().find(|a| a["id"] == l["account_id"]).and_then(|a| a["name"].as_str())).collect::<Vec<_>>().join(", ") };
        (vec![ui::cell(r["name"].as_str().unwrap_or("").to_string()), ui::cell_muted(label_of(&RULE_TYPES, r["type"].as_str().unwrap_or(""))), ui::cell(conds), ui::cell(to),
            ui::link("Delete", Message::Extras(Msg::BrDelete(id)))], None)
    }).collect();
    body = body.push(ui::table(&[ui::col("Rule", 20), ui::col("Applies to", 14), ui::col("When", 34), ui::col("Then", 22), ui::col("", 8)], rows, "No bank rules yet."));
    body = body.push(ui::muted("Rules fill in matching bank statement lines for you when you review them in Banking."));
    ui::page(ui::h1("Bank rules"), vec![ui::btn_primary("New rule", Some(Message::Extras(Msg::BrNew(true))))], body.max_width(1150).into())
}

// ---------------- Price rules ----------------

/// Price rules under the price levels: quantity breaks, item- and date-specific prices.
pub fn price_rules_section(app: &App) -> Element<'_, Message> {
    let x = &app.extras;
    let mut col = column![row![ui::h2("Price rules"), iced::widget::Space::new().width(Length::Fill), ui::btn("New price rule", Some(Message::Extras(Msg::PrNew(true))))].align_y(Alignment::Center)].spacing(10);
    if let Some(f) = &x.pr_new {
        let items: Vec<Opt> = app.lk.items.iter().filter(|i| i["is_sold"] == 1).map(|i| Opt { id: i["id"].as_i64().unwrap_or(0), label: i["name"].as_str().unwrap_or("").to_string() }).collect();
        let levels: Vec<Opt> = app.lk.price_levels.iter().map(|p| Opt { id: p["id"].as_i64().unwrap_or(0), label: p["name"].as_str().unwrap_or("").to_string() }).collect();
        col = col.push(ui::card(column![
            row![input("RULE NAME", &f[0], 220.0, |v| Message::Extras(Msg::PrField(0, v))),
                column![lbl("ITEM"), pick_list(items, x.pr_item.clone(), |o| Message::Extras(Msg::PrItem(o))).placeholder("Every item").padding([5, 7]).text_size(13).style(theme::pick).width(220)].spacing(3),
                column![lbl("CUSTOMERS AT PRICE LEVEL"), pick_list(levels, x.pr_level.clone(), |o| Message::Extras(Msg::PrLevel(o))).placeholder("Every customer").padding([5, 7]).text_size(13).style(theme::pick).width(200)].spacing(3)].spacing(12),
            row![input("FROM QUANTITY", &f[1], 110.0, |v| Message::Extras(Msg::PrField(1, v))), input("CHANGE PRICE BY %", &f[2], 130.0, |v| Message::Extras(Msg::PrField(2, v))),
                input("START DATE", &f[3], 130.0, |v| Message::Extras(Msg::PrField(3, v))), input("END DATE", &f[4], 130.0, |v| Message::Extras(Msg::PrField(4, v))),
                iced::widget::Space::new().width(Length::Fill), ui::btn("Cancel", Some(Message::Extras(Msg::PrNew(false)))), ui::btn_primary("Save rule", Some(Message::Extras(Msg::PrSave)))].spacing(12).align_y(Alignment::End),
        ].spacing(12)));
    }
    let rows = app.lk.price_rules.iter().map(|r| {
        let id = r["id"].as_i64().unwrap_or(0);
        let pct = r["percent_bp"].as_i64().unwrap_or(0) as f64 / 100.0;
        let qty = r["min_qty_e4"].as_i64().unwrap_or(0) as f64 / 10_000.0;
        let dates = match (r["start_date"].as_str(), r["end_date"].as_str()) { (None, None) => "Always".to_string(), (s, e) => format!("{} – {}", s.map(|d| app.d(d)).unwrap_or_default(), e.map(|d| app.d(d)).unwrap_or_default()) };
        (vec![ui::cell(r["name"].as_str().unwrap_or("").to_string()), ui::cell(r["item_name"].as_str().unwrap_or("Every item").to_string()),
            ui::cell_muted(r["level_name"].as_str().unwrap_or("Every customer").to_string()), ui::cell(if qty > 0.0 { format!("{qty} or more") } else { "Any".into() }),
            ui::cell(format!("{pct:+}%")), ui::cell_muted(dates), ui::link("Remove", Message::Extras(Msg::PrDelete(id)))], None)
    }).collect();
    col = col.push(ui::table(&[ui::col("Rule", 16), ui::col("Item", 18), ui::col("Customers", 16), ui::col("Quantity", 10), ui::col("Change", 9), ui::col("Dates", 20), ui::col("", 9)], rows, "No price rules yet."));
    col.push(ui::muted("The lowest matching price wins. Rules apply after the customer's price level, when an item goes on a sale or its quantity changes.")).into()
}

// ---------------- Retainage ----------------

/// Retainage held back on customers' invoices, and invoicing it when the work is done.
pub fn retainage(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading() };
    let term = if app.skin == crate::theme::Skin::Xero { "Retentions" } else { "Retainage" };
    let rows = d.as_array().cloned().unwrap_or_default().iter().map(|r| {
        let id = r["contact_id"].as_i64().unwrap_or(0);
        (vec![ui::cell(r["name"].as_str().unwrap_or("").to_string()), ui::cell_money(r["held"].as_i64().unwrap_or(0)),
            ui::link(if term == "Retentions" { "Invoice the retention" } else { "Invoice retainage" }, Message::Extras(Msg::RtRelease(id)))], Some(Message::Nav(Route::Contact(id))))
    }).collect();
    let body = column![
        ui::table(&[ui::col("Customer", 40), ui::coln("Held", 20), ui::col("", 20)], rows, "Nothing held back yet."),
        ui::muted("To hold some of an invoice back, use Hold back on the invoice: it adds a line taking that share out of what the customer owes now, kept in Retainage Receivable until you invoice it here."),
    ].spacing(14);
    ui::page(ui::h1(term), vec![], body.max_width(1000).into())
}

// ---------------- Approval rules ----------------

const AP_TYPES: [(&str, &str); 5] = [("ACCPAY", "Bills"), ("PO", "Purchase orders"), ("ACCREC", "Invoices"), ("ACCPAYCREDIT", "Supplier credits"), ("ACCRECCREDIT", "Credit notes")];
const AP_ROLES: [(&str, &str); 5] = [("ADVISER", "Adviser"), ("STANDARD", "Standard"), ("CASHBOOK", "Cashbook"), ("PAYROLL_ADMIN", "Payroll admin"), ("INVOICE_ONLY", "Invoice only")];

/// Who has to approve which documents.
pub fn approval_rules(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading() };
    let x = &app.extras;
    let mut body = column![].spacing(14);
    if let Some((t, min, role)) = &x.ap_new {
        body = body.push(ui::card(row![
            column![lbl("DOCUMENTS"), pick_list(AP_TYPES.iter().map(|k| k.1).collect::<Vec<_>>(), Some(label_of(&AP_TYPES, t)), |l: &'static str| Message::Extras(Msg::ApType(code_of(&AP_TYPES, l)))).padding([5, 7]).text_size(13).style(theme::pick).width(180)].spacing(3),
            input("OF THIS AMOUNT OR MORE", min, 160.0, |v| Message::Extras(Msg::ApMin(v))),
            column![lbl("MUST BE APPROVED BY"), pick_list(AP_ROLES.iter().map(|k| k.1).collect::<Vec<_>>(), Some(label_of(&AP_ROLES, role)), |l: &'static str| Message::Extras(Msg::ApRole(code_of(&AP_ROLES, l)))).padding([5, 7]).text_size(13).style(theme::pick).width(180)].spacing(3),
            iced::widget::Space::new().width(Length::Fill), ui::btn("Cancel", Some(Message::Extras(Msg::ApNew(false)))), ui::btn_primary("Save rule", Some(Message::Extras(Msg::ApSave))),
        ].spacing(12).align_y(Alignment::End)));
    }
    let rows = d.as_array().cloned().unwrap_or_default().iter().map(|r| {
        let id = r["id"].as_i64().unwrap_or(0);
        (vec![ui::cell(label_of(&AP_TYPES, r["doc_type"].as_str().unwrap_or(""))), ui::cell_money(r["min_total"].as_i64().unwrap_or(0)),
            ui::cell(label_of(&AP_ROLES, r["approver_role"].as_str().unwrap_or(""))), ui::link("Remove", Message::Extras(Msg::ApDelete(id)))], None)
    }).collect();
    body = body.push(ui::table(&[ui::col("Documents", 24), ui::coln("Of this amount or more", 20), ui::col("Approved by", 24), ui::col("", 10)], rows, "No approval rules: anyone who can create a document can approve it."));
    body = body.push(ui::muted("Advisers can approve anything. When someone else approves a document a rule covers, it's saved as Awaiting approval for the right person."));
    let title = if crate::vocab::acumatica() { "Approval Maps" } else if app.desktop() { "Approval Workflows" } else { "Approval rules" };
    ui::page(ui::h1(title), vec![ui::btn_primary("New rule", Some(Message::Extras(Msg::ApNew(true))))], body.max_width(1000).into())
}

// ---------------- Item receipts ----------------

/// Receive stock before its bill arrives, then enter the bill for the receipt.
pub fn item_receipts(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading() };
    let x = &app.extras;
    let items: Vec<Opt> = app.lk.items.iter().filter(|i| i["is_tracked"] == 1).map(|i| Opt { id: i["id"].as_i64().unwrap_or(0), label: i["name"].as_str().unwrap_or("").to_string() }).collect();
    let date_shown = if x.ir_date.is_empty() { app.d(&ui::today_iso()) } else { x.ir_date.clone() };
    let mut head = row![
        column![lbl(if app.skin == crate::theme::Skin::Xero { "SUPPLIER" } else { "VENDOR" }), pick_list(app.lk.contact_opts(false), x.ir_vendor.clone(), |o| Message::Extras(Msg::IrVendor(o))).placeholder("Choose").padding([5, 7]).text_size(13).style(theme::pick).width(260)].spacing(3),
        input("DATE", &date_shown, 120.0, |v| Message::Extras(Msg::IrDate(v))), input("REFERENCE / PACKING SLIP", &x.ir_ref, 200.0, |v| Message::Extras(Msg::IrRef(v))),
    ].spacing(12).align_y(Alignment::End);
    if !app.lk.locations.is_empty() {
        let cur = x.ir_loc.clone().unwrap_or(Opt { id: 0, label: ledger_core::locations::MAIN.to_string() });
        head = head.push(column![lbl("LOCATION"), pick_list(app.lk.location_opts(), Some(cur), |o| Message::Extras(Msg::IrLoc(o))).padding([5, 7]).text_size(13).style(theme::pick).width(180)].spacing(3));
    }
    let mut lines = column![row![text("ITEM").size(11).font(BOLD).width(Length::FillPortion(4)), text("QTY").size(11).font(BOLD).width(Length::FillPortion(1)),
        text("UNIT COST").size(11).font(BOLD).width(Length::FillPortion(1)), text("LOT / SERIALS").size(11).font(BOLD).width(Length::FillPortion(3))].spacing(8)].spacing(6);
    let rows_n = x.ir_lines.len().max(1);
    for i in 0..rows_n {
        let l = x.ir_lines.get(i).cloned().unwrap_or_default();
        let lot_mode = l.0.as_ref().and_then(|o| app.lk.item(o.id)).and_then(|it| it["lot_tracking"].as_str()).unwrap_or("NONE").to_string();
        let lot_cell: Element<Message> = if lot_mode == "NONE" { text("-").size(13).color(t().faint).into() } else {
            text_input(if lot_mode == "SERIAL" { "Serial numbers, comma-separated" } else { "Lot number" }, &l.3).on_input(move |v| Message::Extras(Msg::IrLot(i, v))).padding([5, 7]).size(13).style(theme::input).into() };
        lines = lines.push(row![
            container(pick_list(items.clone(), l.0.clone(), move |o| Message::Extras(Msg::IrItem(i, o))).placeholder("Choose an item").padding([5, 7]).text_size(13).style(theme::pick).width(Length::Fill)).width(Length::FillPortion(4)),
            container(text_input("1", &l.1).on_input(move |v| Message::Extras(Msg::IrQty(i, v))).padding([5, 7]).size(13).style(theme::input)).width(Length::FillPortion(1)),
            container(text_input("0.00", &l.2).on_input(move |v| Message::Extras(Msg::IrCost(i, v))).padding([5, 7]).size(13).style(theme::input)).width(Length::FillPortion(1)),
            container(lot_cell).width(Length::FillPortion(3)),
        ].spacing(8).align_y(Alignment::Center));
    }
    let form = ui::card(column![ui::h2("Receive items"), head, lines,
        row![ui::btn("Add line", Some(Message::Extras(Msg::IrAddLine))), iced::widget::Space::new().width(Length::Fill), ui::btn_primary("Receive", Some(Message::Extras(Msg::IrReceive)))].spacing(8),
        ui::muted("The stock goes into inventory now; its cost waits in Goods Received Not Invoiced until you enter the bill."),
    ].spacing(12));
    let qty = |q: i64| { let s = format!("{:.4}", q as f64 / 10_000.0); s.trim_end_matches('0').trim_end_matches('.').to_string() };
    let rows = d.as_array().cloned().unwrap_or_default().iter().map(|r| {
        let id = r["id"].as_i64().unwrap_or(0);
        let state = r["state"].as_str().unwrap_or("").to_string();
        let what = r["lines"].as_array().cloned().unwrap_or_default().iter().map(|l| format!("{} x {}", l["code"].as_str().unwrap_or(""), qty(l["qty_e4"].as_i64().unwrap_or(0)))).collect::<Vec<_>>().join(", ");
        let action: Element<Message> = match state.as_str() {
            "Not billed" => row![ui::link("Enter bill", Message::Extras(Msg::IrBill(id))), ui::link("Void", Message::Extras(Msg::IrVoid(id)))].spacing(10).into(),
            "Billed" => ui::link("Open bill", Message::Nav(Route::Doc { kind: "ACCPAY".into(), id: r["bill_id"].as_i64() })),
            _ => ui::cell(""),
        };
        let color = crate::lists::status_color(match state.as_str() { "Billed" => "Closed", "Voided" => "Voided", _ => "Open" });
        (vec![ui::cell(app.d(r["date"].as_str().unwrap_or(""))), ui::cell(r["contact_name"].as_str().unwrap_or("").to_string()), ui::cell_muted(r["reference"].as_str().unwrap_or("").to_string()),
            ui::cell_muted(what), ui::cell_money(r["total"].as_i64().unwrap_or(0)), ui::chip(&state, color), action], None)
    }).collect();
    let body = column![form, ui::table(&[ui::col("Date", 10), ui::col("Vendor", 16), ui::col("Reference", 12), ui::col("Items", 24), ui::coln("Cost", 10), ui::col("Status", 10), ui::col("", 14)], rows, "No item receipts yet.")].spacing(14);
    ui::page(ui::h1(if app.desktop() { "Receive Items" } else if crate::vocab::acumatica() { "Purchase Receipts" } else { "Item receipts" }), vec![], body.into())
}

// ---------------- Consolidation ----------------

/// Combine several companies' statements: tick the companies, choose dates and intercompany accounts.
pub fn consolidation(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading() };
    let x = &app.extras;
    let k = t();
    let mut pick = column![lbl("COMPANIES")].spacing(4);
    for o in d.as_array().cloned().unwrap_or_default() {
        let file = o["file"].as_str().unwrap_or("").to_string();
        let f2 = file.clone();
        pick = pick.push(iced::widget::checkbox(x.co_files.contains(&file)).label(format!("{}  ({file})", o["name"].as_str().unwrap_or(""))).on_toggle(move |_| Message::Extras(Msg::CoFile(f2.clone()))).text_size(13));
    }
    let today = ui::today_iso();
    let from_ph = app.d(&format!("{}-01-01", &today[..4]));
    let to_ph = app.d(&today);
    let form = ui::card(column![
        pick,
        row![
            column![lbl("FROM"), text_input(&from_ph, &x.co_from).on_input(|v| Message::Extras(Msg::CoFrom(v))).padding([5, 7]).size(13).style(theme::input).width(130)].spacing(3),
            column![lbl("TO"), text_input(&to_ph, &x.co_to).on_input(|v| Message::Extras(Msg::CoTo(v))).padding([5, 7]).size(13).style(theme::input).width(130)].spacing(3),
            input("INTERCOMPANY ACCOUNT CODES TO ELIMINATE", &x.co_elim, 320.0, |v| Message::Extras(Msg::CoElim(v))),
            iced::widget::Space::new().width(Length::Fill), ui::btn_primary("Combine", Some(Message::Extras(Msg::CoRun))),
        ].spacing(12).align_y(Alignment::End),
    ].spacing(12));
    let mut body = column![form].spacing(14);
    if let Some(r) = &x.co_result {
        let cols: Vec<String> = r["columns"].as_array().cloned().unwrap_or_default().iter().map(|c| c.as_str().unwrap_or("").to_string()).collect();
        let w = cols.len();
        let cell = |t: String, head: bool, bold: bool, right: bool, portion: u16| -> Element<'_, Message> {
            let tx = text(if head { t.to_uppercase() } else { t }).size(if head { 11 } else { 13 }).font(if head || bold { BOLD } else { iced::Font::DEFAULT }).color(if head { k.muted } else { k.text });
            container(if right { tx.align_x(iced::alignment::Horizontal::Right).width(Length::Fill) } else { tx.width(Length::Fill) }).width(Length::FillPortion(portion)).into()
        };
        let mut head = row![cell(String::new(), true, false, false, 26)].spacing(8);
        for c in &cols { head = head.push(cell(c.clone(), true, false, true, 12)); }
        let mut grid = column![container(head.padding([8, 12])).style(theme::header_row).width(Length::Fill)];
        for sec in r["sections"].as_array().cloned().unwrap_or_default() {
            if let Some(t) = sec["title"].as_str().filter(|t| !t.is_empty()) { grid = grid.push(container(text(t.to_string()).size(14).font(BOLD)).padding([8, 12])); }
            for row_v in sec["rows"].as_array().cloned().unwrap_or_default() {
                let mut line = row![cell(row_v["label"].as_str().unwrap_or("").to_string(), false, false, false, 26)].spacing(8);
                for n in 0..w { line = line.push(cell(ui::money(row_v["values"][n].as_i64().unwrap_or(0)), false, false, true, 12)); }
                grid = grid.push(container(line).padding([4, 12]));
            }
            let tot = &sec["total"];
            let mut line = row![cell(tot["label"].as_str().unwrap_or("").to_string(), false, true, false, 26)].spacing(8);
            for n in 0..w { line = line.push(cell(ui::money(tot["values"][n].as_i64().unwrap_or(0)), false, true, true, 12)); }
            grid = grid.push(column![ui::hr(), container(line).padding([6, 12])]);
        }
        body = body.push(container(grid).style(theme::card).width(Length::Fill));
        for n in r["notes"].as_array().cloned().unwrap_or_default() { body = body.push(ui::muted(n.as_str().unwrap_or("").to_string())); }
    }
    ui::page(ui::h1(if app.desktop() { "Combine Reports from Multiple Companies" } else { "Consolidated reports" }), vec![], body.into())
}

// ---------------- Landed costs ----------------

/// Spread a freight, duty or insurance bill over the stock it brought in.
pub fn landed_costs(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading() };
    let x = &app.extras;
    let stock_ids: Vec<i64> = d["stock_bills"].as_array().cloned().unwrap_or_default().iter().filter_map(|v| v.as_i64()).collect();
    let used: Vec<i64> = d["applied"].as_array().cloned().unwrap_or_default().iter().filter_map(|l| l["source_bill_id"].as_i64()).collect();
    let bills = d["bills"].as_array().cloned().unwrap_or_default();
    let label = |b: &Value| format!("{} — {} — {}", b["number"].as_str().unwrap_or(""), b["contact_name"].as_str().unwrap_or(""), ui::money(b["total"].as_i64().unwrap_or(0)));
    let cost_bills: Vec<Opt> = bills.iter().filter(|b| b["id"].as_i64().is_some_and(|id| !stock_ids.contains(&id) && !used.contains(&id)))
        .map(|b| Opt { id: b["id"].as_i64().unwrap_or(0), label: label(b) }).collect();
    let mut receipts = column![lbl("STOCK IT BELONGS TO")].spacing(4);
    let stock_bills: Vec<&Value> = bills.iter().filter(|b| b["id"].as_i64().is_some_and(|id| stock_ids.contains(&id))).collect();
    if stock_bills.is_empty() { receipts = receipts.push(ui::muted("No bills with stock items yet.")); }
    for b in stock_bills {
        let id = b["id"].as_i64().unwrap_or(0);
        receipts = receipts.push(iced::widget::checkbox(x.lc_receipts.contains(&id)).label(label(b)).on_toggle(move |_| Message::Extras(Msg::LcReceipt(id))).text_size(13));
    }
    let form = ui::card(column![
        ui::h2("Add a landed cost"),
        column![lbl("FREIGHT, DUTY OR INSURANCE BILL"), pick_list(cost_bills, x.lc_bill.clone(), |o| Message::Extras(Msg::LcBill(o))).placeholder("Choose a bill").padding([5, 7]).text_size(13).style(theme::pick).width(420)].spacing(3),
        receipts,
        row![iced::widget::checkbox(x.lc_qty).label("Spread by quantity (otherwise by value)").on_toggle(|v| Message::Extras(Msg::LcByQty(v))).text_size(13),
            iced::widget::Space::new().width(Length::Fill), ui::btn_primary("Add to stock cost", Some(Message::Extras(Msg::LcApply)))].align_y(Alignment::Center),
        ui::muted("The bill's cost moves from its expense account into inventory, raising the cost of the items it's spread over."),
    ].spacing(10));
    let rows = d["applied"].as_array().cloned().unwrap_or_default().iter().map(|l| {
        let spread = l["lines"].as_array().cloned().unwrap_or_default().iter().map(|x| format!("{} {}", x["code"].as_str().unwrap_or(""), ui::money(x["amount"].as_i64().unwrap_or(0)))).collect::<Vec<_>>().join(", ");
        (vec![ui::cell(app.d(l["date"].as_str().unwrap_or(""))), ui::cell(format!("{} — {}", l["number"].as_str().unwrap_or(""), l["contact_name"].as_str().unwrap_or(""))),
            ui::cell_muted(if l["method"] == "QTY" { "By quantity" } else { "By value" }), ui::cell_money(l["amount"].as_i64().unwrap_or(0)), ui::cell_muted(spread)], None)
    }).collect();
    let body = column![form, ui::table(&[ui::col("Date", 12), ui::col("Bill", 24), ui::col("Spread", 12), ui::coln("Amount", 12), ui::col("Added to", 40)], rows, "No landed costs yet.")].spacing(14);
    ui::page(ui::h1(if app.desktop() { "Landed Cost" } else { "Landed costs" }), vec![], body.max_width(1150).into())
}

// ---------------- Custom fields ----------------

const CF_ENTITIES: [(&str, &str); 3] = [("contact", "Customers & vendors"), ("item", "Products & services"), ("employee", "Employees")];
const CF_KINDS: [(&str, &str); 4] = [("TEXT", "Text"), ("NUMBER", "Number"), ("DATE", "Date"), ("YESNO", "Yes / No")];

/// Define custom fields for each kind of record.
pub fn custom_fields(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading() };
    let x = &app.extras;
    let fields = d.as_array().cloned().unwrap_or_default();
    let mut body = column![].spacing(14);
    for (entity, title) in CF_ENTITIES {
        let mut col = column![row![ui::h2(title), iced::widget::Space::new().width(Length::Fill), ui::btn("Add field", Some(Message::Extras(Msg::CfNew(Some(entity)))))].align_y(Alignment::Center)].spacing(6);
        let mine: Vec<&Value> = fields.iter().filter(|f| f["entity"] == entity).collect();
        if mine.is_empty() { col = col.push(ui::muted("No custom fields yet.")); }
        for f in mine {
            let id = f["id"].as_i64().unwrap_or(0);
            col = col.push(row![text(f["name"].as_str().unwrap_or("").to_string()).size(14).width(Length::Fill), text(label_of(&CF_KINDS, f["kind"].as_str().unwrap_or(""))).size(13).color(t().muted).width(120),
                ui::link("Remove", Message::Extras(Msg::CfDelete(id)))].spacing(10).align_y(Alignment::Center));
        }
        if let Some((e, name, kind)) = &x.cf_new {
            if *e == entity {
                col = col.push(row![input("FIELD NAME", name, 260.0, |v| Message::Extras(Msg::CfName(v))),
                    column![lbl("TYPE"), pick_list(CF_KINDS.iter().map(|k| k.1).collect::<Vec<_>>(), Some(label_of(&CF_KINDS, kind)), |l: &'static str| Message::Extras(Msg::CfKind(code_of(&CF_KINDS, l))))
                        .padding([5, 7]).text_size(13).style(theme::pick).width(140)].spacing(3),
                    iced::widget::Space::new().width(Length::Fill), ui::btn("Cancel", Some(Message::Extras(Msg::CfNew(None)))), ui::btn_primary("Save field", Some(Message::Extras(Msg::CfSave)))].spacing(10).align_y(Alignment::End));
            }
        }
        body = body.push(ui::card(col));
    }
    body = body.push(ui::muted("Custom fields show on each customer, vendor and item when you edit it."));
    ui::page(ui::h1(if crate::vocab::acumatica() { "User-Defined Fields" } else { "Custom fields" }), vec![], body.max_width(900).into())
}

/// A record's custom fields to fill in (shown on Edit Customer/Vendor and Edit Item).
pub fn custom_values_card<'a>(app: &'a App, entity: &'static str, id: i64, fields: &'a [Value]) -> Element<'a, Message> {
    if fields.is_empty() { return iced::widget::Space::new().into(); }
    let x = &app.extras;
    let mut col = column![ui::h2("Custom fields")].spacing(8);
    for f in fields {
        let fid = f["id"].as_i64().unwrap_or(0);
        let saved = f["value"].as_str().unwrap_or("").to_string();
        let kind = f["kind"].as_str().unwrap_or("TEXT");
        let shown = x.cf_vals.get(&fid).cloned().unwrap_or_else(|| if kind == "DATE" && !saved.is_empty() { app.d(&saved) } else { saved.clone() });
        let field: Element<Message> = if kind == "YESNO" {
            pick_list(vec!["Yes", "No"], (!shown.is_empty()).then_some(if shown == "Yes" { "Yes" } else { "No" }), move |v: &'static str| Message::Extras(Msg::CfValue(fid, v.to_string())))
                .placeholder("—").padding([5, 7]).text_size(13).style(theme::pick).width(Length::Fixed(140.0)).into()
        } else {
            text_input(match kind { "NUMBER" => "0", "DATE" => "Date", _ => "" }, &shown).on_input(move |v| Message::Extras(Msg::CfValue(fid, v))).padding([5, 7]).size(13).style(theme::input).width(Length::Fixed(260.0)).into()
        };
        col = col.push(row![text(f["name"].as_str().unwrap_or("").to_string()).size(14).width(200), field].spacing(10).align_y(Alignment::Center));
    }
    col = col.push(row![iced::widget::Space::new().width(Length::Fill), ui::btn("Save custom fields", if x.cf_vals.is_empty() { None } else { Some(Message::Extras(Msg::CfSaveValues(entity, id))) })]);
    ui::card(col)
}

// ---------------- Revenue recognition ----------------

/// Deferral schedules: spread an invoice's income over the months it's earned in.
pub fn revenue_recognition(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading() };
    let x = &app.extras;
    let invoices: Vec<Opt> = d["invoices"].as_array().cloned().unwrap_or_default().iter().map(|i| Opt { id: i["id"].as_i64().unwrap_or(0),
        label: format!("{} — {} — {}", i["number"].as_str().unwrap_or(""), i["contact_name"].as_str().unwrap_or(""), ui::money(i["total"].as_i64().unwrap_or(0))) }).collect();
    let new = ui::card(column![
        ui::h2("Defer an invoice"),
        row![column![lbl("INVOICE"), pick_list(invoices, x.rr_invoice.clone(), |o| Message::Extras(Msg::RrInvoice(o))).placeholder("Choose an approved invoice").padding([5, 7]).text_size(13).style(theme::pick).width(360)].spacing(3),
            input("MONTHS", &x.rr_months, 90.0, |v| Message::Extras(Msg::RrMonths(v))), input("FIRST MONTH", &x.rr_start, 130.0, |v| Message::Extras(Msg::RrStart(v))),
            iced::widget::Space::new().width(Length::Fill), ui::btn_primary("Create schedule", Some(Message::Extras(Msg::RrCreate)))].spacing(12).align_y(Alignment::End),
        ui::muted("The invoice's income moves to Deferred Revenue on its date, then comes back into income in equal monthly parts."),
    ].spacing(10));
    let rows = d["schedules"].as_array().cloned().unwrap_or_default().iter().map(|s| {
        let (total, done) = (s["total"].as_i64().unwrap_or(0), s["recognised"].as_i64().unwrap_or(0));
        (vec![ui::cell(s["number"].as_str().unwrap_or("").to_string()), ui::cell(s["contact_name"].as_str().unwrap_or("").to_string()),
            ui::cell_muted(format!("{} months from {}", s["months"].as_i64().unwrap_or(0), app.d(s["start_date"].as_str().unwrap_or("")))),
            ui::cell_money(total), ui::cell_money(done), ui::cell_money(total - done),
            ui::cell_muted(s["next_date"].as_str().map(|n| app.d(n)).unwrap_or_else(|| "Done".into()))],
         s["document_id"].as_i64().map(|id| Message::Nav(Route::Doc { kind: "ACCREC".into(), id: Some(id) })))
    }).collect();
    let to_shown = if x.rr_to.is_empty() { app.d(&ui::today_iso()) } else { x.rr_to.clone() };
    let run = ui::card(row![
        column![ui::h2("Recognise revenue"), ui::muted("Posts every month due up to the date: Deferred Revenue back into income.")].spacing(4).width(Length::Fill),
        input("UP TO", &to_shown, 130.0, |v| Message::Extras(Msg::RrTo(v))),
        ui::btn_primary("Recognise", Some(Message::Extras(Msg::RrRun))),
    ].spacing(12).align_y(Alignment::End));
    let body = column![new, run,
        ui::table(&[ui::col("Invoice", 12), ui::col("Customer", 18), ui::col("Schedule", 20), ui::coln("Deferred", 12), ui::coln("Recognised", 12), ui::coln("Remaining", 12), ui::col("Next", 12)], rows, "No revenue schedules yet."),
    ].spacing(14);
    let title = if crate::vocab::acumatica() { "Deferral Schedules" } else if crate::vocab::sage() { "Deferred Income" } else { "Revenue recognition" };
    ui::page(ui::h1(title), vec![], body.max_width(1200).into())
}

// ---------------- Lots and serial numbers ----------------

/// Which items are tracked by lot or serial number, and what's on hand of each, with expiry.
pub fn lots(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading() };
    let x = &app.extras;
    let k = t();
    let tracked: Vec<Opt> = app.lk.items.iter().filter(|i| i["is_tracked"] == 1).map(|i| Opt { id: i["id"].as_i64().unwrap_or(0), label: i["name"].as_str().unwrap_or("").to_string() }).collect();
    let mode = x.lot_item.as_ref().and_then(|o| app.lk.item(o.id)).and_then(|i| i["lot_tracking"].as_str()).unwrap_or("NONE");
    let opt = |label: &'static str, key: &'static str| -> Element<'_, Message> {
        iced::widget::button(text(label).size(13)).style(if mode == key && x.lot_item.is_some() { theme::primary } else { theme::secondary }).padding([6, 12]).on_press(Message::Extras(Msg::LotMode(key))).into()
    };
    let setup = ui::card(column![
        ui::h2("Track an item"),
        row![column![lbl("ITEM"), pick_list(tracked, x.lot_item.clone(), |o| Message::Extras(Msg::LotItem(o))).placeholder("Choose a stock item").padding([5, 7]).text_size(13).style(theme::pick).width(260)].spacing(3),
            iced::widget::Space::new().width(12), opt("Not tracked", "NONE"), opt("By lot number", "LOT"), opt("By serial number", "SERIAL")].spacing(8).align_y(Alignment::End),
        ui::muted("Bills and invoices for a tracked item then ask for its lot number, or one serial number per unit."),
    ].spacing(10));
    let qty = |q: i64| { let s = format!("{:.4}", q as f64 / 10_000.0); s.trim_end_matches('0').trim_end_matches('.').to_string() };
    let today = ui::today_iso();
    let rows = d.as_array().cloned().unwrap_or_default().into_iter().map(|l| {
        let item = l["item_id"].as_i64().unwrap_or(0);
        let lot = l["lot"].as_str().unwrap_or("").to_string();
        let key = format!("{item}|{lot}");
        let expiry = l["expiry"].as_str().map(String::from);
        let expired = expiry.as_deref().is_some_and(|e| e < today.as_str());
        let shown = x.lot_exp.get(&key).cloned().unwrap_or_else(|| expiry.as_deref().map(|e| app.d(e)).unwrap_or_default());
        let (item2, lot2, lot3) = (item, lot.clone(), lot.clone());
        (vec![
            ui::cell(format!("{} {}", l["code"].as_str().unwrap_or(""), l["name"].as_str().unwrap_or("")).trim().to_string()),
            ui::cell_muted(if l["lot_tracking"] == "SERIAL" { "Serial" } else { "Lot" }),
            ui::cell(lot.clone()),
            text_input("No expiry", &shown).on_input(move |v| Message::Extras(Msg::LotExpiry(item2, lot2.clone(), v))).padding([4, 7]).size(13).style(theme::input).into(),
            if x.lot_exp.contains_key(&key) { ui::link("Save", Message::Extras(Msg::LotExpirySave(item, lot3))) } else if expired { text("Expired").size(13).color(k.bad).into() } else { ui::cell("") },
            ui::num(qty(l["qty_e4"].as_i64().unwrap_or(0))),
        ], None)
    }).collect();
    let body = column![setup,
        ui::table(&[ui::col("Item", 28), ui::col("Type", 8), ui::col("Lot / serial", 18), ui::col("Expiry", 16), ui::col("", 10), ui::coln("On hand", 10)], rows, "Nothing on hand by lot or serial number yet.")].spacing(14);
    let title = if crate::vocab::sage() { "Batches & Serial Numbers" } else if crate::vocab::acumatica() { "Lot/Serial Numbers" } else { "Lots and serial numbers" };
    ui::page(ui::h1(title), vec![], body.max_width(1150).into())
}

// ---------------- Locations ----------------

/// Inventory locations: the list, stock on hand at each, and moving stock between them.
pub fn locations(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading() };
    let x = &app.extras;
    let k = t();
    let locs = d["locations"].as_array().cloned().unwrap_or_default();
    let title = if crate::vocab::acumatica() { "Warehouses" } else if app.desktop() { "Inventory Site List" } else { "Locations" };
    let mut body = column![].spacing(14);
    if let Some((name, address)) = &x.loc_new {
        body = body.push(ui::card(row![
            input("NAME", name, 220.0, |v| Message::Extras(Msg::LocField(0, v))), input("ADDRESS", address, 320.0, |v| Message::Extras(Msg::LocField(1, v))),
            iced::widget::Space::new().width(Length::Fill), ui::btn("Cancel", Some(Message::Extras(Msg::LocNew(false)))), ui::btn_primary("Save location", Some(Message::Extras(Msg::LocSave))),
        ].spacing(12).align_y(Alignment::End)));
    }
    // Stock grid: item rows, a column per location.
    let mut names = vec![ledger_core::locations::MAIN.to_string()];
    names.extend(locs.iter().map(|l| l["name"].as_str().unwrap_or("").to_string()));
    let mut by_item: Vec<(String, Vec<i64>)> = Vec::new();
    for r in d["rows"].as_array().cloned().unwrap_or_default() {
        let label = format!("{} {}", r["code"].as_str().unwrap_or(""), r["name"].as_str().unwrap_or("")).trim().to_string();
        if by_item.last().map(|b| &b.0) != Some(&label) { by_item.push((label.clone(), vec![0; locs.len() + 1])); }
        let at = r["location_id"].as_i64().and_then(|id| locs.iter().position(|l| l["id"].as_i64() == Some(id))).map(|p| p + 1).unwrap_or(0);
        if let Some(b) = by_item.last_mut() { b.1[at] += r["qty_e4"].as_i64().unwrap_or(0); }
    }
    let qty = |q: i64| { let s = format!("{:.4}", q as f64 / 10_000.0); s.trim_end_matches('0').trim_end_matches('.').to_string() };
    // (Column titles are the location names, so the grid is laid out here rather than with ui::table.)
    let cell = |t: String, w: u16, right: bool, head: bool| -> Element<'_, Message> {
        let tx = text(if head { t.to_uppercase() } else { t }).size(if head { 11 } else { 14 }).font(if head { BOLD } else { iced::Font::DEFAULT }).color(if head { k.muted } else { k.text });
        container(if right { tx.align_x(iced::alignment::Horizontal::Right).width(Length::Fill) } else { tx.width(Length::Fill) }).width(Length::FillPortion(w)).into()
    };
    let mut head = row![cell("Item".into(), 30, false, true)].spacing(10);
    for n in &names { head = head.push(cell(n.clone(), 14, true, true)); }
    let empty = by_item.is_empty();
    let mut grid = column![container(head.padding([9, 14])).style(theme::header_row).width(Length::Fill)];
    for (label, v) in by_item {
        let mut r = row![cell(label, 30, false, false)].spacing(10);
        for q in v { r = r.push(cell(qty(q), 14, true, false)); }
        grid = grid.push(column![container(r.padding([8, 14])), ui::hr()]);
    }
    if empty { grid = grid.push(container(ui::muted("No tracked stock yet.")).padding(20)); }
    // The locations themselves.
    let mut list = column![row![ui::h2(title), iced::widget::Space::new().width(Length::Fill), ui::btn_primary("New location", Some(Message::Extras(Msg::LocNew(true))))].align_y(Alignment::Center)].spacing(6);
    list = list.push(row![text(ledger_core::locations::MAIN).size(14).width(Length::Fill), text("Stock with no other location").size(13).color(k.muted)]);
    for l in &locs {
        let id = l["id"].as_i64().unwrap_or(0);
        list = list.push(row![text(l["name"].as_str().unwrap_or("").to_string()).size(14).width(Length::Fill), text(l["address"].as_str().unwrap_or("").to_string()).size(13).color(k.muted).width(Length::Fill), ui::link("Remove", Message::Extras(Msg::LocDelete(id)))].spacing(10));
    }
    body = body.push(ui::card(list));
    // Transfer.
    let items: Vec<Opt> = app.lk.items.iter().filter(|i| i["is_tracked"] == 1).map(|i| Opt { id: i["id"].as_i64().unwrap_or(0), label: i["name"].as_str().unwrap_or("").to_string() }).collect();
    let pick = |label: &'static str, opts: Vec<Opt>, cur: Option<Opt>, f: fn(Opt) -> Msg, w: f32| -> Element<'_, Message> {
        column![lbl(label), pick_list(opts, cur, move |o| Message::Extras(f(o))).placeholder("Choose").padding([5, 7]).text_size(13).style(theme::pick).width(Length::Fixed(w))].spacing(3).into()
    };
    let date_shown = if x.tr_date.is_empty() { app.d(&ui::today_iso()) } else { x.tr_date.clone() };
    body = body.push(ui::card(column![
        ui::h2("Move stock"),
        row![pick("ITEM", items, x.tr_item.clone(), Msg::TrItem, 220.0), pick("FROM", app.lk.location_opts(), x.tr_from.clone(), Msg::TrFrom, 180.0), pick("TO", app.lk.location_opts(), x.tr_to.clone(), Msg::TrTo, 180.0),
            input("QUANTITY", &x.tr_qty, 100.0, |v| Message::Extras(Msg::TrQty(v))), input("DATE", &date_shown, 120.0, |v| Message::Extras(Msg::TrDate(v))),
            iced::widget::Space::new().width(Length::Fill), ui::btn_primary("Move", Some(Message::Extras(Msg::TrMove)))].spacing(10).align_y(Alignment::End),
    ].spacing(10)));
    body = body.push(ui::h2("Stock on hand"));
    body = body.push(container(grid).style(theme::card).width(Length::Fill));
    body = body.push(ui::muted("Bills, invoices and credit notes take a location once you have one. Cost is one average per item across locations."));
    ui::page(ui::h1(title), vec![], body.into())
}

// ---------------- Batch payments ----------------

/// Supplier payments made together (Pay bills with several bills), each with a bank file
/// (ABA in Australia, BACS in the UK, NACHA in the US) to upload to the bank.
pub fn batch_payments(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading() };
    let format = match app.country().as_str() { "GB" | "UK" => "BACS", "US" => "NACHA", _ => "ABA" };
    let rows = d.as_array().cloned().unwrap_or_default().iter().filter(|b| b["status"] != "DELETED").map(|b| {
        let id = b["id"].as_i64().unwrap_or(0);
        let pay = b["type"] != "RECBATCH";
        (vec![ui::cell(app.d(b["date"].as_str().unwrap_or(""))), ui::cell_muted(if pay { "Payments" } else { "Receipts" }), ui::cell(b["reference"].as_str().unwrap_or("").to_string()),
            ui::cell_muted(b["account_name"].as_str().unwrap_or("").to_string()), ui::cell(b["payment_count"].as_i64().unwrap_or(0).to_string()), ui::cell_money(b["total"].as_i64().unwrap_or(0)),
            if pay { ui::link(if format == "BACS" { "Bank file (BACS)" } else if format == "NACHA" { "Bank file (NACHA)" } else { "Bank file (ABA)" }, Message::Extras(Msg::BatchBankFile(id))) } else { ui::cell("") }], None)
    }).collect();
    let body = column![
        ui::table(&[ui::col("Date", 12), ui::col("Type", 10), ui::col("Reference", 18), ui::col("Bank account", 18), ui::col("Payments", 9), ui::coln("Total", 12), ui::col("", 16)], rows,
            "No batch payments yet. Pay several bills together in Pay bills and they're saved as one batch."),
        ui::muted("A bank file pays every supplier in the batch in one upload. Suppliers need their bank details on their contact."),
    ].spacing(14);
    ui::page(ui::h1("Batch payments"), vec![ui::btn_primary(app.term("pay_bill"), Some(Message::Nav(Route::PayBills)))], body.max_width(1150).into())
}

// ---------------- Payment reminders ----------------

/// Payment reminders (dunning levels): emails sent to customers a set number of days after an
/// invoice falls due, one per level, until it's paid.
pub fn payment_reminders(app: &App) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return loading() };
    let on = d["on"] == true;
    let rules = d["rules"].as_array().cloned().unwrap_or_default();
    let rows = rules.iter().enumerate().map(|(n, r)| {
        let id = r["id"].as_i64().unwrap_or(0);
        let days = r["days"].as_i64().unwrap_or(0);
        let active = r["active"] == 1;
        let when = if days < 0 { format!("{} days before the due date", -days) } else if days == 0 { "On the due date".into() } else { format!("{days} days after the due date") };
        (vec![
            ui::cell(format!("Level {}", n + 1)), ui::cell(when), ui::cell_muted(r["template_name"].as_str().unwrap_or("Payment reminder").to_string()),
            iced::widget::checkbox(active).label("Active").on_toggle(move |v| Message::Extras(Msg::DunActive(id, days, v))).text_size(13).into(),
            ui::link("Remove", Message::Extras(Msg::DunDelete(id))),
        ], None)
    }).collect();
    let body = column![
        ui::card(row![
            column![ui::h2("Send payment reminders"), text(if on { "On: customers with overdue invoices get the reminder for each level, once." } else { "Off: no reminders are sent." }).size(13).color(if on { k.ok } else { k.muted })].spacing(4).width(Length::Fill),
            iced::widget::toggler(on).on_toggle(|v| Message::Extras(Msg::DunOn(v))),
        ].align_y(Alignment::Center)),
        ui::table(&[ui::col("Level", 10), ui::col("When", 30), ui::col("Email", 30), ui::col("", 14), ui::col("", 10)], rows, "No reminder levels yet."),
        ui::card(row![
            input("NEW LEVEL: DAYS AFTER DUE", &app.extras.dun_days, 200.0, |v| Message::Extras(Msg::DunDays(v))),
            container(ui::btn_primary("Add level", Some(Message::Extras(Msg::DunAdd)))).padding(iced::Padding { top: 16.0, ..Default::default() }),
            iced::widget::Space::new().width(Length::Fill),
            container(ui::btn("Send due reminders now", if on { Some(Message::Extras(Msg::DunRun)) } else { None })).padding(iced::Padding { top: 16.0, ..Default::default() }),
        ].spacing(12).align_y(Alignment::Center)),
        ui::muted("Reminders go out with the daily background run, through the email settings (or to the Outbox until email is set up). Use a negative number to remind before the due date."),
    ].spacing(14);
    ui::page(ui::h1("Payment reminders"), vec![], body.max_width(1000).into())
}

// ---------------- Find & recode ----------------

/// Find posted invoice, bill and bank lines by account, tax rate, contact, date or text, tick
/// them, and move them to another account or tax rate (each transaction is reposted).
pub fn recode(app: &App) -> Element<'_, Message> {
    let k = t();
    let x = &app.extras;
    let pick = |label: &'static str, opts: Vec<Opt>, cur: Option<Opt>, f: fn(Opt) -> Msg, ph: &'static str, w: f32| -> Element<'_, Message> {
        column![lbl(label), pick_list(opts, cur, move |o| Message::Extras(f(o))).placeholder(ph).padding([5, 7]).text_size(13).style(theme::pick).width(Length::Fixed(w))].spacing(3).into()
    };
    let txt = |k: &'static str| x.rc_text.get(k).cloned().unwrap_or_default();
    let filters = ui::card(column![
        ui::h2("Find"),
        row![
            pick("ACCOUNT", app.lk.account_opts(|_| true), x.rc_account.clone(), Msg::RcAccount, "Any account", 240.0),
            pick("TAX RATE", app.lk.tax_opts(), x.rc_tax.clone(), Msg::RcTax, "Any tax rate", 200.0),
            pick("CONTACT", { let mut seen = std::collections::HashSet::new(); app.lk.contact_opts(true).into_iter().chain(app.lk.contact_opts(false)).filter(|o| seen.insert(o.id)).collect() }, x.rc_contact.clone(), Msg::RcContact, "Any contact", 220.0),
        ].spacing(12),
        row![
            input("FROM", &txt("from"), 130.0, |v| Message::Extras(Msg::RcText("from", v))),
            input("TO", &txt("to"), 130.0, |v| Message::Extras(Msg::RcText("to", v))),
            input("DESCRIPTION CONTAINS", &txt("q"), 260.0, |v| Message::Extras(Msg::RcText("q", v))),
            ui::hspace(),
            ui::btn("Clear", Some(Message::Extras(Msg::RcClear))), ui::btn_primary("Search", Some(Message::Extras(Msg::RcFind))),
        ].spacing(12).align_y(Alignment::End),
    ].spacing(12));
    let Some(d) = &app.data else { return ui::page(ui::h1("Find and recode"), vec![], column![filters, loading()].spacing(14).into()) };
    let lines = d.as_array().cloned().unwrap_or_default();
    let all = !lines.is_empty() && lines.iter().all(|l| l["line_id"].as_i64().is_some_and(|id| x.rc_sel.contains(&id)));
    let type_name = |ty: &str| match ty { "ACCREC" => "Invoice", "ACCPAY" => "Bill", "ACCRECCREDIT" | "ACCPAYCREDIT" => "Credit note", "SPEND" => "Spend money", "RECEIVE" => "Receive money", other => other }.to_string();
    let rows = lines.iter().map(|l| {
        let id = l["line_id"].as_i64().unwrap_or(0);
        (vec![
            iced::widget::checkbox(x.rc_sel.contains(&id)).on_toggle(move |_| Message::Extras(Msg::RcToggle(id))).into(),
            ui::cell(app.d(l["date"].as_str().unwrap_or(""))), ui::cell_muted(type_name(l["type"].as_str().unwrap_or(""))), ui::cell(l["number"].as_str().unwrap_or("").to_string()),
            ui::cell(l["contact"].as_str().unwrap_or("").to_string()), ui::cell_muted(l["description"].as_str().unwrap_or("").to_string()),
            ui::cell(format!("{} {}", l["account_code"].as_str().unwrap_or(""), l["account_name"].as_str().unwrap_or("")).trim().to_string()),
            ui::cell_muted(l["tax_rate_name"].as_str().unwrap_or("").to_string()), ui::cell_money(l["net_amount"].as_i64().unwrap_or(0)),
        ], Some(Message::Extras(Msg::RcToggle(id))))
    }).collect();
    let head = row![
        iced::widget::checkbox(all).label(format!("{} line(s) found, {} ticked", lines.len(), x.rc_sel.len())).on_toggle(|v| Message::Extras(Msg::RcAll(v))).text_size(13),
    ];
    let change = ui::card(column![
        ui::h2("Recode the ticked lines"),
        row![
            pick("NEW ACCOUNT", app.lk.account_opts(|a| a["type"] != "BANK"), x.rc_to_account.clone(), Msg::RcToAccount, "Leave as is", 260.0),
            pick("NEW TAX RATE", app.lk.tax_opts(), x.rc_to_tax.clone(), Msg::RcToTax, "Leave as is", 220.0),
            ui::hspace(),
            ui::btn_primary(&format!("Recode {} line(s)", x.rc_sel.len()), if x.busy || x.rc_sel.is_empty() { None } else { Some(Message::Extras(Msg::RcApply)) }),
        ].spacing(12).align_y(Alignment::End),
        text("Each changed transaction is reversed and posted again with the new coding; payments stay as they are. The change is in the audit log.").size(12).color(k.muted),
    ].spacing(10));
    let cols = [ui::col("", 3), ui::col("Date", 10), ui::col("Type", 10), ui::col("Number", 10), ui::col("Contact", 15), ui::col("Description", 20), ui::col("Account", 16), ui::col("Tax rate", 12), ui::coln("Amount", 10)];
    ui::page(ui::h1("Find and recode"), vec![], column![filters, change, head, ui::table(&cols, rows, "Nothing matches — change the search.")].spacing(14).into())
}

// ---------------- Import / export ----------------

/// What can be imported: (code, label, needs a date).
pub const IMPORTS: [(&str, &str); 6] = [("contacts", "Customers and suppliers"), ("accounts", "Chart of accounts"), ("items", "Products and services"), ("invoices", "Invoices"), ("bills", "Bills"), ("journals", "Opening balances")];
pub const EXPORTS: [(&str, &str); 6] = [("contacts", "Contacts"), ("accounts", "Chart of accounts"), ("items", "Products and services"), ("invoices", "Invoices"), ("bills", "Bills"), ("journals", "General ledger (journals)")];

fn ie_params(app: &App) -> (&'static str, Option<String>) {
    let what = app.extras.ie_what.unwrap_or("contacts");
    let date = (what == "journals").then(|| ui::to_iso(&if app.extras.ie_date.is_empty() { app.d(&ui::today_iso()) } else { app.extras.ie_date.clone() }, app.date_style())).flatten();
    (what, date)
}

pub fn import_export(app: &App) -> Element<'_, Message> {
    let k = t();
    let what = app.extras.ie_what.unwrap_or("contacts");
    let mut imp = column![ui::h2("Import"),
        ui::muted("CSV files from Xero, QuickBooks, MYOB or a spreadsheet — the first row names the columns (Name, Email, *Code, InvoiceNumber…). You'll see what will happen before anything is changed."),
        row![column![lbl("WHAT ARE YOU IMPORTING?"), pick_list(IMPORTS.iter().map(|x| x.1).collect::<Vec<_>>(), Some(label_of(&IMPORTS, what)), |l: &'static str| Message::Extras(Msg::IeWhat(code_of(&IMPORTS, l))))
            .padding([5, 7]).text_size(13).style(theme::pick).width(260)].spacing(3)].spacing(14).align_y(Alignment::End)].spacing(10);
    if what == "journals" {
        let d = if app.extras.ie_date.is_empty() { app.d(&ui::today_iso()) } else { app.extras.ie_date.clone() };
        imp = imp.push(input("CONVERSION DATE (BALANCES AS AT)", &d, 200.0, |x| Message::Extras(Msg::IeDate(x))));
    }
    imp = imp.push(row![ui::btn_primary("Choose CSV file…", Some(Message::Extras(Msg::IePick))), text(app.extras.ie_file.as_ref().map(|f| f.0.clone()).unwrap_or_default()).size(13).color(k.muted)].spacing(12).align_y(Alignment::Center));
    if let Some(p) = &app.extras.ie_preview {
        let errors = p["errors"].as_array().cloned().unwrap_or_default();
        let mut c = column![text(format!("Ready to import: {} new, {} skipped.", p["created"].as_i64().unwrap_or(0), p["skipped"].as_i64().unwrap_or(0))).size(14).font(BOLD)].spacing(4);
        for m in p["messages"].as_array().cloned().unwrap_or_default().iter().take(8) { c = c.push(text(m.as_str().unwrap_or("").to_string()).size(12).color(k.muted)); }
        for e in errors.iter().take(8) { c = c.push(text(e.as_str().unwrap_or("").to_string()).size(12).color(k.bad)); }
        imp = imp.push(container(c).padding(12).style(theme::soft).width(iced::Fill));
        imp = imp.push(row![ui::hspace(), ui::btn_primary("Import", if app.extras.busy || p["created"].as_i64().unwrap_or(0) == 0 { None } else { Some(Message::Extras(Msg::IeRun)) })]);
    }
    let mut exp = column![ui::h2("Export"), ui::muted("Each export is a CSV file in Documents\\LedgerCraft, in the same columns the import reads.")].spacing(10);
    let mut r = row![].spacing(8);
    for (code, label) in EXPORTS { r = r.push(ui::btn(label, Some(Message::Extras(Msg::IeExport(code))))); }
    exp = exp.push(r);
    exp = exp.push(row![ui::btn("Back up the whole company file", Some(Message::BackupCompany))]);
    ui::page(ui::h1("Import and export"), vec![], column![ui::card(imp), ui::card(exp)].spacing(14).max_width(1100).into())
}

// ---------------- Custom reports ----------------

pub const CR_TYPES: [(&str, &str); 10] = [("REVENUE", "Income"), ("DIRECTCOSTS", "Cost of sales"), ("EXPENSE", "Expenses"), ("OTHERINCOME", "Other income"), ("ASSET", "Assets"), ("BANK", "Bank accounts"),
    ("CURRENT", "Current assets"), ("LIABILITY", "Liabilities"), ("CURRLIAB", "Current liabilities"), ("EQUITY", "Equity")];

pub fn custom_reports(app: &App) -> Element<'_, Message> {
    let k = t();
    let Some(d) = &app.data else { return loading() };
    let mut body = column![].spacing(12);
    if let Some((name, monthly, rows)) = &app.extras.cr_new {
        let mut list = column![].spacing(6);
        for (n, r) in rows.iter().enumerate() {
            let label = text_input("Row label", &r.label).on_input(move |v| Message::Extras(Msg::CrLabel(n, v))).padding([4, 6]).size(13).style(theme::input).width(220);
            let what: Element<Message> = match r.kind {
                "heading" => text("Heading").size(12).color(k.muted).into(),
                "formula" => row![text(format!("r{} =", n + 1)).size(12).color(k.muted), text_input("e.g. r1 - r2", &r.expr).on_input(move |v| Message::Extras(Msg::CrExpr(n, v))).padding([4, 6]).size(13).style(theme::input).width(260)].spacing(6).align_y(Alignment::Center).into(),
                _ => row![text(format!("r{} = total of", n + 1)).size(12).color(k.muted),
                    pick_list(CR_TYPES.iter().map(|x| x.1).collect::<Vec<_>>(), r.types.first().map(|t| label_of(&CR_TYPES, t)), move |l: &'static str| Message::Extras(Msg::CrType(n, code_of(&CR_TYPES, l))))
                        .padding([4, 6]).text_size(13).style(theme::pick).width(180),
                    iced::widget::checkbox(r.sign < 0).label("Show credits as positive").on_toggle(move |v| Message::Extras(Msg::CrSign(n, v))).text_size(12)].spacing(8).align_y(Alignment::Center).into(),
            };
            list = list.push(row![label, what, ui::hspace(), ui::link("Remove", Message::Extras(Msg::CrRemove(n)))].spacing(12).align_y(Alignment::Center));
        }
        body = body.push(ui::card(column![
            ui::h2("New custom report"),
            row![input("REPORT NAME", name, 300.0, |x| Message::Extras(Msg::CrName(x))),
                container(iced::widget::checkbox(*monthly).label("A column for each month").on_toggle(|v| Message::Extras(Msg::CrMonthly(v))).text_size(13)).padding(iced::Padding { top: 18.0, ..Default::default() })].spacing(16),
            list,
            row![ui::btn("Add account row", Some(Message::Extras(Msg::CrAdd("types")))), ui::btn("Add formula", Some(Message::Extras(Msg::CrAdd("formula")))), ui::btn("Add heading", Some(Message::Extras(Msg::CrAdd("heading")))),
                ui::hspace(), ui::btn_primary("Save", Some(Message::Extras(Msg::CrSave))), ui::btn("Cancel", Some(Message::Extras(Msg::CrNew(false))))].spacing(8),
            text("Formulas use the row numbers: r1 + r2, (r1 - r2) / r1 * 100 …").size(12).color(k.muted),
        ].spacing(12)));
    }
    let rows = d.as_array().cloned().unwrap_or_default().iter().filter(|l| l["kind"] == "CUSTOM").map(|l| {
        let id = l["id"].as_i64().unwrap_or(0);
        (vec![ui::cell(l["name"].as_str().unwrap_or("").to_string()), ui::cell_muted(l["updated_at"].as_str().or(l["created_at"].as_str()).map(|x| app.d(x.get(..10).unwrap_or(x))).unwrap_or_default()),
            row![ui::link("Run", Message::Nav(Route::Report(format!("layout:{id}")))), ui::link("Delete", Message::Extras(Msg::CrDelete(id)))].spacing(14).into()],
         Some(Message::Nav(Route::Report(format!("layout:{id}")))))
    }).collect();
    body = body.push(ui::table(&[ui::col("Report", 50), ui::col("Saved", 20), ui::col("", 20)], rows, "No custom reports yet — New report builds one from account totals and formulas."));
    ui::page(ui::h1("Custom reports"), vec![ui::btn_primary("New report", Some(Message::Extras(Msg::CrNew(true))))], body.max_width(1100).into())
}

// ---------------- Payroll settings ----------------

pub const FREQUENCIES: [(&str, &str); 5] = [("WEEKLY", "Weekly"), ("FORTNIGHTLY", "Fortnightly"), ("FOURWEEKLY", "Every four weeks"), ("MONTHLY", "Monthly"), ("QUARTERLY", "Quarterly")];

pub fn payroll_settings(app: &App) -> Element<'_, Message> {
    let Some(d) = app.data.as_ref().filter(|d| d["calendars"].is_array()) else { return loading() };
    let freq_word = |f: &str| if app.country() == "US" && f == "FORTNIGHTLY" { "Every other week" } else { label_of(&FREQUENCIES, f) };
    let cal_rows = d["calendars"].as_array().cloned().unwrap_or_default().iter().map(|c| (vec![ui::cell(c["name"].as_str().unwrap_or("").to_string()),
        ui::cell_muted(freq_word(c["frequency"].as_str().unwrap_or(""))), ui::cell_muted(app.d(c["start_date"].as_str().unwrap_or("")))], None)).collect();
    let mut cals = column![row![ui::h2(if app.skin == crate::theme::Skin::Xero { "Calendars" } else { "Pay schedules" }), ui::hspace(), ui::btn("Add", Some(Message::Extras(Msg::PsCalNew(true))))].align_y(Alignment::Center),
        ui::table(&[ui::col("Name", 30), ui::col("How often", 20), ui::col("First period starts", 20)], cal_rows, "No pay schedules yet.")].spacing(10);
    if let Some((name, freq, start)) = &app.extras.ps_cal {
        cals = cals.push(container(row![input("NAME", name, 220.0, |x| Message::Extras(Msg::PsCalField(0, x))),
            column![lbl("HOW OFTEN"), pick_list(FREQUENCIES.iter().map(|f| f.1).collect::<Vec<_>>(), Some(label_of(&FREQUENCIES, freq)), |l: &'static str| Message::Extras(Msg::PsCalFreq(code_of(&FREQUENCIES, l))))
                .padding([5, 7]).text_size(13).style(theme::pick).width(200)].spacing(3),
            input("FIRST PERIOD STARTS", start, 160.0, |x| Message::Extras(Msg::PsCalField(1, x))), ui::hspace(),
            ui::btn_primary("Save", Some(Message::Extras(Msg::PsCalSave))), ui::btn("Cancel", Some(Message::Extras(Msg::PsCalNew(false))))].spacing(12).align_y(Alignment::End)).padding(12).style(theme::soft));
    }
    let leave_rows = d["leave_types"].as_array().cloned().unwrap_or_default().iter().map(|l| (vec![ui::cell(l["name"].as_str().unwrap_or("").to_string()),
        ui::num(format!("{:.2}", l["accrual_hours_per_year_e2"].as_i64().unwrap_or(0) as f64 / 100.0))], None)).collect();
    let mut leave = column![row![ui::h2(if app.country() == "US" { "Paid time off" } else { "Leave types" }), ui::hspace(), ui::btn("Add", Some(Message::Extras(Msg::PsLeaveNew(true))))].align_y(Alignment::Center),
        ui::table(&[ui::col("Name", 40), ui::coln("Hours accrued a year (full-time)", 30)], leave_rows, "No leave types yet.")].spacing(10);
    if let Some((name, hours)) = &app.extras.ps_leave {
        leave = leave.push(container(row![input("NAME", name, 260.0, |x| Message::Extras(Msg::PsLeaveField(0, x))), input("HOURS A YEAR", hours, 140.0, |x| Message::Extras(Msg::PsLeaveField(1, x))), ui::hspace(),
            ui::btn_primary("Save", Some(Message::Extras(Msg::PsLeaveSave))), ui::btn("Cancel", Some(Message::Extras(Msg::PsLeaveNew(false))))].spacing(12).align_y(Alignment::End)).padding(12).style(theme::soft));
    }
    let juris = d["jurisdiction_name"].as_str().unwrap_or("").to_string();
    ui::page(ui::h1("Payroll settings"), vec![ui::btn("Pay items", Some(Message::Nav(Route::PayrollItems)))], column![
        ui::card(cals), ui::card(leave),
        ui::card(column![ui::h2("Tax"), ui::muted(format!("Payroll tax rules: {juris}. {}", d["disclaimer"].as_str().unwrap_or("")))].spacing(6)),
    ].spacing(14).max_width(1000).into())
}

// ---------------- Projects ----------------

/// The New project form above the projects list.
pub fn project_new(app: &App) -> Element<'_, Message> {
    let Some((name, cust)) = &app.extras.pj_new else { return iced::widget::Space::new().into() };
    ui::card(row![input("PROJECT NAME", name, 300.0, |x| Message::Extras(Msg::PjNewName(x))),
        column![lbl("CUSTOMER"), pick_list(app.lk.contact_opts(true), cust.clone(), |o| Message::Extras(Msg::PjNewCustomer(o))).placeholder("Choose a customer").padding([5, 7]).text_size(13).style(theme::pick).width(260)].spacing(3),
        iced::widget::Space::new().width(iced::Fill), ui::btn_primary("Create", Some(Message::Extras(Msg::PjCreate))), ui::btn("Cancel", Some(Message::Extras(Msg::PjNew(false))))].spacing(14).align_y(Alignment::End))
}

/// The selected project: tasks, time and expenses, with forms to add each.
pub fn project_panel(app: &App) -> Element<'_, Message> {
    let k = t();
    let Some(p) = app.extras.pj_detail.as_ref().filter(|p| p["id"].as_i64() == app.extras.pj_sel) else {
        return ui::muted("Select a project to see and add its tasks, time and expenses.");
    };
    let tasks = p["tasks"].as_array().cloned().unwrap_or_default();
    let task_rows = tasks.iter().map(|t| (vec![ui::cell(t["name"].as_str().unwrap_or("").to_string()),
        ui::cell_muted(if t["charge_type"] == "FIXED" { "Fixed price" } else { "Hourly" }), ui::cell_money(t["rate_e2"].as_i64().unwrap_or(0) / 100)], None)).collect();
    let mut tcol = column![row![ui::h2("Tasks"), ui::hspace(), ui::btn("Add task", Some(Message::Extras(Msg::PjTaskOpen(true))))].align_y(Alignment::Center),
        ui::table(&[ui::col("Task", 40), ui::col("Charged", 20), ui::coln("Rate / price", 20)], task_rows, "No tasks yet — add one to record time against it.")].spacing(8);
    if let Some((name, kind, rate)) = &app.extras.pj_task {
        tcol = tcol.push(container(row![input("TASK", name, 240.0, |x| Message::Extras(Msg::PjTaskField(0, x))),
            column![lbl("CHARGED"), pick_list(vec!["Hourly", "Fixed price"], Some(if *kind == "FIXED" { "Fixed price" } else { "Hourly" }), |l: &'static str| Message::Extras(Msg::PjTaskKind(if l == "Hourly" { "TIME" } else { "FIXED" })))
                .padding([5, 7]).text_size(13).style(theme::pick).width(140)].spacing(3),
            input(if *kind == "FIXED" { "PRICE" } else { "RATE PER HOUR" }, rate, 130.0, |x| Message::Extras(Msg::PjTaskField(1, x))), ui::hspace(),
            ui::btn_primary("Save", Some(Message::Extras(Msg::PjTaskSave))), ui::btn("Cancel", Some(Message::Extras(Msg::PjTaskOpen(false))))].spacing(12).align_y(Alignment::End)).padding(12).style(theme::soft));
    }
    let time_rows = p["time"].as_array().cloned().unwrap_or_default().iter().map(|e| (vec![ui::cell_muted(app.d(e["date"].as_str().unwrap_or(""))),
        ui::cell(e["task"].as_str().unwrap_or("").to_string()), ui::cell_muted(e["description"].as_str().unwrap_or("").to_string()),
        ui::num(format!("{}:{:02}", e["minutes"].as_i64().unwrap_or(0) / 60, e["minutes"].as_i64().unwrap_or(0) % 60)),
        ui::cell_muted(if e["invoiced_document_id"].is_null() { "Unbilled" } else { "Invoiced" })], None)).collect();
    let mut time = column![row![ui::h2("Time"), ui::hspace(), ui::btn("Add time", if tasks.is_empty() { None } else { Some(Message::Extras(Msg::PjTimeOpen(true))) })].align_y(Alignment::Center),
        ui::table(&[ui::col("Date", 14), ui::col("Task", 22), ui::col("Notes", 34), ui::coln("Time", 10), ui::col("", 12)], time_rows, "No time recorded.")].spacing(8);
    if let Some((task, date, dur, notes)) = &app.extras.pj_time {
        let topts: Vec<Opt> = tasks.iter().map(|t| Opt { id: t["id"].as_i64().unwrap_or(0), label: t["name"].as_str().unwrap_or("").to_string() }).collect();
        time = time.push(container(row![column![lbl("TASK"), pick_list(topts, task.clone(), |o| Message::Extras(Msg::PjTimeTask(o))).placeholder("Task").padding([5, 7]).text_size(13).style(theme::pick).width(200)].spacing(3),
            input("DATE", date, 130.0, |x| Message::Extras(Msg::PjTimeField(0, x))), input("TIME (H:MM)", dur, 110.0, |x| Message::Extras(Msg::PjTimeField(1, x))),
            input("NOTES", notes, 260.0, |x| Message::Extras(Msg::PjTimeField(2, x))), ui::hspace(),
            ui::btn_primary("Save", Some(Message::Extras(Msg::PjTimeSave))), ui::btn("Cancel", Some(Message::Extras(Msg::PjTimeOpen(false))))].spacing(12).align_y(Alignment::End)).padding(12).style(theme::soft));
    }
    let cost_rows = p["costs"].as_array().cloned().unwrap_or_default().iter().map(|c| (vec![ui::cell_muted(app.d(c["date"].as_str().unwrap_or(""))),
        ui::cell(c["description"].as_str().unwrap_or("").to_string()), ui::cell_money(c["amount"].as_i64().unwrap_or(0)),
        ui::cell_muted(if c["chargeable"].as_i64() == Some(1) { "Chargeable" } else { "Not chargeable" })], None)).collect();
    let mut costs = column![row![ui::h2("Expenses"), ui::hspace(), ui::btn("Add expense", Some(Message::Extras(Msg::PjCostOpen(true))))].align_y(Alignment::Center),
        ui::table(&[ui::col("Date", 14), ui::col("Description", 44), ui::coln("Amount", 14), ui::col("", 16)], cost_rows, "No expenses on this project.")].spacing(8);
    if let Some((date, desc, amount, charge)) = &app.extras.pj_cost {
        costs = costs.push(container(row![input("DATE", date, 130.0, |x| Message::Extras(Msg::PjCostField(0, x))), input("DESCRIPTION", desc, 280.0, |x| Message::Extras(Msg::PjCostField(1, x))),
            input("AMOUNT", amount, 120.0, |x| Message::Extras(Msg::PjCostField(2, x))),
            container(iced::widget::checkbox(*charge).label("Charge to customer").on_toggle(|v| Message::Extras(Msg::PjCostCharge(v))).text_size(13)).padding(iced::Padding { top: 16.0, ..Default::default() }), ui::hspace(),
            ui::btn_primary("Save", Some(Message::Extras(Msg::PjCostSave))), ui::btn("Cancel", Some(Message::Extras(Msg::PjCostOpen(false))))].spacing(12).align_y(Alignment::End)).padding(12).style(theme::soft));
    }
    let closed = p["status"] == "CLOSED";
    ui::card(column![
        row![column![text(p["name"].as_str().unwrap_or("").to_string()).size(18).font(BOLD), text(p["contact_name"].as_str().unwrap_or("").to_string()).size(13).color(k.muted)].spacing(2), ui::hspace(),
            ui::btn(if closed { "Reopen" } else { "Mark complete" }, Some(Message::Extras(Msg::PjStatus(if closed { "INPROGRESS" } else { "CLOSED" }))))].align_y(Alignment::Center),
        tcol, time, costs,
    ].spacing(16))
}

// ---------------- Currencies ----------------

pub fn currencies(app: &App) -> Element<'_, Message> {
    let Some(d) = &app.data else { return loading() };
    let home = app.setting("base_currency");
    let list = d.as_array().cloned().unwrap_or_default();
    let rows = list.iter().filter(|c| c["enabled"].as_i64() == Some(1) || c["code"] == home.as_str()).map(|c| {
        let code = c["code"].as_str().unwrap_or("").to_string();
        let is_home = code == home;
        let rate = c["latest_rate_e8"].as_i64().map(|r| format!("{:.4}", r as f64 / 1e8)).unwrap_or_else(|| "—".into());
        let actions: Element<Message> = if is_home { ui::cell_muted("Home currency") } else {
            row![ui::link("Set rate", Message::Extras(Msg::FxRateOpen(code.clone()))), ui::link("Turn off", Message::Extras(Msg::FxEnable(code.clone(), false)))].spacing(14).into()
        };
        (vec![ui::cell(code.clone()), ui::cell(c["name"].as_str().unwrap_or("").to_string()), ui::num(if is_home { "1.0000".to_string() } else { rate }),
            ui::cell_muted(c["latest_rate_date"].as_str().map(|x| app.d(x)).unwrap_or_default()), actions], None)
    }).collect();
    let add: Vec<String> = list.iter().filter(|c| c["enabled"].as_i64() != Some(1) && c["code"] != home.as_str()).map(|c| format!("{} — {}", c["code"].as_str().unwrap_or(""), c["name"].as_str().unwrap_or(""))).collect();
    let mut body = column![
        ui::table(&[ui::col("Code", 8), ui::col("Currency", 30), ui::coln("Rate", 14), ui::col("As at", 14), ui::col("", 22)], rows, "Only your home currency is in use."),
        row![text("Add a currency").size(13), pick_list(add, None::<String>, |s: String| Message::Extras(Msg::FxEnable(s.split(" — ").next().unwrap_or("").to_string(), true)))
            .placeholder("Choose…").padding([5, 7]).text_size(13).style(theme::pick).width(300), ui::hspace(),
            ui::btn("Update rates now", Some(Message::Extras(Msg::FxFetch)))].spacing(10).align_y(Alignment::Center),
    ].spacing(12);
    if let Some((code, date, rate)) = &app.extras.fx_rate {
        body = body.push(ui::card(row![ui::h2(format!("{code} rate")), input("DATE", date, 140.0, |x| Message::Extras(Msg::FxRateField(0, x))),
            input(if home.is_empty() { "RATE" } else { "PER 1 OF YOUR HOME CURRENCY" }, rate, 200.0, |x| Message::Extras(Msg::FxRateField(1, x))), ui::hspace(),
            ui::btn_primary("Save rate", Some(Message::Extras(Msg::FxRateSave)))].spacing(14).align_y(Alignment::End)));
    }
    let rd = if app.extras.fx_reval_date.is_empty() { app.d(&ui::today_iso()) } else { app.extras.fx_reval_date.clone() };
    body = body.push(ui::card(row![column![ui::h2("Revalue foreign currency balances"), ui::muted("Posts unrealised gains and losses on open invoices, bills and foreign bank accounts at the rate on this date.")].spacing(4).width(iced::Fill),
        input("REVALUE AT", &rd, 140.0, |x| Message::Extras(Msg::FxRevalDate(x))), container(ui::btn_primary("Revalue", Some(Message::Extras(Msg::FxRevalue)))).padding(iced::Padding { top: 16.0, ..Default::default() })].spacing(14).align_y(Alignment::Center)));
    ui::page(ui::h1("Currencies"), vec![], body.max_width(1100).into())
}
