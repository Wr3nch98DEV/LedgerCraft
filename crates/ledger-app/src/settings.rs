//! Settings: layout (QuickBooks-style / Xero-style), theme and company details.

use crate::app::{App, Message, Route};
use crate::icons::icon;
use crate::theme::{self, Skin, t};
use crate::ui::{self, BOLD};
use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Fill};

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
        ui::card(column![
            ui::h2("Company"),
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
            ui::muted("Editing company details, users, invoice branding, email and other advanced settings is still in the web version (start.bat web) during the native rewrite."),
        ].spacing(10)),
        ui::card(column![
            ui::h2("Company file"),
            row![ui::muted("Open a different company or create a new one."), ui::hspace(), ui::btn("Switch company", Some(Message::Nav(Route::Setup)))].align_y(Alignment::Center),
        ].spacing(10)),
    ].spacing(16);
    let title = app.term("settings");
    ui::page(ui::h1(title), vec![], container(body).max_width(980).into())
}
