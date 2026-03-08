// Generated automatically by iced_lucide at build time.
// Do not edit manually.
// 62a25911db26b2ff1cf9049afc82b76df5e09965d8a524de4c7cccfd5df91c17
use iced::Font;
use iced::widget::{Text, text};

pub const FONT: &[u8] = include_bytes!("../fonts/lucide.ttf");

/// All icons as `(name, codepoint_str)` pairs.
/// Use this to populate an icon-picker widget.
#[allow(dead_code)]
pub const ALL_ICONS: &[(&str, &str)] = &[
    ("bold", "\u{E05D}"),
    ("clipboard_copy", "\u{E225}"),
    ("heading", "\u{E384}"),
    ("italic", "\u{E0FB}"),
    ("moon", "\u{E11E}"),
    ("redo", "\u{E143}"),
    ("sun", "\u{E178}"),
    ("text_align_center", "\u{E182}"),
    ("text_align_end", "\u{E183}"),
    ("text_align_justify", "\u{E184}"),
    ("text_align_start", "\u{E185}"),
    ("underline", "\u{E19A}"),
    ("undo", "\u{E19B}"),
];

pub fn bold<'a>() -> Text<'a> {
    icon("\u{E05D}")
}

pub fn clipboard_copy<'a>() -> Text<'a> {
    icon("\u{E225}")
}

pub fn heading<'a>() -> Text<'a> {
    icon("\u{E384}")
}

pub fn italic<'a>() -> Text<'a> {
    icon("\u{E0FB}")
}

pub fn moon<'a>() -> Text<'a> {
    icon("\u{E11E}")
}

pub fn redo<'a>() -> Text<'a> {
    icon("\u{E143}")
}

pub fn sun<'a>() -> Text<'a> {
    icon("\u{E178}")
}

pub fn text_align_center<'a>() -> Text<'a> {
    icon("\u{E182}")
}

pub fn text_align_end<'a>() -> Text<'a> {
    icon("\u{E183}")
}

pub fn text_align_justify<'a>() -> Text<'a> {
    icon("\u{E184}")
}

pub fn text_align_start<'a>() -> Text<'a> {
    icon("\u{E185}")
}

pub fn underline<'a>() -> Text<'a> {
    icon("\u{E19A}")
}

pub fn undo<'a>() -> Text<'a> {
    icon("\u{E19B}")
}

/// Render any Lucide icon by its codepoint string.
/// Use this together with [`ALL_ICONS`] to display icons dynamically:
/// ```ignore
/// for (name, cp) in ALL_ICONS {
///     button(render(cp)).on_press(Msg::Pick(name.to_string()))
/// }
/// ```
pub fn render(codepoint: &str) -> Text<'_> {
    text(codepoint).font(Font::with_name("lucide"))
}

fn icon(codepoint: &str) -> Text<'_> {
    render(codepoint)
}
