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
    /// Desktop bills: line belongs to the Items tab (else the Expenses tab).
    pub on_items: bool,
}

pub struct Editor {
    pub kind: String,
    /// Desktop grid: type-ahead boxes for each line's item and account (QuickFill), and the
    /// (item, account) per line they were built for.
    pub item_boxes: Vec<combo_box::State<Opt>>,
    pub acct_boxes: Vec<combo_box::State<Opt>>,
    pub box_sig: Vec<(Option<i64>, Option<i64>)>,
    /// Form template (branding theme); None = the default.
    pub template: Option<Opt>,
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
    /// Email the document once the save succeeds ("Save and send").
    pub send_after: bool,
    /// Save as "Awaiting approval" (Xero's Save ▸ Submit for approval).
    pub submit: bool,
    /// The user chose to approve over the customer's credit limit.
    pub allow_over: bool,
    /// Progress invoice: the percentage being entered (None = closed).
    pub progress: Option<String>,
    /// QuickBooks invoices: the customer's open delayed charges/credits, and those added.
    pub delayed: Vec<Value>,
    pub added: Vec<i64>,
    /// Desktop bills: Items tab showing (else Expenses).
    pub items_tab: bool,
    /// Desktop "Save & Close": close the window once saved.
    pub close_after: bool,
    /// Desktop history panel: the customer's/vendor's recent transactions.
    pub history: Vec<Value>,
    /// Desktop "Memorize": the schedule panel.
    pub memo_open: bool,
    pub memo_freq: &'static str,
    pub memo_next: String,
    pub memo_auto: bool,
    /// Apply Credits: open documents on the other side (credits for an invoice/bill, or
    /// invoices/bills for a credit) as (document, amount to apply).
    pub apply: Option<Vec<(Value, String)>>,
    /// Desktop: the Give Refund / pay panel is open.
    pub refund_open: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Loaded(Result<Value, String>),
    Contact(Opt),
    ContactText(String),
    Date(String),
    Due(String),
    Reference(String),
    Template(Opt),
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
    /// Save the document as a PDF and open it (to view or print).
    Print,
    Printed(Result<String, String>),
    SaveAndSend,
    /// Acumatica record toolbar: go to the first / previous / next / last document of this type.
    Step(&'static str),
    /// Xero: save for someone else to approve.
    Submit,
    /// Approve anyway, over the customer's credit limit.
    OverLimit,
    /// Progress invoicing on an estimate: open the box, the percentage, create the invoice.
    ProgressOpen, ProgressPct(String), ProgressCreate,
    Send,
    Sent(Result<Value, String>),
    Delayed(Result<Value, String>),
    AddDelayed(i64),
    /// Desktop bills: switch between the Expenses and Items tabs.
    BillTab(bool),
    /// Desktop: save, then close the window.
    SaveClose,
    /// Desktop history panel data.
    History(Result<Value, String>),
    /// Desktop Memorize panel.
    RefundOpen,
    /// Attach a file to the saved document / open an attachment.
    Attach, Attached(Result<Option<String>, String>), OpenAttachment(i64),
    ApplyOpen, ApplyDocs(Result<Value, String>), ApplyAmt(usize, String), ApplySave, Applied(Result<Value, String>),
    Memorize, MemoFreq(&'static str), MemoNext(String), MemoAuto(bool), MemoSave, Memorized(Result<Value, String>),
    /// Desktop Expenses tab: an amount typed straight in (quantity 1).
    Amount(usize, String),
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
pub(crate) fn div_round(n: i128, d: i128) -> i64 { let q = (n.abs() + d.abs() / 2) / d.abs(); (if (n < 0) != (d < 0) { -q } else { q }) as i64 }
/// Line amount in cents from e4 quantity × e4 price.
pub(crate) fn line_cents(q: i64, p: i64) -> i64 { div_round(q as i128 * p as i128, 1_000_000) }

/// Standard base64 (for sending file contents to the engine).
pub fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// A file's MIME type from its extension.
pub fn mime_for(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "pdf" => "application/pdf", "png" => "image/png", "jpg" | "jpeg" => "image/jpeg", "gif" => "image/gif", "txt" => "text/plain", "csv" => "text/csv",
        "doc" => "application/msword", "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel", "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", _ => "application/octet-stream",
    }
}

/// What a document's credit is applied against: invoices ↔ credit memos, bills ↔ vendor credits.
pub fn apply_other(kind: &str) -> Option<&'static str> {
    match kind { "ACCREC" => Some("ACCRECCREDIT"), "ACCRECCREDIT" => Some("ACCREC"), "ACCPAY" => Some("ACCPAYCREDIT"), "ACCPAYCREDIT" => Some("ACCPAY"), _ => None }
}

/// Memorize "How Often" choices → engine schedule (unit, count).
pub const MEMO_FREQS: [&str; 5] = ["Weekly", "Every two weeks", "Monthly", "Quarterly", "Annually"];
pub fn memo_schedule(f: &str) -> (&'static str, i64) {
    match f { "Weekly" => ("WEEKLY", 1), "Every two weeks" => ("WEEKLY", 2), "Quarterly" => ("MONTHLY", 3), "Annually" => ("MONTHLY", 12), _ => ("MONTHLY", 1) }
}

pub(crate) fn sales(kind: &str) -> bool { matches!(kind, "ACCREC" | "QUOTE" | "ACCRECCREDIT" | "SALESRECEIPT" | "DELAYEDCHARGE" | "DELAYEDCREDIT" | "SALESORDER") }
pub(crate) fn delayed(kind: &str) -> bool { matches!(kind, "DELAYEDCHARGE" | "DELAYEDCREDIT") }
/// Non-posting documents stored as quotes with a subtype (delayed charges/credits, sales orders).
pub(crate) fn quote_variant(kind: &str) -> bool { delayed(kind) || kind == "SALESORDER" }
pub(crate) fn ledger_doc(kind: &str) -> bool { matches!(kind, "ACCREC" | "ACCPAY" | "ACCRECCREDIT" | "ACCPAYCREDIT") }

pub fn title_for(app: &App, kind: &str) -> &'static str {
    match kind { "SALESRECEIPT" => "Sales receipt", "DELAYEDCHARGE" => "Delayed charge", "DELAYEDCREDIT" => "Delayed credit", "SALESORDER" => "Sales order", "ACCREC" => app.term("invoice"), "QUOTE" => app.term("quote"), "ACCRECCREDIT" => app.term("credit_note"), "ACCPAY" => app.term("bill"), "ACCPAYCREDIT" => app.term("supplier_credit"), "PO" => app.term("po"), _ => "Document" }
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
pub(crate) fn parse_e4(s: &str) -> Option<i64> {
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
            kind: kind.to_string(), template: None, item_boxes: vec![], acct_boxes: vec![], box_sig: vec![], id: None, doc: None, contact: None, contact_text: String::new(),
            contacts: combo_box::State::new(app.lk.contact_opts(sales(kind))),
            date: ui::date(&today, style), due: if matches!(kind, "ACCREC" | "ACCPAY") { ui::date(&ui::add_days_iso(&today, 14), style) } else { String::new() },
            reference: String::new(), number: String::new(),
            lat: if app.country() == "US" { "EXCLUSIVE".into() } else { "EXCLUSIVE".into() },
            lines: vec![Line { qty: "1".into(), ..Default::default() }, Line { qty: "1".into(), ..Default::default() }],
            terms: String::new(), busy: false, editing: true,
            pay_amount: String::new(), pay_date: ui::date(&today, style), pay_account: app.lk.bank_opts().into_iter().next(), error: None, send_after: false, submit: false, allow_over: false, progress: None,
            delayed: vec![], added: vec![], items_tab: false, close_after: false, history: vec![], memo_open: false, memo_freq: "Monthly", memo_next: String::new(), memo_auto: false, apply: None, refund_open: false,
        };
        if let Some(a) = default_account(app, kind) { for l in &mut e.lines { l.account = Some(a.clone()); l.tax = default_tax(app, Some(a.id)); } }
        e
    }

    pub fn editable(&self) -> bool {
        if quote_variant(&self.kind) { return self.doc.as_ref().map(|d| d["status"] == "SENT" && self.editing).unwrap_or(true); }
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

    pub fn payload(&self, app: &App, status: &str) -> Result<Value, String> {
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
        let kind = match self.kind.as_str() { "SALESRECEIPT" => "ACCREC", k if quote_variant(k) => "QUOTE", k => k };
        let mut doc = json!({"type": kind, "date": date, "due_date": due, "reference": self.reference, "line_amount_type": self.lat, "terms": self.terms, "lines": lines, "status": status, "allow_over_limit": self.allow_over});
        if self.kind == "QUOTE" { doc["expiry_date"] = doc["due_date"].take(); }
        if self.kind == "PO" { doc["delivery_date"] = doc["due_date"].take(); }
        if !self.number.trim().is_empty() { doc["number"] = json!(self.number.trim()); }
        if let Some(t) = &self.template { doc["branding_theme_id"] = json!(t.id); }
        match &self.contact { Some(c) => doc["contact_id"] = json!(c.id), None => doc["contact_name"] = json!(self.contact_text.trim()) }
        let mut p = json!({"id": self.id, "doc": doc});
        if !self.added.is_empty() { p["close_delayed"] = json!(self.added); }
        if delayed(&self.kind) { p["credit"] = json!(self.kind == "DELAYEDCREDIT"); }
        Ok(p)
    }
}

pub(crate) fn hint(s: DateStyle) -> &'static str { match s { DateStyle::Us => "MM/DD/YYYY", DateStyle::Intl => "DD/MM/YYYY", DateStyle::Long => "12 Oct 2026" } }

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

/// Desktop: load the chosen customer's/vendor's recent transactions for the history panel.
fn history_task(app: &App, e: &Editor) -> Task<Message> {
    let Some(c) = e.contact.as_ref().filter(|_| app.desktop()) else { return Task::none() };
    let kind = if sales(&e.kind) { "SALES" } else { "ACCPAY" };
    app.be.call("docs.list", json!({"type": kind, "contact_id": c.id, "limit": 6}), |r| Message::Doc(Msg::History(r)))
}

pub fn open(app: &mut App, kind: String, id: Option<i64>) -> Task<Message> {
    app.doc = Some(Editor::new(app, &kind));
    let prefill = app.prefill_contact.take();
    if let Some(e) = app.doc.as_mut() {
        e.id = id;
        e.editing = id.is_none();
        if let Some(cid) = prefill.filter(|_| id.is_none()) {
            e.contact = app.lk.contact_opts(sales(&kind)).into_iter().find(|o| o.id == cid);
            e.contacts = combo_box::State::with_selection(app.lk.contact_opts(sales(&kind)), e.contact.as_ref());
        }
    }
    match id {
        Some(id) => app.be.call("docs.get", json!({"id": id}), |r| Message::Doc(Msg::Loaded(r))),
        None => { app.data = Some(json!({})); Task::none() }
    }
}

/// An editor line from a saved document line (`negate` turns a delayed credit into a minus line).
fn line_from(app: &App, taxes: &[Opt], l: &Value, negate: bool) -> Line {
    let q = l["qty_e4"].as_i64().unwrap_or(0);
    Line {
        item: l["item_id"].as_i64().map(|id| Opt { id, label: format!("{} — {}", l["item_code"].as_str().unwrap_or(""), app.lk.item(id).and_then(|i| i["name"].as_str()).unwrap_or("")) }),
        description: l["description"].as_str().unwrap_or("").to_string(),
        qty: fmt_e4(if negate { -q } else { q }),
        price: fmt_price(l["unit_e2"].as_i64().unwrap_or(0)),
        account: l["account_id"].as_i64().map(|id| Opt { id, label: format!("{} {}", l["account_code"].as_str().unwrap_or(""), l["account_name"].as_str().unwrap_or("")) }),
        tax: l["tax_rate_id"].as_i64().and_then(|id| taxes.iter().find(|t| t.id == id).cloned()),
        on_items: l["item_id"].as_i64().is_some(),
    }
}

fn fill_from(app: &App, e: &mut Editor, d: &Value) {
    let style = app.date_style();
    e.kind = match d["subtype"].as_str() { Some(s) if quote_variant(s) => s, _ => d["type"].as_str().unwrap_or(&e.kind) }.to_string();
    e.contact = d["contact_id"].as_i64().map(|id| Opt { id, label: d["contact_name"].as_str().unwrap_or("").to_string() });
    e.contacts = combo_box::State::with_selection(app.lk.contact_opts(sales(&e.kind)), e.contact.as_ref());
    e.date = ui::date(d["date"].as_str().unwrap_or(""), style);
    let due = d["due_date"].as_str().or(d["expiry_date"].as_str()).or(d["delivery_date"].as_str()).unwrap_or("");
    e.due = if due.is_empty() { String::new() } else { ui::date(due, style) };
    e.reference = d["reference"].as_str().unwrap_or("").to_string();
    e.template = d["branding_theme_id"].as_i64().and_then(|id| app.lk.template_opts().into_iter().find(|t| t.id == id));
    e.number = d["number"].as_str().unwrap_or("").to_string();
    e.lat = d["line_amount_type"].as_str().unwrap_or("EXCLUSIVE").to_string();
    e.terms = d["terms"].as_str().unwrap_or("").to_string();
    let taxes = app.lk.tax_opts();
    e.lines = d["lines"].as_array().cloned().unwrap_or_default().iter().map(|l| line_from(app, &taxes, l, false)).collect();
    let due_amt = d["amount_due"].as_i64().unwrap_or(0);
    e.pay_amount = if due_amt > 0 { ui::plain(due_amt) } else { String::new() };
    e.doc = Some(d.clone());
    e.editing = false;
}

/// Keep the grid's type-ahead boxes in step with the lines (rebuilt when lines or their
/// items/accounts change, so each box shows its line's choice).
pub fn sync_boxes(app: &mut App) {
    if !app.desktop() { return; }
    let items = app.lk.item_opts();
    let accts = app.lk.account_opts(|a| a["type"] != "BANK");
    let Some(e) = app.doc.as_mut() else { return };
    let sig: Vec<(Option<i64>, Option<i64>)> = e.lines.iter().map(|l| (l.item.as_ref().map(|o| o.id), l.account.as_ref().map(|o| o.id))).collect();
    if sig == e.box_sig && e.item_boxes.len() == e.lines.len() { return; }
    e.item_boxes = e.lines.iter().map(|l| combo_box::State::with_selection(items.clone(), l.item.as_ref())).collect();
    e.acct_boxes = e.lines.iter().map(|l| combo_box::State::with_selection(accts.clone(), l.account.as_ref())).collect();
    e.box_sig = sig;
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    let style = app.date_style();
    let Some(mut e) = app.doc.take() else { return Task::none() };
    let task = match m {
        Msg::Loaded(Ok(d)) => {
            fill_from(app, &mut e, &d);
            app.data = Some(json!({}));
            let t = history_task(app, &e);
            app.doc = Some(e);
            return t;
        }
        Msg::Attach => {
            let Some(id) = e.id else { app.doc = Some(e); return app.toast("Save it first, then attach files", true) };
            let be = app.be.clone();
            app.doc = Some(e);
            return Task::perform(async move {
                let Some(file) = rfd::AsyncFileDialog::new().set_title("Attach File").pick_file().await else { return Ok(None) };
                let name = file.file_name();
                let bytes = file.read().await;
                let mime = mime_for(&name);
                tokio::task::spawn_blocking(move || be.call_now("attachments.add", json!({"entity": "document", "entity_id": id, "filename": name, "mime": mime, "data": b64(&bytes)})).map(|_| Some(name)))
                    .await.unwrap_or_else(|x| Err(x.to_string()))
            }, |r| Message::Doc(Msg::Attached(r)));
        }
        Msg::Attached(r) => match r {
            Ok(Some(name)) => { app.doc = Some(e); let t1 = app.toast(format!("Attached {name}"), false); return Task::batch([t1, app.load()]); }
            Ok(None) => Task::none(),
            Err(err) => app.toast(err, true),
        },
        Msg::OpenAttachment(aid) => {
            let st = app.be.st.clone();
            app.doc = Some(e);
            return Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<Option<String>, String> {
                    let (name, _, data) = { let g = st.org.lock().unwrap(); g.as_ref().ok_or("no company open")?.ledger.attachment(aid).map_err(|x| x.to_string())? };
                    let dir = std::env::temp_dir().join("LedgerCraft attachments");
                    std::fs::create_dir_all(&dir).map_err(|x| x.to_string())?;
                    let path = dir.join(&name);
                    std::fs::write(&path, data).map_err(|x| x.to_string())?;
                    crate::pdf::open_file(&path);
                    Ok(None)
                }).await.unwrap_or_else(|x| Err(x.to_string()))
            }, |r| Message::Doc(Msg::Attached(r)));
        }
        Msg::RefundOpen => { e.refund_open = !e.refund_open; Task::none() }
        Msg::ApplyOpen => {
            if e.apply.is_some() { e.apply = None; app.doc = Some(e); return Task::none(); }
            let (Some(c), Some(other)) = (e.contact.as_ref(), apply_other(&e.kind)) else { app.doc = Some(e); return Task::none() };
            let p = json!({"type": other, "contact_id": c.id, "tab": "awaiting", "limit": 200});
            app.doc = Some(e);
            return app.be.call("docs.list", p, |r| Message::Doc(Msg::ApplyDocs(r)));
        }
        Msg::ApplyDocs(r) => {
            let mine = e.doc.as_ref().and_then(|d| d["amount_due"].as_i64()).unwrap_or(0);
            let mut left = mine;
            // Suggest applying oldest first, up to what's available on each side.
            e.apply = Some(r.ok().and_then(|v| v["rows"].as_array().cloned()).unwrap_or_default().into_iter().map(|d| {
                let due = d["amount_due"].as_i64().unwrap_or(0);
                let amt = due.min(left).max(0);
                left -= amt;
                (d, if amt > 0 { ui::plain(amt) } else { String::new() })
            }).collect());
            Task::none()
        }
        Msg::ApplyAmt(n, v) => { if let Some(a) = e.apply.as_mut() { if let Some(x) = a.get_mut(n) { x.1 = v; } } Task::none() }
        Msg::ApplySave => {
            let (Some(id), Some(list)) = (e.id, e.apply.clone()) else { app.doc = Some(e); return Task::none() };
            let credit_side = e.kind.ends_with("CREDIT");
            let pairs: Vec<(i64, i64, String)> = list.iter().filter(|(_, a)| ui::parse_money(a).unwrap_or(0) > 0)
                .map(|(d, a)| { let other = d["id"].as_i64().unwrap_or(0); if credit_side { (id, other, a.replace(',', "")) } else { (other, id, a.replace(',', "")) } }).collect();
            if pairs.is_empty() { app.doc = Some(e); return app.toast("Enter an amount to apply", true); }
            let be = app.be.clone();
            e.busy = true;
            app.doc = Some(e);
            return Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<Value, String> {
                    for (credit, doc, amt) in &pairs { be.call_now("credits.allocate", json!({"credit_id": credit, "document_id": doc, "amount": amt}))?; }
                    Ok(json!({"applied": pairs.len()}))
                }).await.unwrap_or_else(|x| Err(x.to_string()))
            }, |r| Message::Doc(Msg::Applied(r)));
        }
        Msg::Applied(r) => {
            e.busy = false;
            match r {
                Ok(_) => { e.apply = None; app.doc = Some(e); let t1 = app.toast("Credit applied", false); let t2 = app.load(); let t3 = app.load_lookups(); return Task::batch([t1, t2, t3]); }
                Err(err) => app.toast(err, true),
            }
        }
        Msg::Memorize => {
            e.memo_open = !e.memo_open;
            if e.memo_next.is_empty() {
                // Next occurrence: a month after this transaction's date.
                let d = ui::to_iso(&e.date, style).unwrap_or_else(ui::today_iso);
                e.memo_next = ui::date(&chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok().and_then(|x| x.checked_add_months(chrono::Months::new(1))).map(|x| x.to_string()).unwrap_or(d), style);
            }
            Task::none()
        }
        Msg::MemoFreq(f) => { e.memo_freq = f; Task::none() }
        Msg::MemoNext(v) => { e.memo_next = v; Task::none() }
        Msg::MemoAuto(v) => { e.memo_auto = v; Task::none() }
        Msg::MemoSave => {
            let Some(next) = ui::to_iso(&e.memo_next, style) else { app.doc = Some(e); return app.toast("Check the next date", true) };
            let p = match e.payload(app, "AUTHORISED") { Ok(p) => p, Err(err) => { app.doc = Some(e); return app.toast(err, true) } };
            let kind = if sales(&e.kind) { "ACCREC" } else { "ACCPAY" };
            let (unit, count) = memo_schedule(e.memo_freq);
            let days = match (ui::to_iso(&e.date, style), ui::to_iso(&e.due, style)) {
                (Some(a), Some(b)) => chrono::NaiveDate::parse_from_str(&b, "%Y-%m-%d").ok().zip(chrono::NaiveDate::parse_from_str(&a, "%Y-%m-%d").ok()).map(|(b, a)| (b - a).num_days()).unwrap_or(14),
                _ => 14,
            };
            let body = json!({"repeating": {"kind": kind, "template": p["doc"], "period_unit": unit, "period_count": count, "start_date": next, "next_date": next,
                "due_terms_type": "DAYSAFTERBILLDATE", "due_terms_days": days.max(0), "save_as": if e.memo_auto { "AUTHORISED" } else { "DRAFT" }}});
            app.doc = Some(e);
            return app.be.call("repeating.save", body, |r| Message::Doc(Msg::Memorized(r)));
        }
        Msg::Memorized(r) => match r {
            Ok(_) => { e.memo_open = false; app.toast("Memorized — it's in Lists ▸ Memorized Transaction List", false) }
            Err(err) => app.toast(err, true),
        },
        Msg::History(r) => { e.history = r.ok().and_then(|v| v["rows"].as_array().cloned()).unwrap_or_default(); Task::none() }
        Msg::Loaded(Err(err)) => { app.error = Some(err); Task::none() }
        Msg::Contact(c) => {
            // Use the contact's defaults for the first line's account (QuickBooks/Xero both do this).
            e.contact_text = c.label.clone();
            let cid = c.id;
            e.contact = Some(c);
            // QuickBooks offers the customer's open delayed charges/credits to add to an invoice.
            e.delayed.clear();
            let hist = history_task(app, &e);
            if e.kind == "ACCREC" && app.skin == Skin::QuickBooks && e.added.is_empty() {
                app.doc = Some(e);
                return Task::batch([hist, app.be.call("docs.openDelayed", json!({"contact_id": cid}), |r| Message::Doc(Msg::Delayed(r)))]);
            }
            hist
        }
        Msg::ContactText(s) => { e.contact_text = s; e.contact = None; Task::none() }
        Msg::Delayed(r) => { e.delayed = r.ok().and_then(|v| v.as_array().cloned()).unwrap_or_default(); Task::none() }
        Msg::AddDelayed(id) => {
            if let Some(pos) = e.delayed.iter().position(|d| d["id"].as_i64() == Some(id)) {
                let d = e.delayed.remove(pos);
                let credit = d["subtype"] == "DELAYEDCREDIT";
                let taxes = app.lk.tax_opts();
                // Drop the blank starter lines, then append the charge's lines.
                e.lines.retain(|l| l.item.is_some() || !l.description.trim().is_empty() || parse_e4(&l.price).unwrap_or(0) != 0);
                for l in d["lines"].as_array().cloned().unwrap_or_default() { e.lines.push(line_from(app, &taxes, &l, credit)); }
                if e.lat == "EXCLUSIVE" { if let Some(lat) = d["line_amount_type"].as_str() { e.lat = lat.to_string(); } }
                e.added.push(id);
            }
            Task::none()
        }
        Msg::Date(s) => {
            let s = ui::date_keys(&e.date, &s, style);
            // Keep the due date's offset (terms) when the date changes.
            if let (Some(old), Some(new), Some(due)) = (ui::to_iso(&e.date, style), ui::to_iso(&s, style), ui::to_iso(&e.due, style)) {
                let days = chrono::NaiveDate::parse_from_str(&due, "%Y-%m-%d").ok().zip(chrono::NaiveDate::parse_from_str(&old, "%Y-%m-%d").ok()).map(|(d, o)| (d - o).num_days());
                if let Some(days) = days { e.due = ui::date(&ui::add_days_iso(&new, days), style); }
            }
            e.date = s; Task::none()
        }
        Msg::Due(s) => { e.due = ui::date_keys(&e.due, &s, style); Task::none() }
        Msg::Reference(s) => { e.reference = s; Task::none() }
        Msg::Template(t) => { e.template = Some(t); Task::none() }
        Msg::Number(s) => { e.number = s; Task::none() }
        Msg::Lat(l) => { e.lat = l.0.to_string(); Task::none() }
        Msg::Item(i, it) => {
            if let Some(item) = app.lk.item(it.id).cloned() {
                let s = sales(&e.kind);
                let l = &mut e.lines[i];
                l.description = item[if s { "sales_description" } else { "purchase_description" }].as_str().filter(|x| !x.is_empty()).or(item["name"].as_str()).unwrap_or("").to_string();
                let mut price = item[if s { "sale_unit_e2" } else { "purchase_unit_e2" }].as_i64().unwrap_or(0);
                // The customer's price level raises or lowers the item's sales price.
                if let Some(bp) = e.contact.as_ref().filter(|_| s).and_then(|c| app.lk.price_level_bp(c.id)) { price = ledger_core::pricelevels::apply_level(price, bp); }
                l.price = fmt_price(price);
                if l.qty.trim().is_empty() { l.qty = "1".into(); }
                let acc = item[if s { "sale_account_id" } else { "purchase_account_id" }].as_i64();
                if let Some(a) = acc.and_then(|id| app.lk.account_opts(|_| true).into_iter().find(|o| o.id == id)) { l.account = Some(a); }
                let tax = item[if s { "sale_tax_rate_id" } else { "purchase_tax_rate_id" }].as_i64();
                if let Some(tx) = tax.and_then(|id| app.lk.tax_opts().into_iter().find(|o| o.id == id)) { l.tax = Some(tx); }
            }
            e.lines[i].item = Some(it);
            if i + 1 == e.lines.len() { let on_items = e.lines[i].on_items; e.lines.push(Line { qty: "1".into(), account: default_account(app, &e.kind), tax: default_tax(app, default_account(app, &e.kind).map(|a| a.id)), on_items, ..Default::default() }); }
            Task::none()
        }
        Msg::Desc(i, s) => { e.lines[i].description = s; Task::none() }
        Msg::Qty(i, s) => { e.lines[i].qty = s; Task::none() }
        Msg::Price(i, s) => {
            e.lines[i].price = s;
            if i + 1 == e.lines.len() { let on_items = e.lines[i].on_items; e.lines.push(Line { qty: "1".into(), account: default_account(app, &e.kind), tax: default_tax(app, default_account(app, &e.kind).map(|a| a.id)), on_items, ..Default::default() }); }
            Task::none()
        }
        Msg::Account(i, a) => { e.lines[i].tax = default_tax(app, Some(a.id)).or(e.lines[i].tax.take()); e.lines[i].account = Some(a); Task::none() }
        Msg::Tax(i, t) => { e.lines[i].tax = Some(t); Task::none() }
        Msg::AddLine => { let on_items = e.items_tab; e.lines.push(Line { qty: "1".into(), account: default_account(app, &e.kind), tax: default_tax(app, default_account(app, &e.kind).map(|a| a.id)), on_items, ..Default::default() }); Task::none() }
        Msg::BillTab(items) => {
            e.items_tab = items;
            // Each tab starts with a blank line to type into.
            if !e.lines.iter().any(|l| l.on_items == items) { e.lines.push(Line { qty: "1".into(), account: default_account(app, &e.kind), tax: default_tax(app, default_account(app, &e.kind).map(|a| a.id)), on_items: items, ..Default::default() }); }
            Task::none()
        }
        Msg::Amount(i, s) => { e.lines[i].qty = "1".into(); e.lines[i].price = s; if i + 1 == e.lines.len() { e.lines.push(Line { qty: "1".into(), account: default_account(app, &e.kind), tax: default_tax(app, default_account(app, &e.kind).map(|a| a.id)), on_items: e.lines[i].on_items, ..Default::default() }); } Task::none() }
        Msg::RemoveLine(i) => { if e.lines.len() > 1 { e.lines.remove(i); } Task::none() }
        Msg::Terms(s) => { e.terms = s; Task::none() }
        Msg::Save(approve) => save(app, &mut e, approve, false),
        Msg::SaveAndNew => save(app, &mut e, true, true),
        Msg::SaveClose => { e.close_after = true; save(app, &mut e, true, false) }
        Msg::SaveAndSend => { e.send_after = true; save(app, &mut e, true, false) }
        Msg::Step(dir) => {
            let (kind, cur) = (e.kind.clone(), e.id);
            let (be, k2) = (app.be.clone(), kind.clone());
            Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Option<i64> {
                    let list = be.call_now("docs.list", json!({"type": kind, "limit": 2000})).ok()?;
                    let mut rows: Vec<(String, i64)> = list["rows"].as_array()?.iter().filter(|r| r["type"] == kind.as_str())
                        .filter_map(|r| Some((r["date"].as_str().unwrap_or("").to_string(), r["id"].as_i64()?))).collect();
                    rows.sort();
                    let ids: Vec<i64> = rows.into_iter().map(|r| r.1).collect();
                    let at = cur.and_then(|c| ids.iter().position(|x| *x == c));
                    let to = match (dir, at) {
                        ("first", _) => Some(0), ("last", _) => ids.len().checked_sub(1),
                        ("prev", Some(i)) => i.checked_sub(1), ("prev", None) => ids.len().checked_sub(1),
                        ("next", Some(i)) => Some(i + 1), _ => None,
                    }?;
                    ids.get(to).copied().filter(|id| Some(*id) != cur)
                }).await.ok().flatten()
            }, move |id| match id { Some(id) => Message::Nav(Route::Doc { kind: k2.clone(), id: Some(id) }), None => Message::Noop })
        }
        Msg::Submit => { e.submit = true; save(app, &mut e, false, false) }
        Msg::ProgressOpen => {
            // Suggest what's left to invoice.
            let (total, done) = e.doc.as_ref().map(|d| (d["total"].as_i64().unwrap_or(0), d["progress_invoiced"].as_i64().unwrap_or(0))).unwrap_or((0, 0));
            let left = if total > 0 { ((total - done) as f64 * 100.0 / total as f64).round() } else { 100.0 };
            e.progress = if e.progress.is_some() { None } else { Some(format!("{left:.0}")) };
            Task::none()
        }
        Msg::ProgressPct(s) => { e.progress = Some(s.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect()); Task::none() }
        Msg::ProgressCreate => {
            let Some(id) = e.id else { app.doc = Some(e); return Task::none() };
            let Some(pct) = e.progress.as_deref().and_then(|s| s.parse::<f64>().ok()).filter(|p| *p > 0.0 && *p <= 100.0) else { app.doc = Some(e); return app.toast("Enter a percentage between 0 and 100", true) };
            e.busy = true;
            e.progress = None;
            app.doc = Some(e);
            return app.be.call("docs.progressInvoice", json!({"id": id, "percent_bp": (pct * 100.0).round() as i64}), |r| Message::Doc(Msg::ActionDone(r, "to_invoice")));
        }
        Msg::OverLimit => { e.allow_over = true; e.error = None; save(app, &mut e, true, false) }
        Msg::Send => {
            let Some(id) = e.id else { app.doc = Some(e); return Task::none() };
            app.doc = Some(e);
            return app.be.call("docs.email", json!({"id": id}), |r| Message::Doc(Msg::Sent(r)));
        }
        Msg::Sent(r) => match r {
            Ok(v) => {
                let msg = if v["queued"] == true { "Emailed - it goes out within a few minutes" } else { "Saved to the Outbox — set up email sending in Settings ▸ Email (Send Forms in QuickBooks Desktop-style)" };
                let t1 = app.toast(msg, false);
                app.doc = Some(e);
                return Task::batch([t1, app.load()]);
            }
            Err(err) => app.toast(err, true),
        },
        Msg::Saved(r, new_after) => {
            e.busy = false;
            match r {
                Ok(v) => {
                    let id = v["id"].as_i64();
                    if e.send_after {
                        e.send_after = false;
                        if let Some(id) = id {
                            let kind = e.kind.clone();
                            app.doc = Some(e);
                            let t0 = app.be.call("docs.email", json!({"id": id}), |r| Message::Doc(Msg::Sent(r)));
                            let t1 = app.load_lookups();
                            let t2 = app.go(Route::Doc { kind, id: Some(id) });
                            return Task::batch([t2, t0, t1]);
                        }
                    }
                    let label = title_for(app, &e.kind);
                    let t1 = app.toast(format!("{label} saved"), false);
                    if e.close_after && app.desktop() {
                        let t2 = app.load_lookups();
                        let i = app.desk.active;
                        let t3 = app.close_window(i);
                        return Task::batch([t1, t2, t3]);
                    }
                    let t2 = app.load_lookups();
                    let saved_kind = if e.kind == "SALESRECEIPT" { "ACCREC".to_string() } else { e.kind.clone() };
                    let next = if new_after { Route::Doc { kind: e.kind.clone(), id: None } } else { Route::Doc { kind: saved_kind, id } };
                    app.doc = Some(e);
                    let t3 = app.go(next);
                    return Task::batch([t1, t2, t3]);
                }
                Err(err) => { e.close_after = false; e.error = Some(err.clone()); app.toast(err.trim_start_matches("CREDIT_LIMIT: ").to_string(), true) }
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
                    let msg = match what { "void" => "Voided", "delete" => "Deleted", "approve" => "Approved", "to_invoice" => "Invoice created", "to_bill" => "Bill created", "copy" => "Copy created", "accept" => "Marked as accepted", "sent" => "Marked as sent", "pay" => if e.kind.ends_with("CREDIT") { "Refund recorded" } else { "Payment recorded" }, _ => "Done" };
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
        Msg::PayDate(s) => { e.pay_date = ui::date_keys(&e.pay_date, &s, style); Task::none() }
        Msg::PayAccount(a) => { e.pay_account = Some(a); Task::none() }
        Msg::AddPayment => {
            let (Some(id), Some(acc)) = (e.id, e.pay_account.clone()) else { app.doc = Some(e); return app.toast("Choose the account the money went to", true) };
            let Some(date) = ui::to_iso(&e.pay_date, style) else { app.doc = Some(e); return app.toast(format!("Enter the payment date as {}", hint(style)), true) };
            if ui::parse_money(&e.pay_amount).unwrap_or(0) <= 0 { app.doc = Some(e); return app.toast("Enter the amount paid", true); }
            e.busy = true;
            app.be.call("payments.add", json!({"document_id": id, "account_id": acc.id, "date": date, "amount": e.pay_amount.replace(',', ""), "reference": ""}), |r| Message::Doc(Msg::ActionDone(r, "pay")))
        }
        Msg::Cancel => { app.doc = Some(e); return app.update(Message::Back); }
        Msg::Print => {
            let Some(id) = e.id else { app.doc = Some(e); return app.toast("Save it first, then print", true) };
            let be = app.be.clone();
            let style = app.date_style();
            let title = printed_title(app, &e.kind);
            let accent = if app.skin == Skin::QuickBooks { (0.16, 0.6, 0.27) } else { (0.05, 0.58, 0.53) };
            app.doc = Some(e);
            return Task::perform(async move {
                tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let mut d = be.call_now("docs.get", json!({"id": id}))?;
                    let mut st = be.call_now("settings.get", json!({}))?;
                    let (mut title, mut accent) = (title, accent);
                    if let Ok(themes) = be.call_now("branding.list", json!({})) { crate::pdf::apply_template(&mut d, &mut st, &themes, &mut title, &mut accent); }
                    let bytes = crate::pdf::document(&d, &st, &title, &|x| ui::date(x, style), accent);
                    let name = format!("{}.pdf", d["number"].as_str().filter(|n| !n.is_empty()).unwrap_or("document").replace(|c: char| !c.is_alphanumeric() && c != '-', "_"));
                    let path = crate::pdf::output_dir().join(name);
                    std::fs::write(&path, bytes).map_err(|e| format!("Couldn't save the PDF: {e}"))?;
                    crate::pdf::open_file(&path);
                    Ok(path.display().to_string())
                }).await.unwrap_or_else(|e| Err(e.to_string()))
            }, |r| Message::Doc(Msg::Printed(r)));
        }
        Msg::Printed(r) => match r { Ok(p) => app.toast(format!("Saved {p}"), false), Err(err) => app.toast(err, true) },
    };
    app.doc = Some(e);
    task
}

/// The link above the title: the list this document belongs to (Xero: "Invoices", "Bills"…).
fn crumb<'a>(app: &App, kind: &str) -> Element<'a, Message> {
    if app.skin != Skin::Xero { return row![ui::link(if sales(kind) { "Sales" } else { "Purchases" }, Message::Back)].into(); }
    let (label, route) = match kind {
        "ACCREC" => ("Invoices", Route::Invoices), "ACCRECCREDIT" | "ACCPAYCREDIT" => ("Credit notes", Route::CreditNotes),
        "QUOTE" => ("Quotes", Route::Quotes), "PO" => ("Purchase orders", Route::PurchaseOrders), "ACCPAY" => ("Bills to pay", Route::Bills),
        _ => return row![ui::link(if sales(kind) { "Sales" } else { "Purchases" }, Message::Back)].into(),
    };
    if !sales(kind) { return row![ui::link(label, Message::Nav(route))].into(); }
    row![ui::link("Sales overview", Message::Nav(Route::AllSales)), text("›").size(14).color(t().faint), ui::link(label, Message::Nav(route))].spacing(6).align_y(Alignment::Center).into()
}

fn save(app: &mut App, e: &mut Editor, approve: bool, new_after: bool) -> Task<Message> {
    if e.busy { return Task::none(); }
    let status = if approve { "AUTHORISED" } else if std::mem::take(&mut e.submit) { "SUBMITTED" } else { "DRAFT" };
    if e.kind == "SALESORDER" {
        return match e.payload(app, "SENT") {
            Ok(p) => { e.busy = true; e.error = None; app.be.call("docs.saveSalesOrder", p, move |r| Message::Doc(Msg::Saved(r, new_after))) }
            Err(err) => { e.error = Some(err.clone()); app.toast(err, true) }
        };
    }
    if delayed(&e.kind) {
        return match e.payload(app, "SENT") {
            Ok(p) => { e.busy = true; e.error = None; app.be.call("docs.saveDelayed", p, move |r| Message::Doc(Msg::Saved(r, new_after))) }
            Err(err) => { e.error = Some(err.clone()); app.toast(err, true) }
        };
    }
    if e.kind == "SALESRECEIPT" {
        let Some(acc) = e.pay_account.clone() else { e.error = Some("Choose the account the money went into".into()); return app.toast("Choose the account the money went into", true) };
        return match e.payload(app, "AUTHORISED") {
            Ok(mut p) => { p["account_id"] = json!(acc.id); e.busy = true; e.error = None; app.be.call("docs.salesReceipt", p, move |r| Message::Doc(Msg::Saved(r, new_after))) }
            Err(err) => { e.error = Some(err.clone()); app.toast(err, true) }
        };
    }
    match e.payload(app, status) {
        Ok(p) => { e.busy = true; e.error = None; app.be.call("docs.save", p, move |r| Message::Doc(Msg::Saved(r, new_after))) }
        Err(err) => { e.error = Some(err.clone()); app.toast(err, true) }
    }
}

// ============================ view ============================

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(e) = &app.doc else { return ui::muted("…") };
    if app.desktop() && (e.id.is_none() || e.doc.is_some()) { return crate::docdesk::view(app, e); }
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
    let is_receipt = e.kind == "SALESRECEIPT" || e.doc.as_ref().map(|d| d["subtype"] == "RECEIPT").unwrap_or(false);
    let name = if is_receipt { "Sales receipt" } else { title_for(app, &e.kind) };
    let title = if e.number.is_empty() { format!("New {}", name.to_lowercase()) } else { format!("{name} {}", e.number) };
    let date_hint = hint(app.date_style());

    // ---- header fields ----
    // Xero labels the contact "To" on sales and "From" on purchases.
    let contact_label = match (app.skin, sales(&e.kind)) { (Skin::Xero, true) => "To", (Skin::Xero, false) => "From", (_, true) => "Customer", _ => app.term("supplier") };
    let contact_field: Element<Message> = if editable {
        column![ui::label(contact_label), combo_box(&e.contacts, &format!("Choose a {}", contact_label.to_lowercase()), e.contact.as_ref(), |c| Message::Doc(Msg::Contact(c)))
            .on_input(|s| Message::Doc(Msg::ContactText(s))).padding([7, 10]).size(14).input_style(theme::input)].spacing(5).width(Fill).into()
    } else {
        column![ui::label(contact_label), ui::link(e.contact.as_ref().map(|c| c.label.clone()).unwrap_or_default(), e.contact.as_ref().map(|c| Message::Nav(Route::Contact(c.id))).unwrap_or(Message::Noop))].spacing(8).width(Fill).into()
    };
    let date_label = match e.kind.as_str() { "ACCREC" => if app.skin == Skin::QuickBooks { "Invoice date" } else { "Date" }, "QUOTE" => if app.skin == Skin::QuickBooks { "Estimate date" } else { "Date" }, _ => "Date" };
    let due_label = match e.kind.as_str() { "QUOTE" => "Expiration date", "PO" => "Delivery date", "ACCRECCREDIT" | "ACCPAYCREDIT" | "SALESRECEIPT" | "DELAYEDCHARGE" | "DELAYEDCREDIT" => "", _ if is_receipt => "", _ => "Due date" };
    let date_label = if is_receipt { "Sales receipt date" } else { date_label };
    let field = |lbl: &str, ph: &str, val: &str, f: fn(String) -> Msg| -> Element<'_, Message> {
        if editable { ui::field_ph(lbl, ph, val, move |s| Message::Doc(f(s))) } else { column![ui::label(lbl.to_string()), ui::body(val.to_string())].spacing(8).width(Fill).into() }
    };
    let mut fields = row![contact_field].spacing(14);
    fields = fields.push(field(date_label, date_hint, &e.date, Msg::Date));
    if !due_label.is_empty() { fields = fields.push(field(due_label, date_hint, &e.due, Msg::Due)); }
    if e.kind == "SALESRECEIPT" && editable {
        fields = fields.push(column![ui::label("Deposit to"), pick_list(app.lk.bank_opts(), e.pay_account.clone(), |a| Message::Doc(Msg::PayAccount(a))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill));
    }
    let mut fields2 = row![].spacing(14);
    fields2 = fields2.push(field(if app.skin == Skin::QuickBooks { if delayed(&e.kind) { "No." } else if sales(&e.kind) { "Invoice no." } else { "Bill no." } } else { match e.kind.as_str() { "ACCREC" => "Invoice number", "QUOTE" => "Quote number", "PO" => "Order number", "ACCRECCREDIT" | "ACCPAYCREDIT" => "Credit note number", _ => "Number" } }, "Auto", &e.number, Msg::Number));
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
    // Profitability (QuickBooks): what the items on this sale cost, and the margin left.
    if sales(&e.kind) && !e.kind.ends_with("CREDIT") {
        let (cost, costed_sales) = e.lines.iter().fold((0i64, 0i64), |(c, sl), l| {
            let Some(item) = l.item.as_ref().and_then(|o| app.lk.items.iter().find(|x| x["id"].as_i64() == Some(o.id))) else { return (c, sl) };
            let unit = item["avg_cost"].as_i64().filter(|v| *v > 0 && item["is_tracked"] == 1).unwrap_or(item["purchase_unit_e2"].as_i64().unwrap_or(0) / 100);
            if unit <= 0 { return (c, sl); }
            let q = parse_e4(&l.qty).unwrap_or(0);
            (c + div_round(unit as i128 * q as i128, 10_000), sl + line_cents(q, parse_e4(&l.price).unwrap_or(0)))
        });
        if cost > 0 {
            let margin = costed_sales - cost;
            let pct = if costed_sales != 0 { margin as f64 * 100.0 / costed_sales as f64 } else { 0.0 };
            totals = totals.push(ui::hr()).push(text("PROFITABILITY").size(11).font(BOLD).color(k.muted))
                .push(row![ui::muted("Cost of items"), ui::hspace(), ui::muted(ui::money(cost))])
                .push(row![text("Gross margin").size(14), ui::hspace(), text(format!("{} ({pct:.0}%)", ui::money(margin))).size(14).color(if margin < 0 { k.bad } else { k.ok })]);
        }
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
            Skin::QuickBooks if vocab::acumatica() => {
                // Acumatica: a new document is saved On Hold; Release posts it.
                if e.doc.is_none() || matches!(status, "DRAFT" | "SUBMITTED") {
                    actions.push(ui::btn("Save", m(if status == "SUBMITTED" { Msg::Submit } else { Msg::Save(false) })));
                    // On Hold → Balanced (ready to release) → Release posts it.
                    if ledger_doc(&e.kind) && status != "SUBMITTED" { actions.push(ui::btn("Remove Hold", m(Msg::Submit))); }
                    if sales(&e.kind) && ledger_doc(&e.kind) { actions.push(ui::btn("Release & Email", m(Msg::SaveAndSend))); }
                    actions.push(ui::btn_primary("Release", m(Msg::Save(true))));
                } else {
                    actions.push(ui::btn_primary("Save", m(Msg::Save(true))));
                }
            }
            Skin::QuickBooks => {
                if e.doc.is_none() || status == "DRAFT" { actions.push(ui::btn("Save draft", m(Msg::Save(false)))); }
                actions.push(ui::btn("Save and new", m(Msg::SaveAndNew)));
                if delayed(&e.kind) { actions.push(ui::btn_primary("Save and close", m(Msg::Save(true)))); }
                else if sales(&e.kind) { actions.push(ui::btn("Save and close", m(Msg::Save(true)))); actions.push(ui::btn_primary("Save and send", m(Msg::SaveAndSend))); }
                else { actions.push(ui::btn_primary(if e.kind == "PO" { "Save" } else { "Save and close" }, m(Msg::Save(true)))); }
            }
            Skin::Xero if vocab::sage() => {
                // Sage: Save as draft, Save, and Save & Email for sales.
                if e.doc.is_none() || status == "DRAFT" { actions.push(ui::btn("Save as draft", m(Msg::Save(false)))); }
                if sales(&e.kind) && ledger_doc(&e.kind) {
                    actions.push(ui::btn("Save", m(Msg::Save(true))));
                    actions.push(ui::btn_primary("Save & Email", m(Msg::SaveAndSend)));
                } else {
                    actions.push(ui::btn_primary("Save", m(Msg::Save(true))));
                }
            }
            Skin::Xero => {
                // Xero: Save (draft / submit for approval) and Approve (or Approve & email for sales).
                if status != "SUBMITTED" { actions.push(ui::btn("Save draft", m(Msg::Save(false)))); }
                if ledger_doc(&e.kind) {
                    actions.push(ui::btn(if status == "SUBMITTED" { "Save" } else { "Submit for approval" }, m(Msg::Submit)));
                }
                if sales(&e.kind) && ledger_doc(&e.kind) {
                    actions.push(ui::btn("Approve", m(Msg::Save(true))));
                    actions.push(ui::btn_primary("Approve & email", m(Msg::SaveAndSend)));
                } else {
                    actions.push(ui::btn_primary("Approve", m(Msg::Save(true))));
                }
            }
        }
    } else {
        match (e.kind.as_str(), status) {
            ("QUOTE", "SENT" | "AUTHORISED" | "DRAFT") => { actions.push(ui::btn("Mark accepted", m(Msg::Action("accept")))); actions.push(ui::btn_primary(if app.skin == Skin::QuickBooks { "Create invoice" } else { "Convert to invoice" }, m(Msg::Action("to_invoice")))); }
            ("QUOTE", "ACCEPTED") => actions.push(ui::btn_primary(if app.skin == Skin::QuickBooks { "Create invoice" } else { "Convert to invoice" }, m(Msg::Action("to_invoice")))),
            _ => {}
        }
        if e.kind == "QUOTE" && matches!(status, "SENT" | "AUTHORISED" | "ACCEPTED" | "DRAFT") && e.id.is_some() {
            actions.insert(0, ui::btn("Progress invoice", m(Msg::ProgressOpen)));
        }
        match (e.kind.as_str(), status) {
            ("PO", "AUTHORISED") => actions.push(ui::btn_primary("Copy to bill", m(Msg::Action("to_bill")))),
            _ => {}
        }
        if (status == "AUTHORISED" || delayed(&e.kind) && status == "SENT") && e.doc.as_ref().map(|d| d["amount_paid"].as_i64().unwrap_or(0) == 0 && d["amount_credited"].as_i64().unwrap_or(0) == 0).unwrap_or(false) {
            actions.push(ui::btn("Edit", m(Msg::Edit)));
        }
        actions.push(ui::btn(if app.skin == Skin::QuickBooks { "Print or preview" } else { "Print / PDF" }, m(Msg::Print)));
        if sales(&e.kind) && matches!(status, "AUTHORISED" | "SENT" | "PAID") { actions.push(ui::btn(if app.skin == Skin::QuickBooks { "Send" } else { "Email" }, m(Msg::Send))); }
        actions.push(ui::btn("Copy", m(Msg::Action("copy"))));
        if matches!(status, "AUTHORISED" | "PAID") && ledger_doc(&e.kind) { actions.push(ui::btn_danger("Void", m(Msg::Action("void")))); }
    }
    if editable && e.id.is_some() && matches!(status, "DRAFT" | "SUBMITTED") { actions.insert(0, ui::btn_danger("Delete", m(Msg::Action("delete")))); }
    if editable && e.id.is_some() { actions.insert(0, ui::btn(if app.skin == Skin::QuickBooks { "Print or preview" } else { "Print / PDF" }, m(Msg::Print))); }

    // ---- assemble per layout ----
    let k2 = k;
    let status_chip = ui::chip(status_label, crate::lists::status_color(status_label));
    let error: Element<Message> = match &e.error {
        // Over the customer's credit limit: say so, and let the user go ahead.
        Some(err) if err.starts_with("CREDIT_LIMIT:") => container(row![
            text(err.trim_start_matches("CREDIT_LIMIT: ").to_string()).size(13).color(k2.bad).width(Fill),
            ui::btn("Save anyway", m(Msg::OverLimit)),
        ].spacing(10).align_y(Alignment::Center)).padding([8, 12]).style(theme::bad_soft).width(Fill).into(),
        Some(err) => container(text(err.clone()).size(13).color(k2.bad)).padding([8, 12]).style(theme::bad_soft).width(Fill).into(),
        None => Space::new().into(),
    };
    // Progress invoicing: how much of the estimate is invoiced, and the box for the next part.
    let progress_panel: Element<Message> = match (&e.doc, e.kind.as_str()) {
        (Some(d), "QUOTE") if d["progress_invoiced"].as_i64().unwrap_or(0) > 0 || e.progress.is_some() => {
            let (qt, done) = (d["total"].as_i64().unwrap_or(0), d["progress_invoiced"].as_i64().unwrap_or(0));
            let pct = if qt > 0 { done as f64 * 100.0 / qt as f64 } else { 0.0 };
            let mut col = column![row![ui::h2("Progress invoicing"), ui::hspace(), text(format!("Invoiced so far: {} of {} ({pct:.0}%)", ui::money(done), ui::money(qt))).size(14)].align_y(Alignment::Center)].spacing(10);
            if let Some(p) = &e.progress {
                col = col.push(row![
                    text("Invoice").size(14), text_input("50", p).on_input(|s| Message::Doc(Msg::ProgressPct(s))).on_submit(Message::Doc(Msg::ProgressCreate)).padding([6, 9]).size(14).style(theme::input).width(80),
                    text("% of the estimate").size(14), ui::hspace(),
                    ui::btn("Cancel", m(Msg::ProgressOpen)), ui::btn_primary("Create invoice", m(Msg::ProgressCreate)),
                ].spacing(8).align_y(Alignment::Center));
            }
            ui::card(col)
        }
        _ => Space::new().into(),
    };
    let body: Element<Message> = match app.skin {
        Skin::QuickBooks if vocab::acumatica() => {
            // Acumatica record screen: toolbar along the top, summary area, then the details grid.
            let tb = |ic: &'static str, tip: &'static str, msg: Option<Message>| -> Element<Message> {
                iced::widget::tooltip(button(icon(ic, 16.0, if msg.is_some() { k.text } else { k.faint })).style(theme::row_btn).padding(6).on_press_maybe(msg),
                    container(text(tip).size(12)).padding([4, 8]).style(theme::menu), iced::widget::tooltip::Position::Bottom).into()
            };
            let can_delete = e.id.is_some() && matches!(status, "DRAFT" | "SUBMITTED");
            let toolbar = container(row![
                tb("save", "Save", if editable { m(match status { "SUBMITTED" => Msg::Submit, "DRAFT" => Msg::Save(false), _ if e.doc.is_none() => Msg::Save(false), _ => Msg::Save(true) }) } else { None }),
                tb("undo", "Cancel", Some(Message::Doc(Msg::Cancel))),
                tb("plus", "Add new record", Some(Message::Nav(Route::Doc { kind: e.kind.clone(), id: None }))),
                tb("trash", "Delete", if can_delete { m(Msg::Action("delete")) } else { None }),
                container(Space::new()).width(1).height(20).style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.line)), ..Default::default() }),
                tb("first", "First", m(Msg::Step("first"))), tb("prev", "Previous", m(Msg::Step("prev"))),
                tb("next", "Next", m(Msg::Step("next"))), tb("last", "Last", m(Msg::Step("last"))),
                container(Space::new()).width(1).height(20).style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.line)), ..Default::default() }),
            ].extend(actions).spacing(6).align_y(Alignment::Center)).padding([6, 10]).style(theme::topbar).width(Fill);
            let summary = ui::card(column![row![column![fields].width(Fill), Space::new().width(30),
                column![text("BALANCE").size(12).font(BOLD).color(k.muted), text(ui::money(if e.doc.is_some() { due_amt } else { total })).size(24).font(BOLD)].spacing(2).align_x(Alignment::End)].align_y(Alignment::Start), fields2].spacing(14));
            let tabs = row![text("Details").size(14).font(BOLD), text("Financial").size(14).color(k.muted), text("Address").size(14).color(k.muted)].spacing(18);
            column![toolbar, error, summary, progress_panel, tabs, table, add_line, ui::card(bottom), pay_panel].spacing(12).into()
        }
        Skin::QuickBooks => {
            // QuickBooks: big "balance due" on the right of the header card.
            let balance = column![text(if ledger_doc(&e.kind) { if sales(&e.kind) { "BALANCE DUE" } else { "BALANCE DUE" } } else { "TOTAL" }).size(12).font(BOLD).color(k.muted), text(ui::money(if e.doc.is_some() { due_amt } else { total })).size(30).font(BOLD)]
                .spacing(2).align_x(Alignment::End);
            let header = ui::card(column![row![column![fields].width(Fill), Space::new().width(30), balance].align_y(Alignment::Start), fields2].spacing(16));
            column![error, header, pay_panel, progress_panel, add_delayed(app, e), table, add_line, ui::card(bottom), footer_bar(actions)].spacing(14).into()
        }
        Skin::Xero => {
            let header = ui::card(column![fields, fields2].spacing(14));
            // Xero only offers Cancel while editing; an approved document just has its options.
            let cancel: Element<Message> = if editable { ui::btn("Cancel", Some(Message::Doc(Msg::Cancel))) } else { Space::new().into() };
            column![error, header, progress_panel, table, add_line, ui::card(bottom), pay_panel, row![cancel, ui::hspace()].extend(actions).spacing(8)].spacing(14).into()
        }
    };
    let title_el = column![
        crumb(app, &e.kind),
        row![ui::h1(title), status_chip].spacing(12).align_y(Alignment::Center),
    ].spacing(4);
    ui::page(title_el.into(), vec![], body)
}

/// QuickBooks "Add to invoice": the customer's open delayed charges and credits.
fn add_delayed<'a>(app: &'a App, e: &'a Editor) -> Element<'a, Message> {
    if e.delayed.is_empty() || !e.editable() { return Space::new().into(); }
    let k = t();
    let mut col = column![row![icon("plus", 16.0, k.accent), text("Add to invoice").size(15).font(BOLD)].spacing(8).align_y(Alignment::Center)].spacing(8);
    for d in &e.delayed {
        let credit = d["subtype"] == "DELAYEDCREDIT";
        let total = d["total"].as_i64().unwrap_or(0);
        let desc = d["lines"][0]["description"].as_str().unwrap_or("").to_string();
        col = col.push(row![
            text(if credit { "Delayed Credit" } else { "Delayed Charge" }).size(14).font(BOLD).width(Length::FillPortion(14)),
            text(app.d(d["date"].as_str().unwrap_or(""))).size(13).color(k.muted).width(Length::FillPortion(12)),
            text(desc).size(13).width(Length::FillPortion(36)),
            text(ui::money(if credit { -total } else { total })).size(14).width(Length::FillPortion(12)).align_x(iced::alignment::Horizontal::Right),
            ui::btn("Add", Some(Message::Doc(Msg::AddDelayed(d["id"].as_i64().unwrap_or(0))))),
        ].spacing(12).align_y(Alignment::Center));
    }
    container(col).padding([12, 16]).style(theme::accent_soft).width(Fill).into()
}

/// The heading printed on the document.
pub(crate) fn printed_title(app: &App, kind: &str) -> String {
    let registered = matches!(app.setting("tax_registered").as_str(), "1" | "true");
    match kind {
        "ACCREC" if registered && matches!(app.country().as_str(), "AU" | "NZ") => "Tax Invoice".into(),
        "ACCREC" if registered && matches!(app.country().as_str(), "GB" | "IE") => "VAT Invoice".into(),
        _ => title_for(app, kind).to_string(),
    }
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
        app.lk = crate::app::Lookups { accounts: get("accounts.list", json!({})), tax_rates: get("taxRates.list", json!({})), items: get("items.list", json!({})), contacts: get("contacts.list", json!({"filter": "all"})), price_levels: get("priceLevels.list", json!({})), templates: get("branding.list", json!({})) };
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
