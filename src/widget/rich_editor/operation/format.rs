//! Formatting operations — bold, italic, underline, alignment, font, size.

use crate::core::text::Alignment;
use crate::core::text::rich_editor::{Editor, ParagraphStyle, Style as RichStyle};
use markright_document::{self as document, Op};
use std::ops::Range;

use super::{Cursor, FormatAction, Position, ordered_positions};

/// Apply a format action to the editor.
///
/// Returns ops for selection-based formatting (and SetAlignment which always
/// applies). Returns an empty vec when there's no selection — the caller should
/// update pending_style instead.
pub(crate) fn format<E: Editor>(editor: &mut E, fmt: &FormatAction) -> Vec<Op> {
    let cursor = editor.cursor();
    let has_selection = cursor.selection.is_some();

    match fmt {
        FormatAction::ToggleBold => {
            if !has_selection {
                return vec![];
            }
            let is_bold = style_at_selection_start(editor, &cursor)
                .bold
                .unwrap_or(false);
            let style = RichStyle {
                bold: Some(!is_bold),
                ..RichStyle::default()
            };
            format_selection(editor, &style)
        }
        FormatAction::ToggleItalic => {
            if !has_selection {
                return vec![];
            }
            let is_italic = style_at_selection_start(editor, &cursor)
                .italic
                .unwrap_or(false);
            let style = RichStyle {
                italic: Some(!is_italic),
                ..RichStyle::default()
            };
            format_selection(editor, &style)
        }
        FormatAction::ToggleUnderline => {
            if !has_selection {
                return vec![];
            }
            let is_underline = style_at_selection_start(editor, &cursor)
                .underline
                .unwrap_or(false);
            let style = RichStyle {
                underline: Some(!is_underline),
                ..RichStyle::default()
            };
            format_selection(editor, &style)
        }
        FormatAction::SetAlignment(alignment) => set_alignment(editor, *alignment),
        FormatAction::SetFont(font) => {
            if !has_selection {
                return vec![];
            }
            let style = RichStyle {
                font: Some(*font),
                ..RichStyle::default()
            };
            format_selection(editor, &style)
        }
        FormatAction::SetFontSize(size) => {
            if !has_selection {
                return vec![];
            }
            let style = RichStyle {
                size: Some(*size),
                ..RichStyle::default()
            };
            format_selection(editor, &style)
        }
    }
}

/// Apply a span style to the current selection.
fn format_selection<E: Editor>(editor: &mut E, style: &RichStyle) -> Vec<Op> {
    let cursor = editor.cursor();
    let Some(ref sel) = cursor.selection else {
        return vec![];
    };
    let (start, end) = ordered_positions(&cursor.position, sel);
    set_span_style_range(editor, start, end, style)
}

/// Set alignment on lines covered by the current cursor/selection.
fn set_alignment<E: Editor>(editor: &mut E, alignment: Alignment) -> Vec<Op> {
    let cursor = editor.cursor();
    let lines = if let Some(ref sel) = cursor.selection {
        let (start, end) = ordered_positions(&cursor.position, sel);
        start.line..=end.line
    } else {
        cursor.position.line..=cursor.position.line
    };
    lines
        .map(|line| {
            let old_style = editor.paragraph_style(line);
            let new_style = ParagraphStyle {
                alignment: Some(alignment),
                ..old_style.clone()
            };
            set_paragraph_style(editor, line, &new_style)
        })
        .collect()
}

/// Returns the style at the first non-empty character in the selection.
///
/// Skips blank lines at the start so that the toggle state reflects actual
/// content, not unformatted newlines.
fn style_at_selection_start<E: Editor>(editor: &E, cursor: &Cursor) -> RichStyle {
    let (start, end) = match &cursor.selection {
        Some(sel) => ordered_positions(&cursor.position, sel),
        None => {
            return editor.style_at(cursor.position.line, cursor.position.column);
        }
    };

    for line in start.line..=end.line {
        let col_start = if line == start.line { start.column } else { 0 };
        let col_end = if line == end.line {
            end.column
        } else {
            editor.line(line).map(|l| l.text.len()).unwrap_or(0)
        };
        if col_start < col_end {
            return editor.style_at(line, col_start);
        }
    }

    editor.style_at(start.line, start.column)
}

/// Apply a span style across a multi-line selection.
fn set_span_style_range<E: Editor>(
    editor: &mut E,
    start: &Position,
    end: &Position,
    style: &RichStyle,
) -> Vec<Op> {
    let mut ops = Vec::new();

    for line in start.line..=end.line {
        let col_start = if line == start.line { start.column } else { 0 };
        let col_end = if line == end.line {
            end.column
        } else {
            editor.line(line).map(|l| l.text.len()).unwrap_or(0)
        };

        if col_start < col_end {
            ops.push(set_span_style(editor, line, col_start..col_end, style));
        }
    }

    ops
}

/// Set a character style on a single-line range.
fn set_span_style<E: Editor>(
    editor: &mut E,
    line: usize,
    range: Range<usize>,
    style: &RichStyle,
) -> Op {
    let old_runs = document::read_style_runs(editor, line, range.clone());
    editor.set_span_style(line, range.clone(), style);
    Op::SetSpanStyle {
        line,
        range,
        style: style.clone(),
        old_runs,
    }
}

/// Set paragraph style on a single line.
fn set_paragraph_style<E: Editor>(editor: &mut E, line: usize, style: &ParagraphStyle) -> Op {
    let old_style = editor.paragraph_style(line);
    editor.set_paragraph_style(line, style);
    Op::SetParagraphStyle {
        line,
        style: style.clone(),
        old_style,
    }
}
