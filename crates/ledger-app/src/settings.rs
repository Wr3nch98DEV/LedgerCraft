//! Settings: layout (QuickBooks-style / Xero-style), theme and company details.

use crate::app::{App, Message, Route};
use crate::icons::icon;
use crate::theme::{Skin, t};
use crate::ui::{self, BOLD};
use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Fill, Task};
use serde_json::json;

/// Editable company fields (settings key, label).
pub const FIELDS: [(&str, &str); 7] = [("org_name", "Company name"), ("legal_name", "Legal name"), ("tax_number", "Tax number (EIN / ABN / VAT)"), ("email", "Company email"),
    ("phone", "Phone"), ("website", "Website"), ("address", "Address")];

#[derive(Default)]
pub struct State { pub values: Vec<String>, pub editing: bool, pub busy: bool }

#[derive(Debug, Clone)]
pub enum Msg { Edit, Field(usize, String), Save, Saved(Result<serde_json::Value, String>), Cancel }

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    match m {
        Msg::Edit => { app.settings.values = FIELDS.iter().map(|(k, _)| app.setting(k)).collect(); app.settings.editing = true; Task::none() }
        Msg::Field(n, v) => { if let Some(x) = app.settings.values.get_mut(n) { *x = v; } Task::none() }
        Msg::Cancel => { app.settings.editing = false; Task::none() }
        Msg::Save => {
            if app.settings.values.first().map(|v| v.trim().is_empty()).unwrap_or(true) { return app.toast("Company name can't be empty", true); }
            let mut p = serde_json::Map::new();
            for ((k, _), v) in FIELDS.iter().zip(app.settings.values.iter()) { p.insert(k.to_string(), json!(v.trim())); }
            app.settings.busy = true;
            app.be.call("settings.update", serde_json::Value::Object(p), |r| Message::Settings(Msg::Saved(r)))
        }
        Msg::Saved(r) => {
            app.settings.busy = false;
            match r {
                Ok(_) => { app.settings.editing = false; let t = app.toast("Company details saved", false); Task::batch([t, app.be.call("session.get", json!({}), Message::Session)]) }
                Err(e) => app.toast(e, true),
            }
        }
    }
}

fn layout_card<'a>(app: &App, skin: Skin, desc: &'a str) -> Element<'a, Message> {
    let k = t();
    let active = app.skin == skin;
    button(column![
        row![icon("layout", 20.0, if active { k.accent } else { k.muted }), text(skin.label()).size(16).font(BOLD), ui::hspace(), if active { ui::chip("In use", k.accent) } else { ui::chip("Switch", k.muted) }].spacing(10).align_y(Alignment::Center),
        ui::muted(desc),
    ].spacing(8))
    .padding(16)
    .width(Fill)
    .on_press(Message::SetSkin(skin))
    .style(move |_, s| {
        let hover = matches!(s, button::Status::Hovered);
        button::Style { background: Some(iced::Background::Color(if hover { k.hover } else { k.panel })), text_color: k.text,
            border: iced::Border { color: if active { k.accent } else { k.line }, width: if active { 2.0 } else { 1.0 }, radius: 8.0.into() }, shadow: Default::default(), snap: true }
    })
    .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let k = t();
    let s = |key: &str| app.setting(key);
    let info = |label: &str, value: String| -> Element<'_, Message> { row![text(label.to_string()).size(14).color(k.muted).width(220), text(if value.is_empty() { "—".into() } else { value }).size(14)].into() };
    let months = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let fy = s("fy_end_month").parse::<usize>().ok().filter(|m| (1..=12).contains(m)).map(|m| format!("{} {}", s("fy_end_day"), months[m - 1])).unwrap_or_default();
    let body = column![
        ui::card(column![
            ui::h2("Layout"),
            ui::muted("Choose the layout you're used to. Both work on the same books: switching never changes your data, and each person on the team can use their own."),
            row![
                layout_card(app, Skin::QuickBooks, "Left-hand menu, + New with Customers / Vendors / Employees / Other, Business overview, Banking For review, US-style terms (Estimates, Vendors, Credit memos)."),
                layout_card(app, Skin::Xero, "Top menu bar (Business, Accounting, Contacts), dashboard with bank account cards, Reconcile, Xero terms (Quotes, Suppliers, Credit notes)."),
            ].spacing(14),
        ].spacing(12)),
        ui::card(column![
            ui::h2("Appearance"),
            row![ui::muted(if app.dark { "Dark mode is on." } else { "Light mode is on." }), ui::hspace(), ui::btn(if app.dark { "Use light mode" } else { "Use dark mode" }, Some(Message::ToggleTheme))].align_y(Alignment::Center),
        ].spacing(12)),
        if app.settings.editing {
            let mut col = column![ui::h2("Company")].spacing(12);
            for (n, (_, label)) in FIELDS.iter().enumerate() {
                col = col.push(ui::field(label, app.settings.values.get(n).map(|s| s.as_str()).unwrap_or(""), move |v| Message::Settings(Msg::Field(n, v))));
            }
            col = col.push(row![ui::btn("Cancel", Some(Message::Settings(Msg::Cancel))), ui::hspace(), ui::btn_primary("Save", if app.settings.busy { None } else { Some(Message::Settings(Msg::Save)) })]);
            ui::card(col)
        } else { ui::card(column![
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
            ui::gap(4.0),
            ui::muted("Users, invoice branding, email and other advanced settings are still in the web version (start.bat web) during the native rewrite."),
        ].spacing(10)) },
        ui::card(column![
            ui::h2("Company file"),
            row![ui::muted("Open a different company or create a new one."), ui::hspace(), ui::btn("Switch company", Some(Message::Nav(Route::Setup)))].align_y(Alignment::Center),
        ].spacing(10)),
    ].spacing(16);
    let title = app.term("settings");
    ui::page(ui::h1(title), vec![], container(body).max_width(980).into())
}
