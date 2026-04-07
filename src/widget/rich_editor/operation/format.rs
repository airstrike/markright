//! Formatting operations — bold, italic, underline, alignment, font, size,
//! list style, indentation, and line spacing.

use crate::core::text::LineHeight;
use crate::core::text::rich_editor::{Editor, span};
use markright_core::{
    self as document, Alignment, Name, Op, Paragraph, SpanAttr, Theme, paragraph,
};
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
/// `paragraphs` is the current per-line paragraph data from Content.
/// `theme` provides the paragraph name → style mapping.
pub fn format<E: Editor>(
    editor: &mut E,
    fmt: &Format,
    paragraphs: &[Paragraph],
    theme: &Theme,
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
        Format::SetAlignment(alignment) => set_alignment(editor, *alignment, paragraphs),
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
        Format::SetList(list_val) => set_paragraph_field(editor, paragraphs, |para| {
            let same_kind = matches!(
                (&para.style.list, list_val),
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
                para.style.list = None;
                para.style.level = 0;
            } else {
                para.style.list = list_val.clone();
                // Entering a list puts text at level 1 (bullet occupies level-0 margin).
                if para.style.level == 0 {
                    para.style.level = 1;
                }
            }
        }),
        Format::IndentList => set_paragraph_field(editor, paragraphs, |para| {
            if para.style.list.is_some() {
                // Inside a list: Tab demotes (increases nesting depth).
                if para.style.level < 8 {
                    para.style.level += 1;
                    match &mut para.style.list {
                        Some(paragraph::List::Bullet(b)) => {
                            *b = list::bullet_for_level(para.style.level.saturating_sub(1));
                        }
                        Some(paragraph::List::Ordered(n)) => {
                            *n = list::number_for_level(para.style.level.saturating_sub(1));
                        }
                        _ => {}
                    }
                }
            } else {
                // No list: Tab just indents.
                if para.style.level < 8 {
                    para.style.level += 1;
                }
            }
        }),
        Format::DedentList => set_paragraph_field(editor, paragraphs, |para| {
            if para.style.list.is_some() {
                // Inside a list: Shift+Tab promotes (decreases nesting).
                // Level 1 is the base list level — going below removes the list.
                if para.style.level > 1 {
                    para.style.level -= 1;
                    match &mut para.style.list {
                        Some(paragraph::List::Bullet(b)) => {
                            *b = list::bullet_for_level(para.style.level.saturating_sub(1));
                        }
                        Some(paragraph::List::Ordered(n)) => {
                            *n = list::number_for_level(para.style.level.saturating_sub(1));
                        }
                        _ => {}
                    }
                } else {
                    // At base list level — remove the list entirely.
                    para.style.list = None;
                    para.style.level = 0;
                }
            } else if para.style.level > 0 {
                // No list: Shift+Tab just dedents.
                para.style.level -= 1;
            }
        }),
        Format::SetLineHeight(lh) => set_line_height(editor, *lh, paragraphs),
        Format::SetLineSpacing(spacing) => {
            let spacing = *spacing;
            set_paragraph_field(editor, paragraphs, |para| {
                para.style.line_spacing = Some(spacing);
            })
        }
        Format::SetName(name) => {
            let name = *name;
            set_name(editor, paragraphs, theme, name)
        }
        Format::SetSpaceBefore(space) => {
            let space = *space;
            set_paragraph_field(editor, paragraphs, |para| {
                para.style.space_before = space;
                para.set_override(markright_core::OverrideSet::SPACE_BEFORE);
            })
        }
        Format::SetSpaceAfter(space) => {
            let space = *space;
            set_paragraph_field(editor, paragraphs, |para| {
                para.style.spacing_after = space;
                para.set_override(markright_core::OverrideSet::SPACING_AFTER);
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

        ops.push(set_attr_on_line(editor, line, col_start..col_end, attr));
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

/// Set line height on lines covered by the current cursor/selection.
fn set_line_height<E: Editor>(
    editor: &mut E,
    line_height: LineHeight,
    paragraphs: &[Paragraph],
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
            let old = paragraphs.get(line).cloned().unwrap_or_default();
            let mut new = old.clone();
            new.style.line_height = Some(line_height);
            new.set_override(markright_core::OverrideSet::LINE_HEIGHT);
            editor.set_paragraph_style(line, &new.style);
            Op::SetParagraph {
                line,
                paragraph: Box::new(new),
                old_paragraph: Box::new(old),
            }
        })
        .collect()
}

/// Set alignment on lines covered by the current cursor/selection.
fn set_alignment<E: Editor>(
    editor: &mut E,
    alignment: Alignment,
    paragraphs: &[Paragraph],
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
            let old = paragraphs.get(line).cloned().unwrap_or_default();
            let mut new = old.clone();
            new.style.alignment = Some(alignment.to_iced());
            new.set_override(markright_core::OverrideSet::ALIGNMENT);
            editor.set_paragraph_style(line, &new.style);
            Op::SetParagraph {
                line,
                paragraph: Box::new(new),
                old_paragraph: Box::new(old),
            }
        })
        .collect()
}

/// Set a paragraph-level field on lines covered by the current cursor/selection.
///
/// `apply` is a closure that mutates a cloned `Paragraph` to produce the
/// new value. Returns one `SetParagraph` op per affected line.
fn set_paragraph_field<E: Editor>(
    editor: &E,
    paragraphs: &[Paragraph],
    apply: impl Fn(&mut Paragraph),
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
            let old = paragraphs.get(line).cloned().unwrap_or_default();
            let mut new = old.clone();
            apply(&mut new);
            Op::SetParagraph {
                line,
                paragraph: Box::new(new),
                old_paragraph: Box::new(old),
            }
        })
        .collect()
}

/// Set the paragraph name on lines covered by the current cursor/selection.
///
/// Uses the theme to apply the appropriate visual style for the given name,
/// preserving any user overrides.
fn set_name<E: Editor>(
    editor: &mut E,
    paragraphs: &[Paragraph],
    theme: &Theme,
    name: Name,
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
            let old = paragraphs.get(line).cloned().unwrap_or_default();
            let mut new = old.clone();
            theme.apply(&mut new, name);
            editor.set_paragraph_style(line, &new.style);
            Op::SetParagraph {
                line,
                paragraph: Box::new(new),
                old_paragraph: Box::new(old),
            }
        })
        .collect()
}

/// Returns the style at the first non-empty character in the selection.
///
/// Skips blank lines at the start so that the toggle state reflects actual
/// content, not unformatted newlines.
fn style_at_selection_start<E: Editor>(editor: &E, cursor: &Cursor) -> span::Style {
    let (start, end) = match &cursor.selection {
        Some(sel) => ordered_positions(&cursor.position, sel),
        None => {
            return editor.span_style_at(cursor.position.line, cursor.position.column);
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
            return editor.span_style_at(line, col_start);
        }
    }

    editor.span_style_at(start.line, start.column)
}
