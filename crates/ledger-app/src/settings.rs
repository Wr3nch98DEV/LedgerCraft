//! Settings: layout (QuickBooks-style / Xero-style), theme, company details, users and email.

use crate::app::{App, Message, Route};
use crate::backend::Reply;
use crate::icons::icon;
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD};
use iced::widget::{button, column, container, pick_list, row, scrollable, text};
use iced::{Alignment, Element, Fill, Length, Task};
use serde_json::{Value, json};

/// Editable company fields (settings key, label).
pub const FIELDS: [(&str, &str); 7] = [("org_name", "Company name"), ("legal_name", "Legal name"), ("tax_number", "Tax number (EIN / ABN / VAT)"), ("email", "Company email"),
    ("phone", "Phone"), ("website", "Website"), ("address", "Address")];

/// Outgoing mail fields (settings key, label). The password is write-only.
pub const SMTP: [(&str, &str); 5] = [("smtp_host", "SMTP server"), ("smtp_port", "Port"), ("smtp_user", "User name"), ("smtp_pass", "Password"), ("smtp_from", "Send from (email address)")];

/// Engine roles, in the order offered.
pub const ROLES: [&str; 4] = ["ADVISER", "STANDARD", "INVOICE_ONLY", "READONLY"];

/// A role as each product names it.
pub fn role_label(skin: Skin, role: &str) -> &'static str {
    let qb = skin == Skin::QuickBooks;
    match role {
        "ADVISER" => if qb { "Admin" } else { "Adviser" },
        "STANDARD" => if qb { "Standard (all access)" } else { "Standard" },
        "INVOICE_ONLY" => "Invoice only",
        "READONLY" => if qb { "Reports only" } else { "Read only" },
        _ => "Custom",
    }
}

/// Connection security for outgoing mail (the engine's `smtp_tls` values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Security { #[default] StartTls, Tls, None }
impl Security {
    pub const ALL: [Security; 3] = [Security::StartTls, Security::Tls, Security::None];
    pub fn key(self) -> &'static str { match self { Security::StartTls => "STARTTLS", Security::Tls => "TLS", Security::None => "NONE" } }
    pub fn from_key(s: &str) -> Security { match s { "TLS" => Security::Tls, "NONE" => Security::None, _ => Security::StartTls } }
}
impl std::fmt::Display for Security {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self { Security::StartTls => "STARTTLS (port 587)", Security::Tls => "TLS (port 465)", Security::None => "None (local relay)" })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoleOpt { pub key: &'static str, pub label: &'static str }
impl std::fmt::Display for RoleOpt { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.label) } }

#[derive(Default)]
pub struct State {
    /// Desktop Preferences: selected category.
    pub cat: Option<&'static str>,
    pub values: Vec<String>,
    pub editing: bool,
    pub busy: bool,
    pub users: Vec<Value>,
    pub users_error: Option<String>,
    pub adding: bool,
    pub u_name: String,
    pub u_email: String,
    pub u_role: Option<&'static str>,
    pub smtp: Vec<String>,
    pub smtp_tls: Security,
    pub smtp_editing: bool,
    pub test_to: String,
    /// Set Closing Date: [closing date for most users, closing date for everyone] while editing.
    pub lock: Option<[String; 2]>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Cat(&'static str), Edit, Field(usize, String), Save, Saved(Result<Value, String>), Cancel,
    Users(Reply), AddUser, UName(String), UEmail(String), URole(RoleOpt), SaveUser, UserSaved(Reply), SetActive(i64, bool),
    LockEdit, LockField(usize, String), LockSave, LockSaved(Reply), EditSmtp, Smtp(usize, String), SmtpTls(Security), SaveSmtp, SmtpSaved(Reply), TestTo(String), SendTest, TestSent(Reply),
}

pub fn load(app: &mut App) -> Task<Message> {
    app.settings.adding = false;
    app.settings.smtp_editing = false;
    app.be.call("users.list", json!({}), |r| Message::Settings(Msg::Users(r)))
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    match m {
        Msg::Cat(c) => { app.settings.cat = Some(c); Task::none() }
        Msg::Edit => { app.settings.values = FIELDS.iter().map(|(k, _)| app.setting(k)).collect(); app.settings.editing = true; Task::none() }
        Msg::Field(n, v) => { if let Some(x) = app.settings.values.get_mut(n) { *x = v; } Task::none() }
        Msg::Cancel => { app.settings.lock = None; app.settings.editing = false; app.settings.adding = false; app.settings.smtp_editing = false; Task::none() }
        Msg::Save => {
            if app.settings.values.first().map(|v| v.trim().is_empty()).unwrap_or(true) { return app.toast("Company name can't be empty", true); }
            let mut p = serde_json::Map::new();
            for ((k, _), v) in FIELDS.iter().zip(app.settings.values.iter()) { p.insert(k.to_string(), json!(v.trim())); }
            app.settings.busy = true;
            app.be.call("settings.update", Value::Object(p), |r| Message::Settings(Msg::Saved(r)))
        }
        Msg::Saved(r) => {
            app.settings.busy = false;
            match r {
                Ok(_) => { app.settings.editing = false; let t = app.toast("Company details saved", false); Task::batch([t, app.be.call("session.get", json!({}), Message::Session)]) }
                Err(e) => app.toast(e, true),
            }
        }
        // ---- closing date ----
        Msg::LockEdit => { app.settings.lock = Some([app.d(&app.setting("lock_date_all")), app.d(&app.setting("lock_date_others"))]); Task::none() }
        Msg::LockField(n, v) => { if let Some(l) = app.settings.lock.as_mut() { l[n.min(1)] = v; } Task::none() }
        Msg::LockSave => {
            let Some(l) = app.settings.lock.clone() else { return Task::none() };
            let style = app.date_style();
            let mut iso = [String::new(), String::new()];
            for n in 0..2 {
                let v = l[n].trim();
                if v.is_empty() { continue; }
                let Some(d) = ui::to_iso(v, style) else { return app.toast("Check the closing date", true) };
                iso[n] = d;
            }
            if !iso[0].is_empty() && !iso[1].is_empty() && iso[1] < iso[0] { return app.toast("The date for other users can't be earlier than the closing date", true); }
            app.settings.busy = true;
            app.be.call("settings.update", json!({"lock_date_all": iso[0], "lock_date_others": iso[1]}), |r| Message::Settings(Msg::LockSaved(r)))
        }
        Msg::LockSaved(r) => {
            app.settings.busy = false;
            match r {
                Ok(_) => { app.settings.lock = None; let t = app.toast("Closing date saved", false); Task::batch([t, app.be.call("session.get", json!({}), Message::Session)]) }
                Err(e) => app.toast(e, true),
            }
        }
        // ---- users ----
        Msg::Users(r) => {
            match r {
                Ok(v) => { app.settings.users = v.as_array().cloned().unwrap_or_default(); app.settings.users_error = None; }
                Err(e) => { app.settings.users.clear(); app.settings.users_error = Some(e); }
            }
            Task::none()
        }
        Msg::AddUser => { app.settings.adding = true; app.settings.u_name.clear(); app.settings.u_email.clear(); app.settings.u_role = Some("STANDARD"); Task::none() }
        Msg::UName(v) => { app.settings.u_name = v; Task::none() }
        Msg::UEmail(v) => { app.settings.u_email = v; Task::none() }
        Msg::URole(r) => { app.settings.u_role = Some(r.key); Task::none() }
        Msg::SaveUser => {
            let s = &app.settings;
            if s.u_name.trim().is_empty() { return app.toast("Enter the person's name", true); }
            if !s.u_email.contains('@') { return app.toast("Enter a valid email address", true); }
            let body = json!({"name": s.u_name.trim(), "email": s.u_email.trim(), "role": s.u_role.unwrap_or("STANDARD")});
            app.settings.busy = true;
            app.be.call("users.create", body, |r| Message::Settings(Msg::UserSaved(r)))
        }
        Msg::UserSaved(r) => {
            app.settings.busy = false;
            match r {
                Ok(_) => { app.settings.adding = false; let t1 = app.toast("User added", false); Task::batch([t1, app.be.call("users.list", json!({}), |r| Message::Settings(Msg::Users(r)))]) }
                Err(e) => app.toast(e, true),
            }
        }
        Msg::SetActive(id, on) => app.be.call("users.setActive", json!({"id": id, "active": on}), move |r| Message::Settings(Msg::UserSaved(r.map(|_| json!({"active": on}))))),
        // ---- email ----
        Msg::EditSmtp => {
            app.settings.smtp = SMTP.iter().map(|(k, _)| if *k == "smtp_pass" { String::new() } else { app.setting(k) }).collect();
            app.settings.smtp_tls = Security::from_key(&app.setting("smtp_tls"));
            app.settings.smtp_editing = true;
            Task::none()
        }
        Msg::Smtp(n, v) => { if let Some(x) = app.settings.smtp.get_mut(n) { *x = v; } Task::none() }
        Msg::SmtpTls(v) => { app.settings.smtp_tls = v; Task::none() }
        Msg::SaveSmtp => {
            let mut p = serde_json::Map::new();
            for ((k, _), v) in SMTP.iter().zip(app.settings.smtp.iter()) {
                // A blank password keeps the saved one.
                if *k == "smtp_pass" && v.is_empty() { continue; }
                p.insert(k.to_string(), json!(v.trim()));
            }
            if let Some(port) = p.get("smtp_port").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                if port.parse::<u16>().is_err() { return app.toast("The port must be a number, e.g. 587", true); }
            }
            p.insert("smtp_tls".into(), json!(app.settings.smtp_tls.key()));
            app.settings.busy = true;
            app.be.call("settings.update", Value::Object(p), |r| Message::Settings(Msg::SmtpSaved(r)))
        }
        Msg::SmtpSaved(r) => {
            app.settings.busy = false;
            match r {
                Ok(_) => { app.settings.smtp_editing = false; let t = app.toast("Email settings saved", false); Task::batch([t, app.be.call("session.get", json!({}), Message::Session)]) }
                Err(e) => app.toast(e, true),
            }
        }
        Msg::TestTo(v) => { app.settings.test_to = v; Task::none() }
        Msg::SendTest => {
            if !app.settings.test_to.contains('@') { return app.toast("Enter an address to send the test to", true); }
            app.settings.busy = true;
            app.be.call("email.test", json!({"to": app.settings.test_to.trim()}), |r| Message::Settings(Msg::TestSent(r)))
        }
        Msg::TestSent(r) => {
            app.settings.busy = false;
            match r { Ok(_) => app.toast("Test email sent", false), Err(e) => app.toast(format!("Couldn't send: {e}"), true) }
        }
    }
}

fn layout_card<'a>(app: &App, layout: crate::theme::Layout, desc: &'a str) -> Element<'a, Message> {
    let k = t();
    let active = app.layout == layout;
    button(column![
        row![icon("layout", 20.0, if active { k.accent } else { k.muted }), text(layout.label()).size(16).font(BOLD), ui::hspace(), if active { ui::chip("In use", k.accent) } else { ui::chip("Switch", k.muted) }].spacing(10).align_y(Alignment::Center),
        ui::muted(desc),
    ].spacing(8))
    .padding(16)
    .width(Fill)
    .on_press(Message::SetLayout(layout))
    .style(move |_, s| {
        let hover = matches!(s, button::Status::Hovered);
        button::Style { background: Some(iced::Background::Color(if hover { k.hover } else { k.panel })), text_color: k.text,
            border: iced::Border { color: if active { k.accent } else { k.line }, width: if active { 2.0 } else { 1.0 }, radius: 8.0.into() }, shadow: Default::default(), snap: true }
    })
    .into()
}

fn users_card(app: &App) -> Element<'_, Message> {
    let k = t();
    let qb = app.skin == Skin::QuickBooks;
    let s = &app.settings;
    let mut col = column![row![ui::h2(if qb { "Manage users" } else { "Users" }), ui::hspace(),
        if s.users_error.is_none() && !s.adding { ui::btn_primary(if qb { "Add user" } else { "Invite a user" }, Some(Message::Settings(Msg::AddUser))) } else { iced::widget::Space::new().into() }].align_y(Alignment::Center)].spacing(10);
    if let Some(e) = &s.users_error {
        return ui::card(col.push(ui::muted(format!("You can't manage users: {e}"))));
    }
    if s.adding {
        let roles: Vec<RoleOpt> = ROLES.iter().map(|r| RoleOpt { key: r, label: role_label(app.skin, r) }).collect();
        let sel = s.u_role.map(|r| RoleOpt { key: r, label: role_label(app.skin, r) });
        col = col.push(container(column![
            row![ui::field("Name", &s.u_name, |v| Message::Settings(Msg::UName(v))), ui::field("Email", &s.u_email, |v| Message::Settings(Msg::UEmail(v))),
                column![ui::label(if qb { "User type" } else { "Role" }), pick_list(roles, sel, |r| Message::Settings(Msg::URole(r))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill)].spacing(14),
            ui::muted("They sign in with their email; set a password for them from the web version's Users page or they set one on first sign-in."),
            row![ui::hspace(), ui::btn("Cancel", Some(Message::Settings(Msg::Cancel))), ui::btn_primary("Save", if s.busy { None } else { Some(Message::Settings(Msg::SaveUser)) })].spacing(8),
        ].spacing(10)).padding(14).style(theme::soft).width(Fill));
    }
    let rows: Vec<(Vec<Element<Message>>, Option<Message>)> = s.users.iter().map(|u| {
        let id = u["id"].as_i64().unwrap_or(0);
        let active = u["active"].as_i64().map(|a| a != 0).or_else(|| u["active"].as_bool()).unwrap_or(true);
        let status: Element<Message> = if active { ui::chip("Active", k.ok) } else { ui::chip("Inactive", k.muted) };
        let action: Element<Message> = if id == 1 { ui::cell_muted(if qb { "Primary admin" } else { "Owner" }) }
            else { ui::btn(if active { "Deactivate" } else { "Reactivate" }, Some(Message::Settings(Msg::SetActive(id, !active)))) };
        (vec![ui::cell(u["name"].as_str().unwrap_or("").to_string()), ui::cell_muted(u["email"].as_str().unwrap_or("").to_string()),
            ui::cell(role_label(app.skin, u["role"].as_str().unwrap_or(""))), status, action], None)
    }).collect();
    col = col.push(ui::table(&[ui::col("Name", 20), ui::col("Email", 26), ui::col(if qb { "User type" } else { "Role" }, 18), ui::col("Status", 10), ui::col("", 14)], rows, "No users."));
    ui::card(col)
}

fn email_card(app: &App) -> Element<'_, Message> {
    let k = t();
    let s = &app.settings;
    let configured = !app.setting("smtp_host").is_empty();
    if s.smtp_editing {
        let mut col = column![ui::h2("Email (outgoing mail)"), ui::muted("Invoices, statements and reminders are sent through your own mail server. For Gmail use smtp.gmail.com, port 587 and an app password.")].spacing(12);
        let field = |n: usize| -> Element<'_, Message> {
            let (key, label) = SMTP[n];
            let ph = if key == "smtp_pass" && configured { "Leave blank to keep the saved password" } else { "" };
            let input = iced::widget::text_input(ph, s.smtp.get(n).map(|x| x.as_str()).unwrap_or("")).on_input(move |v| Message::Settings(Msg::Smtp(n, v))).padding([7, 10]).size(14).style(theme::input);
            let input = if key == "smtp_pass" { input.secure(true) } else { input };
            column![ui::label(label), input].spacing(5).width(Fill).into()
        };
        col = col.push(row![field(0), container(field(1)).width(Length::Fixed(120.0))].spacing(14));
        col = col.push(row![field(2), field(3)].spacing(14));
        col = col.push(row![field(4), column![ui::label("Security"), pick_list(&Security::ALL[..], Some(s.smtp_tls), |v| Message::Settings(Msg::SmtpTls(v))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill)].spacing(14));
        col = col.push(row![ui::btn("Cancel", Some(Message::Settings(Msg::Cancel))), ui::hspace(), ui::btn_primary("Save", if s.busy { None } else { Some(Message::Settings(Msg::SaveSmtp)) })]);
        return ui::card(col);
    }
    let status: Element<Message> = if configured {
        row![icon("check", 14.0, k.ok), text(format!("Sending through {} as {}", app.setting("smtp_host"), app.setting("smtp_from"))).size(14)].spacing(8).align_y(Alignment::Center).into()
    } else { ui::muted("Not set up — documents can still be printed or saved as PDF.") };
    let mut col = column![row![ui::h2("Email (outgoing mail)"), ui::hspace(), ui::btn(if configured { "Edit" } else { "Set up email" }, Some(Message::Settings(Msg::EditSmtp)))].align_y(Alignment::Center), status].spacing(10);
    if configured {
        col = col.push(row![
            iced::widget::text_input("Send a test email to…", &s.test_to).on_input(|v| Message::Settings(Msg::TestTo(v))).padding([7, 10]).size(14).style(theme::input).width(Length::Fixed(320.0)),
            ui::btn("Send test email", if s.busy { None } else { Some(Message::Settings(Msg::SendTest)) }),
        ].spacing(8).align_y(Alignment::Center));
    }
    ui::card(col)
}

fn layout_section(app: &App) -> Element<'_, Message> {
    ui::card(column![
        ui::h2("Layout"),
        ui::muted("Choose the layout you're used to. All three work on the same books: switching never changes your data, and each person on the team can use their own."),
        row![
            layout_card(app, crate::theme::Layout::Desktop, "Menu bar (File, Edit, Lists, Company, Customers, Vendors, Banking, Reports…), left icon bar, Home page workflow, Customer and Vendor Centers, several windows open at once, Ctrl+I / Ctrl+W shortcuts."),
            layout_card(app, crate::theme::Layout::Online, "Left-hand menu, + New with Customers / Vendors / Employees / Other, Business overview, Banking For review, US-style terms (Estimates, Vendors, Credit memos)."),
            layout_card(app, crate::theme::Layout::Xero, "Top menu bar (Business, Accounting, Contacts), dashboard with bank account cards, Reconcile, Xero terms (Quotes, Suppliers, Credit notes)."),
        ].spacing(14),
    ].spacing(12))
}

fn appearance_section(app: &App) -> Element<'_, Message> {
    ui::card(column![
        ui::h2("Appearance"),
        row![ui::muted(if app.dark { "Dark mode is on." } else { "Light mode is on." }), ui::hspace(), ui::btn(if app.dark { "Use light mode" } else { "Use dark mode" }, Some(Message::ToggleTheme))].align_y(Alignment::Center),
    ].spacing(12))
}

fn company_section(app: &App) -> Element<'_, Message> {
    let k = t();
    let s = |key: &str| app.setting(key);
    let info = |label: &str, value: String| -> Element<'_, Message> { row![text(label.to_string()).size(14).color(k.muted).width(220), text(if value.is_empty() { "—".into() } else { value }).size(14)].into() };
    let months = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let fy = s("fy_end_month").parse::<usize>().ok().filter(|m| (1..=12).contains(m)).map(|m| format!("{} {}", s("fy_end_day"), months[m - 1])).unwrap_or_default();
    if app.settings.editing {
        let mut col = column![ui::h2("Company")].spacing(12);
        for (n, (_, label)) in FIELDS.iter().enumerate() {
            col = col.push(ui::field(label, app.settings.values.get(n).map(|s| s.as_str()).unwrap_or(""), move |v| Message::Settings(Msg::Field(n, v))));
        }
        col = col.push(row![ui::btn("Cancel", Some(Message::Settings(Msg::Cancel))), ui::hspace(), ui::btn_primary("Save", if app.settings.busy { None } else { Some(Message::Settings(Msg::Save)) })]);
        return ui::card(col);
    }
    ui::card(column![
        row![ui::h2("Company"), ui::hspace(), ui::btn("Edit", Some(Message::Settings(Msg::Edit)))].align_y(Alignment::Center),
        info("Company name", s("org_name")),
        info("Legal name", s("legal_name")),
        info("Country", s("country")),
        info("Home currency", s("base_currency")),
        info(&format!("{} registered", if s("tax_label").is_empty() { "Tax".into() } else { s("tax_label") }), if s("tax_registered") == "1" || s("tax_registered") == "true" { "Yes".into() } else { "No".into() }),
        info("Tax number", s("tax_number")),
        info("Financial year ends", fy),
        info("Address", s("address").replace('\n', ", ")),
        info("Email", s("email")),
    ].spacing(10))
}

/// Accounting ▸ Set Closing Date: transactions on or before it can't be added, changed or deleted.
fn closing_section(app: &App) -> Element<'_, Message> {
    let k = t();
    let all = app.setting("lock_date_all");
    let others = app.setting("lock_date_others");
    let shown = |iso: &str| if iso.is_empty() { "None".to_string() } else { app.d(iso) };
    let body: Element<Message> = match &app.settings.lock {
        Some(l) => column![
            ui::field("Closing date", &l[0], |v| Message::Settings(Msg::LockField(0, v))),
            ui::field("Later closing date for other users, except advisers (optional)", &l[1], |v| Message::Settings(Msg::LockField(1, v))),
            ui::muted("Leave a date blank to remove it."),
            row![ui::btn("Cancel", Some(Message::Settings(Msg::Cancel))), ui::hspace(), ui::btn_primary("OK", if app.settings.busy { None } else { Some(Message::Settings(Msg::LockSave)) })],
        ].spacing(10).into(),
        None => column![
            row![text("Closing date").size(14).color(k.muted).width(220), text(shown(&all)).size(14)],
            row![text("For other users").size(14).color(k.muted).width(220), text(shown(&others)).size(14)],
        ].spacing(8).into(),
    };
    ui::card(column![
        row![ui::h2("Closing Date"), ui::hspace(), ui::btn("Set Date…", Some(Message::Settings(Msg::LockEdit)))].align_y(Alignment::Center),
        ui::muted("Transactions dated on or before the closing date can't be added, changed or deleted — so filed periods stay as they were."),
        body,
    ].spacing(10))
}

fn file_section(_app: &App) -> Element<'_, Message> {
    ui::card(column![
        ui::h2("Company file"),
        row![ui::muted("Open a different company or create a new one."), ui::hspace(), ui::btn("Switch company", Some(Message::Nav(Route::Setup)))].align_y(Alignment::Center),
        ui::muted("Invoice templates are in Lists ▸ Templates. Bank feeds, currencies and other advanced settings are still in the web version (start.bat web) during the native rewrite."),
    ].spacing(10))
}

/// QuickBooks Desktop Preferences categories (those LedgerCraft has settings for are enabled).
pub const DESK_CATS: [&str; 12] = ["Accounting", "Bills", "Checking", "Desktop View", "General", "Payments", "Reports & Graphs", "Sales & Customers", "Sales Tax", "Send Forms", "Tax: 1099", "Users & Roles"];

fn desk_view(app: &App) -> Element<'_, Message> {
    let k = t();
    let cat = app.settings.cat.unwrap_or("Desktop View");
    let mut list = column![].spacing(1);
    for c in DESK_CATS {
        let on = c == cat;
        list = list.push(button(text(c).size(13).font(if on { BOLD } else { iced::Font::DEFAULT })).style(theme::list_item(on)).padding([5, 10]).width(Fill).on_press(Message::Settings(Msg::Cat(c))));
    }
    let note = |t_: &'static str| -> Element<'_, Message> { ui::card(ui::muted(t_)) };
    let body: Element<Message> = match cat {
        "Desktop View" => column![layout_section(app), appearance_section(app)].spacing(14).into(),
        "Accounting" => column![closing_section(app), company_section(app)].spacing(14).into(),
        "General" => column![company_section(app), file_section(app)].spacing(14).into(),
        "Send Forms" => email_card(app),
        "Users & Roles" => users_card(app),
        "Sales Tax" => ui::card(column![ui::muted("Sales tax rates and agencies are managed in Manage Sales Tax."), row![ui::btn("Manage Sales Tax…", Some(Message::Nav(Route::Taxes)))]].spacing(10)),
        "Tax: 1099" => ui::card(column![ui::muted("Mark vendors as eligible for 1099 in the Vendor Center or New Vendor window; payments to them are totalled on the 1099 Summary."),
            row![ui::btn("1099 Summary", Some(Message::Nav(Route::Report("contractor-1099".into()))))]].spacing(10)),
        _ => note("These preferences use LedgerCraft's defaults for now."),
    };
    let tabs = row![text("My Preferences").size(13).font(BOLD), text("Company Preferences").size(13).color(k.muted)].spacing(20);
    column![
        text("Preferences").size(22).font(BOLD),
        row![
            container(scrollable(list).style(theme::scroll)).width(210).height(Fill).padding(6).style(theme::card),
            scrollable(column![tabs, body].spacing(12).width(Fill)).style(theme::scroll).height(Fill),
        ].spacing(14).height(Fill),
    ].spacing(10).padding(14).height(Fill).into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    if app.desktop() { return desk_view(app); }
    let body = column![layout_section(app), appearance_section(app), company_section(app), closing_section(app), users_card(app), email_card(app), file_section(app)].spacing(16);
    let title = app.term("settings");
    ui::page(ui::h1(title), vec![], container(body).max_width(980).into())
}
