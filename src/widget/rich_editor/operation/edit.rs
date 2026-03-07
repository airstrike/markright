//! Text editing operations — insert, delete, enter, backspace, paste.

use crate::core::text::editor as iced_editor;
use crate::core::text::rich_editor::{Editor, Style as RichStyle};
use markright_document::{self as document, Op, StyleRun, StyledText};
use std::sync::Arc;

use super::{Position, iced_edit, ordered_positions};

/// Insert a single character with a resolved style.
pub(crate) fn insert<E: Editor>(editor: &mut E, c: char, style: RichStyle) -> Op {
    let cursor = editor.cursor();
    let content = StyledText {
        text: c.to_string(),
        runs: vec![StyleRun {
            range: 0..c.len_utf8(),
            style,
        }],
    };
    insert_text(
        editor,
        cursor.position.line,
        cursor.position.column,
        content,
    )
}

/// Paste text with a resolved style.
///
/// Single-line paste produces an op. Multi-line paste applies directly (not yet
/// undoable) and returns an empty vec.
pub(crate) fn paste<E: Editor>(editor: &mut E, text: Arc<String>, style: RichStyle) -> Vec<Op> {
    let cursor = editor.cursor();
    let line = cursor.position.line;
    let col = cursor.position.column;

    if !text.contains('\n') && !text.contains('\r') {
        let content = StyledText {
            text: text.to_string(),
            runs: vec![StyleRun {
                range: 0..text.len(),
                style,
            }],
        };
        vec![insert_text(editor, line, col, content)]
    } else {
        editor.perform(iced_edit(iced_editor::Edit::Paste(text)));
        vec![]
    }
}

/// Enter key — split the line at the cursor.
pub(crate) fn enter<E: Editor>(editor: &mut E) -> Op {
    let cursor = editor.cursor();
    split_line(editor, cursor.position.line, cursor.position.column)
}

/// Backspace — handles selection delete, character delete, or line merge.
pub(crate) fn backspace<E: Editor>(editor: &mut E) -> Vec<Op> {
    let cursor = editor.cursor();
    let line = cursor.position.line;
    let col = cursor.position.column;

    if let Some(ref sel) = cursor.selection {
        let (start, end) = ordered_positions(&cursor.position, sel);
        delete_selection(editor, start, end)
    } else if col > 0 {
        vec![delete_char_before(editor, line, col)]
    } else if line > 0 {
        vec![merge_line_backward(editor, line)]
    } else {
        vec![]
    }
}

/// Delete key — handles selection delete, character delete, or line merge.
pub(crate) fn delete<E: Editor>(editor: &mut E) -> Vec<Op> {
    let cursor = editor.cursor();
    let line = cursor.position.line;
    let col = cursor.position.column;

    if let Some(ref sel) = cursor.selection {
        let (start, end) = ordered_positions(&cursor.position, sel);
        delete_selection(editor, start, end)
    } else {
        let line_text = editor.line(line).map(|l| l.text.to_string());
        match line_text {
            Some(ref text) if col < text.len() => {
                vec![delete_char_at(editor, line, col)]
            }
            Some(_) if line + 1 < editor.line_count() => {
                vec![merge_line_forward(editor, line, col)]
            }
            _ => vec![],
        }
    }
}

/// Insert styled text at `(line, col)`.
fn insert_text<E: Editor>(editor: &mut E, line: usize, col: usize, content: StyledText) -> Op {
    if content.text.len() == 1 {
        let c = content.text.chars().next().expect("non-empty text");
        editor.perform(iced_edit(iced_editor::Edit::Insert(c)));
    } else {
        editor.perform(iced_edit(iced_editor::Edit::Paste(Arc::new(
            content.text.clone(),
        ))));
    }

    for run in &content.runs {
        let abs_start = col + run.range.start;
        let abs_end = col + run.range.end;
        editor.set_span_style(line, abs_start..abs_end, &run.style);
    }

    Op::InsertText { line, col, content }
}

/// Split a line at `(line, col)` — the Enter key.
fn split_line<E: Editor>(editor: &mut E, line: usize, col: usize) -> Op {
    editor.perform(iced_edit(iced_editor::Edit::Enter));
    Op::SplitLine { line, col }
}

/// Delete the character immediately before `col` on `line`.
fn delete_char_before<E: Editor>(editor: &mut E, line: usize, col: usize) -> Op {
    let line_text = editor
        .line(line)
        .map(|l| l.text.to_string())
        .unwrap_or_default();

    let char_start = line_text[..col]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0);

    let deleted_text = &line_text[char_start..col];
    let styled = document::read_styled_text(editor, line, char_start..col, deleted_text);

    editor.perform(iced_edit(iced_editor::Edit::Backspace));

    Op::DeleteText {
        line,
        col: char_start,
        content: styled,
    }
}

/// Delete the character at `col` on `line`.
fn delete_char_at<E: Editor>(editor: &mut E, line: usize, col: usize) -> Op {
    let line_text = editor
        .line(line)
        .map(|l| l.text.to_string())
        .unwrap_or_default();

    let char_end = line_text[col..]
        .char_indices()
        .nth(1)
        .map(|(i, _)| col + i)
        .unwrap_or(line_text.len());

    let deleted_text = &line_text[col..char_end];
    let styled = document::read_styled_text(editor, line, col..char_end, deleted_text);

    editor.perform(iced_edit(iced_editor::Edit::Delete));

    Op::DeleteText {
        line,
        col,
        content: styled,
    }
}

/// Backspace at the start of `line` — merges with the previous line.
fn merge_line_backward<E: Editor>(editor: &mut E, line: usize) -> Op {
    let prev_line_len = editor.line(line - 1).map(|l| l.text.len()).unwrap_or(0);

    editor.perform(iced_edit(iced_editor::Edit::Backspace));

    Op::MergeLine {
        line: line - 1,
        col: prev_line_len,
    }
}

/// Delete at the end of `line` — merges the next line in.
fn merge_line_forward<E: Editor>(editor: &mut E, line: usize, col: usize) -> Op {
    editor.perform(iced_edit(iced_editor::Edit::Delete));
    Op::MergeLine { line, col }
}

/// Delete a multi-line (or single-line) selection.
///
/// Captures all text and styles, records decomposed ops, then applies one
/// `Delete` to the editor.
fn delete_selection<E: Editor>(editor: &mut E, start: &Position, end: &Position) -> Vec<Op> {
    let mut ops = Vec::new();

    if start.line == end.line {
        let line_text = editor
            .line(start.line)
            .map(|l| l.text.to_string())
            .unwrap_or_default();
        let end_col = end.column.min(line_text.len());
        let start_col = start.column.min(end_col);

        if start_col < end_col {
            let deleted = &line_text[start_col..end_col];
            let styled =
                document::read_styled_text(editor, start.line, start_col..end_col, deleted);
            ops.push(Op::DeleteText {
                line: start.line,
                col: start_col,
                content: styled,
            });
        }
    } else {
        let first_text = editor
            .line(start.line)
            .map(|l| l.text.to_string())
            .unwrap_or_default();

        if start.column < first_text.len() {
            let tail = &first_text[start.column..];
            let styled = document::read_styled_text(
                editor,
                start.line,
                start.column..first_text.len(),
                tail,
            );
            ops.push(Op::DeleteText {
                line: start.line,
                col: start.column,
                content: styled,
            });
        }

        for orig_line in (start.line + 1)..=end.line {
            let line_text = editor
                .line(orig_line)
                .map(|l| l.text.to_string())
                .unwrap_or_default();

            ops.push(Op::MergeLine {
                line: start.line,
                col: start.column,
            });

            let del_end = if orig_line == end.line {
                end.column.min(line_text.len())
            } else {
                line_text.len()
            };

            if del_end > 0 {
                let deleted = &line_text[..del_end];
                let styled = document::read_styled_text(editor, orig_line, 0..del_end, deleted);
                ops.push(Op::DeleteText {
                    line: start.line,
                    col: start.column,
                    content: styled,
                });
            }
        }
    }

    editor.perform(iced_edit(iced_editor::Edit::Delete));

    ops
}
