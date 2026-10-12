//! Reusable building blocks: formatting, buttons, fields, cards and a clickable data table.

use crate::theme::{self, t};
use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Color, Element, Fill, Font, Length, Padding};

pub const BOLD: Font = Font { weight: iced::font::Weight::Semibold, ..Font::DEFAULT };

// ---------- formatting ----------

/// Cents → "1,234.56" (negative with a leading minus).
pub fn money(cents: i64) -> String {
    let neg = cents < 0;
    let a = cents.unsigned_abs();
    let whole = (a / 100).to_string();
    let mut g = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 { g.push(','); }
        g.push(ch);
    }
    format!("{}{g}.{:02}", if neg { "-" } else { "" }, a % 100)
}
/// Cents → "1234.56" for editable inputs.
pub fn plain(cents: i64) -> String { format!("{}{}.{:02}", if cents < 0 { "-" } else { "" }, cents.unsigned_abs() / 100, cents.unsigned_abs() % 100) }

/// Parse "1,234.5" → cents.
pub fn parse_money(s: &str) -> Option<i64> {
    let s = s.trim().replace(',', "");
    if s.is_empty() { return Some(0); }
    let neg = s.starts_with('-');
    let s = s.trim_start_matches('-');
    let (w, f) = s.split_once('.').unwrap_or((s, ""));
    if !w.chars().all(|c| c.is_ascii_digit()) || !f.chars().all(|c| c.is_ascii_digit()) || f.len() > 2 { return None; }
    let w: i64 = if w.is_empty() { 0 } else { w.parse().ok()? };
    let f: i64 = format!("{f:0<2}").parse().ok()?;
    let v = w * 100 + f;
    Some(if neg { -v } else { v })
}

/// Date display style for the layout: US-style MM/DD/YYYY (QuickBooks, US companies),
/// DD/MM/YYYY (QuickBooks elsewhere) or "12 Oct 2026" (Xero).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DateStyle { Us, Intl, Long }

pub fn date(s: &str, style: DateStyle) -> String {
    let Ok(d) = chrono::NaiveDate::parse_from_str(s.get(..10).unwrap_or(s), "%Y-%m-%d") else { return s.to_string() };
    match style { DateStyle::Us => d.format("%m/%d/%Y").to_string(), DateStyle::Intl => d.format("%d/%m/%Y").to_string(), DateStyle::Long => d.format("%-d %b %Y").to_string() }
}


/// Parse a date typed in the layout's format (also accepts ISO) into YYYY-MM-DD.
pub fn to_iso(input: &str, style: DateStyle) -> Option<String> {
    let s = input.trim();
    if s.is_empty() { return None; }
    let fmts: &[&str] = match style { DateStyle::Us => &["%m/%d/%Y", "%Y-%m-%d"], DateStyle::Intl => &["%d/%m/%Y", "%Y-%m-%d"], DateStyle::Long => &["%d %b %Y", "%d/%m/%Y", "%Y-%m-%d"] };
    fmts.iter().find_map(|f| chrono::NaiveDate::parse_from_str(s, f).ok()).map(|d| d.format("%Y-%m-%d").to_string())
}
/// The first of the month `n` months after `d` (which should be a 1st).
pub fn add_months_date(d: chrono::NaiveDate, n: i32) -> chrono::NaiveDate {
    use chrono::Datelike;
    let m0 = d.year() * 12 + d.month0() as i32 + n;
    chrono::NaiveDate::from_ymd_opt(m0.div_euclid(12), m0.rem_euclid(12) as u32 + 1, 1).unwrap_or(d)
}
pub fn today_iso() -> String { chrono::Local::now().date_naive().format("%Y-%m-%d").to_string() }
pub fn add_days_iso(iso: &str, days: i64) -> String {
    chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d").map(|d| (d + chrono::Duration::days(days)).format("%Y-%m-%d").to_string()).unwrap_or_else(|_| iso.to_string())
}

// ---------- text ----------
pub fn h1<'a, M: 'a>(s: impl Into<String>) -> Element<'a, M> { text(s.into()).size(24).font(BOLD).into() }
pub fn h2<'a, M: 'a>(s: impl Into<String>) -> Element<'a, M> { text(s.into()).size(17).font(BOLD).into() }
pub fn label<'a, M: 'a>(s: impl Into<String>) -> Element<'a, M> { text(s.into()).size(12).color(t().muted).font(BOLD).into() }
pub fn muted<'a, M: 'a>(s: impl Into<String>) -> Element<'a, M> { text(s.into()).size(13).color(t().muted).into() }
pub fn body<'a, M: 'a>(s: impl Into<String>) -> Element<'a, M> { text(s.into()).size(14).into() }
pub fn num<'a, M: 'a>(s: impl Into<String>) -> Element<'a, M> { text(s.into()).size(14).align_x(iced::alignment::Horizontal::Right).width(Fill).into() }

// ---------- buttons ----------
pub fn btn_primary<'a, M: Clone + 'a>(label: &str, msg: Option<M>) -> Element<'a, M> {
    button(text(label.to_string()).size(14).font(BOLD)).padding([7, 16]).style(theme::primary).on_press_maybe(msg).into()
}
pub fn btn<'a, M: Clone + 'a>(label: &str, msg: Option<M>) -> Element<'a, M> {
    button(text(label.to_string()).size(14)).padding([7, 14]).style(theme::secondary).on_press_maybe(msg).into()
}
pub fn btn_danger<'a, M: Clone + 'a>(label: &str, msg: Option<M>) -> Element<'a, M> {
    button(text(label.to_string()).size(14)).padding([7, 14]).style(theme::danger).on_press_maybe(msg).into()
}
pub fn link<'a, M: Clone + 'a>(label: impl Into<String>, msg: M) -> Element<'a, M> {
    button(text(label.into()).size(14)).padding(0).style(theme::link).on_press(msg).into()
}

// ---------- layout ----------
pub fn card<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> Element<'a, M> { container(content).padding(18).width(Fill).style(theme::card).into() }
pub fn hspace<'a, M: 'a>() -> Element<'a, M> { Space::new().width(Fill).into() }
pub fn gap<'a, M: 'a>(h: f32) -> Element<'a, M> { Space::new().height(h).into() }
pub fn hr<'a, M: 'a>() -> Element<'a, M> { container(Space::new()).width(Fill).height(1).style(theme::rule).into() }

pub fn chip<'a, M: 'a>(label: &str, color: Color) -> Element<'a, M> {
    container(text(label.to_string()).size(12).font(BOLD).wrapping(iced::widget::text::Wrapping::None)).padding([2, 9]).style(theme::chip(color)).into()
}

/// A labelled text field.
pub fn field<'a, M: Clone + 'a>(lbl: &str, value: &str, on_input: impl Fn(String) -> M + 'a) -> Element<'a, M> {
    column![label(lbl), text_input("", value).on_input(on_input).padding([7, 10]).size(14).style(theme::input)].spacing(5).width(Fill).into()
}
pub fn field_ph<'a, M: Clone + 'a>(lbl: &str, ph: &str, value: &str, on_input: impl Fn(String) -> M + 'a) -> Element<'a, M> {
    column![label(lbl), text_input(ph, value).on_input(on_input).padding([7, 10]).size(14).style(theme::input)].spacing(5).width(Fill).into()
}

/// Page wrapper: title row + scrolling content.
pub fn page<'a, M: 'a>(title: Element<'a, M>, actions: Vec<Element<'a, M>>, content: Element<'a, M>) -> Element<'a, M> {
    let head = row![title, hspace()].extend(actions).spacing(8).align_y(Alignment::Center);
    scrollable(column![head, content].spacing(16).padding(Padding { top: 22.0, right: 28.0, bottom: 40.0, left: 28.0 }).max_width(1500))
        .style(theme::scroll).height(Fill).width(Fill).into()
}

// ---------- data table ----------

#[derive(Clone, Copy)]
pub struct Col { pub title: &'static str, pub width: u16, pub right: bool }
pub const fn col(title: &'static str, width: u16) -> Col { Col { title, width, right: false } }
pub const fn coln(title: &'static str, width: u16) -> Col { Col { title, width, right: true } }

/// A table whose rows are clickable. `cells` returns the cell elements for each row.
pub fn table<'a, M: Clone + 'a>(cols: &[Col], rows: Vec<(Vec<Element<'a, M>>, Option<M>)>, empty: &str) -> Element<'a, M> {
    let header = container(
        row(cols.iter().map(|c| {
            let tx = text(c.title.to_uppercase()).size(11).font(BOLD).color(t().muted);
            let tx = if c.right { tx.align_x(iced::alignment::Horizontal::Right) } else { tx };
            container(tx.width(Fill)).width(Length::FillPortion(c.width)).into()
        })).spacing(12).padding([9, 14]),
    ).style(theme::header_row).width(Fill);
    let mut body = column![header].width(Fill);
    if rows.is_empty() {
        body = body.push(container(muted(empty.to_string())).padding(28).center_x(Fill));
    }
    for (cells, msg) in rows {
        let r = row(cells.into_iter().zip(cols.iter()).map(|(e, c)| container(e).width(Length::FillPortion(c.width)).into())).spacing(12).align_y(Alignment::Center);
        let r = button(r.padding([9, 14])).style(theme::row_btn).padding(0).width(Fill).on_press_maybe(msg);
        body = body.push(column![r, hr()]);
    }
    container(body).style(theme::card).width(Fill).padding(0).clip(true).into()
}

/// A right-aligned money cell.
pub fn cell_money<'a, M: 'a>(cents: i64) -> Element<'a, M> { num(money(cents)) }
pub fn cell<'a, M: 'a>(s: impl Into<String>) -> Element<'a, M> { text(s.into()).size(14).into() }
pub fn cell_muted<'a, M: 'a>(s: impl Into<String>) -> Element<'a, M> { text(s.into()).size(13).color(t().muted).into() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn money_formats() {
        assert_eq!(money(0), "0.00");
        assert_eq!(money(123456), "1,234.56");
        assert_eq!(money(-5), "-0.05");
        assert_eq!(money(100000000), "1,000,000.00");
    }
    #[test]
    fn money_parses() {
        assert_eq!(parse_money("1,234.5"), Some(123450));
        assert_eq!(parse_money("-0.05"), Some(-5));
        assert_eq!(parse_money(""), Some(0));
        assert_eq!(parse_money("1.234"), None);
        assert_eq!(parse_money("abc"), None);
    }
    #[test]
    fn date_entry() {
        assert_eq!(to_iso("10/12/2026", DateStyle::Us).as_deref(), Some("2026-10-12"));
        assert_eq!(to_iso("12/10/2026", DateStyle::Intl).as_deref(), Some("2026-10-12"));
        assert_eq!(to_iso("12 Oct 2026", DateStyle::Long).as_deref(), Some("2026-10-12"));
        assert_eq!(to_iso("2026-10-12", DateStyle::Us).as_deref(), Some("2026-10-12"));
        assert_eq!(to_iso("31/31/2026", DateStyle::Intl), None);
    }
    #[test]
    fn dates() {
        assert_eq!(date("2026-10-12", DateStyle::Us), "10/12/2026");
        assert_eq!(date("2026-10-12", DateStyle::Intl), "12/10/2026");
        assert_eq!(date("2026-10-12", DateStyle::Long), "12 Oct 2026");
    }
}
