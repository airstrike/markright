//! Rich text editor content — wraps the editor and manages pending style
//! and undo/redo history. All edits flow through [`Content::perform`].

use crate::core::Font;
use crate::core::text::editor::Position;
use crate::core::text::rich_editor::{self, Editor as _, Style as RichStyle};
use markright_document::{History, Op};

use std::borrow::Cow;
use std::cell::RefCell;

use super::action::{self, Action, Edit, FormatAction};
use super::cursor;
use super::operation;

pub use crate::core::text::editor::{Cursor, Line, LineEnding};
pub use markright_document::{StyleRun, StyledLine};

/// Returns the style at the first non-empty character in a selection.
///
/// Skips blank lines so the reported style reflects actual content.
fn style_at_selection_start<E: rich_editor::Editor>(
    editor: &E,
    pos: &Position,
    sel: &Position,
) -> RichStyle {
    let (start, end) = operation::ordered_positions(pos, sel);
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
    /// Document-level default font, used as fallback when a span has no
    /// explicit font set.
    default_font: Option<Font>,
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
            default_font: None,
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
    ///
    /// When a selection is active, reports the style at the first non-empty
    /// character in the selection (matching the toggle logic in format ops).
    /// Without a selection, bias-left reads the character before the cursor.
    pub fn cursor_context(&self) -> cursor::Context {
        let internal = self.0.borrow();
        let editor_cursor = internal.editor.cursor();

        let char_style = if let Some(ref pending) = internal.pending_style {
            pending.clone()
        } else if let Some(ref sel) = editor_cursor.selection {
            // With a selection: read from the first non-empty content character
            style_at_selection_start(&internal.editor, &editor_cursor.position, sel)
        } else {
            // No selection: bias-left
            let line = editor_cursor.position.line;
            let col = editor_cursor.position.column;
            internal.editor.style_at(line, col.saturating_sub(1))
        };

        let line = editor_cursor.position.line;
        let para_style = internal.editor.paragraph_style(line);

        cursor::Context {
            character: cursor::Character {
                bold: char_style.bold.unwrap_or(false),
                italic: char_style.italic.unwrap_or(false),
                underline: char_style.underline.unwrap_or(false),
                font: char_style.font.or(internal.default_font),
                size: char_style.size,
                color: char_style.color,
            },
            paragraph: cursor::Paragraph {
                alignment: super::Alignment::from_iced(para_style.alignment),
                spacing_after: para_style.spacing_after.unwrap_or(0.0),
            },
            position: cursor::Position {
                line: editor_cursor.position.line,
                column: editor_cursor.position.column,
            },
        }
    }

    /// Returns per-line styled content for debugging/inspection.
    pub fn styled_line(&self, index: usize) -> Option<markright_document::StyledLine> {
        let internal = self.0.borrow();
        let line = internal.editor.line(index)?;
        let len = line.text.len();
        Some(markright_document::read_styled_line(
            &internal.editor,
            index,
            0..len,
        ))
    }

    /// Returns whether the content is empty.
    pub fn is_empty(&self) -> bool {
        self.0.borrow().editor.is_empty()
    }

    /// Returns a Debug-formatted dump of cursor, style, and paragraph state.
    pub fn debug_state(&self) -> String {
        use std::fmt::Write;
        let internal = self.0.borrow();
        let c = internal.editor.cursor();
        let col = c.position.column.saturating_sub(1);
        let style = internal.editor.style_at(c.position.line, col);
        let para = internal.editor.paragraph_style(c.position.line);
        let mut out = String::new();
        let _ = write!(out, "{c:#?}\n{style:#?}\n{para:#?}");
        out
    }

    /// Returns whether undo is available.
    pub fn can_undo(&self) -> bool {
        self.0.borrow().history.can_undo()
    }

    /// Number of undo groups.
    pub fn undo_len(&self) -> usize {
        self.0.borrow().history.undo_len()
    }

    /// Number of redo groups.
    pub fn redo_len(&self) -> usize {
        self.0.borrow().history.redo_len()
    }

    /// Returns whether redo is available.
    pub fn can_redo(&self) -> bool {
        self.0.borrow().history.can_redo()
    }

    /// Sets the document-level default font.
    ///
    /// This font is used as a fallback when a span has no explicit font set.
    /// Existing spans without an explicit font are updated immediately.
    pub fn set_default_font(&self, font: Font) {
        let mut internal = self.0.borrow_mut();
        internal.default_font = Some(font);
        internal.apply_default_font_to_all();
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

impl<R: rich_editor::Renderer> Internal<R> {
    fn perform(&mut self, action: Action) {
        match action {
            Action::Edit(edit) => self.perform_edit(edit),
            Action::Undo => self.perform_undo(),
            Action::Redo => self.perform_redo(),
            ref other => {
                if let Some(iced_action) = action::to_iced_action(other) {
                    self.editor.perform(iced_action);
                }
                self.pending_style = None;
            }
        }
    }

    fn perform_edit(&mut self, edit: Edit) {
        match edit {
            Edit::Insert(c) => {
                let style = self.resolve_style();
                let mut ops = self.delete_selection_if_any();
                let op = operation::insert(&mut self.editor, c, style);
                ops.push(op);
                self.record_group(ops);
            }
            Edit::Paste(ref text) => {
                let style = self.resolve_style();
                let mut ops = self.delete_selection_if_any();
                ops.extend(operation::paste(&mut self.editor, text.clone(), style));
                self.record_group(ops);
                self.pending_style = None;
            }
            Edit::Enter => {
                let mut ops = self.delete_selection_if_any();
                let op = operation::enter(&mut self.editor);
                ops.push(op);
                self.record_group(ops);
                self.pending_style = None;
            }
            Edit::Backspace => {
                let ops = operation::backspace(&mut self.editor);
                self.record_group(ops);
                self.pending_style = None;
            }
            Edit::Delete => {
                let ops = operation::delete(&mut self.editor);
                self.record_group(ops);
                self.pending_style = None;
            }
            Edit::Format(ref fmt) => {
                let ops = operation::format(&mut self.editor, fmt);
                if !ops.is_empty() {
                    self.record_group(ops);
                } else {
                    self.update_pending_style(fmt);
                }
            }
        }
    }

    /// Delete the current selection (if any) and return the ops.
    ///
    /// After this call the cursor is at the start of where the selection was,
    /// with no selection — ready for an insert or enter.
    fn delete_selection_if_any(&mut self) -> Vec<Op> {
        if self.editor.cursor().selection.is_some() {
            operation::backspace(&mut self.editor)
        } else {
            Vec::new()
        }
    }

    fn resolve_style(&self) -> RichStyle {
        let mut style = self.pending_style.clone().unwrap_or_else(|| {
            let cursor = self.editor.cursor();
            self.editor.style_at(
                cursor.position.line,
                cursor.position.column.saturating_sub(1),
            )
        });
        if style.font.is_none() {
            style.font = self.default_font;
        }
        style
    }

    fn record_group(&mut self, ops: Vec<Op>) {
        if ops.is_empty() {
            return;
        }
        self.history.begin_group();
        for op in ops {
            self.history.record(op);
        }
        self.history.end_group();
    }

    fn update_pending_style(&mut self, fmt: &FormatAction) {
        let cursor = self.editor.cursor();
        let current = self.pending_style.get_or_insert_with(|| {
            self.editor.style_at(
                cursor.position.line,
                cursor.position.column.saturating_sub(1),
            )
        });
        match fmt {
            FormatAction::ToggleBold => current.bold = Some(!current.bold.unwrap_or(false)),
            FormatAction::ToggleItalic => current.italic = Some(!current.italic.unwrap_or(false)),
            FormatAction::ToggleUnderline => {
                current.underline = Some(!current.underline.unwrap_or(false));
            }
            FormatAction::SetFont(font) => current.font = Some(*font),
            FormatAction::SetFontSize(size) => current.size = Some(*size),
            FormatAction::SetAlignment(_) => {}
        }
    }

    /// Apply the default font to all spans that have no explicit font set.
    fn apply_default_font_to_all(&mut self) {
        let Some(font) = self.default_font else {
            return;
        };
        let line_count = self.editor.line_count();
        for line in 0..line_count {
            let len = self.editor.line(line).map(|l| l.text.len()).unwrap_or(0);
            if len == 0 {
                continue;
            }
            let runs = markright_document::read_style_runs(&self.editor, line, 0..len);
            for run in runs {
                if run.style.font.is_none() {
                    let mut style = run.style;
                    style.font = Some(font);
                    self.editor.set_span_style(line, run.range, &style);
                }
            }
        }
    }

    fn perform_undo(&mut self) {
        let Some(group) = self.history.undo() else {
            return;
        };

        let mut redo_ops = Vec::new();
        for op in group.into_iter().rev() {
            for inv_op in op.inverse() {
                let captured = operation::capture_op_state(&self.editor, &inv_op);
                operation::apply_op(&mut self.editor, &captured);
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
            for inv_op in op.inverse() {
                let captured = operation::capture_op_state(&self.editor, &inv_op);
                operation::apply_op(&mut self.editor, &captured);
                undo_ops.push(captured);
            }
        }

        self.history.push_undo(undo_ops);
        self.pending_style = None;
    }
}
