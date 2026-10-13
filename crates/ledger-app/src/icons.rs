//! Small line icons drawn for LedgerCraft (24×24, stroke style). Tinted at render time.

use iced::widget::svg;
use iced::{Color, Element};

fn path(name: &str) -> &'static str {
    match name {
        "home" => r#"<path d="M3 11l9-7 9 7"/><path d="M5 10v10h14V10"/><path d="M10 20v-6h4v6"/>"#,
        "bank" => r#"<path d="M3 9l9-5 9 5"/><path d="M4 10h16"/><path d="M6 10v8M10 10v8M14 10v8M18 10v8"/><path d="M3 20h18"/>"#,
        "sales" => r#"<path d="M4 4h12l4 4v12H4z"/><path d="M8 12h8M8 16h5M8 8h4"/>"#,
        "expenses" => r#"<rect x="3" y="6" width="18" height="13" rx="2"/><path d="M3 10h18"/><path d="M7 15h4"/>"#,
        "projects" => r#"<rect x="3" y="4" width="7" height="16" rx="1.5"/><rect x="14" y="4" width="7" height="10" rx="1.5"/>"#,
        "payroll" => r#"<circle cx="9" cy="8" r="3.5"/><path d="M2.5 20c.8-3.6 3.4-5.5 6.5-5.5s5.7 1.9 6.5 5.5"/><path d="M17 8h5M19.5 5.5v5"/>"#,
        "reports" => r#"<path d="M4 20V10M10 20V4M16 20v-7M22 20H2"/>"#,
        "taxes" => r#"<path d="M6 3h12v18l-3-2-3 2-3-2-3 2z"/><path d="M9 9l6 6M9.5 14.5h0M14.5 9.5h0"/>"#,
        "mileage" => r#"<path d="M5 17h14l-1.5-6.5A2 2 0 0 0 15.6 9H8.4a2 2 0 0 0-1.9 1.5z"/><circle cx="8" cy="17.5" r="1.8"/><circle cx="16" cy="17.5" r="1.8"/>"#,
        "accounting" => r#"<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M8 7h8M8 11h2M12 11h2M16 11h0M8 15h2M12 15h2M8 18h8"/>"#,
        "users" => r#"<circle cx="9" cy="8" r="3.5"/><path d="M2.5 20c.8-3.6 3.4-5.5 6.5-5.5s5.7 1.9 6.5 5.5"/><path d="M16 4.5a3.5 3.5 0 0 1 0 7M18 14.8c2 .7 3.2 2.5 3.5 5.2"/>"#,
        "apps" => r#"<rect x="4" y="4" width="6" height="6" rx="1"/><rect x="14" y="4" width="6" height="6" rx="1"/><rect x="4" y="14" width="6" height="6" rx="1"/><rect x="14" y="14" width="6" height="6" rx="1"/>"#,
        "calendar" => r#"<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M8 3v4M16 3v4"/>"#,
        "chart" => r#"<path d="M3 3v18h18"/><path d="M7 15l4-4 3 3 5-6"/>"#,
        "docs" => r#"<path d="M8 3h9l3 3v12H8z"/><path d="M4 7v14h12"/>"#,
        "register" => r#"<rect x="3" y="4" width="18" height="16" rx="1.5"/><path d="M3 9h18M3 14h18M9 4v16"/>"#,
        "check-write" => r#"<rect x="2" y="6" width="20" height="12" rx="1.5"/><path d="M5 14h6M14 14h5M5 10h3"/>"#,
        "deposit" => r#"<path d="M12 3v10M8 9l4 4 4-4"/><path d="M4 15v5h16v-5"/>"#,
        "reconcile" => r#"<path d="M9 6h11M9 12h11M9 18h11"/><path d="M3 6l1.5 1.5L7 5M3 12l1.5 1.5L7 11M3 18l1.5 1.5L7 17"/>"#,
        "receive" => r#"<rect x="2" y="7" width="20" height="11" rx="1.5"/><circle cx="12" cy="12.5" r="2.5"/><path d="M5 10v5M19 10v5"/>"#,
        "card" => r#"<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M2 10h20M6 15h4"/>"#,
        "arrow" => r#"<path d="M4 12h15M14 7l5 5-5 5"/>"#,
        "truck" => r#"<path d="M2 6h12v10H2zM14 10h4l3 3v3h-7"/><circle cx="6" cy="17.5" r="1.8"/><circle cx="17" cy="17.5" r="1.8"/>"#,
        "list" => r#"<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>"#,
        "clock" => r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"#,
        "refresh" => r#"<path d="M20 11a8 8 0 1 0-2.3 5.7"/><path d="M20 4v7h-7"/>"#,
        "plus" => r#"<path d="M12 5v14M5 12h14"/>"#,
        "search" => r#"<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/>"#,
        "bell" => r#"<path d="M6 17V11a6 6 0 0 1 12 0v6l1.5 2h-15z"/><path d="M10 21h4"/>"#,
        "help" => r#"<circle cx="12" cy="12" r="9"/><path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.7.3-1 .9-1 1.7"/><path d="M12 17h0"/>"#,
        "gear" => r#"<circle cx="12" cy="12" r="3"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>"#,
        "sun" => r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>"#,
        "moon" => r#"<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"/>"#,
        "chevron" => r#"<path d="M9 6l6 6-6 6"/>"#,
        "down" => r#"<path d="M6 9l6 6 6-6"/>"#,
        "menu" => r#"<path d="M4 6h16M4 12h16M4 18h16"/>"#,
        "x" => r#"<path d="M6 6l12 12M18 6L6 18"/>"#,
        "check" => r#"<path d="M5 12.5l4.5 4.5L19 7.5"/>"#,
        "star" => r#"<path d="M12 3.5l2.6 5.3 5.8.8-4.2 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8-4.2-4.1 5.8-.8z"/>"#,
        "star-on" => r#"<path fill="black" d="M12 3.5l2.6 5.3 5.8.8-4.2 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8-4.2-4.1 5.8-.8z"/>"#,
        "trash" => r#"<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>"#,
        "first" => r#"<path d="M17 6l-6 6 6 6M7 6v12"/>"#,
        "prev" => r#"<path d="M15 6l-6 6 6 6"/>"#,
        "next" => r#"<path d="M9 6l6 6-6 6"/>"#,
        "last" => r#"<path d="M7 6l6 6-6 6M17 6v12"/>"#,
        "save" => r#"<path d="M5 4h11l3 3v13H5zM8 4v5h7V4M8 20v-6h8v6"/>"#,
        "undo" => r#"<path d="M9 8H5V4M5 8a8 8 0 1 1 1.8 8.5"/>"#,
        "swap" => r#"<path d="M4 8h14l-3-3M20 16H6l3 3"/>"#,
        "file" => r#"<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4"/>"#,
        "contacts" => r#"<rect x="4" y="3" width="16" height="18" rx="2"/><circle cx="12" cy="10" r="3"/><path d="M7.5 17.5c.9-2 2.5-3 4.5-3s3.6 1 4.5 3"/>"#,
        "box" => r#"<path d="M3 7l9-4 9 4v10l-9 4-9-4z"/><path d="M3 7l9 4 9-4M12 11v10"/>"#,
        "layout" => r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16M9 10h12"/>"#,
        _ => r#"<circle cx="12" cy="12" r="8"/>"#,
    }
}

/// An icon at the given size, tinted with `color`.
pub fn icon<'a, M: 'a>(name: &str, size: f32, color: Color) -> Element<'a, M> {
    let src = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{}</svg>"#,
        path(name)
    );
    svg(svg::Handle::from_memory(src.into_bytes()))
        .width(size)
        .height(size)
        .style(move |_, _| svg::Style { color: Some(color) })
        .into()
}
