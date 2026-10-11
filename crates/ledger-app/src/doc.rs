//! Shared editor for invoices, estimates/quotes, credit memos/notes, bills, vendor credits and
//! purchase orders. One screen model; the QuickBooks-style and Xero-style views arrange it differently.

use crate::app::{App, Message, Opt, Route};
use crate::icons::icon;
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD, DateStyle};
use crate::vocab;
use iced::widget::{Space, button, column, combo_box, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Fill, Length, Task};
use serde_json::{Value, json};

#[derive(Debug, Clone, Default)]
pub struct Line {
    pub item: Option<Opt>,
    pub description: String,
    pub qty: String,
    pub price: String,
    pub account: Option<Opt>,
    pub tax: Option<Opt>,
}

pub struct Editor {
    pub kind: String,
    pub id: Option<i64>,
    /// The saved document as the engine returned it (status, totals, payments…).
    pub doc: Option<Value>,
    pub contact: Option<Opt>,
    pub contact_text: String,
    pub contacts: combo_box::State<Opt>,
    pub date: String,
    pub due: String,
    pub reference: String,
    pub number: String,
    pub lat: String,
    pub lines: Vec<Line>,
    pub terms: String,
    pub busy: bool,
    /// Editing an approved document (Xero "Edit"; QuickBooks always allows editing open documents).
    pub editing: bool,
    pub pay_amount: String,
    pub pay_date: String,
    pub pay_account: Option<Opt>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Loaded(Result<Value, String>),
    Contact(Opt),
    ContactText(String),
    Date(String),
    Due(String),
    Reference(String),
    Number(String),
    Lat(LatOpt),
    Item(usize, Opt),
    Desc(usize, String),
    Qty(usize, String),
    Price(usize, String),
    Account(usize, Opt),
    Tax(usize, Opt),
    AddLine,
    RemoveLine(usize),
    Terms(String),
    /// Save; `true` = approve/post (QuickBooks "Save"), `false` = keep as draft.
    Save(bool),
    SaveAndNew,
    Saved(Result<Value, String>, bool),
    Edit,
    Action(&'static str),
    ActionDone(Result<Value, String>, &'static str),
    PayAmount(String),
    PayDate(String),
    PayAccount(Opt),
    AddPayment,
    Cancel,
}

/// How line amounts relate to tax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatOpt(pub &'static str);
impl std::fmt::Display for LatOpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 { "INCLUSIVE" => "Tax inclusive", "NOTAX" => "No tax", _ => "Tax exclusive" })
    }
}
const LATS: [LatOpt; 3] = [LatOpt("EXCLUSIVE"), LatOpt("INCLUSIVE"), LatOpt("NOTAX")];

/// Integer division rounding half away from zero (the engine's rounding rule).
fn div_round(n: i128, d: i128) -> i64 { let q = (n.abs() + d.abs() / 2) / d.abs(); (if (n < 0) != (d < 0) { -q } else { q }) as i64 }
/// Line amount in cents from e4 quantity × e4 price.
fn line_cents(q: i64, p: i64) -> i64 { div_round(q as i128 * p as i128, 1_000_000) }

fn sales(kind: &str) -> bool { matches!(kind, "ACCREC" | "QUOTE" | "ACCRECCREDIT") }
fn ledger_doc(kind: &str) -> bool { matches!(kind, "ACCREC" | "ACCPAY" | "ACCRECCREDIT" | "ACCPAYCREDIT") }

pub fn title_for(app: &App, kind: &str) -> &'static str {
    match kind { "ACCREC" => app.term("invoice"), "QUOTE" => app.term("quote"), "ACCRECCREDIT" => app.term("credit_note"), "ACCPAY" => app.term("bill"), "ACCPAYCREDIT" => app.term("supplier_credit"), "PO" => app.term("po"), _ => "Document" }
}

fn fmt_e4(v: i64) -> String {
    let neg = v < 0;
    let a = v.unsigned_abs();
    let mut s = format!("{}.{:04}", a / 10000, a % 10000);
    while s.ends_with('0') && s.len() - s.find('.').unwrap() > 3 { s.pop(); }
    if s.ends_with(".00") && a % 10000 == 0 { s.truncate(s.len() - 3); }
    if neg { format!("-{s}") } else { s }
}

/// Prices always show at least two decimals ("145.00", "42.5025").
fn fmt_price(v: i64) -> String { let s = fmt_e4(v); if s.contains('.') { s } else { format!("{s}.00") } }

/// Parse a quantity or price with up to 4 decimals into an e4 integer.
fn parse_e4(s: &str) -> Option<i64> {
    let s = s.trim().replace(',', "");
    if s.is_empty() { return Some(0); }
    let neg = s.starts_with('-');
    let s = s.trim_start_matches('-');
    let (w, f) = s.split_once('.').unwrap_or((s, ""));
    if !w.chars().all(|c| c.is_ascii_digit()) || !f.chars().all(|c| c.is_ascii_digit()) || f.len() > 4 { return None; }
    let v = w.parse::<i64>().unwrap_or(0) * 10000 + format!("{f:0<4}").parse::<i64>().ok()?;
    Some(if neg { -v } else { v })
}

impl Editor {
    fn new(app: &App, kind: &str) -> Editor {
        let style = app.date_style();
        let today = ui::today_iso();
        let mut e = Editor {
            kind: kind.to_string(), id: None, doc: None, contact: None, contact_text: String::new(),
            contacts: combo_box::State::new(app.lk.contact_opts(sales(kind))),
            date: ui::date(&today, style), due: if matches!(kind, "ACCREC" | "ACCPAY") { ui::date(&ui::add_days_iso(&today, 14), style) } else { String::new() },
            reference: String::new(), number: String::new(),
            lat: if app.country() == "US" { "EXCLUSIVE".into() } else { "EXCLUSIVE".into() },
            lines: vec![Line { qty: "1".into(), ..Default::default() }, Line { qty: "1".into(), ..Default::default() }],
            terms: String::new(), busy: false, editing: true,
            pay_amount: String::new(), pay_date: ui::date(&today, style), pay_account: app.lk.bank_opts().into_iter().next(), error: None,
        };
        if let Some(a) = default_account(app, kind) { for l in &mut e.lines { l.account = Some(a.clone()); l.tax = default_tax(app, Some(a.id)); } }
        e
    }

    fn editable(&self) -> bool {
        match self.doc.as_ref().and_then(|d| d["status"].as_str()) { None | Some("DRAFT") | Some("SUBMITTED") => true, Some("AUTHORISED") | Some("SENT") => self.editing, _ => false }
    }

    /// (subtotal, tax, total) preview in cents. The engine recalculates exactly on save.
    pub fn totals(&self, app: &App) -> (i64, i64, i64) {
        let mut sub = 0i64;
        let mut tax = 0i64;
        for l in &self.lines {
            let q = parse_e4(&l.qty).unwrap_or(0);
            let p = parse_e4(&l.price).unwrap_or(0);
            let amt = line_cents(q, p);
            let rate = if self.lat == "NOTAX" { 0 } else { app.lk.tax_rate_bp(l.tax.as_ref().map(|t| t.id)) };
            if self.lat == "INCLUSIVE" {
                let t = amt - div_round(amt as i128 * 10000, 10000 + rate as i128);
                sub += amt - t; tax += t;
            } else {
                sub += amt; tax += div_round(amt as i128 * rate as i128, 10000);
            }
        }
        (sub, tax, sub + tax)
    }

    fn payload(&self, app: &App, status: &str) -> Result<Value, String> {
        let style = app.date_style();
        let date = ui::to_iso(&self.date, style).ok_or_else(|| format!("Enter the date as {}", hint(style)))?;
        let due = if self.due.trim().is_empty() { None } else { Some(ui::to_iso(&self.due, style).ok_or_else(|| format!("Enter the due date as {}", hint(style)))?) };
        if self.contact.is_none() && self.contact_text.trim().is_empty() { return Err(format!("Choose a {}", if sales(&self.kind) { "customer".to_string() } else { app.term("supplier").to_lowercase() })); }
        let mut lines = vec![];
        for (n, l) in self.lines.iter().enumerate() {
            let empty = l.item.is_none() && l.description.trim().is_empty() && parse_e4(&l.price).unwrap_or(0) == 0;
            if empty { continue; }
            if parse_e4(&l.qty).is_none() { return Err(format!("Line {}: quantity isn't a number", n + 1)); }
            if parse_e4(&l.price).is_none() { return Err(format!("Line {}: price isn't a number", n + 1)); }
            lines.push(json!({"item_id": l.item.as_ref().map(|i| i.id), "description": l.description, "quantity": l.qty.replace(',', ""), "unit_price": l.price.replace(',', ""),
                "account_id": l.account.as_ref().map(|a| a.id), "tax_rate_id": l.tax.as_ref().map(|t| t.id)}));
        }
        if lines.is_empty() { return Err("Add at least one line".into()); }
        let mut doc = json!({"type": self.kind, "date": date, "due_date": due, "reference": self.reference, "line_amount_type": self.lat, "terms": self.terms, "lines": lines, "status": status});
        if self.kind == "QUOTE" { doc["expiry_date"] = doc["due_date"].take(); }
        if self.kind == "PO" { doc["delivery_date"] = doc["due_date"].take(); }
        if !self.number.trim().is_empty() { doc["number"] = json!(self.number.trim()); }
        match &self.contact { Some(c) => doc["contact_id"] = json!(c.id), None => doc["contact_name"] = json!(self.contact_text.trim()) }
        Ok(json!({"id": self.id, "doc": doc}))
    }
}

fn hint(s: DateStyle) -> &'static str { match s { DateStyle::Us => "MM/DD/YYYY", DateStyle::Intl => "DD/MM/YYYY", DateStyle::Long => "12 Oct 2026" } }

fn default_account(app: &App, kind: &str) -> Option<Opt> {
    let want = if sales(kind) { "REVENUE" } else { "EXPENSE" };
    let code = if sales(kind) { "200" } else { "" };
    app.lk.accounts.iter().find(|a| a["code"] == code && a["status"] == "ACTIVE").or_else(|| if sales(kind) { app.lk.accounts.iter().find(|a| a["class"] == want && a["status"] == "ACTIVE") } else { None })
        .map(|a| Opt { id: a["id"].as_i64().unwrap_or(0), label: format!("{} {}", a["code"].as_str().unwrap_or(""), a["name"].as_str().unwrap_or("")) })
}
fn default_tax(app: &App, account: Option<i64>) -> Option<Opt> {
    let tid = account.and_then(|id| app.lk.accounts.iter().find(|a| a["id"] == id)).and_then(|a| a["tax_rate_id"].as_i64())?;
    app.lk.tax_opts().into_iter().find(|t| t.id == tid)
}

pub fn open(app: &mut App, kind: String, id: Option<i64>) -> Task<Message> {
    app.doc = Some(Editor::new(app, &kind));
    if let Some(e) = app.doc.as_mut() { e.id = id; e.editing = id.is_none(); }
    match id {
        Some(id) => app.be.call("docs.get", json!({"id": id}), |r| Message::Doc(Msg::Loaded(r))),
        None => { app.data = Some(json!({})); Task::none() }
    }
}

fn fill_from(app: &App, e: &mut Editor, d: &Value) {
    let style = app.date_style();
    e.kind = d["type"].as_str().unwrap_or(&e.kind).to_string();
    e.contact = d["contact_id"].as_i64().map(|id| Opt { id, label: d["contact_name"].as_str().unwrap_or("").to_string() });
    e.contacts = combo_box::State::with_selection(app.lk.contact_opts(sales(&e.kind)), e.contact.as_ref());
    e.date = ui::date(d["date"].as_str().unwrap_or(""), style);
    let due = d["due_date"].as_str().or(d["expiry_date"].as_str()).or(d["delivery_date"].as_str()).unwrap_or("");
    e.due = if due.is_empty() { String::new() } else { ui::date(due, style) };
    e.reference = d["reference"].as_str().unwrap_or("").to_string();
    e.number = d["number"].as_str().unwrap_or("").to_string();
    e.lat = d["line_amount_type"].as_str().unwrap_or("EXCLUSIVE").to_string();
    e.terms = d["terms"].as_str().unwrap_or("").to_string();
    let taxes = app.lk.tax_opts();
    e.lines = d["lines"].as_array().cloned().unwrap_or_default().iter().map(|l| Line {
        item: l["item_id"].as_i64().map(|id| Opt { id, label: format!("{} — {}", l["item_code"].as_str().unwrap_or(""), app.lk.item(id).and_then(|i| i["name"].as_str()).unwrap_or("")) }),
        description: l["description"].as_str().unwrap_or("").to_string(),
        qty: fmt_e4(l["qty_e4"].as_i64().unwrap_or(0)),
        price: fmt_price(l["unit_e2"].as_i64().unwrap_or(0)),
        account: l["account_id"].as_i64().map(|id| Opt { id, label: format!("{} {}", l["account_code"].as_str().unwrap_or(""), l["account_name"].as_str().unwrap_or("")) }),
        tax: l["tax_rate_id"].as_i64().and_then(|id| taxes.iter().find(|t| t.id == id).cloned()),
    }).collect();
    let due_amt = d["amount_due"].as_i64().unwrap_or(0);
    e.pay_amount = if due_amt > 0 { ui::plain(due_amt) } else { String::new() };
    e.doc = Some(d.clone());
    e.editing = false;
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    let style = app.date_style();
    let Some(mut e) = app.doc.take() else { return Task::none() };
    let task = match m {
        Msg::Loaded(Ok(d)) => { fill_from(app, &mut e, &d); app.data = Some(json!({})); Task::none() }
        Msg::Loaded(Err(err)) => { app.error = Some(err); Task::none() }
        Msg::Contact(c) => {
            // Use the contact's defaults for the first line's account (QuickBooks/Xero both do this).
            e.contact_text = c.label.clone();
            e.contact = Some(c);
            Task::none()
        }
        Msg::ContactText(s) => { e.contact_text = s; e.contact = None; Task::none() }
        Msg::Date(s) => {
            // Keep the due date's offset (terms) when the date changes.
            if let (Some(old), Some(new), Some(due)) = (ui::to_iso(&e.date, style), ui::to_iso(&s, style), ui::to_iso(&e.due, style)) {
                let days = chrono::NaiveDate::parse_from_str(&due, "%Y-%m-%d").ok().zip(chrono::NaiveDate::parse_from_str(&old, "%Y-%m-%d").ok()).map(|(d, o)| (d - o).num_days());
                if let Some(days) = days { e.due = ui::date(&ui::add_days_iso(&new, days), style); }
            }
            e.date = s; Task::none()
        }
        Msg::Due(s) => { e.due = s; Task::none() }
        Msg::Reference(s) => { e.reference = s; Task::none() }
        Msg::Number(s) => { e.number = s; Task::none() }
        Msg::Lat(l) => { e.lat = l.0.to_string(); Task::none() }
        Msg::Item(i, it) => {
            if let Some(item) = app.lk.item(it.id).cloned() {
                let s = sales(&e.kind);
                let l = &mut e.lines[i];
                l.description = item[if s { "sales_description" } else { "purchase_description" }].as_str().filter(|x| !x.is_empty()).or(item["name"].as_str()).unwrap_or("").to_string();
                l.price = fmt_price(item[if s { "sale_unit_e2" } else { "purchase_unit_e2" }].as_i64().unwrap_or(0));
                if l.qty.trim().is_empty() { l.qty = "1".into(); }
                let acc = item[if s { "sale_account_id" } else { "purchase_account_id" }].as_i64();
                if let Some(a) = acc.and_then(|id| app.lk.account_opts(|_| true).into_iter().find(|o| o.id == id)) { l.account = Some(a); }
                let tax = item[if s { "sale_tax_rate_id" } else { "purchase_tax_rate_id" }].as_i64();
                if let Some(tx) = tax.and_then(|id| app.lk.tax_opts().into_iter().find(|o| o.id == id)) { l.tax = Some(tx); }
            }
            e.lines[i].item = Some(it);
            if i + 1 == e.lines.len() { e.lines.push(Line { qty: "1".into(), account: default_account(app, &e.kind), tax: default_tax(app, default_account(app, &e.kind).map(|a| a.id)), ..Default::default() }); }
            Task::none()
        }
        Msg::Desc(i, s) => { e.lines[i].description = s; Task::none() }
        Msg::Qty(i, s) => { e.lines[i].qty = s; Task::none() }
        Msg::Price(i, s) => {
            e.lines[i].price = s;
            if i + 1 == e.lines.len() { e.lines.push(Line { qty: "1".into(), account: default_account(app, &e.kind), tax: default_tax(app, default_account(app, &e.kind).map(|a| a.id)), ..Default::default() }); }
            Task::none()
        }
        Msg::Account(i, a) => { e.lines[i].tax = default_tax(app, Some(a.id)).or(e.lines[i].tax.take()); e.lines[i].account = Some(a); Task::none() }
        Msg::Tax(i, t) => { e.lines[i].tax = Some(t); Task::none() }
        Msg::AddLine => { e.lines.push(Line { qty: "1".into(), account: default_account(app, &e.kind), tax: default_tax(app, default_account(app, &e.kind).map(|a| a.id)), ..Default::default() }); Task::none() }
        Msg::RemoveLine(i) => { if e.lines.len() > 1 { e.lines.remove(i); } Task::none() }
        Msg::Terms(s) => { e.terms = s; Task::none() }
        Msg::Save(approve) => save(app, &mut e, approve, false),
        Msg::SaveAndNew => save(app, &mut e, true, true),
        Msg::Saved(r, new_after) => {
            e.busy = false;
            match r {
                Ok(v) => {
                    let id = v["id"].as_i64();
                    let label = title_for(app, &e.kind);
                    let t1 = app.toast(format!("{label} saved"), false);
                    let t2 = app.load_lookups();
                    let next = if new_after { Route::Doc { kind: e.kind.clone(), id: None } } else { Route::Doc { kind: e.kind.clone(), id } };
                    app.doc = Some(e);
                    let t3 = app.go(next);
                    return Task::batch([t1, t2, t3]);
                }
                Err(err) => { e.error = Some(err.clone()); app.toast(err, true) }
            }
        }
        Msg::Edit => { e.editing = true; Task::none() }
        Msg::Action(what) => {
            let Some(id) = e.id else { app.doc = Some(e); return Task::none() };
            e.busy = true;
            let (method, params) = match what {
                "void" => ("docs.setStatus", json!({"id": id, "status": "VOIDED"})),
                "delete" => ("docs.setStatus", json!({"id": id, "status": "DELETED"})),
                "approve" => ("docs.setStatus", json!({"id": id, "status": "AUTHORISED"})),
                "to_invoice" => ("docs.copy", json!({"id": id, "to": "ACCREC"})),
                "to_bill" => ("docs.copy", json!({"id": id, "to": "ACCPAY"})),
                "copy" => ("docs.copy", json!({"id": id, "to": e.kind})),
                "accept" => ("docs.setStatus", json!({"id": id, "status": "ACCEPTED"})),
                "sent" => ("docs.markSent", json!({"id": id})),
                _ => { app.doc = Some(e); return Task::none() }
            };
            app.be.call(method, params, move |r| Message::Doc(Msg::ActionDone(r, what)))
        }
        Msg::ActionDone(r, what) => {
            e.busy = false;
            match r {
                Ok(v) => {
                    let msg = match what { "void" => "Voided", "delete" => "Deleted", "approve" => "Approved", "to_invoice" => "Invoice created", "to_bill" => "Bill created", "copy" => "Copy created", "accept" => "Marked as accepted", "sent" => "Marked as sent", "pay" => "Payment recorded", _ => "Done" };
                    let t1 = app.toast(msg, false);
                    let next = match (what, v["id"].as_i64()) {
                        ("to_invoice", Some(id)) => Route::Doc { kind: "ACCREC".into(), id: Some(id) },
                        ("to_bill", Some(id)) => Route::Doc { kind: "ACCPAY".into(), id: Some(id) },
                        ("copy", Some(id)) => Route::Doc { kind: e.kind.clone(), id: Some(id) },
                        ("delete", _) => if sales(&e.kind) { Route::Invoices } else { Route::Bills },
                        _ => Route::Doc { kind: e.kind.clone(), id: e.id },
                    };
                    app.doc = Some(e);
                    if app.route == next { let t = app.load(); return Task::batch([t1, t]); }
                    let t2 = app.go(next);
                    return Task::batch([t1, t2]);
                }
                Err(err) => app.toast(err, true),
            }
        }
        Msg::PayAmount(s) => { e.pay_amount = s; Task::none() }
        Msg::PayDate(s) => { e.pay_date = s; Task::none() }
        Msg::PayAccount(a) => { e.pay_account = Some(a); Task::none() }
        Msg::AddPayment => {
            let (Some(id), Some(acc)) = (e.id, e.pay_account.clone()) else { app.doc = Some(e); return app.toast("Choose the account the money went to", true) };
            let Some(date) = ui::to_iso(&e.pay_date, style) else { app.doc = Some(e); return app.toast(format!("Enter the payment date as {}", hint(style)), true) };
            if ui::parse_money(&e.pay_amount).unwrap_or(0) <= 0 { app.doc = Some(e); return app.toast("Enter the amount paid", true); }
            e.busy = true;
            app.be.call("payments.add", json!({"document_id": id, "account_id": acc.id, "date": date, "amount": e.pay_amount.replace(',', ""), "reference": ""}), |r| Message::Doc(Msg::ActionDone(r, "pay")))
        }
        Msg::Cancel => { app.doc = Some(e); return app.update(Message::Back); }
    };
    app.doc = Some(e);
    task
}

fn save(app: &mut App, e: &mut Editor, approve: bool, new_after: bool) -> Task<Message> {
    if e.busy { return Task::none(); }
    let status = if approve { "AUTHORISED" } else { "DRAFT" };
    match e.payload(app, status) {
        Ok(p) => { e.busy = true; e.error = None; app.be.call("docs.save", p, move |r| Message::Doc(Msg::Saved(r, new_after))) }
        Err(err) => { e.error = Some(err.clone()); app.toast(err, true) }
    }
}

// ============================ view ============================

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(e) = &app.doc else { return ui::muted("…") };
    if let (Some(err), None) = (&app.error, &e.doc) {
        if e.id.is_some() { return ui::page(ui::h1(title_for(app, &e.kind)), vec![], ui::card(column![ui::h2("Couldn't open"), ui::muted(err.clone())].spacing(6))); }
    }
    if e.id.is_some() && e.doc.is_none() { return ui::page(ui::h1(title_for(app, &e.kind)), vec![], ui::muted("Loading…")); }
    let k = t();
    let editable = e.editable();
    let status = e.doc.as_ref().and_then(|d| d["status"].as_str()).unwrap_or("");
    let overdue = e.doc.as_ref().map(|d| d["overdue"].as_i64().unwrap_or(0) == 1).unwrap_or(false);
    let status_label = if status.is_empty() { "New" } else { vocab::status(app.skin, &e.kind, status, overdue) };
    let (sub, tax, total) = match (&e.doc, editable) { (Some(d), false) => (d["subtotal"].as_i64().unwrap_or(0), d["total_tax"].as_i64().unwrap_or(0), d["total"].as_i64().unwrap_or(0)), _ => e.totals(app) };
    let due_amt = e.doc.as_ref().map(|d| d["amount_due"].as_i64().unwrap_or(0)).unwrap_or(total);
    let name = title_for(app, &e.kind);
    let title = if e.number.is_empty() { format!("New {}", name.to_lowercase()) } else { format!("{name} {}", e.number) };
    let date_hint = hint(app.date_style());

    // ---- header fields ----
    let contact_label = if sales(&e.kind) { "Customer" } else { app.term("supplier") };
    let contact_field: Element<Message> = if editable {
        column![ui::label(contact_label), combo_box(&e.contacts, &format!("Choose a {}", contact_label.to_lowercase()), e.contact.as_ref(), |c| Message::Doc(Msg::Contact(c)))
            .on_input(|s| Message::Doc(Msg::ContactText(s))).padding([7, 10]).size(14).input_style(theme::input)].spacing(5).width(Fill).into()
    } else {
        column![ui::label(contact_label), ui::link(e.contact.as_ref().map(|c| c.label.clone()).unwrap_or_default(), e.contact.as_ref().map(|c| Message::Nav(Route::Contact(c.id))).unwrap_or(Message::Noop))].spacing(8).width(Fill).into()
    };
    let date_label = match e.kind.as_str() { "ACCREC" => "Invoice date", "QUOTE" => if app.skin == Skin::QuickBooks { "Estimate date" } else { "Date" }, _ => "Date" };
    let due_label = match e.kind.as_str() { "QUOTE" => "Expiration date", "PO" => "Delivery date", "ACCRECCREDIT" | "ACCPAYCREDIT" => "", _ => "Due date" };
    let field = |lbl: &str, ph: &str, val: &str, f: fn(String) -> Msg| -> Element<'_, Message> {
        if editable { ui::field_ph(lbl, ph, val, move |s| Message::Doc(f(s))) } else { column![ui::label(lbl.to_string()), ui::body(val.to_string())].spacing(8).width(Fill).into() }
    };
    let mut fields = row![contact_field].spacing(14);
    fields = fields.push(field(date_label, date_hint, &e.date, Msg::Date));
    if !due_label.is_empty() { fields = fields.push(field(due_label, date_hint, &e.due, Msg::Due)); }
    let mut fields2 = row![].spacing(14);
    fields2 = fields2.push(field(if app.skin == Skin::QuickBooks { if sales(&e.kind) { "Invoice no." } else { "Bill no." } } else { "Number" }, "Auto", &e.number, Msg::Number));
    fields2 = fields2.push(field(if app.skin == Skin::QuickBooks { "Ref no. / PO" } else { "Reference" }, "", &e.reference, Msg::Reference));
    fields2 = fields2.push(column![ui::label("Amounts are"), if editable {
        Element::from(pick_list(&LATS[..], Some(LatOpt(match e.lat.as_str() { "INCLUSIVE" => "INCLUSIVE", "NOTAX" => "NOTAX", _ => "EXCLUSIVE" })), |l| Message::Doc(Msg::Lat(l))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill))
    } else { ui::body(LatOpt(match e.lat.as_str() { "INCLUSIVE" => "INCLUSIVE", "NOTAX" => "NOTAX", _ => "EXCLUSIVE" }).to_string()) }].spacing(5).width(Fill));

    // ---- lines ----
    let qb_bill = app.skin == Skin::QuickBooks && !sales(&e.kind);
    let items = app.lk.item_opts();
    let accounts = app.lk.account_opts(|a| a["type"] != "BANK");
    let taxes = app.lk.tax_opts();
    // Columns: (title, portion)
    let cols: Vec<(&str, u16, bool)> = match (app.skin, qb_bill) {
        (Skin::QuickBooks, false) => vec![("#", 2, false), ("Product/service", 16, false), ("Description", 24, false), ("Qty", 6, true), ("Rate", 9, true), ("Amount", 10, true), ("Tax", 12, false), ("", 3, false)],
        (Skin::QuickBooks, true) => vec![("#", 2, false), ("Category", 18, false), ("Description", 26, false), ("Qty", 6, true), ("Rate", 9, true), ("Amount", 10, true), ("Tax", 12, false), ("", 3, false)],
        (Skin::Xero, _) => vec![("Item", 12, false), ("Description", 20, false), ("Qty", 6, true), ("Price", 8, true), ("Account", 15, false), ("Tax rate", 12, false), ("Amount", 9, true), ("", 3, false)],
    };
    let head = container(row(cols.iter().map(|(h, w, right)| {
        let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
        container(if *right { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
    })).spacing(8).padding([8, 10])).style(theme::header_row).width(Fill);
    let mut table = column![head].width(Fill);
    for (n, l) in e.lines.iter().enumerate() {
        let q = parse_e4(&l.qty).unwrap_or(0);
        let p = parse_e4(&l.price).unwrap_or(0);
        let amount = line_cents(q, p);
        let cell_in = |val: &str, f: fn(usize, String) -> Msg, right: bool| -> Element<'_, Message> {
            if editable {
                let ti = text_input("", val).on_input(move |s| Message::Doc(f(n, s))).padding([6, 6]).size(14).style(theme::cell_input);
                if right { ti.align_x(iced::alignment::Horizontal::Right).into() } else { ti.into() }
            } else if right { ui::num(val.to_string()) } else { ui::cell(val.to_string()) }
        };
        let pick = |opts: &Vec<Opt>, sel: &Option<Opt>, f: fn(usize, Opt) -> Msg, ph: &str| -> Element<'_, Message> {
            if editable {
                pick_list(opts.clone(), sel.clone(), move |o| Message::Doc(f(n, o))).placeholder(ph.to_string()).padding([6, 6]).text_size(13).style(theme::pick).width(Fill).into()
            } else { ui::cell(sel.as_ref().map(|o| o.label.clone()).unwrap_or_default()) }
        };
        let del: Element<Message> = if editable && e.lines.len() > 1 { button(icon("trash", 14.0, k.muted)).style(theme::ghost).padding(4).on_press(Message::Doc(Msg::RemoveLine(n))).into() } else { Space::new().into() };
        let cells: Vec<Element<Message>> = match (app.skin, qb_bill) {
            (Skin::QuickBooks, false) => vec![ui::cell_muted((n + 1).to_string()), pick(&items, &l.item, Msg::Item, "Choose"), cell_in(&l.description, Msg::Desc, false), cell_in(&l.qty, Msg::Qty, true), cell_in(&l.price, Msg::Price, true), ui::num(ui::money(amount)), pick(&taxes, &l.tax, Msg::Tax, "Tax"), del],
            (Skin::QuickBooks, true) => vec![ui::cell_muted((n + 1).to_string()), pick(&accounts, &l.account, Msg::Account, "Category"), cell_in(&l.description, Msg::Desc, false), cell_in(&l.qty, Msg::Qty, true), cell_in(&l.price, Msg::Price, true), ui::num(ui::money(amount)), pick(&taxes, &l.tax, Msg::Tax, "Tax"), del],
            (Skin::Xero, _) => vec![pick(&items, &l.item, Msg::Item, "Item"), cell_in(&l.description, Msg::Desc, false), cell_in(&l.qty, Msg::Qty, true), cell_in(&l.price, Msg::Price, true), pick(&accounts, &l.account, Msg::Account, "Account"), pick(&taxes, &l.tax, Msg::Tax, "Tax"), ui::num(ui::money(amount)), del],
        };
        let r = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(8).align_y(Alignment::Center).padding([4, 10]);
        table = table.push(column![r, ui::hr()]);
    }
    let table = container(table).style(theme::card).width(Fill).clip(true);
    let add_line: Element<Message> = if editable { row![ui::btn("Add lines", Some(Message::Doc(Msg::AddLine)))].into() } else { Space::new().into() };

    // ---- totals ----
    let tax_label = app.setting("tax_label");
    let tot_row = |l: String, v: i64, big: bool| -> Element<'_, Message> {
        row![text(l).size(if big { 17 } else { 14 }).font(if big { BOLD } else { iced::Font::DEFAULT }).width(Fill), text(ui::money(v)).size(if big { 17 } else { 14 }).font(if big { BOLD } else { iced::Font::DEFAULT })].into()
    };
    let mut totals = column![tot_row("Subtotal".into(), sub, false), tot_row(format!("Total {}", if tax_label.is_empty() { "tax".to_string() } else { tax_label.clone() }), tax, false), ui::hr(), tot_row("Total".into(), total, true)].spacing(8).width(320);
    if let Some(d) = &e.doc {
        for p in d["payments"].as_array().cloned().unwrap_or_default() { totals = totals.push(row![ui::muted(format!("Payment {}", app.d(p["date"].as_str().unwrap_or("")))), ui::hspace(), ui::muted(format!("-{}", ui::money(p["amount"].as_i64().unwrap_or(0))))]); }
        for a in d["allocations"].as_array().cloned().unwrap_or_default() { totals = totals.push(row![ui::muted(format!("Credit {}", a["credit_number"].as_str().unwrap_or(""))), ui::hspace(), ui::muted(format!("-{}", ui::money(a["amount"].as_i64().unwrap_or(0))))]); }
        if ledger_doc(&e.kind) { totals = totals.push(ui::hr()).push(tot_row(app.term("due").into(), due_amt, true)); }
    }
    let memo: Element<Message> = if editable {
        column![ui::label(if app.skin == Skin::QuickBooks { "Message displayed on invoice" } else { "Notes / terms" }), text_input("", &e.terms).on_input(|s| Message::Doc(Msg::Terms(s))).padding([7, 10]).size(14).style(theme::input)].spacing(5).width(Fill).into()
    } else if !e.terms.is_empty() { column![ui::label("Notes"), ui::body(e.terms.clone())].spacing(5).width(Fill).into() } else { Space::new().width(Fill).into() };
    let bottom = row![memo, Space::new().width(40), totals].align_y(Alignment::Start);

    // ---- payment panel (approved, unpaid) ----
    let pay_panel: Element<Message> = if !editable && status == "AUTHORISED" && ledger_doc(&e.kind) && due_amt > 0 {
        let banks = app.lk.bank_opts();
        ui::card(column![
            ui::h2(if sales(&e.kind) { app.term("receive_payment") } else { if app.skin == Skin::QuickBooks { "Pay bill" } else { "Make a payment" } }),
            row![
                ui::field("Amount", &e.pay_amount, |s| Message::Doc(Msg::PayAmount(s))),
                ui::field_ph("Payment date", date_hint, &e.pay_date, |s| Message::Doc(Msg::PayDate(s))),
                column![ui::label(if sales(&e.kind) { "Deposit to" } else { "Payment account" }), pick_list(banks, e.pay_account.clone(), |a| Message::Doc(Msg::PayAccount(a))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill),
                column![Space::new().height(18), ui::btn_primary(if sales(&e.kind) { "Record payment" } else { "Record payment" }, if e.busy { None } else { Some(Message::Doc(Msg::AddPayment)) })].width(iced::Shrink),
            ].spacing(12).align_y(Alignment::End),
        ].spacing(12))
    } else { Space::new().into() };

    // ---- actions ----
    let mut actions: Vec<Element<Message>> = vec![];
    let busy = e.busy;
    let m = |x: Msg| if busy { None } else { Some(Message::Doc(x)) };
    if editable {
        match app.skin {
            Skin::QuickBooks => {
                if e.doc.is_none() || status == "DRAFT" { actions.push(ui::btn("Save draft", m(Msg::Save(false)))); }
                actions.push(ui::btn("Save and new", m(Msg::SaveAndNew)));
                actions.push(ui::btn_primary(if e.kind == "QUOTE" || e.kind == "PO" { "Save" } else { "Save and close" }, m(Msg::Save(true))));
            }
            Skin::Xero => {
                actions.push(ui::btn("Save draft", m(Msg::Save(false))));
                actions.push(ui::btn_primary("Approve", m(Msg::Save(true))));
            }
        }
    } else {
        match (e.kind.as_str(), status) {
            ("QUOTE", "SENT" | "AUTHORISED" | "DRAFT") => { actions.push(ui::btn("Mark accepted", m(Msg::Action("accept")))); actions.push(ui::btn_primary(if app.skin == Skin::QuickBooks { "Create invoice" } else { "Convert to invoice" }, m(Msg::Action("to_invoice")))); }
            ("QUOTE", "ACCEPTED") => actions.push(ui::btn_primary(if app.skin == Skin::QuickBooks { "Create invoice" } else { "Convert to invoice" }, m(Msg::Action("to_invoice")))),
            ("PO", "AUTHORISED") => actions.push(ui::btn_primary("Copy to bill", m(Msg::Action("to_bill")))),
            _ => {}
        }
        if status == "AUTHORISED" && e.doc.as_ref().map(|d| d["amount_paid"].as_i64().unwrap_or(0) == 0 && d["amount_credited"].as_i64().unwrap_or(0) == 0).unwrap_or(false) {
            actions.push(ui::btn("Edit", m(Msg::Edit)));
        }
        actions.push(ui::btn("Copy", m(Msg::Action("copy"))));
        if matches!(status, "AUTHORISED" | "PAID") && ledger_doc(&e.kind) { actions.push(ui::btn_danger("Void", m(Msg::Action("void")))); }
    }
    if editable && e.id.is_some() && matches!(status, "DRAFT" | "SUBMITTED") { actions.insert(0, ui::btn_danger("Delete", m(Msg::Action("delete")))); }

    // ---- assemble per layout ----
    let k2 = k;
    let status_chip = ui::chip(status_label, crate::lists::status_color(status_label));
    let error: Element<Message> = match &e.error { Some(err) => container(text(err.clone()).size(13).color(k2.bad)).padding([8, 12]).style(theme::bad_soft).width(Fill).into(), None => Space::new().into() };
    let body: Element<Message> = match app.skin {
        Skin::QuickBooks => {
            // QuickBooks: big "balance due" on the right of the header card.
            let balance = column![text(if ledger_doc(&e.kind) { if sales(&e.kind) { "BALANCE DUE" } else { "BALANCE DUE" } } else { "TOTAL" }).size(12).font(BOLD).color(k.muted), text(ui::money(if e.doc.is_some() { due_amt } else { total })).size(30).font(BOLD)]
                .spacing(2).align_x(Alignment::End);
            let header = ui::card(column![row![column![fields].width(Fill), Space::new().width(30), balance].align_y(Alignment::Start), fields2].spacing(16));
            column![error, header, pay_panel, table, add_line, ui::card(bottom), footer_bar(actions)].spacing(14).into()
        }
        Skin::Xero => {
            let header = ui::card(column![fields, fields2].spacing(14));
            column![error, header, table, add_line, ui::card(bottom), pay_panel, row![ui::btn("Cancel", Some(Message::Doc(Msg::Cancel))), ui::hspace()].extend(actions).spacing(8)].spacing(14).into()
        }
    };
    let title_el = column![
        row![ui::link(if sales(&e.kind) { "Sales" } else { "Purchases" }, Message::Back)],
        row![ui::h1(title), status_chip].spacing(12).align_y(Alignment::Center),
    ].spacing(4);
    ui::page(title_el.into(), vec![], body)
}

/// QuickBooks-style sticky-looking footer bar with Cancel on the left and save actions on the right.
fn footer_bar<'a>(actions: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    container(row![ui::btn("Cancel", Some(Message::Doc(Msg::Cancel))), ui::hspace()].extend(actions).spacing(8).align_y(Alignment::Center))
        .padding([12, 16]).style(theme::topbar).width(Fill).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn e4_roundtrip() {
        assert_eq!(parse_e4("1"), Some(10000));
        assert_eq!(parse_e4("42.5"), Some(425000));
        assert_eq!(parse_e4("1,234.5678"), Some(12345678));
        assert_eq!(parse_e4("1.23456"), None);
        assert_eq!(fmt_e4(425000), "42.50");
        assert_eq!(fmt_e4(10000), "1");
        assert_eq!(fmt_e4(12345678), "1234.5678");
        assert_eq!(fmt_price(1450000), "145.00");
        assert_eq!(fmt_price(425025), "42.5025");
    }

    /// The editor's payload is accepted by the engine, and its preview totals match what's posted.
    #[test]
    fn invoice_round_trip_through_engine() {
        let dir = std::env::temp_dir().join(format!("lc-app-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (mut app, _) = App::boot(dir.clone(), None, None);
        app.be.call_now("orgs.create", json!({"org_name": "Test Co", "country": "AU", "demo": false, "owner_name": "Tess"})).unwrap();
        app.session = Some(app.be.call_now("session.get", json!({})).unwrap());
        let get = |m: &str, p: Value| app.be.call_now(m, p).unwrap().as_array().cloned().unwrap();
        app.lk = crate::app::Lookups { accounts: get("accounts.list", json!({})), tax_rates: get("taxRates.list", json!({})), items: get("items.list", json!({})), contacts: get("contacts.list", json!({"filter": "all"})) };
        let mut e = Editor::new(&app, "ACCREC");
        e.contact_text = "New Customer Pty".into();
        e.lines[0].description = "Consulting".into();
        e.lines[0].qty = "3".into();
        e.lines[0].price = "33.33".into();
        let gst = app.lk.tax_opts().into_iter().find(|t| t.label == "GST on Income").unwrap();
        e.lines[0].tax = Some(gst);
        let (sub, tax, total) = e.totals(&app);
        assert_eq!((sub, tax, total), (9999, 1000, 10999));
        let id = app.be.call_now("docs.save", e.payload(&app, "AUTHORISED").unwrap()).unwrap()["id"].as_i64().unwrap();
        let d = app.be.call_now("docs.get", json!({"id": id})).unwrap();
        assert_eq!(d["status"], "AUTHORISED");
        assert_eq!(d["total"].as_i64(), Some(total));
        assert_eq!(d["contact_name"], "New Customer Pty");
        // A bad date is reported in the layout's format rather than sent to the engine.
        e.date = "31/31/2026".into();
        assert!(e.payload(&app, "DRAFT").unwrap_err().contains("DD/MM/YYYY"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
