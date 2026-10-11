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
}
impl Default for State {
    fn default() -> Self { State { orgs: vec![], name: String::new(), owner: String::new(), country: COUNTRIES[0], skin: Skin::QuickBooks, demo: false, busy: false } }
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
