//! Smaller forms: new customer/vendor, new product or service, journal entry.

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
}

#[derive(Debug, Clone)]
pub enum Msg {
    Name(String), Email(String), Phone(String), Code(String), Price(String), Cost(String),
    SaleAccount(Opt), PurchaseAccount(Opt), Tax(Opt), Service(bool),
    Date(String), Memo(String), JAccount(usize, Opt), JDesc(usize, String), JDebit(usize, String), JCredit(usize, String), AddLine,
    Save, Saved(Result<Value, String>),
}

pub fn open(app: &mut App) -> Task<Message> {
    let mut s = State { customer: !matches!(app.route, Route::NewContact(false)), service: true, date: ui::date(&ui::today_iso(), app.date_style()), ..Default::default() };
    let find = |code: &str| app.lk.account_opts(|a| a["code"] == code).into_iter().next();
    s.sale_account = find("200");
    s.purchase_account = find("310").or_else(|| find("429"));
    s.lines = vec![JLine::default(), JLine::default()];
    app.form = Some(s);
    app.data = Some(json!({}));
    Task::none()
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    let style = app.date_style();
    let Some(mut s) = app.form.take() else { return Task::none() };
    let task = match m {
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
        Msg::Date(v) => { s.date = v; Task::none() }
        Msg::Memo(v) => { s.memo = v; Task::none() }
        Msg::JAccount(n, a) => { s.lines[n].account = Some(a); if n + 1 == s.lines.len() { s.lines.push(JLine::default()); } Task::none() }
        Msg::JDesc(n, v) => { s.lines[n].description = v; Task::none() }
        Msg::JDebit(n, v) => { s.lines[n].debit = v; if !s.lines[n].debit.trim().is_empty() { s.lines[n].credit.clear(); } Task::none() }
        Msg::JCredit(n, v) => { s.lines[n].credit = v; if !s.lines[n].credit.trim().is_empty() { s.lines[n].debit.clear(); } Task::none() }
        Msg::AddLine => { s.lines.push(JLine::default()); Task::none() }
        Msg::Save => {
            if s.busy { app.form = Some(s); return Task::none(); }
            let call = match &app.route {
                Route::NewContact(_) => {
                    if s.name.trim().is_empty() { app.form = Some(s); return app.toast("Enter a name", true); }
                    ("contacts.create", json!({"name": s.name.trim(), "email": s.email.trim(), "phone": s.phone.trim(), "is_customer": s.customer, "is_supplier": !s.customer}))
                }
                Route::NewProduct => {
                    if s.name.trim().is_empty() { app.form = Some(s); return app.toast("Enter a name", true); }
                    let code = if s.code.trim().is_empty() { s.name.trim().to_uppercase().chars().filter(|c| c.is_alphanumeric()).take(10).collect() } else { s.code.trim().to_string() };
                    ("items.create", json!({"code": code, "name": s.name.trim(), "sales_description": s.name.trim(), "is_sold": true, "is_purchased": !s.service || !s.cost.trim().is_empty(),
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
                        Route::NewProduct => ("Product saved", Route::Products),
                        _ => ("Journal entry posted", Route::Report("journal-report".into())),
                    };
                    let t1 = app.toast(msg, false);
                    let t2 = app.load_lookups();
                    let t3 = app.go(next);
                    return Task::batch([t1, t2, t3]);
                }
                Err(e) => app.toast(e, true),
            }
        }
    };
    app.form = Some(s);
    task
}

fn totals(s: &State) -> (i64, i64) {
    s.lines.iter().fold((0, 0), |(d, c), l| (d + ui::parse_money(&l.debit).unwrap_or(0), c + ui::parse_money(&l.credit).unwrap_or(0)))
}

fn footer<'a>(busy: bool) -> Element<'a, Message> {
    container(row![ui::btn("Cancel", Some(Message::Back)), ui::hspace(), ui::btn_primary("Save", if busy { None } else { Some(Message::Form(Msg::Save)) })].spacing(8))
        .padding([12, 16]).style(theme::topbar).width(Fill).into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(s) = &app.form else { return ui::muted("…") };
    let k = t();
    match &app.route {
        Route::NewContact(customer) => {
            let title = if *customer { "New customer" } else if app.skin == Skin::QuickBooks { "New vendor" } else { "New supplier" };
            ui::page(ui::h1(title), vec![], column![
                ui::card(column![
                    ui::field(if *customer { "Customer display name" } else { "Company or name" }, &s.name, |v| Message::Form(Msg::Name(v))),
                    row![ui::field("Email", &s.email, |v| Message::Form(Msg::Email(v))), ui::field("Phone", &s.phone, |v| Message::Form(Msg::Phone(v)))].spacing(14),
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
                    ui::muted("Inventory items with quantity tracking can be set up in the web version for now."),
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
