//! Debug panel — cursor/style summary + live `.mr` serialization.

use iced::advanced::graphics::core::font;
use iced::widget::{button, column, container, right, scrollable, stack, text};
use iced::{Element, Fill};

use markright::widget::rich_editor::{Alignment, Content};

const SIZE: f32 = 11.0;
pub const PANEL_W: f32 = 260.0;

fn align_char(a: Alignment) -> char {
    match a {
        Alignment::Left => 'L',
        Alignment::Center => 'C',
        Alignment::Right => 'R',
        Alignment::Justified => 'J',
    }
}

/// Build the full debug string for display and clipboard copy.
pub fn to_string(content: &Content<iced::Renderer>) -> String {
    let mut out = String::new();

    // Cursor/selection summary (at the top for quick reference)
    let cursor = content.cursor();
    let ctx = content.cursor_context();

    out.push_str(&format!(
        "cursor   {}:{}\n",
        cursor.position.line, cursor.position.column
    ));
    if let Some(sel) = cursor.selection {
        out.push_str(&format!(
            "select   {}:{} \u{2192} {}:{}\n",
            cursor.position.line, cursor.position.column, sel.line, sel.column
        ));
    } else {
        out.push_str("select   \u{2500}\n");
    }

    // Style summary
    let mut style_parts = Vec::new();
    if ctx.character.bold {
        style_parts.push("B".to_string());
    }
    if ctx.character.italic {
        style_parts.push("I".to_string());
    }
    if ctx.character.underline {
        style_parts.push("U".to_string());
    }
    if let Some(font) = ctx.character.font
        && let iced::font::Family::Name(n) = font.family
    {
        style_parts.push(n.to_string());
    }
    if let Some(size) = ctx.character.size {
        style_parts.push(format!("{size:.0}px"));
    }
    if let Some(color) = ctx.character.color {
        style_parts.push(format!(
            "#{:02x}{:02x}{:02x}",
            (color.r * 255.0) as u8,
            (color.g * 255.0) as u8,
            (color.b * 255.0) as u8,
        ));
    }
    if style_parts.is_empty() {
        style_parts.push("\u{2500}".to_string());
    }
    out.push_str(&format!("style    {}\n", style_parts.join(" ")));

    out.push_str(&format!(
        "para     {} spacing:{:.0}\n",
        align_char(ctx.paragraph.alignment),
        ctx.paragraph.spacing_after,
    ));

    let undo_n = content.undo_len();
    let redo_n = content.redo_len();
    out.push_str(&format!(
        "undo     {}\n",
        if undo_n > 0 {
            format!("{undo_n} groups")
        } else {
            "\u{2500}".to_string()
        }
    ));
    out.push_str(&format!(
        "redo     {}\n",
        if redo_n > 0 {
            format!("{redo_n} groups")
        } else {
            "\u{2500}".to_string()
        }
    ));

    out.push('\n');
    out.push_str(&content.serialize());

    out
}

const LIGA: font::Tag = font::Tag::new(b"liga");

pub fn view<'a, Message: Clone + 'a>(
    content: &Content<iced::Renderer>,
    on_copy: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    let debug_str = to_string(content);

    let copy_btn = button(crate::icon::clipboard_copy().size(16))
        .padding([4, 8])
        .style(crate::theme::button::icon)
        .on_press(on_copy(debug_str.clone()));

    let debug_text = text(debug_str)
        .font(iced::Font::with_family("Fira Code"))
        .size(SIZE)
        .line_height(1.0)
        .font_feature(font::Feature::off(LIGA));
    let body = scrollable(container(column![debug_text]).padding(8).width(PANEL_W)).height(Fill);

    container(stack![body, right(copy_btn).padding(5),])
        .width(PANEL_W)
        .into()
}
