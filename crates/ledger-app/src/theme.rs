//! Design tokens for each layout (skin) in dark and light mode, plus style functions for widgets.
//!
//! Tokens are published per frame into a thread-local so style closures can read them without
//! threading a palette through every view function (iced calls views on the UI thread).

use iced::widget::{button, container, pick_list, scrollable, text_input};
use iced::{Background, Border, Color, Shadow, Theme, Vector};
use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skin { QuickBooks, Xero }

impl Skin {
    pub fn label(self) -> &'static str { match self { Skin::QuickBooks => "QuickBooks-style", Skin::Xero => "Xero-style" } }
}

/// The screen layout. Desktop and Online are both QuickBooks (same terms, `Skin::QuickBooks`);
/// they differ in navigation and screen design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout { Desktop, Online, Xero, Acumatica, Sage }

impl Layout {
    pub const ALL: [Layout; 5] = [Layout::Desktop, Layout::Online, Layout::Xero, Layout::Acumatica, Layout::Sage];
    pub fn key(self) -> &'static str { match self { Layout::Desktop => "desktop", Layout::Online => "quickbooks", Layout::Xero => "xero", Layout::Acumatica => "acumatica", Layout::Sage => "sage" } }
    pub fn from_key(s: &str) -> Layout { match s { "xero" => Layout::Xero, "acumatica" | "myob" => Layout::Acumatica, "sage" => Layout::Sage, "quickbooks" | "online" | "qbo" => Layout::Online, _ => Layout::Desktop } }
    /// Sage shares Xero's words (suppliers, credit notes, quotes); Acumatica shares QuickBooks'.
    pub fn skin(self) -> Skin { if matches!(self, Layout::Xero | Layout::Sage) { Skin::Xero } else { Skin::QuickBooks } }
    pub fn label(self) -> &'static str { match self { Layout::Desktop => "QuickBooks Desktop-style", Layout::Online => "QuickBooks Online-style", Layout::Xero => "Xero-style", Layout::Acumatica => "MYOB Acumatica-style", Layout::Sage => "Sage Accounting-style" } }
}
impl From<Skin> for Layout { fn from(s: Skin) -> Layout { match s { Skin::QuickBooks => Layout::Online, Skin::Xero => Layout::Xero } } }

/// Colours for a layout. Desktop has its own: charcoal icon bar and menu chrome, flat panels.
pub fn tokens_for(layout: Layout, dark: bool) -> Tokens {
    match (layout, dark) {
        (Layout::Desktop, true) => Tokens {
            dark, bg: hex(0x16181a), bg2: hex(0x1c1f22), panel: hex(0x212427), panel2: hex(0x292d31), hover: hex(0x30353a),
            line: hex(0x353a40), line2: hex(0x474e55), text: hex(0xe9ecef), muted: hex(0xa3abb3), faint: hex(0x6f7780),
            accent: hex(0x3cb04a), accent_ink: hex(0xffffff), accent_soft: hexa(0x3cb04a, 0.16), ok: hex(0x4cc35a), warn: hex(0xf0b43c),
            bad: hex(0xf06a6a), info: hex(0x6ab0f3), nav_bg: hex(0x101214), nav_text: hex(0xe9ecef), nav_muted: hex(0xa3abb3),
            nav_active: hex(0x3cb04a), nav_hover: hex(0x24282c),
        },
        (Layout::Desktop, false) => Tokens {
            dark, bg: hex(0xe6e9ec), bg2: hex(0xdde1e5), panel: hex(0xffffff), panel2: hex(0xf1f3f5), hover: hex(0xe3ecf7),
            line: hex(0xc5cbd1), line2: hex(0xa9b1b9), text: hex(0x1d2124), muted: hex(0x5a6168), faint: hex(0x8a9198),
            accent: hex(0x2ca01c), accent_ink: hex(0xffffff), accent_soft: hexa(0x2ca01c, 0.12), ok: hex(0x2a8a1c), warn: hex(0xb06d00),
            bad: hex(0xc8281e), info: hex(0x0b5fa5), nav_bg: hex(0x2f3439), nav_text: hex(0xffffff), nav_muted: hex(0xc3c9cf),
            nav_active: hex(0x6fd15f), nav_hover: hex(0x40464c),
        },
        // MYOB Acumatica-style: deep navy main menu, blue-violet accent, square-ish panels.
        (Layout::Acumatica, true) => Tokens {
            dark, bg: hex(0x0f131a), bg2: hex(0x141a23), panel: hex(0x18202b), panel2: hex(0x1f2834), hover: hex(0x263141),
            line: hex(0x2b3646), line2: hex(0x3c4a5e), text: hex(0xe8edf4), muted: hex(0xa0abba), faint: hex(0x6c7889),
            accent: hex(0x6d8dff), accent_ink: hex(0xffffff), accent_soft: hexa(0x6d8dff, 0.16), ok: hex(0x43c27a), warn: hex(0xf0b43c),
            bad: hex(0xf06a6a), info: hex(0x5fb3f0), nav_bg: hex(0x0b1018), nav_text: hex(0xe8edf4), nav_muted: hex(0x93a0b2),
            nav_active: hex(0x8aa4ff), nav_hover: hex(0x1a2331),
        },
        (Layout::Acumatica, false) => Tokens {
            dark, bg: hex(0xeef1f5), bg2: hex(0xe4e8ee), panel: hex(0xffffff), panel2: hex(0xf3f5f8), hover: hex(0xe6ecf8),
            line: hex(0xccd3dd), line2: hex(0xaeb8c6), text: hex(0x1c2430), muted: hex(0x56606e), faint: hex(0x8a94a2),
            accent: hex(0x3b5bdb), accent_ink: hex(0xffffff), accent_soft: hexa(0x3b5bdb, 0.10), ok: hex(0x1f8a4c), warn: hex(0xb06d00),
            bad: hex(0xc8281e), info: hex(0x0b6bb5), nav_bg: hex(0x1d2838), nav_text: hex(0xffffff), nav_muted: hex(0xb7c1cf),
            nav_active: hex(0x9fb4ff), nav_hover: hex(0x2a374b),
        },
        // Sage Accounting-style: black top bar, bright green accent.
        (Layout::Sage, true) => Tokens {
            dark, bg: hex(0x0c0e0d), bg2: hex(0x121513), panel: hex(0x171a18), panel2: hex(0x1d211f), hover: hex(0x232826),
            line: hex(0x2a302d), line2: hex(0x3a423e), text: hex(0xeaefec), muted: hex(0x9ca8a2), faint: hex(0x6b7671),
            accent: hex(0x00c837), accent_ink: hex(0x000000), accent_soft: hexa(0x00c837, 0.15), ok: hex(0x2fcf62), warn: hex(0xf2b33d),
            bad: hex(0xf06a6a), info: hex(0x5aa9f0), nav_bg: hex(0x000000), nav_text: hex(0xffffff), nav_muted: hex(0xa7b0ab),
            nav_active: hex(0x00e03d), nav_hover: hex(0x1a1d1b),
        },
        (Layout::Sage, false) => Tokens {
            dark, bg: hex(0xf2f5f3), bg2: hex(0xe8ece9), panel: hex(0xffffff), panel2: hex(0xf6f8f7), hover: hex(0xe9f2ec),
            line: hex(0xd3dad6), line2: hex(0xb7c1bb), text: hex(0x1a1f1c), muted: hex(0x56615b), faint: hex(0x86918b),
            accent: hex(0x008a26), accent_ink: hex(0xffffff), accent_soft: hexa(0x008a26, 0.10), ok: hex(0x008a26), warn: hex(0xb06d00),
            bad: hex(0xc8281e), info: hex(0x0b6bb5), nav_bg: hex(0x000000), nav_text: hex(0xffffff), nav_muted: hex(0xb9c2bd),
            nav_active: hex(0x00e03d), nav_hover: hex(0x1f2421),
        },
        _ => tokens(layout.skin(), dark),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub dark: bool,
    pub bg: Color,
    pub bg2: Color,
    pub panel: Color,
    pub panel2: Color,
    pub hover: Color,
    pub line: Color,
    pub line2: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub accent: Color,
    pub accent_ink: Color,
    pub accent_soft: Color,
    pub ok: Color,
    pub warn: Color,
    pub bad: Color,
    pub info: Color,
    /// Navigation chrome (QuickBooks sidebar / Xero top bar).
    pub nav_bg: Color,
    pub nav_text: Color,
    pub nav_muted: Color,
    pub nav_active: Color,
    pub nav_hover: Color,
}

fn hex(v: u32) -> Color { Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8) }
fn hexa(v: u32, a: f32) -> Color { let c = hex(v); Color { a, ..c } }

pub fn tokens(skin: Skin, dark: bool) -> Tokens {
    match (skin, dark) {
        // QuickBooks-style: dark sidebar, green accent, white cards on a grey canvas (light).
        (Skin::QuickBooks, true) => Tokens {
            dark, bg: hex(0x0f1114), bg2: hex(0x15181c), panel: hex(0x1b1f24), panel2: hex(0x21262c), hover: hex(0x272d34),
            line: hex(0x2c333b), line2: hex(0x3a434d), text: hex(0xe8ebef), muted: hex(0x9aa4af), faint: hex(0x6b7480),
            accent: hex(0x2fae4e), accent_ink: hex(0xffffff), accent_soft: hexa(0x2fae4e, 0.16), ok: hex(0x41c068), warn: hex(0xf2b33d),
            bad: hex(0xf06a6a), info: hex(0x5aa9f0), nav_bg: hex(0x16191d), nav_text: hex(0xe8ebef), nav_muted: hex(0x9aa4af),
            nav_active: hex(0x2fae4e), nav_hover: hex(0x23282e),
        },
        (Skin::QuickBooks, false) => Tokens {
            dark, bg: hex(0xf4f5f8), bg2: hex(0xeceef1), panel: hex(0xffffff), panel2: hex(0xf8f9fa), hover: hex(0xeef1f4),
            line: hex(0xdcdfe3), line2: hex(0xc4c9cf), text: hex(0x393a3d), muted: hex(0x6b6c72), faint: hex(0x8d9096),
            accent: hex(0x2a9a45), accent_ink: hex(0xffffff), accent_soft: hexa(0x2a9a45, 0.12), ok: hex(0x2a9a45), warn: hex(0xc27a00),
            bad: hex(0xd52b1e), info: hex(0x0077c5), nav_bg: hex(0x393a3d), nav_text: hex(0xffffff), nav_muted: hex(0xbabec5),
            nav_active: hex(0x2fae4e), nav_hover: hex(0x4a4b4f),
        },
        // Xero-style: navy top bar, teal accent (today's LedgerCraft palette).
        (Skin::Xero, true) => Tokens {
            dark, bg: hex(0x0a0f1c), bg2: hex(0x0e1526), panel: hex(0x121a2d), panel2: hex(0x172036), hover: hex(0x1c2742),
            line: hex(0x24304b), line2: hex(0x2e3c5c), text: hex(0xe6eaf2), muted: hex(0x93a0b8), faint: hex(0x66738c),
            accent: hex(0x2dd4bf), accent_ink: hex(0x04201c), accent_soft: hexa(0x2dd4bf, 0.12), ok: hex(0x34d399), warn: hex(0xfbbf24),
            bad: hex(0xf87171), info: hex(0x60a5fa), nav_bg: hex(0x0d1424), nav_text: hex(0xe6eaf2), nav_muted: hex(0x93a0b8),
            nav_active: hex(0x2dd4bf), nav_hover: hex(0x1c2742),
        },
        (Skin::Xero, false) => Tokens {
            dark, bg: hex(0xf4f6fa), bg2: hex(0xeef1f6), panel: hex(0xffffff), panel2: hex(0xf8fafc), hover: hex(0xeef2f8),
            line: hex(0xe2e8f0), line2: hex(0xcbd5e1), text: hex(0x0f172a), muted: hex(0x526077), faint: hex(0x8592a8),
            accent: hex(0x0d9488), accent_ink: hex(0xffffff), accent_soft: hexa(0x0d9488, 0.1), ok: hex(0x059669), warn: hex(0xb45309),
            bad: hex(0xdc2626), info: hex(0x2563eb), nav_bg: hex(0x16233a), nav_text: hex(0xffffff), nav_muted: hex(0xb4c0d4),
            nav_active: hex(0x2dd4bf), nav_hover: hex(0x23314b),
        },
    }
}

thread_local! { static CURRENT: Cell<Tokens> = Cell::new(tokens(Skin::QuickBooks, true)); }

/// Publish the tokens used by style functions for this frame.
pub fn set_current(t: Tokens) { CURRENT.with(|c| c.set(t)); }
/// Tokens for the current frame.
pub fn t() -> Tokens { CURRENT.with(|c| c.get()) }

pub fn iced_theme(tk: Tokens) -> Theme {
    Theme::custom("LedgerCraft", iced::theme::Palette { background: tk.bg, text: tk.text, primary: tk.accent, success: tk.ok, warning: tk.warn, danger: tk.bad })
}

fn radius(r: f32) -> iced::border::Radius { r.into() }

// ---------- containers ----------
pub fn page(_: &Theme) -> container::Style { let k = t(); container::Style { background: Some(Background::Color(k.bg)), text_color: Some(k.text), ..Default::default() } }
pub fn card(_: &Theme) -> container::Style {
    let k = t();
    container::Style {
        background: Some(Background::Color(k.panel)), text_color: Some(k.text),
        border: Border { color: k.line, width: 1.0, radius: radius(if k.dark { 10.0 } else { 6.0 }) },
        shadow: if k.dark { Shadow::default() } else { Shadow { color: Color { a: 0.06, ..Color::BLACK }, offset: Vector::new(0.0, 1.0), blur_radius: 3.0 } },
        ..Default::default()
    }
}
pub fn soft(_: &Theme) -> container::Style { let k = t(); container::Style { background: Some(Background::Color(k.bg2)), border: Border { radius: radius(6.0), ..Default::default() }, ..Default::default() } }
pub fn bad_soft(_: &Theme) -> container::Style { let k = t(); container::Style { background: Some(Background::Color(Color { a: 0.14, ..k.bad })), border: Border { radius: radius(6.0), ..Default::default() }, ..Default::default() } }
pub fn accent_soft(_: &Theme) -> container::Style { let k = t(); container::Style { background: Some(Background::Color(k.accent_soft)), border: Border { radius: radius(6.0), ..Default::default() }, ..Default::default() } }
pub fn nav(_: &Theme) -> container::Style { let k = t(); container::Style { background: Some(Background::Color(k.nav_bg)), text_color: Some(k.nav_text), ..Default::default() } }
pub fn topbar(_: &Theme) -> container::Style {
    let k = t();
    container::Style { background: Some(Background::Color(k.panel)), border: Border { color: k.line, width: 1.0, radius: radius(0.0) }, ..Default::default() }
}
pub fn rule(_: &Theme) -> container::Style { let k = t(); container::Style { background: Some(Background::Color(k.line)), ..Default::default() } }
pub fn header_row(_: &Theme) -> container::Style { let k = t(); container::Style { background: Some(Background::Color(k.panel2)), text_color: Some(k.muted), ..Default::default() } }
pub fn menu(_: &Theme) -> container::Style {
    let k = t();
    container::Style {
        background: Some(Background::Color(k.panel)), text_color: Some(k.text), border: Border { color: k.line2, width: 1.0, radius: radius(8.0) },
        shadow: Shadow { color: Color { a: 0.35, ..Color::BLACK }, offset: Vector::new(0.0, 8.0), blur_radius: 24.0 }, ..Default::default()
    }
}
pub fn chip(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style { background: Some(Background::Color(Color { a: 0.16, ..color })), text_color: Some(color), border: Border { radius: radius(999.0), ..Default::default() }, ..Default::default() }
}
pub fn toast(_: &Theme) -> container::Style {
    let k = t();
    container::Style { background: Some(Background::Color(k.panel2)), text_color: Some(k.text), border: Border { color: k.line2, width: 1.0, radius: radius(8.0) },
        shadow: Shadow { color: Color { a: 0.3, ..Color::BLACK }, offset: Vector::new(0.0, 6.0), blur_radius: 18.0 }, ..Default::default() }
}

// ---------- buttons ----------
fn btn_base(bg: Color, fg: Color, border: Color, status: button::Status, hover_bg: Color) -> button::Style {
    let bg = match status { button::Status::Hovered | button::Status::Pressed => hover_bg, _ => bg };
    let mut s = button::Style { background: Some(Background::Color(bg)), text_color: fg, border: Border { color: border, width: 1.0, radius: radius(6.0) }, shadow: Shadow::default(), snap: true };
    if status == button::Status::Disabled { s.text_color = Color { a: 0.45, ..fg }; s.background = Some(Background::Color(Color { a: 0.5, ..bg })); }
    s
}
pub fn primary(_: &Theme, s: button::Status) -> button::Style { let k = t(); btn_base(k.accent, k.accent_ink, k.accent, s, Color { a: 0.88, ..k.accent }) }
pub fn secondary(_: &Theme, s: button::Status) -> button::Style { let k = t(); btn_base(k.panel2, k.text, k.line2, s, k.hover) }
pub fn danger(_: &Theme, s: button::Status) -> button::Style { let k = t(); btn_base(Color::TRANSPARENT, k.bad, Color { a: 0.45, ..k.bad }, s, Color { a: 0.12, ..k.bad }) }
pub fn ghost(_: &Theme, s: button::Status) -> button::Style { let k = t(); btn_base(Color::TRANSPARENT, k.text, Color::TRANSPARENT, s, k.hover) }
pub fn link(_: &Theme, s: button::Status) -> button::Style {
    let k = t();
    button::Style { background: None, text_color: if s == button::Status::Hovered { Color { a: 0.8, ..k.accent } } else { k.accent }, border: Border::default(), shadow: Shadow::default(), snap: true }
}
/// A whole table row that can be clicked.
pub fn row_btn(_: &Theme, s: button::Status) -> button::Style {
    let k = t();
    let bg = match s { button::Status::Hovered | button::Status::Pressed => Some(Background::Color(k.hover)), _ => None };
    button::Style { background: bg, text_color: k.text, border: Border::default(), shadow: Shadow::default(), snap: true }
}
pub fn nav_item(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, s| {
        let k = t();
        let bg = if active { Some(Background::Color(k.nav_hover)) } else { match s { button::Status::Hovered | button::Status::Pressed => Some(Background::Color(k.nav_hover)), _ => None } };
        button::Style { background: bg, text_color: if active { k.nav_text } else { k.nav_muted }, border: Border { radius: radius(6.0), ..Default::default() }, shadow: Shadow::default(), snap: true }
    }
}
/// A selectable row in a list inside the page (not the sidebar): readable in light and dark.
pub fn list_item(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, s| {
        let k = t();
        let bg = if active { Some(Background::Color(k.accent_soft)) } else { match s { button::Status::Hovered | button::Status::Pressed => Some(Background::Color(k.hover)), _ => None } };
        button::Style { background: bg, text_color: k.text, border: Border { radius: radius(6.0), ..Default::default() }, shadow: Shadow::default(), snap: true }
    }
}
pub fn tab(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, s| {
        let k = t();
        let fg = if active { k.text } else if s == button::Status::Hovered { k.text } else { k.muted };
        button::Style { background: None, text_color: fg, border: Border::default(), shadow: Shadow::default(), snap: true }
    }
}
pub fn menu_item(_: &Theme, s: button::Status) -> button::Style {
    let k = t();
    let bg = match s { button::Status::Hovered | button::Status::Pressed => Some(Background::Color(k.hover)), _ => None };
    button::Style { background: bg, text_color: k.text, border: Border { radius: radius(5.0), ..Default::default() }, shadow: Shadow::default(), snap: true }
}

// ---------- inputs ----------
pub fn input(_: &Theme, s: text_input::Status) -> text_input::Style {
    let k = t();
    let border = match s { text_input::Status::Focused { .. } => k.accent, text_input::Status::Hovered => k.line2, _ => k.line };
    text_input::Style { background: Background::Color(k.bg2), border: Border { color: border, width: 1.0, radius: radius(6.0) }, icon: k.muted, placeholder: k.faint, value: k.text, selection: Color { a: 0.35, ..k.accent } }
}
/// Borderless input used inside line-item grids.
pub fn cell_input(_: &Theme, s: text_input::Status) -> text_input::Style {
    let k = t();
    let (bg, border) = match s { text_input::Status::Focused { .. } => (k.bg2, k.accent), text_input::Status::Hovered => (k.bg2, k.line2), _ => (Color::TRANSPARENT, k.line) };
    text_input::Style { background: Background::Color(bg), border: Border { color: border, width: 1.0, radius: radius(4.0) }, icon: k.muted, placeholder: k.faint, value: k.text, selection: Color { a: 0.35, ..k.accent } }
}
pub fn pick(_: &Theme, s: pick_list::Status) -> pick_list::Style {
    let k = t();
    let border = match s { pick_list::Status::Hovered | pick_list::Status::Opened { .. } => k.line2, _ => k.line };
    pick_list::Style { text_color: k.text, placeholder_color: k.faint, handle_color: k.muted, background: Background::Color(k.bg2), border: Border { color: border, width: 1.0, radius: radius(6.0) } }
}
pub fn scroll(theme: &Theme, s: scrollable::Status) -> scrollable::Style {
    let k = t();
    let mut st = scrollable::default(theme, s);
    st.vertical_rail.scroller.background = Background::Color(k.line2);
    st.horizontal_rail.scroller.background = Background::Color(k.line2);
    st.vertical_rail.background = None;
    st.horizontal_rail.background = None;
    st
}
