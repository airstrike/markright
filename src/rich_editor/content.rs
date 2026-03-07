use iced_core::text::Alignment;
use iced_core::text::rich_editor::{self, Editor as _, ParagraphStyle, Style as RichStyle};

use std::borrow::Cow;
use std::cell::RefCell;

use super::action::{self, Action, Edit, FormatAction};
use super::cursor;

pub use iced_core::text::editor::{Cursor, Line, LineEnding, Position};

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
                alignment: para_style.alignment.unwrap_or(Alignment::Default),
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
            ref other => {
                if let Some(iced_action) = action::to_iced_action(other) {
                    self.editor.perform(iced_action);
                }
                // Clear pending style on cursor movement
                self.pending_style = None;
            }
        }
    }

    fn perform_edit(&mut self, edit: Edit) {
        match edit {
            Edit::Format(fmt) => self.perform_format(fmt),
            Edit::Insert(c) => {
                let cursor_before = self.editor.cursor();
                self.editor.perform(iced_core::text::editor::Action::Edit(
                    iced_core::text::editor::Edit::Insert(c),
                ));

                // Apply pending style to the inserted character
                if let Some(ref style) = self.pending_style {
                    let line = cursor_before.position.line;
                    let col = cursor_before.position.column;
                    self.editor
                        .set_span_style(line, col..col + c.len_utf8(), style);
                }
            }
            Edit::Paste(ref text) => {
                let cursor_before = self.editor.cursor();
                self.editor.perform(iced_core::text::editor::Action::Edit(
                    iced_core::text::editor::Edit::Paste(text.clone()),
                ));

                // Apply pending style to pasted text
                if let Some(ref style) = self.pending_style {
                    let line = cursor_before.position.line;
                    let col = cursor_before.position.column;
                    // Note: paste may span multiple lines. For simplicity,
                    // apply to the first line only for now.
                    let end = col + text.len();
                    self.editor.set_span_style(line, col..end, style);
                    self.pending_style = None;
                }
            }
            Edit::Enter => {
                self.editor.perform(iced_core::text::editor::Action::Edit(
                    iced_core::text::editor::Edit::Enter,
                ));
                self.pending_style = None;
            }
            Edit::Backspace => {
                self.editor.perform(iced_core::text::editor::Action::Edit(
                    iced_core::text::editor::Edit::Backspace,
                ));
                self.pending_style = None;
            }
            Edit::Delete => {
                self.editor.perform(iced_core::text::editor::Action::Edit(
                    iced_core::text::editor::Edit::Delete,
                ));
                self.pending_style = None;
            }
        }
    }

    fn perform_format(&mut self, fmt: FormatAction) {
        let cursor = self.editor.cursor();
        let has_selection = cursor.selection.is_some();

        match fmt {
            FormatAction::ToggleBold => {
                if has_selection {
                    let is_bold = self.style_at_selection_start(&cursor).bold.unwrap_or(false);
                    self.apply_span_formatting(|editor, line, range| {
                        editor.set_span_style(
                            line,
                            range,
                            &RichStyle {
                                bold: Some(!is_bold),
                                ..RichStyle::default()
                            },
                        );
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
                    self.apply_span_formatting(|editor, line, range| {
                        editor.set_span_style(
                            line,
                            range,
                            &RichStyle {
                                italic: Some(!is_italic),
                                ..RichStyle::default()
                            },
                        );
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
                    self.apply_span_formatting(|editor, line, range| {
                        editor.set_span_style(
                            line,
                            range,
                            &RichStyle {
                                underline: Some(!is_underline),
                                ..RichStyle::default()
                            },
                        );
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
                let line = cursor.position.line;
                self.editor.set_paragraph_style(
                    line,
                    &ParagraphStyle {
                        alignment: Some(alignment),
                        ..ParagraphStyle::default()
                    },
                );
            }
            FormatAction::SetFont(font) => {
                if has_selection {
                    self.apply_span_formatting(move |editor, line, range| {
                        editor.set_span_style(
                            line,
                            range,
                            &RichStyle {
                                font: Some(font),
                                ..RichStyle::default()
                            },
                        );
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
                    self.apply_span_formatting(move |editor, line, range| {
                        editor.set_span_style(
                            line,
                            range,
                            &RichStyle {
                                size: Some(size),
                                ..RichStyle::default()
                            },
                        );
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

    /// Apply a formatting operation across the current selection.
    fn apply_span_formatting(
        &mut self,
        apply: impl Fn(&mut R::RichEditor, usize, std::ops::Range<usize>),
    ) {
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
                apply(&mut self.editor, line, col_start..col_end);
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
