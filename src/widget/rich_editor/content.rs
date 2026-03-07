use crate::core::text::rich_editor::{self, Editor as _, ParagraphStyle, Style as RichStyle};
use markright_document::{self as document, History, Op, StyleRun, StyledText};

use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::Arc;

use super::action::{self, Action, Edit, FormatAction};
use super::cursor;

pub use crate::core::text::editor::{Cursor, Line, LineEnding, Position};

/// The content of a rich text editor -- wraps the rich editor which manages
/// both text and formatting via cosmic-text's AttrsList.
///
/// This is the single source of truth: all edits and formatting changes go
/// through [`Content::perform`].
pub struct Content<R: rich_editor::Renderer>(pub(crate) RefCell<Internal<R>>);

pub(crate) struct Internal<R: rich_editor::Renderer> {
    pub(crate) editor: R::RichEditor,
    /// Pending character style for typing with no selection.
    /// When the user toggles bold with no selection, this records the intent
    /// so the next Insert applies it.
    pending_style: Option<RichStyle>,
    /// Undo/redo history of document operations.
    history: History,
}

impl<R: rich_editor::Renderer> Content<R> {
    /// Create an empty [`Content`].
    pub fn new() -> Self {
        Self::with_text("")
    }

    /// Create a [`Content`] with the given text.
    pub fn with_text(text: &str) -> Self {
        Self(RefCell::new(Internal {
            editor: R::RichEditor::with_text(text),
            pending_style: None,
            history: History::new(),
        }))
    }

    /// Perform an [`Action`] on the content.
    pub fn perform(&self, action: Action) {
        let mut internal = self.0.borrow_mut();
        internal.perform(action);
    }

    /// Returns the current cursor position.
    pub fn cursor(&self) -> Cursor {
        self.0.borrow().editor.cursor()
    }

    /// Returns the selected text, if any.
    pub fn selection(&self) -> Option<String> {
        self.0.borrow().editor.copy()
    }

    /// Returns the full text content.
    pub fn text(&self) -> String {
        let internal = self.0.borrow();
        let mut contents = String::new();
        let count = internal.editor.line_count();
        for i in 0..count {
            if let Some(line) = internal.editor.line(i) {
                contents.push_str(&line.text);
                if i + 1 < count {
                    contents.push_str(if line.ending == LineEnding::None {
                        LineEnding::default().as_str()
                    } else {
                        line.ending.as_str()
                    });
                }
            }
        }
        contents
    }

    /// Returns the number of lines.
    pub fn line_count(&self) -> usize {
        self.0.borrow().editor.line_count()
    }

    /// Returns the text of a specific line.
    pub fn line(&self, index: usize) -> Option<Line<'_>> {
        let internal = self.0.borrow();
        let line = internal.editor.line(index)?;
        Some(Line {
            text: Cow::Owned(line.text.into_owned()),
            ending: line.ending,
        })
    }

    /// Returns the cursor context (formatting at cursor position).
    pub fn cursor_context(&self) -> cursor::Context {
        let internal = self.0.borrow();
        let editor_cursor = internal.editor.cursor();
        let line = editor_cursor.position.line;
        let col = editor_cursor.position.column;

        // Bias-left: read style from character before cursor
        let char_style = if let Some(ref pending) = internal.pending_style {
            pending.clone()
        } else {
            internal.editor.style_at(line, col.saturating_sub(1))
        };

        let para_style = internal.editor.paragraph_style(line);

        cursor::Context {
            character: cursor::Character {
                bold: char_style.bold.unwrap_or(false),
                italic: char_style.italic.unwrap_or(false),
                underline: char_style.underline.unwrap_or(false),
                font: char_style.font,
                size: char_style.size,
                color: char_style.color,
            },
            paragraph: cursor::Paragraph {
                alignment: para_style.alignment.unwrap_or_default(),
                spacing_after: para_style.spacing_after.unwrap_or(0.0),
            },
            position: cursor::Position {
                line: editor_cursor.position.line,
                column: editor_cursor.position.column,
            },
        }
    }

    /// Returns whether the content is empty.
    pub fn is_empty(&self) -> bool {
        self.0.borrow().editor.is_empty()
    }

    /// Returns whether undo is available.
    pub fn can_undo(&self) -> bool {
        self.0.borrow().history.can_undo()
    }

    /// Returns whether redo is available.
    pub fn can_redo(&self) -> bool {
        self.0.borrow().history.can_redo()
    }
}

impl<R: rich_editor::Renderer> Clone for Content<R> {
    fn clone(&self) -> Self {
        Self::with_text(&self.text())
    }
}

impl<R: rich_editor::Renderer> Default for Content<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: rich_editor::Renderer> std::fmt::Debug for Content<R>
where
    R::RichEditor: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let internal = self.0.borrow();
        f.debug_struct("Content")
            .field("editor", &internal.editor)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Internal implementation
// ---------------------------------------------------------------------------

impl<R: rich_editor::Renderer> Internal<R> {
    // -- Action dispatch -----------------------------------------------------

    fn perform(&mut self, action: Action) {
        match action {
            Action::Edit(edit) => self.perform_edit(edit),
            Action::Undo => self.perform_undo(),
            Action::Redo => self.perform_redo(),
            ref other => {
                if let Some(iced_action) = action::to_iced_action(other) {
                    self.editor.perform(iced_action);
                }
                // Clear pending style on cursor movement
                self.pending_style = None;
            }
        }
    }

    // -- Edit dispatch -------------------------------------------------------

    fn perform_edit(&mut self, edit: Edit) {
        match edit {
            Edit::Format(fmt) => {
                self.history.begin_group();
                self.perform_format(fmt);
                self.history.end_group();
            }
            Edit::Insert(c) => self.perform_insert(c),
            Edit::Paste(ref text) => self.perform_paste(text.clone()),
            Edit::Enter => self.perform_enter(),
            Edit::Backspace => self.perform_backspace(),
            Edit::Delete => self.perform_delete(),
        }
    }

    // -- Insert --------------------------------------------------------------

    fn perform_insert(&mut self, c: char) {
        let cursor = self.editor.cursor();
        let line = cursor.position.line;
        let col = cursor.position.column;

        self.history.begin_group();

        // Determine the style for this character
        let style = self
            .pending_style
            .clone()
            .unwrap_or_else(|| self.editor.style_at(line, col.saturating_sub(1)));

        let op = Op::InsertText {
            line,
            col,
            content: StyledText {
                text: c.to_string(),
                runs: vec![StyleRun {
                    range: 0..c.len_utf8(),
                    style: style.clone(),
                }],
            },
        };
        self.history.record(op);

        // Apply to editor
        self.editor.perform(crate::core::text::editor::Action::Edit(
            crate::core::text::editor::Edit::Insert(c),
        ));
        if let Some(ref style) = self.pending_style {
            self.editor
                .set_span_style(line, col..col + c.len_utf8(), style);
        }

        self.history.end_group();
    }

    // -- Paste ---------------------------------------------------------------

    fn perform_paste(&mut self, text: Arc<String>) {
        let cursor = self.editor.cursor();
        let line = cursor.position.line;
        let col = cursor.position.column;

        self.history.begin_group();

        // For single-line paste (no newlines), record InsertText.
        // Multi-line paste is Phase 4.
        if !text.contains('\n') && !text.contains('\r') {
            let style = self
                .pending_style
                .clone()
                .unwrap_or_else(|| self.editor.style_at(line, col.saturating_sub(1)));
            let op = Op::InsertText {
                line,
                col,
                content: StyledText {
                    text: text.to_string(),
                    runs: vec![StyleRun {
                        range: 0..text.len(),
                        style: style.clone(),
                    }],
                },
            };
            self.history.record(op);
        }

        // Apply to editor
        self.editor.perform(crate::core::text::editor::Action::Edit(
            crate::core::text::editor::Edit::Paste(text.clone()),
        ));
        if let Some(ref style) = self.pending_style {
            let end = col + text.len();
            self.editor.set_span_style(line, col..end, style);
            self.pending_style = None;
        }

        self.history.end_group();
    }

    // -- Enter ---------------------------------------------------------------

    fn perform_enter(&mut self) {
        let cursor = self.editor.cursor();
        let line = cursor.position.line;
        let col = cursor.position.column;

        self.history.begin_group();
        let op = Op::SplitLine { line, col };
        self.history.record(op);

        self.editor.perform(crate::core::text::editor::Action::Edit(
            crate::core::text::editor::Edit::Enter,
        ));
        self.pending_style = None;
        self.history.end_group();
    }

    // -- Backspace -----------------------------------------------------------

    fn perform_backspace(&mut self) {
        let cursor = self.editor.cursor();
        let line = cursor.position.line;
        let col = cursor.position.column;

        self.history.begin_group();

        if cursor.selection.is_some() {
            // Selection delete -- for now, just apply without op recording.
            // Phase 4 will handle multi-line selection delete with proper ops.
            self.editor.perform(crate::core::text::editor::Action::Edit(
                crate::core::text::editor::Edit::Backspace,
            ));
        } else if col > 0 {
            // Single char backspace within a line.
            // Capture the character being deleted.
            let line_text = self.editor.line(line).map(|l| l.text.to_string());
            if let Some(ref text) = line_text {
                // Find the char boundary before col
                let char_start = text[..col]
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let deleted_text = text[char_start..col].to_string();
                let deleted_styled =
                    document::read_styled_text(&self.editor, line, char_start..col, &deleted_text);
                let op = Op::DeleteText {
                    line,
                    col: char_start,
                    content: deleted_styled,
                };
                self.history.record(op);
            }

            self.editor.perform(crate::core::text::editor::Action::Edit(
                crate::core::text::editor::Edit::Backspace,
            ));
        } else if line > 0 {
            // Backspace at line start -- MergeLine.
            let prev_line_len = self
                .editor
                .line(line - 1)
                .map(|l| l.text.len())
                .unwrap_or(0);
            let op = Op::MergeLine {
                line: line - 1,
                col: prev_line_len,
            };
            self.history.record(op);

            self.editor.perform(crate::core::text::editor::Action::Edit(
                crate::core::text::editor::Edit::Backspace,
            ));
        }

        self.pending_style = None;
        self.history.end_group();
    }

    // -- Delete --------------------------------------------------------------

    fn perform_delete(&mut self) {
        let cursor = self.editor.cursor();
        let line = cursor.position.line;
        let col = cursor.position.column;

        self.history.begin_group();

        if cursor.selection.is_some() {
            // Selection delete -- Phase 4.
            self.editor.perform(crate::core::text::editor::Action::Edit(
                crate::core::text::editor::Edit::Delete,
            ));
        } else {
            let line_text = self.editor.line(line).map(|l| l.text.to_string());
            if let Some(ref text) = line_text {
                if col < text.len() {
                    // Delete char at cursor
                    let char_end = text[col..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| col + i)
                        .unwrap_or(text.len());
                    let deleted_text = text[col..char_end].to_string();
                    let deleted_styled = document::read_styled_text(
                        &self.editor,
                        line,
                        col..char_end,
                        &deleted_text,
                    );
                    let op = Op::DeleteText {
                        line,
                        col,
                        content: deleted_styled,
                    };
                    self.history.record(op);

                    self.editor.perform(crate::core::text::editor::Action::Edit(
                        crate::core::text::editor::Edit::Delete,
                    ));
                } else if line + 1 < self.editor.line_count() {
                    // Delete at end of line -- merge next line.
                    let op = Op::MergeLine { line, col };
                    self.history.record(op);

                    self.editor.perform(crate::core::text::editor::Action::Edit(
                        crate::core::text::editor::Edit::Delete,
                    ));
                }
            }
        }

        self.pending_style = None;
        self.history.end_group();
    }

    // -- Format --------------------------------------------------------------

    fn perform_format(&mut self, fmt: FormatAction) {
        let cursor = self.editor.cursor();
        let has_selection = cursor.selection.is_some();

        match fmt {
            FormatAction::ToggleBold => {
                if has_selection {
                    let is_bold = self.style_at_selection_start(&cursor).bold.unwrap_or(false);
                    self.apply_span_formatting_with_history(&RichStyle {
                        bold: Some(!is_bold),
                        ..RichStyle::default()
                    });
                } else {
                    // No selection: toggle pending style
                    let current = self.pending_style.get_or_insert_with(|| {
                        self.editor.style_at(
                            cursor.position.line,
                            cursor.position.column.saturating_sub(1),
                        )
                    });
                    current.bold = Some(!current.bold.unwrap_or(false));
                }
            }
            FormatAction::ToggleItalic => {
                if has_selection {
                    let is_italic = self
                        .style_at_selection_start(&cursor)
                        .italic
                        .unwrap_or(false);
                    self.apply_span_formatting_with_history(&RichStyle {
                        italic: Some(!is_italic),
                        ..RichStyle::default()
                    });
                } else {
                    let current = self.pending_style.get_or_insert_with(|| {
                        self.editor.style_at(
                            cursor.position.line,
                            cursor.position.column.saturating_sub(1),
                        )
                    });
                    current.italic = Some(!current.italic.unwrap_or(false));
                }
            }
            FormatAction::ToggleUnderline => {
                if has_selection {
                    let is_underline = self
                        .style_at_selection_start(&cursor)
                        .underline
                        .unwrap_or(false);
                    self.apply_span_formatting_with_history(&RichStyle {
                        underline: Some(!is_underline),
                        ..RichStyle::default()
                    });
                } else {
                    let current = self.pending_style.get_or_insert_with(|| {
                        self.editor.style_at(
                            cursor.position.line,
                            cursor.position.column.saturating_sub(1),
                        )
                    });
                    current.underline = Some(!current.underline.unwrap_or(false));
                }
            }
            FormatAction::SetAlignment(alignment) => {
                if has_selection {
                    let sel = cursor.selection.as_ref().expect("has_selection checked");
                    let (start, end) = ordered_positions(&cursor.position, sel);
                    for line in start.line..=end.line {
                        let old_style = self.editor.paragraph_style(line);
                        let new_style = ParagraphStyle {
                            alignment: Some(alignment),
                            ..old_style.clone()
                        };
                        let op = Op::SetParagraphStyle {
                            line,
                            style: new_style.clone(),
                            old_style,
                        };
                        self.history.record(op);
                        self.editor.set_paragraph_style(line, &new_style);
                    }
                } else {
                    let line = cursor.position.line;
                    let old_style = self.editor.paragraph_style(line);
                    let new_style = ParagraphStyle {
                        alignment: Some(alignment),
                        ..old_style.clone()
                    };
                    let op = Op::SetParagraphStyle {
                        line,
                        style: new_style.clone(),
                        old_style,
                    };
                    self.history.record(op);
                    self.editor.set_paragraph_style(line, &new_style);
                }
            }
            FormatAction::SetFont(font) => {
                if has_selection {
                    self.apply_span_formatting_with_history(&RichStyle {
                        font: Some(font),
                        ..RichStyle::default()
                    });
                } else {
                    let current = self.pending_style.get_or_insert_with(|| {
                        self.editor.style_at(
                            cursor.position.line,
                            cursor.position.column.saturating_sub(1),
                        )
                    });
                    current.font = Some(font);
                }
            }
            FormatAction::SetFontSize(size) => {
                if has_selection {
                    self.apply_span_formatting_with_history(&RichStyle {
                        size: Some(size),
                        ..RichStyle::default()
                    });
                } else {
                    let current = self.pending_style.get_or_insert_with(|| {
                        self.editor.style_at(
                            cursor.position.line,
                            cursor.position.column.saturating_sub(1),
                        )
                    });
                    current.size = Some(size);
                }
            }
        }
    }

    // -- Undo / Redo ---------------------------------------------------------

    fn perform_undo(&mut self) {
        let Some(group) = self.history.undo() else {
            return;
        };

        let mut redo_ops = Vec::new();

        // Apply inverses in reverse order
        for op in group.into_iter().rev() {
            let inverses = op.inverse();
            for inv_op in inverses {
                let captured = self.capture_op_state(&inv_op);
                self.apply_op(&captured);
                redo_ops.push(captured);
            }
        }

        self.history.push_redo(redo_ops);
        self.pending_style = None;
    }

    fn perform_redo(&mut self) {
        let Some(group) = self.history.redo() else {
            return;
        };

        let mut undo_ops = Vec::new();

        for op in group.into_iter().rev() {
            let inverses = op.inverse();
            for inv_op in inverses {
                let captured = self.capture_op_state(&inv_op);
                self.apply_op(&captured);
                undo_ops.push(captured);
            }
        }

        self.history.push_undo(undo_ops);
        self.pending_style = None;
    }

    // -- Op capture and replay -----------------------------------------------

    /// Fill old_* fields from the current editor state.
    fn capture_op_state(&self, op: &Op) -> Op {
        match op {
            Op::InsertText { .. } | Op::SplitLine { .. } | Op::MergeLine { .. } => op.clone(),
            Op::DeleteText { line, col, content } => {
                // Re-capture what's currently at the position.
                let end = *col + content.text.len();
                let current_text = self
                    .editor
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
                        &self.editor,
                        *line,
                        *col..(*col + current_text.len()),
                        &current_text,
                    ),
                }
            }
            Op::SetSpanStyle {
                line, range, style, ..
            } => {
                let old_runs = document::read_style_runs(&self.editor, *line, range.clone());
                Op::SetSpanStyle {
                    line: *line,
                    range: range.clone(),
                    style: style.clone(),
                    old_runs,
                }
            }
            Op::SetParagraphStyle { line, style, .. } => {
                let old_style = self.editor.paragraph_style(*line);
                Op::SetParagraphStyle {
                    line: *line,
                    style: style.clone(),
                    old_style,
                }
            }
        }
    }

    /// Replay an operation on the editor.
    fn apply_op(&mut self, op: &Op) {
        use crate::core::text::editor as iced_editor;

        match op {
            Op::InsertText { line, col, content } => {
                self.editor.move_to(Cursor {
                    position: Position {
                        line: *line,
                        column: *col,
                    },
                    selection: None,
                });
                if content.text.len() == 1 {
                    let c = content.text.chars().next().expect("non-empty text");
                    self.editor
                        .perform(iced_editor::Action::Edit(iced_editor::Edit::Insert(c)));
                } else {
                    self.editor
                        .perform(iced_editor::Action::Edit(iced_editor::Edit::Paste(
                            Arc::new(content.text.clone()),
                        )));
                }
                // Apply style runs
                for run in &content.runs {
                    let abs_start = *col + run.range.start;
                    let abs_end = *col + run.range.end;
                    self.editor
                        .set_span_style(*line, abs_start..abs_end, &run.style);
                }
            }
            Op::DeleteText { line, col, content } => {
                let end_col = *col + content.text.len();
                self.editor.move_to(Cursor {
                    position: Position {
                        line: *line,
                        column: *col,
                    },
                    selection: Some(Position {
                        line: *line,
                        column: end_col,
                    }),
                });
                self.editor
                    .perform(iced_editor::Action::Edit(iced_editor::Edit::Delete));
            }
            Op::SplitLine { line, col } => {
                self.editor.move_to(Cursor {
                    position: Position {
                        line: *line,
                        column: *col,
                    },
                    selection: None,
                });
                self.editor
                    .perform(iced_editor::Action::Edit(iced_editor::Edit::Enter));
            }
            Op::MergeLine { line, .. } => {
                // Position at start of next line and backspace
                self.editor.move_to(Cursor {
                    position: Position {
                        line: *line + 1,
                        column: 0,
                    },
                    selection: None,
                });
                self.editor
                    .perform(iced_editor::Action::Edit(iced_editor::Edit::Backspace));
            }
            Op::SetSpanStyle {
                line, range, style, ..
            } => {
                self.editor.set_span_style(*line, range.clone(), style);
            }
            Op::SetParagraphStyle { line, style, .. } => {
                self.editor.set_paragraph_style(*line, style);
            }
        }
    }

    // -- Formatting helpers --------------------------------------------------

    /// Returns the Style at the first non-empty character in the selection.
    ///
    /// Skips blank lines at the start of the selection so that the toggle
    /// state reflects actual content, not unformatted newlines.
    fn style_at_selection_start(&self, cursor: &Cursor) -> RichStyle {
        let (start, end) = match &cursor.selection {
            Some(sel) => ordered_positions(&cursor.position, sel),
            None => {
                return self
                    .editor
                    .style_at(cursor.position.line, cursor.position.column);
            }
        };

        for line in start.line..=end.line {
            let col_start = if line == start.line { start.column } else { 0 };
            let col_end = if line == end.line {
                end.column
            } else {
                self.editor.line(line).map(|l| l.text.len()).unwrap_or(0)
            };
            if col_start < col_end {
                return self.editor.style_at(line, col_start);
            }
        }

        self.editor.style_at(start.line, start.column)
    }

    /// Apply a span style across the current selection, capturing old runs and
    /// recording `Op::SetSpanStyle` for each affected line.
    fn apply_span_formatting_with_history(&mut self, style: &RichStyle) {
        let cursor = self.editor.cursor();
        let Some(sel_pos) = cursor.selection else {
            return;
        };
        let (start, end) = ordered_positions(&cursor.position, &sel_pos);

        for line in start.line..=end.line {
            let col_start = if line == start.line { start.column } else { 0 };
            let col_end = if line == end.line {
                end.column
            } else {
                self.editor.line(line).map(|l| l.text.len()).unwrap_or(0)
            };
            if col_start < col_end {
                let old_runs = document::read_style_runs(&self.editor, line, col_start..col_end);
                let op = Op::SetSpanStyle {
                    line,
                    range: col_start..col_end,
                    style: style.clone(),
                    old_runs,
                };
                self.history.record(op);
                self.editor.set_span_style(line, col_start..col_end, style);
            }
        }
    }
}

/// Order two positions so that `start` comes before `end`.
fn ordered_positions<'a>(a: &'a Position, b: &'a Position) -> (&'a Position, &'a Position) {
    if a.line < b.line || (a.line == b.line && a.column <= b.column) {
        (a, b)
    } else {
        (b, a)
    }
}
