//! Formatting operations — bold, italic, underline, alignment, font, size,
//! list style, indentation, and line spacing.

use crate::core::text::rich_editor::{Editor, ParagraphStyle, Style as RichStyle};
use markright_document::{self as document, Alignment, Op, SpanAttr, paragraph};
use std::ops::Range;

use super::super::action::Format;
use super::super::list;
use super::{Cursor, Position, ordered_positions};

/// Apply a format action to the editor.
///
/// Returns ops for selection-based formatting (and SetAlignment which always
/// applies). Returns an empty vec when there's no selection — the caller should
/// update pending_style instead.
///
/// `paragraph_styles` is the current per-line paragraph style storage from Content.
pub fn format<E: Editor>(
    editor: &mut E,
    fmt: &Format,
    paragraph_styles: &[paragraph::Style],
) -> Vec<Op> {
    let cursor = editor.cursor();
    let has_selection = cursor.selection.is_some();

    match fmt {
        Format::ToggleBold => {
            if !has_selection {
                return vec![];
            }
            let is_bold = style_at_selection_start(editor, &cursor)
                .bold
                .unwrap_or(false);
            set_attr_in_selection(editor, SpanAttr::Bold(Some(!is_bold)))
        }
        Format::ToggleItalic => {
            if !has_selection {
                return vec![];
            }
            let is_italic = style_at_selection_start(editor, &cursor)
                .italic
                .unwrap_or(false);
            set_attr_in_selection(editor, SpanAttr::Italic(Some(!is_italic)))
        }
        Format::ToggleUnderline => {
            if !has_selection {
                return vec![];
            }
            let is_underline = style_at_selection_start(editor, &cursor)
                .underline
                .unwrap_or(false);
            set_attr_in_selection(editor, SpanAttr::Underline(Some(!is_underline)))
        }
        Format::SetAlignment(alignment) => set_alignment(editor, *alignment),
        Format::SetFont(font) => {
            if !has_selection {
                return vec![];
            }
            set_attr_in_selection(editor, SpanAttr::Font(Some(*font)))
        }
        Format::SetFontSize(size) => {
            if !has_selection {
                return vec![];
            }
            set_attr_in_selection(editor, SpanAttr::Size(Some(*size)))
        }
        Format::SetColor(color) => {
            if !has_selection {
                return vec![];
            }
            set_attr_in_selection(editor, SpanAttr::Color(*color))
        }
        Format::SetLetterSpacing(ls) => {
            if !has_selection {
                return vec![];
            }
            set_attr_in_selection(editor, SpanAttr::LetterSpacing(Some(*ls)))
        }
        Format::SetList(list) => set_paragraph_field(editor, paragraph_styles, |style| {
            let same_kind = matches!(
                (&style.list, list),
                (
                    Some(paragraph::List::Bullet(_)),
                    Some(paragraph::List::Bullet(_))
                ) | (
                    Some(paragraph::List::Ordered(_)),
                    Some(paragraph::List::Ordered(_))
                )
            );
            if same_kind {
                // Toggle off — same list kind already set.
                style.list = None;
                style.level = 0;
            } else {
                style.list = list.clone();
                // Entering a list puts text at level 1 (bullet occupies level-0 margin).
                if style.level == 0 {
                    style.level = 1;
                }
            }
        }),
        Format::IndentList => set_paragraph_field(editor, paragraph_styles, |style| {
            if style.list.is_some() {
                // Inside a list: Tab demotes (increases nesting depth).
                if style.level < 8 {
                    style.level += 1;
                    match &mut style.list {
                        Some(paragraph::List::Bullet(b)) => {
                            *b = list::bullet_for_level(style.level.saturating_sub(1));
                        }
                        Some(paragraph::List::Ordered(n)) => {
                            *n = list::number_for_level(style.level.saturating_sub(1));
                        }
                        _ => {}
                    }
                }
            } else {
                // No list: Tab just indents.
                if style.level < 8 {
                    style.level += 1;
                }
            }
        }),
        Format::DedentList => set_paragraph_field(editor, paragraph_styles, |style| {
            if style.list.is_some() {
                // Inside a list: Shift+Tab promotes (decreases nesting).
                // Level 1 is the base list level — going below removes the list.
                if style.level > 1 {
                    style.level -= 1;
                    match &mut style.list {
                        Some(paragraph::List::Bullet(b)) => {
                            *b = list::bullet_for_level(style.level.saturating_sub(1));
                        }
                        Some(paragraph::List::Ordered(n)) => {
                            *n = list::number_for_level(style.level.saturating_sub(1));
                        }
                        _ => {}
                    }
                } else {
                    // At base list level — remove the list entirely.
                    style.list = None;
                    style.level = 0;
                }
            } else if style.level > 0 {
                // No list: Shift+Tab just dedents.
                style.level -= 1;
            }
        }),
        Format::SetLineSpacing(spacing) => {
            let spacing = *spacing;
            set_paragraph_field(editor, paragraph_styles, |style| {
                style.line_spacing = Some(spacing);
            })
        }
    }
}

/// Set a single span attribute across the current selection.
fn set_attr_in_selection<E: Editor>(editor: &mut E, attr: SpanAttr) -> Vec<Op> {
    let cursor = editor.cursor();
    let Some(ref sel) = cursor.selection else {
        return vec![];
    };
    let (start, end) = ordered_positions(&cursor.position, sel);
    set_attr_range(editor, start, end, &attr)
}

/// Set a single span attribute across a multi-line range.
fn set_attr_range<E: Editor>(
    editor: &mut E,
    start: &Position,
    end: &Position,
    attr: &SpanAttr,
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
            ops.push(set_attr_on_line(editor, line, col_start..col_end, attr));
        }
    }

    ops
}

/// Set a single span attribute on one line range.
///
/// Reads existing styles per-run, applies only the one attribute via
/// read-modify-write, and returns a `SetSpanAttr` op with old values.
fn set_attr_on_line<E: Editor>(
    editor: &mut E,
    line: usize,
    range: Range<usize>,
    attr: &SpanAttr,
) -> Op {
    let runs = document::read_style_runs(editor, line, range.clone());

    // Collect old values of just this attribute, compressed into runs.
    let mut old_values: Vec<(Range<usize>, SpanAttr)> = Vec::new();
    for run in &runs {
        let old_attr = SpanAttr::from_style(&run.style, attr);
        match old_values.last_mut() {
            Some((last_range, last_attr))
                if *last_attr == old_attr && last_range.end == run.range.start =>
            {
                last_range.end = run.range.end;
            }
            _ => {
                old_values.push((run.range.clone(), old_attr));
            }
        }
    }

    // Apply: read-modify-write each run.
    if runs.is_empty() {
        let style = attr.apply_to(&Default::default());
        editor.set_span_style(line, range.clone(), &style);
    } else {
        for run in &runs {
            let merged = attr.apply_to(&run.style);
            editor.set_span_style(line, run.range.clone(), &merged);
        }
    }

    Op::SetSpanAttr {
        line,
        range,
        attr: attr.clone(),
        old_values,
    }
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
            let old_alignment = Alignment::from_iced(editor.paragraph_style(line).alignment);
            editor.set_paragraph_style(
                line,
                &ParagraphStyle {
                    alignment: Some(alignment.to_iced()),
                    ..Default::default()
                },
            );
            Op::SetAlignment {
                line,
                alignment,
                old_alignment,
            }
        })
        .collect()
}

/// Set a paragraph-level field on lines covered by the current cursor/selection.
///
/// `apply` is a closure that mutates a cloned `paragraph::Style` to produce the
/// new value. Returns one `SetParagraphStyle` op per affected line.
fn set_paragraph_field<E: Editor>(
    editor: &E,
    paragraph_styles: &[paragraph::Style],
    apply: impl Fn(&mut paragraph::Style),
) -> Vec<Op> {
    let cursor = editor.cursor();
    let lines = if let Some(ref sel) = cursor.selection {
        let (start, end) = ordered_positions(&cursor.position, sel);
        start.line..=end.line
    } else {
        cursor.position.line..=cursor.position.line
    };

    lines
        .map(|line| {
            let old_style = paragraph_styles.get(line).cloned().unwrap_or_default();
            let mut new_style = old_style.clone();
            apply(&mut new_style);
            Op::SetParagraphStyle {
                line,
                style: new_style,
                old_style,
            }
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
