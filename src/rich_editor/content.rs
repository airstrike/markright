use iced_core::text::editor::Editor as _;
use iced_core::text::{self};

use std::borrow::Cow;
use std::cell::RefCell;

use super::action::{self, Action, Edit, FormatAction};
use super::cursor;
use crate::document::RichDocument;
use crate::shortcuts::{self, MarkdownAction};

pub use iced_core::text::editor::{Cursor, Line, LineEnding, Position};

/// The content of a rich text editor — wraps both the text buffer (via iced's
/// `R::Editor`) and the rich formatting model ([`RichDocument`]).
///
/// This is the single source of truth: all edits and formatting changes go
/// through [`Content::perform`].
pub struct Content<R: text::Renderer>(pub(crate) RefCell<Internal<R>>);

pub(crate) struct Internal<R: text::Renderer> {
    pub(crate) editor: R::Editor,
    pub(crate) document: RichDocument,
    pub(crate) doc_version: u64,
}

impl<R: text::Renderer> Content<R> {
    /// Create an empty [`Content`].
    pub fn new() -> Self {
        Self::with_text("")
    }

    /// Create a [`Content`] with the given text.
    pub fn with_text(text: &str) -> Self {
        let line_count = text.lines().count().max(1);
        Self(RefCell::new(Internal {
            editor: R::Editor::with_text(text),
            document: RichDocument::with_lines(line_count),
            doc_version: 0,
        }))
    }

    /// Perform an [`Action`] on the content.
    ///
    /// This is the single entry point for all mutations — text edits,
    /// formatting changes, navigation, and selection.
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
        if line >= internal.document.line_count() {
            return cursor::Context::default();
        }
        let span = internal
            .document
            .format_at(line, editor_cursor.position.column);
        let line_fmt = internal.document.line_format(line);
        cursor::Context {
            character: cursor::Character {
                bold: span.bold,
                italic: span.italic,
                underline: span.underline,
                font: span.font,
                size: span.size,
                color: span.color,
            },
            paragraph: cursor::Paragraph {
                alignment: line_fmt.alignment,
                heading: line_fmt.heading_level,
                spacing_after: line_fmt.spacing_after,
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

impl<R: text::Renderer> Clone for Content<R> {
    fn clone(&self) -> Self {
        Self::with_text(&self.text())
    }
}

impl<R: text::Renderer> Default for Content<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: text::Renderer> std::fmt::Debug for Content<R>
where
    R::Editor: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let internal = self.0.borrow();
        f.debug_struct("Content")
            .field("editor", &internal.editor)
            .field("doc_version", &internal.doc_version)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Internal implementation
// ---------------------------------------------------------------------------

impl<R: text::Renderer> Internal<R> {
    fn perform(&mut self, action: Action) {
        match action {
            Action::Edit(edit) => self.perform_edit(edit),
            other => {
                if let Some(iced_action) = action::to_iced_action(&other) {
                    self.editor.perform(iced_action);
                }
            }
        }
    }

    fn perform_edit(&mut self, edit: Edit) {
        match edit {
            Edit::Format(fmt) => self.perform_format(fmt),
            ref text_edit => {
                let before_lines = self.editor.line_count();
                let before_cursor = self.editor.cursor();

                // Forward the text edit to iced's editor.
                if let Some(iced_action) = action::to_iced_action(&Action::Edit(text_edit.clone()))
                {
                    self.editor.perform(iced_action);
                }

                let after_lines = self.editor.line_count();
                let after_cursor = self.editor.cursor();

                // Sync the document model.
                self.sync_document(&before_cursor, before_lines, &after_cursor, after_lines);
                self.doc_version += 1;

                // Detect and apply typing shortcuts (e.g., **bold**, # heading).
                self.detect_and_apply_shortcuts();
            }
        }
    }

    fn perform_format(&mut self, fmt: FormatAction) {
        let cursor = self.editor.cursor();
        match fmt {
            FormatAction::ToggleBold => {
                let is_bold = self.format_at_selection_start(&cursor).bold;
                self.apply_span_formatting(|doc, line, range| {
                    doc.set_format_property(line, range, |f| f.bold = !is_bold);
                });
            }
            FormatAction::ToggleItalic => {
                let is_italic = self.format_at_selection_start(&cursor).italic;
                self.apply_span_formatting(|doc, line, range| {
                    doc.set_format_property(line, range, |f| f.italic = !is_italic);
                });
            }
            FormatAction::ToggleUnderline => {
                let is_underline = self.format_at_selection_start(&cursor).underline;
                self.apply_span_formatting(|doc, line, range| {
                    doc.set_format_property(line, range, |f| f.underline = !is_underline);
                });
            }
            FormatAction::SetHeadingLevel(level) => {
                let line = cursor.position.line;
                if line < self.document.line_count() {
                    self.document.line_format_mut(line).heading_level = level;
                }
            }
            FormatAction::SetAlignment(alignment) => {
                let line = cursor.position.line;
                if line < self.document.line_count() {
                    self.document.line_format_mut(line).alignment = alignment;
                }
            }
            FormatAction::SetFont(font) => {
                self.apply_span_formatting(move |doc, line, range| {
                    doc.set_format_property(line, range, |f| f.font = Some(font));
                });
            }
            FormatAction::SetFontSize(size) => {
                self.apply_span_formatting(move |doc, line, range| {
                    doc.set_format_property(line, range, |f| f.size = Some(size));
                });
            }
        }
        self.doc_version += 1;
    }

    /// Returns the [`SpanFormat`] at the start of the selection.
    ///
    /// For Word-style toggle behavior, we sample the format at the *first*
    /// character of the selection to decide the toggle direction. If there
    /// is no selection, falls back to `cursor.position`.
    fn format_at_selection_start(&self, cursor: &Cursor) -> crate::document::SpanFormat {
        let pos = match &cursor.selection {
            Some(sel) => ordered_positions(&cursor.position, sel).0,
            None => &cursor.position,
        };
        if pos.line < self.document.line_count() {
            self.document.format_at(pos.line, pos.column)
        } else {
            crate::document::SpanFormat::default()
        }
    }

    /// Apply a formatting operation across the current selection.
    fn apply_span_formatting(
        &mut self,
        apply: impl Fn(&mut RichDocument, usize, std::ops::Range<usize>),
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
                apply(&mut self.document, line, col_start..col_end);
            }
        }
    }

    /// Sync the RichDocument structure after a text edit.
    fn sync_document(
        &mut self,
        before_cursor: &Cursor,
        before_lines: usize,
        after_cursor: &Cursor,
        after_lines: usize,
    ) {
        let before = &before_cursor.position;
        let after = &after_cursor.position;

        if after_lines > before_lines {
            let lines_added = after_lines - before_lines;
            for _ in 0..lines_added {
                if before.line < self.document.line_count() {
                    self.document.split_line(before.line, before.column);
                }
            }
        } else if after_lines < before_lines {
            let lines_removed = before_lines - after_lines;
            for _ in 0..lines_removed {
                if after.line < self.document.line_count().saturating_sub(1) {
                    self.document.merge_lines(after.line);
                }
            }
        }

        if after_lines == before_lines && before.line == after.line {
            if after.column > before.column {
                let inserted = after.column - before.column;
                self.document
                    .insert_at(before.line, before.column, inserted);
            } else if after.column < before.column {
                self.document
                    .delete_range(before.line, after.column, before.column);
            }
        }

        self.document.ensure_lines(after_lines);
    }

    /// Detect completed typing shortcuts on the current line and apply them.
    fn detect_and_apply_shortcuts(&mut self) {
        let cursor = self.editor.cursor();
        let line_idx = cursor.position.line;

        let line_text = match self.editor.line(line_idx) {
            Some(l) => l.text.into_owned(),
            None => return,
        };

        let actions = shortcuts::detect_patterns(&line_text);
        if actions.is_empty() {
            return;
        }

        for action in actions {
            match action {
                MarkdownAction::Heading { level, marker } => {
                    self.remove_range_from_editor(line_idx, &marker);
                    self.document
                        .delete_range(line_idx, marker.start, marker.end);
                    self.document.line_format_mut(line_idx).heading_level = Some(level);
                    self.doc_version += 1;
                }
                MarkdownAction::Bold { markers, .. } => {
                    let adjusted = self.remove_markers_from_editor(line_idx, &markers);
                    self.remove_markers_from_document(line_idx, &markers);
                    self.document.toggle_bold(line_idx, adjusted);
                    self.doc_version += 1;
                }
                MarkdownAction::Italic { markers, .. } => {
                    let adjusted = self.remove_markers_from_editor(line_idx, &markers);
                    self.remove_markers_from_document(line_idx, &markers);
                    self.document.toggle_italic(line_idx, adjusted);
                    self.doc_version += 1;
                }
                MarkdownAction::BoldItalic { markers, .. } => {
                    let adjusted = self.remove_markers_from_editor(line_idx, &markers);
                    self.remove_markers_from_document(line_idx, &markers);
                    self.document.toggle_bold(line_idx, adjusted.clone());
                    self.document.toggle_italic(line_idx, adjusted);
                    self.doc_version += 1;
                }
                MarkdownAction::Code { markers, .. } => {
                    let adjusted = self.remove_markers_from_editor(line_idx, &markers);
                    self.remove_markers_from_document(line_idx, &markers);
                    let mono_font = iced_core::Font::with_name("IBM Plex Mono");
                    self.document.set_format_property(line_idx, adjusted, |f| {
                        f.font = Some(mono_font);
                    });
                    self.doc_version += 1;
                }
            }
        }
    }

    /// Remove a byte range from the editor on a given line.
    fn remove_range_from_editor(&mut self, line: usize, range: &std::ops::Range<usize>) {
        use iced_core::text::editor::{Action as IcedAction, Edit as IcedEdit, Motion};

        let range_len = range.end - range.start;
        if range_len == 0 {
            return;
        }

        // Move cursor to end of range.
        self.editor.move_to(Cursor {
            position: Position {
                line,
                column: range.end,
            },
            selection: None,
        });

        // Select backwards.
        for _ in 0..range_len {
            self.editor.perform(IcedAction::Select(Motion::Left));
        }

        // Delete selection.
        self.editor.perform(IcedAction::Edit(IcedEdit::Backspace));
    }

    /// Remove marker ranges from the editor (right-to-left) and return the
    /// adjusted content range.
    fn remove_markers_from_editor(
        &mut self,
        line: usize,
        markers: &[std::ops::Range<usize>],
    ) -> std::ops::Range<usize> {
        let mut sorted_markers: Vec<_> = markers.to_vec();
        sorted_markers.sort_by(|a, b| b.start.cmp(&a.start));

        let first_marker_end = markers
            .iter()
            .map(|m| m.end)
            .min()
            .expect("markers should be non-empty");
        let last_marker_start = markers
            .iter()
            .map(|m| m.start)
            .max()
            .expect("markers should be non-empty");

        let content_start = first_marker_end;
        let content_end = last_marker_start;

        let mut removed_before_content = 0usize;
        for marker in &sorted_markers {
            self.remove_range_from_editor(line, marker);
            if marker.end <= content_start {
                removed_before_content += marker.end - marker.start;
            }
        }

        let adjusted_start = content_start - removed_before_content;
        let adjusted_end = content_end - removed_before_content;
        adjusted_start..adjusted_end
    }

    /// Remove markers from the RichDocument (right-to-left).
    fn remove_markers_from_document(&mut self, line: usize, markers: &[std::ops::Range<usize>]) {
        let mut sorted_markers: Vec<_> = markers.to_vec();
        sorted_markers.sort_by(|a, b| b.start.cmp(&a.start));

        for marker in &sorted_markers {
            self.document.delete_range(line, marker.start, marker.end);
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

#[cfg(test)]
mod tests {
    use super::*;

    type TestContent = Content<iced::Renderer>;

    #[test]
    fn new_content_is_empty() {
        let c = TestContent::new();
        assert!(c.is_empty());
        assert_eq!(c.line_count(), 1);
    }

    #[test]
    fn content_with_text() {
        let c = TestContent::with_text("hello\nworld");
        assert!(!c.is_empty());
        assert_eq!(c.line_count(), 2);
    }

    #[test]
    fn cursor_context_default_on_empty() {
        let c = TestContent::new();
        let ctx = c.cursor_context();
        assert!(!ctx.character.bold);
        assert!(!ctx.character.italic);
        assert!(!ctx.character.underline);
        assert_eq!(ctx.paragraph.heading, None);
    }

    // -----------------------------------------------------------------------
    // Bold toggle tests
    // -----------------------------------------------------------------------

    /// Helper: select all text and toggle bold.
    fn select_all_and_toggle_bold(c: &TestContent) {
        c.perform(Action::SelectAll);
        c.perform(Action::Edit(Edit::Format(FormatAction::ToggleBold)));
    }

    /// Helper: access the internal document for assertions.
    fn with_doc<F, T>(c: &TestContent, f: F) -> T
    where
        F: FnOnce(&crate::document::RichDocument) -> T,
    {
        let internal = c.0.borrow();
        f(&internal.document)
    }

    #[test]
    fn select_all_toggle_bold_on_plain_text() {
        let c = TestContent::with_text("hello");
        select_all_and_toggle_bold(&c);

        let spans = with_doc(&c, |doc| doc.spans(0).to_vec());
        assert!(!spans.is_empty(), "should have bold spans");
        assert!(spans[0].1.bold, "text should be bold");
    }

    #[test]
    fn select_all_toggle_bold_removes_bold() {
        let c = TestContent::with_text("hello");

        // First toggle: make bold
        select_all_and_toggle_bold(&c);
        let bold_after_first = with_doc(&c, |doc| doc.spans(0).iter().all(|(_, f)| f.bold));
        assert!(bold_after_first, "first toggle should make text bold");

        // Second toggle: remove bold
        select_all_and_toggle_bold(&c);
        let spans_after_second = with_doc(&c, |doc| doc.spans(0).to_vec());
        // All spans should be gone (back to default = plain text)
        assert!(
            spans_after_second.is_empty(),
            "second toggle should remove bold, but got: {spans_after_second:?}"
        );
    }

    #[test]
    fn bold_toggle_mixed_makes_all_bold() {
        // Simulate: "hello world" where "world" (6..11) is already bold.
        // Select all + Cmd+B should make everything bold (cursor starts at
        // position 0 which is NOT bold).
        let c = TestContent::with_text("hello world");

        // Make "world" bold by selecting it
        {
            let mut internal = c.0.borrow_mut();
            internal
                .document
                .set_format_property(0, 6..11, |f| f.bold = true);
            internal.doc_version += 1;
        }

        // Verify setup: position 0 is not bold, position 6 is bold
        let fmt_at_0 = with_doc(&c, |doc| doc.format_at(0, 0));
        assert!(!fmt_at_0.bold);
        let fmt_at_6 = with_doc(&c, |doc| doc.format_at(0, 6));
        assert!(fmt_at_6.bold);

        // Select all and toggle bold
        select_all_and_toggle_bold(&c);

        // Everything should be bold now (toggle direction from start = not bold)
        let all_bold = with_doc(&c, |doc| (0..11).all(|col| doc.format_at(0, col).bold));
        assert!(
            all_bold,
            "mixed selection starting non-bold should make all bold"
        );
    }

    #[test]
    fn bold_toggle_mixed_starting_bold_removes_all() {
        // "hello world" where "hello" (0..5) is bold but "world" is not.
        // Select all + Cmd+B → cursor starts at position 0 which IS bold
        // → should remove bold from everything.
        let c = TestContent::with_text("hello world");

        {
            let mut internal = c.0.borrow_mut();
            internal
                .document
                .set_format_property(0, 0..5, |f| f.bold = true);
            internal.doc_version += 1;
        }

        select_all_and_toggle_bold(&c);

        let any_bold = with_doc(&c, |doc| doc.spans(0).iter().any(|(_, f)| f.bold));
        assert!(
            !any_bold,
            "mixed selection starting bold should remove all bold"
        );
    }

    #[test]
    fn toggle_bold_multiline_consistent_direction() {
        // Two lines: "aaa\nbbb", neither bold.
        // Select all + Cmd+B → both lines become bold.
        let c = TestContent::with_text("aaa\nbbb");

        select_all_and_toggle_bold(&c);

        let line0_bold = with_doc(&c, |doc| {
            doc.spans(0).iter().all(|(_, f)| f.bold) && !doc.spans(0).is_empty()
        });
        let line1_bold = with_doc(&c, |doc| {
            doc.spans(1).iter().all(|(_, f)| f.bold) && !doc.spans(1).is_empty()
        });
        assert!(line0_bold, "line 0 should be bold");
        assert!(line1_bold, "line 1 should be bold");
    }

    #[test]
    fn toggle_bold_multiline_mixed_uses_start() {
        // "aaa\nbbb" where line 1 is bold but line 0 is not.
        // Select all → start is (0,0) which is NOT bold → make all bold.
        let c = TestContent::with_text("aaa\nbbb");

        {
            let mut internal = c.0.borrow_mut();
            internal
                .document
                .set_format_property(1, 0..3, |f| f.bold = true);
            internal.doc_version += 1;
        }

        select_all_and_toggle_bold(&c);

        let line0_bold = with_doc(&c, |doc| {
            !doc.spans(0).is_empty() && doc.spans(0).iter().all(|(_, f)| f.bold)
        });
        let line1_bold = with_doc(&c, |doc| {
            !doc.spans(1).is_empty() && doc.spans(1).iter().all(|(_, f)| f.bold)
        });
        assert!(line0_bold, "line 0 should be bold");
        assert!(line1_bold, "line 1 should stay bold");
    }
}
