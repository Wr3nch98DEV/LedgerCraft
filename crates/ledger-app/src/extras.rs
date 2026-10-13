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
}

/// A custom report row: a heading, a total of account types, or a formula over earlier rows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrRow { pub kind: &'static str, pub label: String, pub types: Vec<&'static str>, pub expr: String, pub sign: i64 }

#[derive(Debug, Clone)]
pub enum Msg {
    FaTab(&'static str), FaNew, FaField(&'static str, String), FaType(Opt), FaMethod(&'static str), FaSave, FaCancel, FaRegister(i64),
    FaDepDate(String), FaRun, FaDisposeOpen(i64), FaDisposeField(u8, String), FaDisposeProceeds(Opt), FaDisposeGain(Opt), FaDispose,
    AuditEntity(&'static str),
    FxEnable(String, bool), FxRateOpen(String), FxRateField(u8, String), FxRateSave, FxRevalDate(String), FxRevalue, FxFetch,
    BudPick(i64), BudYear(i32), BudCell(i64, String, String), BudCopyAcross(i64), BudNew(Option<String>), BudCreate, BudSave, BudCreated(Reply),
    ClTab(&'static str), ClNew(bool), ClField(&'static str, String), ClAccount(Opt), ClTax(Opt), ClSave(bool), ClStatus(i64, &'static str), ClCancel,
    BrNew(bool), BrField(&'static str, String), BrBank(Opt), BrAccount(Opt), BrContact(Opt), BrSave, BrDelete(i64),
    IeWhat(&'static str), IePick, IePicked(Result<Option<(String, String)>, String>), IePreview(Reply), IeRun, IeDate(String), IeExport(&'static str), IeExported(Result<String, String>),
    CrNew(bool), CrName(String), CrMonthly(bool), CrAdd(&'static str), CrLabel(usize, String), CrType(usize, &'static str), CrExpr(usize, String), CrSign(usize, bool), CrRemove(usize), CrSave, CrDelete(i64),
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
        Route::ImportExport => { app.data = Some(json!({})); Task::none() }
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
        // ---- common ----
        Msg::Done(r, what) => {
            app.extras.busy = false;
            match r {
                Ok(_) => {
                    app.extras.fa_new = None; app.extras.fa_dispose = None; app.extras.bud_cells.clear(); app.extras.cl_new = None; app.extras.br_new = None; app.extras.cr_new = None;
                    if app.route == Route::ImportExport { app.extras.ie_file = None; app.extras.ie_preview = None; }
                    let t1 = if what.is_empty() { Task::none() } else { app.toast(what, false) };
                    Task::batch([t1, app.load(), app.load_lookups()])
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
