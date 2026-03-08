//! Operation module — standalone functions that apply edits to the editor
//! and return `Op` values for undo/redo tracking.
//!
//! - [`edit`] — text editing (insert, delete, enter, backspace, paste)
//! - [`format`] — formatting (bold, italic, alignment, font, etc.)

mod edit;
mod format;

use crate::core::text::editor as iced_editor;
use crate::core::text::rich_editor::Editor;
use markright_document::{self as document, Alignment, Op, SpanAttr};
use std::sync::Arc;

pub use crate::core::text::editor::{Cursor, Position};

pub use edit::{backspace, delete, enter, insert, paste};
pub use format::format;

fn iced_edit(edit: iced_editor::Edit) -> iced_editor::Action {
    iced_editor::Action::Edit(edit)
}

/// Order two positions so that the earlier one comes first.
pub fn ordered_positions<'a>(a: &'a Position, b: &'a Position) -> (&'a Position, &'a Position) {
    if a.line < b.line || (a.line == b.line && a.column <= b.column) {
        (a, b)
    } else {
        (b, a)
    }
}

/// Replay an operation on the editor.
///
/// Does NOT return an `Op` — this is intentional; replay is not recorded.
pub fn apply_op<E: Editor>(editor: &mut E, op: &Op) {
    match op {
        Op::InsertText { line, col, content } => {
            editor.move_to(Cursor {
                position: Position {
                    line: *line,
                    column: *col,
                },
                selection: None,
            });
            if content.text.len() == 1 {
                let c = content.text.chars().next().expect("non-empty text");
                editor.perform(iced_edit(iced_editor::Edit::Insert(c)));
            } else {
                editor.perform(iced_edit(iced_editor::Edit::Paste(Arc::new(
                    content.text.clone(),
                ))));
            }
            for run in &content.runs {
                let abs_start = *col + run.range.start;
                let abs_end = *col + run.range.end;
                editor.set_span_style(*line, abs_start..abs_end, &run.style);
            }
        }
        Op::DeleteText { line, col, content } => {
            let end_col = *col + content.text.len();
            editor.move_to(Cursor {
                position: Position {
                    line: *line,
                    column: *col,
                },
                selection: Some(Position {
                    line: *line,
                    column: end_col,
                }),
            });
            editor.perform(iced_edit(iced_editor::Edit::Delete));
        }
        Op::SplitLine { line, col } => {
            editor.move_to(Cursor {
                position: Position {
                    line: *line,
                    column: *col,
                },
                selection: None,
            });
            editor.perform(iced_edit(iced_editor::Edit::Enter));
        }
        Op::MergeLine { line, .. } => {
            editor.move_to(Cursor {
                position: Position {
                    line: *line + 1,
                    column: 0,
                },
                selection: None,
            });
            editor.perform(iced_edit(iced_editor::Edit::Backspace));
        }
        Op::SetSpanAttr {
            line, range, attr, ..
        } => {
            // Read existing styles, apply only the one attribute, write back.
            let runs = document::read_style_runs(editor, *line, range.clone());
            if runs.is_empty() {
                // No existing runs — apply attribute to a default style.
                let style = attr.apply_to(&Default::default());
                editor.set_span_style(*line, range.clone(), &style);
            } else {
                for run in &runs {
                    let merged = attr.apply_to(&run.style);
                    editor.set_span_style(*line, run.range.clone(), &merged);
                }
            }
        }
        Op::SetAlignment {
            line, alignment, ..
        } => {
            editor.set_alignment(*line, alignment.to_iced());
        }
        Op::DeleteRange {
            start_line,
            start_col,
            end_line,
            end_col,
            ..
        } => {
            editor.move_to(Cursor {
                position: Position {
                    line: *start_line,
                    column: *start_col,
                },
                selection: Some(Position {
                    line: *end_line,
                    column: *end_col,
                }),
            });
            editor.perform(iced_edit(iced_editor::Edit::Delete));
        }
        Op::InsertRange {
            start_line,
            start_col,
            lines,
        } => {
            editor.move_to(Cursor {
                position: Position {
                    line: *start_line,
                    column: *start_col,
                },
                selection: None,
            });

            // Build full text with newlines and paste
            let full_text = lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            editor.perform(iced_edit(iced_editor::Edit::Paste(Arc::new(full_text))));

            // Apply styles and paragraph formatting per line
            for (i, styled_line) in lines.iter().enumerate() {
                let target_line = *start_line + i;
                let col_offset = if i == 0 { *start_col } else { 0 };
                for run in &styled_line.runs {
                    editor.set_span_style(
                        target_line,
                        (col_offset + run.range.start)..(col_offset + run.range.end),
                        &run.style,
                    );
                }
                editor.set_paragraph_style(target_line, &styled_line.paragraph_style);
            }
        }
    }
}

/// Fill `old_*` fields from the current editor state (for undo/redo).
pub fn capture_op_state<E: Editor>(editor: &E, op: &Op) -> Op {
    match op {
        Op::InsertText { .. } | Op::SplitLine { .. } | Op::MergeLine { .. } => op.clone(),
        Op::DeleteText { line, col, content } => {
            let end = *col + content.text.len();
            let current_text = editor
                .line(*line)
                .map(|l| {
                    let t = l.text;
                    t[*col..end.min(t.len())].to_string()
                })
                .unwrap_or_default();
            Op::DeleteText {
                line: *line,
                col: *col,
                content: document::read_styled_text(
                    editor,
                    *line,
                    *col..(*col + current_text.len()),
                    &current_text,
                ),
            }
        }
        Op::SetSpanAttr {
            line, range, attr, ..
        } => {
            // Read old values of just this attribute per-run.
            let runs = document::read_style_runs(editor, *line, range.clone());
            let mut old_values: Vec<(std::ops::Range<usize>, SpanAttr)> = Vec::new();
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
            Op::SetSpanAttr {
                line: *line,
                range: range.clone(),
                attr: attr.clone(),
                old_values,
            }
        }
        Op::SetAlignment {
            line, alignment, ..
        } => {
            let old_alignment = Alignment::from_iced(editor.paragraph_style(*line).alignment);
            Op::SetAlignment {
                line: *line,
                alignment: *alignment,
                old_alignment,
            }
        }
        // Range ops are self-contained — no state capture needed.
        Op::DeleteRange { .. } | Op::InsertRange { .. } => op.clone(),
    }
}
