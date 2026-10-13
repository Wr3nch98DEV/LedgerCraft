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
}

#[derive(Debug, Clone)]
pub enum Msg {
    FaTab(&'static str), FaNew, FaField(&'static str, String), FaType(Opt), FaMethod(&'static str), FaSave, FaCancel, FaRegister(i64),
    FaDepDate(String), FaRun, FaDisposeOpen(i64), FaDisposeField(u8, String), FaDisposeProceeds(Opt), FaDisposeGain(Opt), FaDispose,
    AuditEntity(&'static str),
    FxEnable(String, bool), FxRateOpen(String), FxRateField(u8, String), FxRateSave, FxRevalDate(String), FxRevalue, FxFetch,
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
        // ---- common ----
        Msg::Done(r, what) => {
            app.extras.busy = false;
            match r {
                Ok(_) => {
                    app.extras.fa_new = None; app.extras.fa_dispose = None;
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
