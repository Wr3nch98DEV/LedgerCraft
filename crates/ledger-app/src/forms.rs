//! Smaller forms: new customer/vendor, new product or service, new account, journal entry, transfer.

use crate::app::{App, Message, Opt, Route};
use crate::icons::icon;
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD};
use iced::widget::{Space, checkbox, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Fill, Length, Task};
use serde_json::{Value, json};

#[derive(Debug, Clone, Default)]
pub struct JLine { pub account: Option<Opt>, pub description: String, pub debit: String, pub credit: String }

#[derive(Default)]
pub struct State {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub customer: bool,
    /// New US vendor: track payments for 1099.
    pub track_1099: bool,
    pub code: String,
    pub price: String,
    pub cost: String,
    pub sale_account: Option<Opt>,
    pub purchase_account: Option<Opt>,
    pub tax: Option<Opt>,
    pub service: bool,
    pub date: String,
    pub memo: String,
    pub lines: Vec<JLine>,
    pub busy: bool,
    pub from: Option<Opt>,
    pub to: Option<Opt>,
    pub amount: String,
    /// New account: QuickBooks account type name (or Xero type name) and detail type.
    pub acct_type: Option<String>,
    pub detail: Option<String>,
    /// Desktop New Customer/Vendor: name parts and address; "Save & New".
    pub first: String,
    pub last: String,
    pub address: String,
    pub new_after: bool,
    /// Desktop New Customer: price level (Payment Settings).
    pub price_level: Option<Opt>,
    /// Edit Customer/Vendor: the contact as saved (everything the form doesn't show is kept).
    pub orig: Option<Value>,
    /// New/Edit Item: an inventory part (tracked), its reorder point, and (editing) inactive.
    pub tracked: bool,
    pub reorder: String,
    pub inactive: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Name(String), Email(String), Phone(String), Code(String), Price(String), Cost(String),
    SaleAccount(Opt), PurchaseAccount(Opt), Tax(Opt), Service(bool), Track1099(bool), PriceLevel(Opt), EditLoaded(crate::backend::Reply), ItemType(String), Reorder(String), Inactive(bool), ItemLoaded(crate::backend::Reply), AccountLoaded(crate::backend::Reply),
    Date(String), Memo(String), From(Opt), To(Opt), Amount(String), First(String), Last(String), Address(String), SaveNew, AcctType(String), Detail(String), JAccount(usize, Opt), JDesc(usize, String), JDebit(usize, String), JCredit(usize, String), AddLine,
    Save, Saved(Result<Value, String>),
}

pub fn open(app: &mut App) -> Task<Message> {
    let mut s = State { customer: !matches!(app.route, Route::NewContact(false)), service: true, date: ui::date(&ui::today_iso(), app.date_style()), ..Default::default() };
    let find = |code: &str| app.lk.account_opts(|a| a["code"] == code).into_iter().next();
    s.sale_account = find("200");
    s.purchase_account = find("310").or_else(|| find("429"));
    s.lines = vec![JLine::default(), JLine::default()];
    let banks = app.lk.bank_opts();
    s.from = banks.first().cloned();
    s.to = banks.get(1).cloned();
    if app.skin == Skin::QuickBooks { s.acct_type = Some("Expenses".into()); s.detail = Some("Advertising/Promotional".into()); } else { s.acct_type = Some("Expense".into()); }
    app.form = Some(s);
    app.data = Some(json!({}));
    Task::none()
}

/// Edit Customer / Edit Vendor: load the contact into the form.
pub fn open_edit(app: &mut App, id: i64) -> Task<Message> {
    app.form = Some(State { service: true, date: ui::date(&ui::today_iso(), app.date_style()), busy: true, ..Default::default() });
    app.data = Some(json!({}));
    app.be.call("contacts.get", json!({"id": id}), |r| Message::Form(Msg::EditLoaded(r)))
}

/// Edit Item: load the item into the form.
pub fn open_edit_item(app: &mut App, id: i64) -> Task<Message> {
    app.form = Some(State { service: true, date: ui::date(&ui::today_iso(), app.date_style()), busy: true, ..Default::default() });
    app.data = Some(json!({}));
    app.be.call("items.get", json!({"id": id}), |r| Message::Form(Msg::ItemLoaded(r)))
}

/// Edit Account: load the account into the form.
pub fn open_edit_account(app: &mut App, id: i64) -> Task<Message> {
    app.form = Some(State { service: true, date: ui::date(&ui::today_iso(), app.date_style()), busy: true, ..Default::default() });
    app.data = Some(json!({}));
    app.be.call("accounts.get", json!({"id": id}), |r| Message::Form(Msg::AccountLoaded(r)))
}

/// An e4 quantity/price as typed text ("12.5", "89.00").
fn e4_text(v: Option<i64>) -> String {
    let Some(v) = v else { return String::new() };
    let s = format!("{:.4}", v as f64 / 10_000.0);
    let t = s.trim_end_matches('0');
    if t.ends_with('.') { format!("{t}00") } else if t.split('.').nth(1).is_some_and(|d| d.len() == 1) { format!("{t}0") } else { t.to_string() }
}

/// The full item for items.update: the saved item with the form's fields over it.
fn merged_item(orig: &Value, s: &State) -> Value {
    let str_of = |k: &str| orig[k].as_str().unwrap_or("").to_string();
    let name = s.name.trim();
    json!({"code": if s.code.trim().is_empty() { str_of("code") } else { s.code.trim().to_string() }, "name": name,
        "sales_description": if str_of("sales_description").is_empty() { name.to_string() } else { str_of("sales_description") }, "purchase_description": str_of("purchase_description"),
        "is_sold": orig["is_sold"].as_i64() != Some(0), "is_purchased": orig["is_purchased"].as_i64() == Some(1) || !s.cost.trim().is_empty() || s.tracked,
        "sale_price": s.price.replace(',', ""), "sale_account_id": s.sale_account.as_ref().map(|a| a.id), "sale_tax_rate_id": s.tax.as_ref().map(|x| x.id),
        "purchase_price": s.cost.replace(',', ""), "purchase_account_id": s.purchase_account.as_ref().map(|a| a.id), "purchase_tax_rate_id": orig["purchase_tax_rate_id"],
        "is_tracked": s.tracked, "inventory_account_id": orig["inventory_account_id"], "reorder_level": s.reorder.replace(',', "")})
}

/// The full contact for contacts.update: the saved record with the form's fields over it.
fn merged_contact(orig: &Value, s: &State) -> Value {
    let str_of = |k: &str| orig[k].as_str().unwrap_or("").to_string();
    let mut c = json!({});
    for k in ["mobile", "website", "tax_number", "account_number", "bank_account", "street_address", "default_currency", "sales_terms_type", "bill_terms_type", "notes"] { c[k] = json!(str_of(k)); }
    for k in ["sales_account_id", "purchase_account_id", "sales_tax_rate_id", "purchase_tax_rate_id", "sales_terms_days", "bill_terms_days", "credit_limit"] { c[k] = orig[k].clone(); }
    c["discount_bp"] = json!(orig["discount_bp"].as_i64().unwrap_or(0));
    c["people"] = json!(orig["people"].as_array().cloned().unwrap_or_default().iter().map(|p| json!({"first_name": p["first_name"].as_str().unwrap_or(""), "last_name": p["last_name"].as_str().unwrap_or(""),
        "email": p["email"].as_str().unwrap_or(""), "include_in_emails": p["include_in_emails"].as_i64().unwrap_or(0) == 1 || p["include_in_emails"].as_bool() == Some(true)})).collect::<Vec<_>>());
    c["groups"] = json!(orig["groups"].as_array().cloned().unwrap_or_default().iter().filter_map(|g| g["id"].as_i64()).collect::<Vec<_>>());
    c["name"] = json!(s.name.trim()); c["first_name"] = json!(s.first.trim()); c["last_name"] = json!(s.last.trim());
    c["email"] = json!(s.email.trim()); c["phone"] = json!(s.phone.trim()); c["postal_address"] = json!(s.address.trim());
    c
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    let style = app.date_style();
    let Some(mut s) = app.form.take() else { return Task::none() };
    let m = match m { Msg::SaveNew => { s.new_after = true; Msg::Save } Msg::Save => { s.new_after = false; Msg::Save } m => m };
    let task = match m {
        Msg::EditLoaded(r) => {
            s.busy = false;
            match r {
                Ok(c) => {
                    let st = |k: &str| c[k].as_str().unwrap_or("").to_string();
                    s.name = st("name"); s.first = st("first_name"); s.last = st("last_name"); s.email = st("email"); s.phone = st("phone"); s.address = st("postal_address");
                    s.customer = c["is_customer"].as_i64() == Some(1) || c["is_supplier"].as_i64() != Some(1);
                    s.track_1099 = c["track_1099"].as_i64() == Some(1);
                    s.inactive = c["status"] != "ACTIVE";
                    s.price_level = c["price_level_id"].as_i64().and_then(|id| app.lk.price_levels.iter().find(|p| p["id"].as_i64() == Some(id))).map(|p| Opt { id: p["id"].as_i64().unwrap_or(0), label: p["name"].as_str().unwrap_or("").to_string() });
                    s.orig = Some(c);
                    Task::none()
                }
                Err(e) => app.toast(e, true),
            }
        }
        Msg::ItemLoaded(r) => {
            s.busy = false;
            match r {
                Ok(it) => {
                    let st = |k: &str| it[k].as_str().unwrap_or("").to_string();
                    let opt = |id: Option<i64>| id.and_then(|id| app.lk.account_opts(|_| true).into_iter().find(|o| o.id == id));
                    s.name = st("name"); s.code = st("code");
                    s.price = e4_text(it["sale_unit_e2"].as_i64()); s.cost = e4_text(it["purchase_unit_e2"].as_i64());
                    s.sale_account = opt(it["sale_account_id"].as_i64()); s.purchase_account = opt(it["purchase_account_id"].as_i64());
                    s.tax = it["sale_tax_rate_id"].as_i64().and_then(|id| app.lk.tax_opts().into_iter().find(|o| o.id == id));
                    s.tracked = it["is_tracked"].as_i64() == Some(1);
                    s.service = !s.tracked && it["is_purchased"].as_i64() != Some(1);
                    s.reorder = e4_text(it["reorder_level_e4"].as_i64()).trim_end_matches(".00").to_string();
                    s.inactive = it["status"] != "ACTIVE";
                    s.orig = Some(it);
                    Task::none()
                }
                Err(e) => app.toast(e, true),
            }
        }
        Msg::AccountLoaded(r) => {
            s.busy = false;
            match r {
                Ok(a) => {
                    let st = |k: &str| a[k].as_str().unwrap_or("").to_string();
                    s.name = st("name"); s.code = st("code"); s.memo = st("description");
                    s.tax = a["tax_rate_id"].as_i64().and_then(|id| app.lk.tax_opts().into_iter().find(|o| o.id == id));
                    s.inactive = a["status"] != "ACTIVE";
                    s.orig = Some(a);
                    Task::none()
                }
                Err(e) => app.toast(e, true),
            }
        }
        Msg::ItemType(t) => { s.tracked = t == "Inventory Part"; s.service = t == "Service"; Task::none() }
        Msg::Reorder(v) => { s.reorder = v; Task::none() }
        Msg::Inactive(v) => { s.inactive = v; Task::none() }
        Msg::Name(v) => { s.name = v; Task::none() }
        Msg::Email(v) => { s.email = v; Task::none() }
        Msg::Phone(v) => { s.phone = v; Task::none() }
        Msg::Code(v) => { s.code = v; Task::none() }
        Msg::Price(v) => { s.price = v; Task::none() }
        Msg::Cost(v) => { s.cost = v; Task::none() }
        Msg::SaleAccount(a) => { s.sale_account = Some(a); Task::none() }
        Msg::PurchaseAccount(a) => { s.purchase_account = Some(a); Task::none() }
        Msg::Tax(x) => { s.tax = Some(x); Task::none() }
        Msg::Service(v) => { s.service = v; Task::none() }
        Msg::Track1099(v) => { s.track_1099 = v; Task::none() }
        Msg::PriceLevel(p) => { s.price_level = Some(p); Task::none() }
        Msg::Date(v) => { s.date = ui::date_keys(&s.date, &v, app.date_style()); Task::none() }
        Msg::From(v) => { s.from = Some(v); Task::none() }
        Msg::To(v) => { s.to = Some(v); Task::none() }
        Msg::Amount(v) => { s.amount = v; Task::none() }
        Msg::First(v) => { s.first = v; Task::none() }
        Msg::Last(v) => { s.last = v; Task::none() }
        Msg::Address(v) => { s.address = v; Task::none() }
        Msg::SaveNew => Task::none(),
        Msg::AcctType(v) => {
            // A new QuickBooks type starts on its first detail type.
            s.detail = crate::coa::QB_TYPES.iter().find(|t| t.name == v).map(|t| t.details[0].0.to_string());
            s.acct_type = Some(v);
            Task::none()
        }
        Msg::Detail(v) => { s.detail = Some(v); Task::none() }
        Msg::Memo(v) => { s.memo = v; Task::none() }
        Msg::JAccount(n, a) => { s.lines[n].account = Some(a); if n + 1 == s.lines.len() { s.lines.push(JLine::default()); } Task::none() }
        Msg::JDesc(n, v) => { s.lines[n].description = v; Task::none() }
        Msg::JDebit(n, v) => { s.lines[n].debit = v; if !s.lines[n].debit.trim().is_empty() { s.lines[n].credit.clear(); } Task::none() }
        Msg::JCredit(n, v) => { s.lines[n].credit = v; if !s.lines[n].credit.trim().is_empty() { s.lines[n].debit.clear(); } Task::none() }
        Msg::AddLine => { s.lines.push(JLine::default()); Task::none() }
        Msg::Save => {
            if s.busy { app.form = Some(s); return Task::none(); }
            let call = match &app.route {
                Route::EditAccount(id) => {
                    let Some(orig) = s.orig.clone() else { app.form = Some(s); return Task::none() };
                    if s.name.trim().is_empty() { app.form = Some(s); return app.toast("Enter the account name", true); }
                    let id = *id;
                    let status = if s.inactive { "ARCHIVED" } else { "ACTIVE" };
                    if orig["status"].as_str() != Some(status) {
                        if let Err(e) = app.be.call_now("accounts.setStatus", json!({"id": id, "status": status})) { app.form = Some(s); return app.toast(e, true); }
                    }
                    let b = |k: &str| orig[k].as_i64() == Some(1);
                    ("accounts.update", json!({"id": id, "account": {"code": s.code.trim(), "name": s.name.trim(), "type": orig["type"], "tax_rate_id": s.tax.as_ref().map(|t| t.id),
                        "description": s.memo.trim(), "enable_payments": b("enable_payments"), "show_in_expense_claims": b("show_in_expense_claims"), "dashboard_watch": b("dashboard_watch"),
                        "detail_type": orig["detail_type"]}}))
                }
                Route::EditItem(id) => {
                    let Some(orig) = s.orig.clone() else { app.form = Some(s); return Task::none() };
                    if s.name.trim().is_empty() { app.form = Some(s); return app.toast("Enter a name", true); }
                    let id = *id;
                    let status = if s.inactive { "ARCHIVED" } else { "ACTIVE" };
                    if orig["status"].as_str() != Some(status) { let _ = app.be.call_now("items.setStatus", json!({"id": id, "status": status})); }
                    ("items.update", json!({"id": id, "item": merged_item(&orig, &s)}))
                }
                Route::EditContact(id) => {
                    let Some(orig) = s.orig.clone() else { app.form = Some(s); return Task::none() };
                    if s.name.trim().is_empty() { app.form = Some(s); return app.toast("Enter a name", true); }
                    let id = *id;
                    // The price level is saved separately (it isn't part of the contact record).
                    let level = s.price_level.as_ref().map(|p| p.id).filter(|x| *x > 0);
                    if orig["price_level_id"].as_i64() != level && s.customer { let _ = app.be.call_now("contacts.setPriceLevel", json!({"id": id, "price_level_id": level})); }
                    let status = if s.inactive { "ARCHIVED" } else { "ACTIVE" };
                    if orig["status"].as_str() != Some(status) {
                        // Refused while there are open invoices or bills.
                        if let Err(e) = app.be.call_now("contacts.setStatus", json!({"id": id, "status": status})) { app.form = Some(s); return app.toast(e, true); }
                    }
                    if s.track_1099 != (orig["track_1099"].as_i64() == Some(1)) && !s.customer { let _ = app.be.call_now("contacts.set1099", json!({"id": id, "on": s.track_1099})); }
                    ("contacts.update", json!({"id": id, "contact": merged_contact(&orig, &s)}))
                }
                Route::NewContact(_) => {
                    if s.name.trim().is_empty() { app.form = Some(s); return app.toast("Enter a name", true); }
                    ("contacts.create", json!({"name": s.name.trim(), "email": s.email.trim(), "phone": s.phone.trim(), "is_customer": s.customer, "is_supplier": !s.customer,
                        "first_name": s.first.trim(), "last_name": s.last.trim(), "postal_address": s.address.trim(),
                        "price_level_id": s.price_level.as_ref().map(|p| p.id).filter(|id| *id > 0)}))
                }
                Route::Transfer => {
                    let (Some(f), Some(t)) = (s.from.clone(), s.to.clone()) else { app.form = Some(s); return app.toast("Choose both accounts", true) };
                    if f.id == t.id { app.form = Some(s); return app.toast("Choose two different accounts", true); }
                    let Some(date) = ui::to_iso(&s.date, style) else { app.form = Some(s); return app.toast("Check the date", true) };
                    if ui::parse_money(&s.amount).unwrap_or(0) <= 0 { app.form = Some(s); return app.toast("Enter the amount", true); }
                    ("transfers.save", json!({"from_account_id": f.id, "to_account_id": t.id, "date": date, "amount": s.amount.replace(',', ""), "reference": s.memo.trim()}))
                }
                Route::NewAccount => {
                    if s.name.trim().is_empty() { app.form = Some(s); return app.toast("Enter the account name", true); }
                    let ty = s.acct_type.clone().unwrap_or_default();
                    let (kind, detail, card) = match app.skin {
                        Skin::QuickBooks => {
                            let Some(t) = crate::coa::QB_TYPES.iter().find(|t| t.name == ty) else { app.form = Some(s); return app.toast("Choose an account type", true) };
                            let d = s.detail.clone().unwrap_or_default();
                            let kind = t.details.iter().find(|(n, _)| *n == d).map(|(_, k)| *k).unwrap_or(t.details[0].1);
                            (kind, d, t.credit_card)
                        }
                        Skin::Xero => (crate::coa::XERO_TYPES.iter().find(|(_, n)| *n == ty).map(|(k, _)| *k).unwrap_or("EXPENSE"), String::new(), false),
                    };
                    ("accounts.create", json!({"code": s.code.trim(), "name": s.name.trim(), "type": kind, "detail_type": detail, "description": s.memo.trim(),
                        "tax_rate_id": s.tax.as_ref().map(|x| x.id), "bank_type": if kind == "BANK" { Some(if card { "CREDITCARD" } else { "BANK" }) } else { None }}))
                }
                Route::NewProduct => {
                    if s.name.trim().is_empty() { app.form = Some(s); return app.toast("Enter a name", true); }
                    let code = if s.code.trim().is_empty() { s.name.trim().to_uppercase().chars().filter(|c| c.is_alphanumeric()).take(10).collect() } else { s.code.trim().to_string() };
                    ("items.create", json!({"code": code, "name": s.name.trim(), "sales_description": s.name.trim(), "is_sold": true, "is_purchased": !s.service || !s.cost.trim().is_empty(),
                        "is_tracked": s.tracked, "reorder_level": s.reorder.replace(',', ""),
                        "sale_price": s.price.replace(',', ""), "sale_account_id": s.sale_account.as_ref().map(|a| a.id), "sale_tax_rate_id": s.tax.as_ref().map(|x| x.id),
                        "purchase_price": s.cost.replace(',', ""), "purchase_account_id": s.purchase_account.as_ref().map(|a| a.id)}))
                }
                _ => {
                    let Some(date) = ui::to_iso(&s.date, style) else { app.form = Some(s); return app.toast("Check the journal date", true) };
                    let lines: Vec<Value> = s.lines.iter().filter(|l| l.account.is_some()).map(|l| json!({"account_id": l.account.as_ref().map(|a| a.id), "description": l.description,
                        "debit": l.debit.replace(',', ""), "credit": l.credit.replace(',', "")})).collect();
                    let (dr, cr) = totals(&s);
                    if dr != cr || dr == 0 { app.form = Some(s); return app.toast("Debits and credits must be equal", true); }
                    ("mj.save", json!({"id": null, "journal": {"date": date, "narration": if s.memo.trim().is_empty() { "Journal entry" } else { s.memo.trim() }, "line_amount_type": "NOTAX", "status": "POSTED", "lines": lines}}))
                }
            };
            s.busy = true;
            app.be.call(call.0, call.1, |r| Message::Form(Msg::Saved(r)))
        }
        Msg::Saved(r) => {
            s.busy = false;
            match r {
                Ok(v) => {
                    let (msg, next) = match &app.route {
                        Route::NewContact(c) => ("Saved", v["id"].as_i64().map(Route::Contact).unwrap_or(if *c { Route::Customers } else { Route::Vendors })),
                        Route::EditContact(id) => ("Changes saved", Route::Contact(*id)),
                        Route::EditItem(_) => ("Item saved", Route::Products),
                        Route::EditAccount(_) => ("Account saved", Route::Chart),
                        Route::NewProduct => ("Product saved", Route::Products),
                        Route::NewAccount => ("Account created", Route::Chart),
                        Route::Transfer => ("Transfer recorded", Route::Banking(None)),
                        _ => ("Journal entry posted", Route::Report("journal-report".into())),
                    };
                    let t1 = app.toast(msg, false);
                    let t2 = app.load_lookups();
                    let t0 = match (&app.route, v["id"].as_i64()) {
                        (Route::NewContact(false), Some(cid)) if s.track_1099 => app.be.call("contacts.set1099", json!({"id": cid, "on": true}), |_| Message::Noop),
                        _ => Task::none(),
                    };
                    if app.desktop() {
                        // Desktop: Save & Close closes the window, Save & New starts again.
                        let t3 = if s.new_after { app.load() } else { let i = app.desk.active; app.close_window(i) };
                        return Task::batch([t0, t1, t2, t3]);
                    }
                    app.form = Some(s);
                    let t3 = app.go(next);
                    return Task::batch([t0, t1, t2, t3]);
                }
                Err(e) => app.toast(e, true),
            }
        }
    };
    app.form = Some(s);
    task
}

pub(crate) fn totals(s: &State) -> (i64, i64) {
    s.lines.iter().fold((0, 0), |(d, c), l| (d + ui::parse_money(&l.debit).unwrap_or(0), c + ui::parse_money(&l.credit).unwrap_or(0)))
}

fn footer<'a>(busy: bool) -> Element<'a, Message> {
    container(row![ui::btn("Cancel", Some(Message::Back)), ui::hspace(), ui::btn_primary("Save", if busy { None } else { Some(Message::Form(Msg::Save)) })].spacing(8))
        .padding([12, 16]).style(theme::topbar).width(Fill).into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(s) = &app.form else { return ui::muted("…") };
    if app.desktop() && !matches!(app.route, Route::NewAccount) { return crate::formdesk::view(app, s); }
    let k = t();
    match &app.route {
        Route::NewContact(customer) => {
            let title = if *customer { "New customer" } else if app.skin == Skin::QuickBooks { "New vendor" } else { "New supplier" };
            ui::page(ui::h1(title), vec![], column![
                ui::card(column![
                    ui::field(if *customer { "Customer display name" } else { "Company or name" }, &s.name, |v| Message::Form(Msg::Name(v))),
                    row![ui::field("Email", &s.email, |v| Message::Form(Msg::Email(v))), ui::field("Phone", &s.phone, |v| Message::Form(Msg::Phone(v)))].spacing(14),
                    if !customer && app.country() == "US" { Element::from(checkbox(s.track_1099).label("Track payments for 1099").on_toggle(|v| Message::Form(Msg::Track1099(v))).text_size(14)) } else { Space::new().into() },
                ].spacing(14)),
                footer(s.busy),
            ].spacing(14).into())
        }
        Route::Transfer => {
            let banks = app.lk.bank_opts();
            let pick = |label: &str, sel: &Option<Opt>, f: fn(Opt) -> Msg| -> Element<'_, Message> {
                column![ui::label(label.to_string()), pick_list(banks.clone(), sel.clone(), move |o| Message::Form(f(o))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill).into()
            };
            ui::page(ui::h1("Transfer"), vec![], column![
                ui::card(column![
                    row![pick("Transfer funds from", &s.from, Msg::From), pick("Transfer funds to", &s.to, Msg::To)].spacing(14),
                    row![ui::field("Transfer amount", &s.amount, |v| Message::Form(Msg::Amount(v))), ui::field("Date", &s.date, |v| Message::Form(Msg::Date(v))), ui::field("Memo", &s.memo, |v| Message::Form(Msg::Memo(v)))].spacing(14),
                ].spacing(14)),
                footer(s.busy),
            ].spacing(14).into())
        }
        Route::NewAccount => {
            let qb = app.skin == Skin::QuickBooks;
            let types: Vec<String> = if qb { crate::coa::QB_TYPES.iter().map(|t| t.name.to_string()).collect() } else { crate::coa::XERO_TYPES.iter().map(|(_, n)| n.to_string()).collect() };
            let details: Vec<String> = crate::coa::QB_TYPES.iter().find(|t| Some(t.name.to_string()) == s.acct_type).map(|t| t.details.iter().map(|(d, _)| d.to_string()).collect()).unwrap_or_default();
            let taxes = app.lk.tax_opts();
            let type_pick = column![ui::label("Account type"), pick_list(types, s.acct_type.clone(), |v| Message::Form(Msg::AcctType(v))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill);
            let mut first = row![type_pick].spacing(14);
            if qb { first = first.push(column![ui::label("Detail type"), pick_list(details, s.detail.clone(), |v| Message::Form(Msg::Detail(v))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill)); }
            let tax = column![ui::label(if qb { "Default tax code" } else { "Tax" }), pick_list(taxes, s.tax.clone(), |x| Message::Form(Msg::Tax(x))).placeholder("None").padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill);
            ui::page(ui::h1(if qb { "New account" } else { "Add new account" }), vec![], column![
                ui::card(column![
                    first,
                    row![ui::field("Name", &s.name, |v| Message::Form(Msg::Name(v))), ui::field_ph(if qb { "Number" } else { "Code" }, "Leave blank to number it for you", &s.code, |v| Message::Form(Msg::Code(v)))].spacing(14),
                    row![ui::field_ph("Description", "Optional", &s.memo, |v| Message::Form(Msg::Memo(v))), tax].spacing(14),
                ].spacing(14)),
                footer(s.busy),
            ].spacing(14).into())
        }
        Route::NewProduct => {
            let income = app.lk.account_opts(|a| a["class"] == "REVENUE");
            let expense = app.lk.account_opts(|a| a["class"] == "EXPENSE" || a["type"] == "DIRECTCOSTS");
            let taxes = app.lk.tax_opts();
            let pick = |label: &str, opts: Vec<Opt>, sel: &Option<Opt>, f: fn(Opt) -> Msg| -> Element<'_, Message> {
                column![ui::label(label.to_string()), pick_list(opts, sel.clone(), move |o| Message::Form(f(o))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill).into()
            };
            ui::page(ui::h1(if app.skin == Skin::QuickBooks { "Product/service information" } else { "New item" }), vec![], column![
                ui::card(column![
                    row![ui::field("Name", &s.name, |v| Message::Form(Msg::Name(v))), ui::field_ph("SKU / code", "Optional", &s.code, |v| Message::Form(Msg::Code(v)))].spacing(14),
                    checkbox(s.service).label("This is a service (no stock)").on_toggle(|v| Message::Form(Msg::Service(v))).text_size(14),
                    row![ui::field("Sales price / rate", &s.price, |v| Message::Form(Msg::Price(v))), pick("Income account", income, &s.sale_account, Msg::SaleAccount), pick("Sales tax", taxes, &s.tax, Msg::Tax)].spacing(14),
                    row![ui::field_ph("Cost", "Optional", &s.cost, |v| Message::Form(Msg::Cost(v))), pick("Expense account", expense, &s.purchase_account, Msg::PurchaseAccount), Space::new().width(Fill)].spacing(14),
                    checkbox(s.tracked).label(if app.skin == Skin::QuickBooks { "Track quantity on hand (inventory)" } else { "Track this item (inventory)" }).on_toggle(|v| Message::Form(Msg::ItemType(if v { "Inventory Part".into() } else { "Non-inventory Part".into() }))).text_size(14),
                    if s.tracked { Element::from(row![ui::field_ph("Reorder point", "Optional", &s.reorder, |v| Message::Form(Msg::Reorder(v))), Space::new().width(Fill), Space::new().width(Fill)].spacing(14)) } else { Space::new().into() },
                ].spacing(14)),
                footer(s.busy),
            ].spacing(14).into())
        }
        _ => {
            let accounts = app.lk.account_opts(|_| true);
            let (dr, cr) = totals(s);
            let cols: Vec<(&str, u16, bool)> = vec![("#", 2, false), ("Account", 24, false), ("Debits", 10, true), ("Credits", 10, true), ("Description", 26, false)];
            let head = container(row(cols.iter().map(|(h, w, r)| {
                let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
                container(if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
            })).spacing(8).padding([8, 10])).style(theme::header_row).width(Fill);
            let mut table = column![head];
            for (n, l) in s.lines.iter().enumerate() {
                let cells: Vec<Element<Message>> = vec![
                    ui::cell_muted((n + 1).to_string()),
                    pick_list(accounts.clone(), l.account.clone(), move |a| Message::Form(Msg::JAccount(n, a))).placeholder("Choose an account").padding([6, 6]).text_size(13).style(theme::pick).width(Fill).into(),
                    text_input("", &l.debit).on_input(move |v| Message::Form(Msg::JDebit(n, v))).padding([6, 6]).size(14).align_x(iced::alignment::Horizontal::Right).style(theme::cell_input).into(),
                    text_input("", &l.credit).on_input(move |v| Message::Form(Msg::JCredit(n, v))).padding([6, 6]).size(14).align_x(iced::alignment::Horizontal::Right).style(theme::cell_input).into(),
                    text_input("", &l.description).on_input(move |v| Message::Form(Msg::JDesc(n, v))).padding([6, 6]).size(14).style(theme::cell_input).into(),
                ];
                let r = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(8).align_y(Alignment::Center).padding([4, 10]);
                table = table.push(column![r, ui::hr()]);
            }
            let balanced = dr == cr && dr != 0;
            let total_row = row![
                container(text("Total").size(14).font(BOLD)).width(Length::FillPortion(26)),
                container(ui::num(ui::money(dr))).width(Length::FillPortion(10)),
                container(ui::num(ui::money(cr))).width(Length::FillPortion(10)),
                container(if balanced { row![icon("check", 14.0, k.ok), text("Balanced").size(13).color(k.ok)].spacing(6).into() }
                    else if dr == 0 && cr == 0 { Element::from(text("Enter debits and credits").size(13).color(k.muted)) }
                    else { Element::from(text(format!("Out of balance by {}", ui::money((dr - cr).abs()))).size(13).color(k.bad)) }).width(Length::FillPortion(26)),
            ].spacing(8).padding([8, 10]);
            ui::page(ui::h1(app.term("journal")), vec![], column![
                ui::card(row![ui::field_ph("Journal date", "", &s.date, |v| Message::Form(Msg::Date(v))), ui::field("Memo / narration", &s.memo, |v| Message::Form(Msg::Memo(v)))].spacing(14)),
                container(column![table, total_row]).style(theme::card).width(Fill).clip(true),
                row![ui::btn("Add lines", Some(Message::Form(Msg::AddLine)))],
                footer(s.busy),
            ].spacing(14).into())
        }
    }
}


#[cfg(test)]
mod tests {
    use serde_json::json;
    /// The payloads the contact, product and journal forms send are accepted by the engine.
    #[test]
    fn form_payloads_accepted() {
        let dir = std::env::temp_dir().join(format!("lc-forms-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let be = crate::backend::Backend::new(dir.clone());
        be.call_now("orgs.create", json!({"org_name": "T", "country": "US", "demo": false, "owner_name": "T"})).unwrap();
        let accts = be.call_now("accounts.list", json!({})).unwrap();
        let id = |code: &str| accts.as_array().unwrap().iter().find(|a| a["code"] == code).unwrap()["id"].as_i64().unwrap();
        let c = be.call_now("contacts.create", json!({"name": "Acme", "email": "a@b.test", "phone": "1", "is_customer": true, "is_supplier": false})).unwrap();
        assert!(c["id"].as_i64().is_some());
        let it = be.call_now("items.create", json!({"code": "WIDGET", "name": "Widget", "sales_description": "Widget", "is_sold": true, "is_purchased": false,
            "sale_price": "12.50", "sale_account_id": id("200"), "sale_tax_rate_id": null, "purchase_price": "", "purchase_account_id": null})).unwrap();
        assert!(it["id"].as_i64().is_some());
        let mj = be.call_now("mj.save", json!({"id": null, "journal": {"date": "2026-10-01", "narration": "Accrual", "line_amount_type": "NOTAX", "status": "POSTED",
            "lines": [{"account_id": id("429"), "description": "", "debit": "100", "credit": ""}, {"account_id": id("200"), "description": "", "debit": "", "credit": "100"}]}})).unwrap();
        assert!(mj["id"].as_i64().is_some());
        let tb = be.call_now("reports.run", json!({"name": "trial-balance", "params": {"date": "2026-12-31"}})).unwrap();
        assert_eq!(tb["balanced"], true);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
