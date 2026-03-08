//! Debug panel — compact box-drawing view of the editor's internal state.

use iced::widget::{column, container, scrollable, text};
use iced::{Element, Fill, Font};

use markright::widget::rich_editor::{Alignment, Content, StyleRun};

const MONO: Font = Font::with_name("GT Pressura Mono");
const SIZE: f32 = 11.0;

fn align_char(a: Alignment) -> char {
    match a {
        Alignment::Left => 'L',
        Alignment::Center => 'C',
        Alignment::Right => 'R',
        Alignment::Justified => 'J',
    }
}

fn style_flags(run: &StyleRun) -> String {
    let mut flags = String::new();
    if run.style.bold == Some(true) {
        flags.push('B');
    }
    if run.style.italic == Some(true) {
        flags.push('I');
    }
    if run.style.underline == Some(true) {
        flags.push('U');
    }
    if run.style.strikethrough == Some(true) {
        flags.push('S');
    }

    let mut extras = Vec::new();
    if let Some(font) = run.style.font {
        if let iced::font::Family::Name(name) = font.family {
            extras.push(name.to_string());
        }
    }
    if let Some(size) = run.style.size {
        extras.push(format!("{size:.0}px"));
    }
    if let Some(color) = run.style.color {
        extras.push(format!(
            "#{:02x}{:02x}{:02x}",
            (color.r * 255.0) as u8,
            (color.g * 255.0) as u8,
            (color.b * 255.0) as u8,
        ));
    }

    if !extras.is_empty() {
        if !flags.is_empty() {
            flags.push(' ');
        }
        flags.push_str(&extras.join(" "));
    }

    if flags.is_empty() {
        "\u{2500}".to_string() // ─ plain
    } else {
        flags
    }
}

pub fn view<'a, Message: 'a>(content: &Content<iced::Renderer>) -> Element<'a, Message> {
    let count = content.line_count();
    let cursor = content.cursor();
    let num_width = if count > 0 {
        (count - 1).to_string().len()
    } else {
        1
    };

    let mut out = String::new();

    for i in 0..count {
        let Some(styled) = content.styled_line(i) else {
            continue;
        };
        let alignment = Alignment::from_iced(styled.paragraph_style.alignment);

        // Top or divider
        if i == 0 {
            out.push_str(&format!(
                "\u{250c}\u{2500} {i:>num_width$} \u{2500} {a} \u{2500}{pad}\u{2510}\n",
                a = align_char(alignment),
                pad = "\u{2500}".repeat(40),
            ));
        } else {
            out.push_str(&format!(
                "\u{251c}\u{2500} {i:>num_width$} \u{2500} {a} \u{2500}{pad}\u{2524}\n",
                a = align_char(alignment),
                pad = "\u{2500}".repeat(40),
            ));
        }

        // Cursor marker
        let cursor_marker = if cursor.position.line == i {
            format!(" \u{25c0} col {}", cursor.position.column)
        } else {
            String::new()
        };

        // Text content (truncate for display)
        let display_text = if styled.text.len() > 48 {
            format!("{}...", &styled.text[..45])
        } else {
            styled.text.clone()
        };
        out.push_str(&format!("\u{2502} {display_text:<48}{cursor_marker}\n"));

        // Style runs
        if styled.runs.is_empty() || styled.text.is_empty() {
            out.push_str(&format!(
                "\u{2502}   {:<48}\n",
                "\u{00b7}" // · no runs
            ));
        } else {
            let runs_str: Vec<String> = styled
                .runs
                .iter()
                .map(|r| format!("{} {}..{}", style_flags(r), r.range.start, r.range.end))
                .collect();
            let joined = runs_str.join("  ");
            out.push_str(&format!("\u{2502}   {joined:<48}\n"));
        }
    }

    // Bottom border
    out.push_str(&format!(
        "\u{2514}{pad}\u{2518}",
        pad = "\u{2500}".repeat(50),
    ));

    // Summary
    out.push_str(&format!(
        "\n{count} lines \u{2502} cursor: {line}:{col}",
        line = cursor.position.line,
        col = cursor.position.column,
    ));
    if let Some(sel) = cursor.selection {
        out.push_str(&format!(" \u{2502} sel: {}:{}", sel.line, sel.column));
    }
    if content.can_undo() {
        out.push_str(" \u{2502} undo");
    }
    if content.can_redo() {
        out.push_str(" \u{2502} redo");
    }

    let debug_text = text(out).font(MONO).size(SIZE);

    scrollable(container(column![debug_text]).padding(12).width(Fill))
        .height(Fill)
        .into()
}
