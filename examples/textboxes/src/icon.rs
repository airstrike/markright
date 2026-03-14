// Generated automatically by iced_lucide at build time.
// Do not edit manually.
// b33fc52304287957de17326d3e0c08967452f545efe65fa99568378ee54f32f1
use iced::Font;
use iced::widget::{Text, text};

pub const FONT: &[u8] = include_bytes!("../fonts/lucide.ttf");

/// All icons as `(name, codepoint_str)` pairs.
/// Use this to populate an icon-picker widget.
#[allow(dead_code)]
pub const ALL_ICONS: &[(&str, &str)] = &[
    ("align_v_bottom", "\u{E278}"),
    ("align_v_center", "\u{E27A}"),
    ("align_v_top", "\u{E279}"),
    ("bold", "\u{E05D}"),
    ("italic", "\u{E0FB}"),
    ("underline", "\u{E19A}"),
];

pub fn align_v_bottom<'a>() -> Text<'a> {
    icon("\u{E278}")
}

pub fn align_v_center<'a>() -> Text<'a> {
    icon("\u{E27A}")
}

pub fn align_v_top<'a>() -> Text<'a> {
    icon("\u{E279}")
}

pub fn bold<'a>() -> Text<'a> {
    icon("\u{E05D}")
}

pub fn italic<'a>() -> Text<'a> {
    icon("\u{E0FB}")
}

pub fn underline<'a>() -> Text<'a> {
    icon("\u{E19A}")
}

/// Render any Lucide icon by its codepoint string.
/// Use this together with [`ALL_ICONS`] to display icons dynamically:
/// ```ignore
/// for (name, cp) in ALL_ICONS {
///     button(render(cp)).on_press(Msg::Pick(name.to_string()))
/// }
/// ```
pub fn render(codepoint: &str) -> Text<'_> {
    text(codepoint).font(Font::with_family("lucide"))
}

fn icon(codepoint: &str) -> Text<'_> {
    render(codepoint)
}
