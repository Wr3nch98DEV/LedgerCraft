//! Company picker and new-company wizard.

use crate::app::{App, Message};
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD};
use iced::widget::{Space, button, checkbox, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Element, Fill, Task};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Country(pub &'static str, pub &'static str);
impl std::fmt::Display for Country { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.1) } }
const COUNTRIES: [Country; 7] = [Country("US", "United States"), Country("AU", "Australia"), Country("NZ", "New Zealand"), Country("GB", "United Kingdom"), Country("CA", "Canada"), Country("IE", "Ireland"), Country("XX", "Other")];

pub struct State {
    pub orgs: Vec<Value>,
    pub name: String,
    pub owner: String,
    pub country: Country,
    pub skin: Skin,
    pub demo: bool,
    pub busy: bool,
    pub email: String,
    pub password: String,
    pub code: String,
    /// Company name shown on the sign-in screen.
    pub login_org: String,
}
impl Default for State {
    fn default() -> Self { State { orgs: vec![], name: String::new(), owner: String::new(), country: COUNTRIES[0], skin: Skin::QuickBooks, demo: false, busy: false, email: String::new(), password: String::new(), code: String::new(), login_org: String::new() } }
}

#[derive(Debug, Clone)]
pub enum Msg {
    Orgs(Result<Value, String>),
    Open(String),
    Opened(Result<Value, String>),
    Name(String),
    Owner(String),
    Country(Country),
    Skin(Skin),
    Demo(bool),
    Create,
    QuickDemo,
    Email(String),
    Password(String),
    Code(String),
    SignIn,
    SignedIn(Result<Value, String>),
}

pub fn load(app: &mut App) -> Task<Message> {
    app.setup.skin = app.skin;
    app.data = Some(json!({}));
    app.be.call("orgs.list", json!({}), |r| Message::Setup(Msg::Orgs(r)))
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    match m {
        Msg::Orgs(Ok(v)) => { app.setup.orgs = v.as_array().cloned().unwrap_or_default(); Task::none() }
        Msg::Orgs(Err(e)) => app.toast(e, true),
        Msg::Open(file) => { app.setup.busy = true; app.be.call("orgs.open", json!({"file": file}), |r| Message::Setup(Msg::Opened(r))) }
        Msg::Opened(r) => {
            app.setup.busy = false;
            match r {
                Ok(_) => { app.route = crate::app::Route::Dashboard; app.be.call("session.get", json!({}), Message::Session) }
                Err(e) => app.toast(e, true),
            }
        }
        Msg::Name(s) => { app.setup.name = s; Task::none() }
        Msg::Owner(s) => { app.setup.owner = s; Task::none() }
        Msg::Country(c) => { app.setup.country = c; Task::none() }
        Msg::Skin(s) => { app.setup.skin = s; Task::none() }
        Msg::Demo(d) => { app.setup.demo = d; Task::none() }
        Msg::Email(v) => { app.setup.email = v; Task::none() }
        Msg::Password(v) => { app.setup.password = v; Task::none() }
        Msg::Code(v) => { app.setup.code = v; Task::none() }
        Msg::SignIn => {
            app.setup.busy = true;
            app.be.call("auth.login", json!({"email": app.setup.email.trim(), "password": app.setup.password, "code": app.setup.code.trim()}), |r| Message::Setup(Msg::SignedIn(r)))
        }
        Msg::SignedIn(r) => {
            app.setup.busy = false;
            match r {
                Ok(v) => {
                    app.be.token = v["token"].as_str().map(String::from);
                    app.setup.password.clear();
                    app.setup.code.clear();
                    app.route = crate::app::Route::Dashboard;
                    app.be.call("session.get", json!({}), Message::Session)
                }
                Err(e) => app.toast(e, true),
            }
        }
        Msg::QuickDemo => {
            app.setup.name = "Sample Company".into();
            app.setup.owner = "Alex Morgan".into();
            app.setup.demo = true;
            update(app, Msg::Create)
        }
        Msg::Create => {
            if app.setup.name.trim().is_empty() { return app.toast("Enter your company name", true); }
            app.setup.busy = true;
            if app.skin != app.setup.skin { app.skin = app.setup.skin; app.save_prefs(); }
            let country = if app.setup.country.0 == "XX" { "" } else { app.setup.country.0 };
            app.be.call("orgs.create", json!({"org_name": app.setup.name.trim(), "country": country, "demo": app.setup.demo, "owner_name": app.setup.owner.trim()}), |r| Message::Setup(Msg::Opened(r)))
        }
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    let k = t();
    let s = &app.setup;
    let brand = row![
        container(text("L").size(20).font(BOLD).color(k.accent_ink)).width(40).height(40).center_x(40).center_y(40)
            .style(move |_: &iced::Theme| container::Style { background: Some(iced::Background::Color(k.accent)), border: iced::Border { radius: 8.0.into(), ..Default::default() }, ..Default::default() }),
        column![text("LedgerCraft").size(22).font(BOLD), ui::muted("Open-source accounting for small business")].spacing(2),
    ].spacing(12).align_y(Alignment::Center);

    let mut existing = column![ui::h2("Your companies")].spacing(8);
    if s.orgs.is_empty() { existing = existing.push(ui::muted("No companies yet — create one below, or try the sample company.")); }
    for o in &s.orgs {
        let file = o["file"].as_str().unwrap_or("").to_string();
        existing = existing.push(button(row![
            text(o["name"].as_str().unwrap_or("").to_string()).size(15).font(BOLD).width(Fill),
            ui::muted(file.clone()),
            if o["current"] == true { ui::chip("Open", k.accent) } else { Space::new().into() },
        ].spacing(10).align_y(Alignment::Center)).style(theme::row_btn).padding([10, 12]).width(Fill).on_press_maybe(if s.busy { None } else { Some(Message::Setup(Msg::Open(file))) }));
    }

    let layouts = row![
        button(text("QuickBooks-style").size(14)).style(if s.skin == Skin::QuickBooks { theme::primary } else { theme::secondary }).padding([7, 14]).on_press(Message::Setup(Msg::Skin(Skin::QuickBooks))),
        button(text("Xero-style").size(14)).style(if s.skin == Skin::Xero { theme::primary } else { theme::secondary }).padding([7, 14]).on_press(Message::Setup(Msg::Skin(Skin::Xero))),
    ].spacing(6);
    let create = column![
        ui::h2("Set up a new company"),
        ui::muted("Everything is stored in one file on this computer. You can change these later."),
        row![ui::field("Company name", &s.name, |v| Message::Setup(Msg::Name(v))), ui::field("Your name", &s.owner, |v| Message::Setup(Msg::Owner(v)))].spacing(14),
        row![
            column![ui::label("Country"), pick_list(&COUNTRIES[..], Some(s.country), |c| Message::Setup(Msg::Country(c))).padding([7, 10]).text_size(14).style(theme::pick).width(Fill)].spacing(5).width(Fill),
            column![ui::label("Layout you're used to"), layouts].spacing(5).width(Fill),
        ].spacing(14),
        checkbox(s.demo).label("Fill with 12 months of sample data (for exploring)").on_toggle(|v| Message::Setup(Msg::Demo(v))).text_size(14),
        row![ui::btn("Just show me a sample company", if s.busy { None } else { Some(Message::Setup(Msg::QuickDemo)) }), ui::hspace(), ui::btn_primary("Create company", if s.busy { None } else { Some(Message::Setup(Msg::Create)) })].spacing(8),
    ].spacing(14);

    let mut content = column![brand, ui::card(existing), ui::card(create), ui::muted("LedgerCraft is not affiliated with or endorsed by Intuit (QuickBooks) or Xero Limited.")].spacing(18).max_width(760);
    if app.session.is_some() { content = content.push(ui::btn("Back", Some(Message::Back))); }
    scrollable(container(content).padding(40).center_x(Fill)).style(theme::scroll).height(Fill).into()
}

/// Sign-in for company files that have passwords set (two-step code when enabled).
pub fn login(app: &App) -> Element<'_, Message> {
    let s = &app.setup;
    let pw = text_input("", &s.password).secure(true).on_input(|v| Message::Setup(Msg::Password(v))).on_submit(Message::Setup(Msg::SignIn)).padding([7, 10]).size(14).style(theme::input);
    let card = ui::card(column![
        ui::h2(format!("Welcome back to {}", if s.login_org.is_empty() { "LedgerCraft" } else { &s.login_org })),
        ui::muted("This company file is protected. Sign in with your email and password."),
        ui::field("Email", &s.email, |v| Message::Setup(Msg::Email(v))),
        column![ui::label("Password"), pw].spacing(5),
        ui::field_ph("Two-step code", "Only if you've turned it on", &s.code, |v| Message::Setup(Msg::Code(v))),
        row![ui::btn("Other companies", Some(Message::Nav(crate::app::Route::Setup))), ui::hspace(), ui::btn_primary("Sign in", if s.busy { None } else { Some(Message::Setup(Msg::SignIn)) })].spacing(8),
    ].spacing(12));
    container(container(card).max_width(440)).padding(60).center_x(Fill).into()
}
