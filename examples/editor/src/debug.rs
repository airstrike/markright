//! Debug panel — compact box-drawing view of the editor's internal state.

use iced::advanced::graphics::core::font;
use iced::widget::{button, column, container, right, scrollable, stack, text};
use iced::{Element, Fill};

use markright::widget::rich_editor::{Alignment, Content, StyleRun, StyledLine};

const SIZE: f32 = 11.0;
const BOX_W: usize = 32;
pub const PANEL_W: f32 = 260.0;

fn align_char(a: Alignment) -> char {
    match a {
        Alignment::Left => 'L',
        Alignment::Center => 'C',
        Alignment::Right => 'R',
        Alignment::Justified => 'J',
    }
}

/// Collect BIU flags present anywhere on a line's style runs.
fn line_flags(runs: &[StyleRun]) -> String {
    let mut b = false;
    let mut i = false;
    let mut u = false;
    for r in runs {
        b = b || r.style.bold == Some(true);
        i = i || r.style.italic == Some(true);
        u = u || r.style.underline == Some(true);
    }
    let mut s = String::new();
    if b {
        s.push('B');
    }
    if i {
        s.push('I');
    }
    if u {
        s.push('U');
    }
    s
}

/// Extract font info from a styled line (paragraph style + span runs).
fn font_info(styled: &StyledLine) -> Option<String> {
    let mut name: Option<&str> = None;
    let mut size: Option<f32> = None;

    // Check paragraph-level style first (line defaults)
    if let Some(font) = styled.paragraph_style.style.font
        && let iced::font::Family::Name(n) = font.family
    {
        name = Some(n);
    }
    if styled.paragraph_style.style.size.is_some() {
        size = styled.paragraph_style.style.size;
    }

    // Override with span-level style if present
    for r in &styled.runs {
        if let Some(font) = r.style.font
            && let iced::font::Family::Name(n) = font.family
        {
            name = Some(n);
        }
        if r.style.size.is_some() {
            size = r.style.size;
        }
    }

    if name.is_none() && size.is_none() {
        return None;
    }
    let mut info = String::new();
    if let Some(n) = name {
        info.push_str(n);
    }
    if let Some(s) = size {
        if !info.is_empty() {
            info.push(' ');
        }
        info.push_str(&format!("{s:.0}px"));
    }
    Some(info)
}

/// Build the full debug string for display and clipboard copy.
pub fn to_string(content: &Content<iced::Renderer>) -> String {
    let count = content.line_count();
    let num_w = if count > 1 {
        (count - 1).to_string().len().max(2)
    } else {
        2
    };

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

    for i in 0..count {
        let Some(styled) = content.styled_line(i) else {
            continue;
        };
        if styled.text.is_empty() {
            continue;
        }

        let alignment = Alignment::from_iced(styled.paragraph_style.alignment);
        let flags = line_flags(&styled.runs);
        let range = format!("0..{}", styled.text.len());

        // Header: ┌ 00 L BI ─────────── 0‥44 ┐
        let left = if flags.is_empty() {
            format!("\u{250c} {i:0>num_w$} {a} ", a = align_char(alignment))
        } else {
            format!(
                "\u{250c} {i:0>num_w$} {a} {flags} ",
                a = align_char(alignment)
            )
        };
        let right = format!(" {range} \u{2510}");
        let fill = BOX_W.saturating_sub(left.chars().count() + right.chars().count());
        out.push_str(&left);
        for _ in 0..fill {
            out.push('\u{2500}');
        }
        out.push_str(&right);
        out.push('\n');

        // Body: │ text │
        for line in wrap_text(&styled.text, BOX_W - 4) {
            out.push_str(&format!("\u{2502} {line:<w$} \u{2502}\n", w = BOX_W - 4));
        }

        // Footer: font info (blank line separator, then info)
        if let Some(info) = font_info(&styled) {
            out.push_str(&format!("\u{2502} {:<w$} \u{2502}\n", "", w = BOX_W - 4));
            out.push_str(&format!("\u{2502} {info:<w$} \u{2502}\n", w = BOX_W - 4));
        }

        // Bottom: └───┘
        out.push('\u{2514}');
        for _ in 0..BOX_W - 2 {
            out.push('\u{2500}');
        }
        out.push('\u{2518}');
        out.push('\n');
    }

    // Raw Debug prints
    out.push('\n');
    out.push_str(&content.debug_state());

    out
}

/// Simple word-boundary-unaware text wrapping.
fn wrap_text(s: &str, width: usize) -> Vec<&str> {
    if s.is_empty() {
        return vec![""];
    }
    let mut lines = Vec::new();
    let mut start = 0;
    while start < s.len() {
        let end = (start + width).min(s.len());
        lines.push(&s[start..end]);
        start = end;
    }
    lines
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
