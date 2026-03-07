//! Operation module — standalone functions that apply edits to the editor
//! and return `Op` values for undo/redo tracking.
//!
//! - [`edit`] — text editing (insert, delete, enter, backspace, paste)
//! - [`format`] — formatting (bold, italic, alignment, font, etc.)

mod edit;
mod format;

use crate::core::text::editor as iced_editor;
use crate::core::text::rich_editor::Editor;
use markright_document::{self as document, Op};
use std::sync::Arc;

use super::action::FormatAction;

pub(crate) use crate::core::text::editor::{Cursor, Position};

pub(crate) use edit::{backspace, delete, enter, insert, paste};
pub(crate) use format::format;

fn iced_edit(edit: iced_editor::Edit) -> iced_editor::Action {
    iced_editor::Action::Edit(edit)
}

/// Order two positions so that the earlier one comes first.
pub(crate) fn ordered_positions<'a>(
    a: &'a Position,
    b: &'a Position,
) -> (&'a Position, &'a Position) {
    if a.line < b.line || (a.line == b.line && a.column <= b.column) {
        (a, b)
    } else {
        (b, a)
    }
}

/// Replay an operation on the editor.
///
/// Does NOT return an `Op` — this is intentional; replay is not recorded.
pub(crate) fn apply_op<E: Editor>(editor: &mut E, op: &Op) {
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
        Op::SetSpanStyle {
            line, range, style, ..
        } => {
            editor.set_span_style(*line, range.clone(), style);
        }
        Op::SetParagraphStyle { line, style, .. } => {
            editor.set_paragraph_style(*line, style);
        }
    }
}

/// Fill `old_*` fields from the current editor state (for undo/redo).
pub(crate) fn capture_op_state<E: Editor>(editor: &E, op: &Op) -> Op {
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
        Op::SetSpanStyle {
            line, range, style, ..
        } => {
            let old_runs = document::read_style_runs(editor, *line, range.clone());
            Op::SetSpanStyle {
                line: *line,
                range: range.clone(),
                style: style.clone(),
                old_runs,
            }
        }
        Op::SetParagraphStyle { line, style, .. } => {
            let old_style = editor.paragraph_style(*line);
            Op::SetParagraphStyle {
                line: *line,
                style: style.clone(),
                old_style,
            }
        }
    }
}
