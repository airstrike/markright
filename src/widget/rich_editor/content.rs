//! Rich text editor content — wraps the editor and manages pending style
//! and undo/redo history. All edits flow through [`Content::perform`].

use crate::core::text::editor::Position;
use crate::core::text::rich_editor::{self, Editor as _, paragraph, span};
use markright_document::{History, Op, StyledLine as DocStyledLine};

use std::borrow::Cow;
use std::cell::RefCell;

use super::action::{self, Action, Edit, Format};
use super::cursor;
use super::list;
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
) -> span::Style {
    let (start, end) = operation::ordered_positions(pos, sel);
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
    pending_style: Option<span::Style>,
    /// Undo/redo history of document operations.
    history: History,
    /// Document-level default style — fills in `None` span fields during
    /// `resolve_style` and `cursor_context`.
    pub(crate) default_style: span::Style,
    /// Per-line paragraph styles (alignment, line height, spacing, indent, level, list).
    /// Kept in sync with the editor's line count.
    pub(crate) paragraph_styles: Vec<paragraph::Style>,
    /// Pixels per indent level for list items.
    pub(crate) list_indent: f32,
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
            default_style: span::Style::default(),
            paragraph_styles: vec![paragraph::Style::default()],
            list_indent: list::DEFAULT_LIST_INDENT,
        }))
    }

    /// Parse `.mr` format markup into a [`Content`].
    ///
    /// This is the primary way to load a saved document.
    pub fn parse(input: &str) -> Result<Self, markright_document::format::ParseError> {
        let lines = markright_document::format::parse(input)?;
        Ok(Self::from_styled_lines(&lines))
    }

    /// Create a [`Content`] from styled lines.
    pub fn from_styled_lines(lines: &[DocStyledLine]) -> Self {
        // Join all line texts with \n
        let plain: String = lines
            .iter()
            .enumerate()
            .fold(String::new(), |mut acc, (i, line)| {
                if i > 0 {
                    acc.push('\n');
                }
                acc.push_str(&line.text);
                acc
            });

        let content = Self::with_text(&plain);
        {
            let mut internal = content.0.borrow_mut();

            let default_style = span::Style::default();

            for (i, line) in lines.iter().enumerate() {
                // Apply paragraph style first so character defaults take effect
                if line.paragraph_style != paragraph::Style::default() {
                    internal
                        .editor
                        .set_paragraph_style(i, &line.paragraph_style);
                }
                // Then apply span overrides — skip default-styled runs so they
                // inherit paragraph character defaults instead of overriding them
                for run in &line.runs {
                    if run.style != default_style {
                        internal
                            .editor
                            .set_span_style(i, run.range.clone(), &run.style);
                    }
                }
            }

            // Set paragraph styles vector
            internal.paragraph_styles = lines.iter().map(|l| l.paragraph_style.clone()).collect();

            // Sync margins for list items
            let margins: Vec<f32> = internal
                .paragraph_styles
                .iter()
                .map(|s| list::compute_margin(s, internal.list_indent))
                .collect();
            for (i, margin) in margins.into_iter().enumerate() {
                internal.editor.set_margin_left(i, margin);
            }
        }
        content
    }

    /// Export all lines as styled lines for serialization.
    pub fn styled_lines(&self) -> Vec<DocStyledLine> {
        let internal = self.0.borrow();
        let count = internal.editor.line_count();
        (0..count)
            .map(|i| {
                let line = internal.editor.line(i);
                let len = line.as_ref().map(|l| l.text.len()).unwrap_or(0);
                let mut styled = markright_document::read_styled_line(&internal.editor, i, 0..len);
                styled.paragraph_style = internal.paragraph_style(i).clone();
                styled
            })
            .collect()
    }

    /// Serialize the content to `.mr` format.
    pub fn serialize(&self) -> String {
        markright_document::format::serialize(&self.styled_lines())
    }

    /// Perform an [`Action`] on the content.
    pub fn perform(&self, action: impl Into<Action>) {
        let mut internal = self.0.borrow_mut();
        internal.perform(action.into());
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

        let mut char_style = if let Some(ref pending) = internal.pending_style {
            pending.clone()
        } else if let Some(ref sel) = editor_cursor.selection {
            // With a selection: read from the first non-empty content character
            style_at_selection_start(&internal.editor, &editor_cursor.position, sel)
        } else {
            // No selection: bias-left
            let line = editor_cursor.position.line;
            let col = editor_cursor.position.column;
            internal.editor.span_style_at(line, col.saturating_sub(1))
        };
        internal.fill_from_defaults(&mut char_style);

        let line = editor_cursor.position.line;
        let para_style = internal.paragraph_style(line).clone();

        cursor::Context {
            character: cursor::Character {
                bold: char_style.bold.unwrap_or(false),
                italic: char_style.italic.unwrap_or(false),
                underline: char_style.underline.unwrap_or(false),
                font: char_style.font,
                size: char_style.size,
                color: char_style.color,
                letter_spacing: char_style.letter_spacing,
            },
            paragraph: cursor::Paragraph {
                alignment: super::Alignment::from_iced(para_style.alignment),
                spacing_after: para_style.spacing_after.unwrap_or(0.0),
                line_height: para_style.line_height,
                style: para_style,
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
        let mut styled = markright_document::read_styled_line(&internal.editor, index, 0..len);
        styled.paragraph_style = internal.paragraph_style(index).clone();
        Some(styled)
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
        let style = internal.editor.span_style_at(c.position.line, col);
        let para = internal.editor.paragraph_style_at(c.position.line);
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

    /// Returns whether the document has been modified since the last save.
    pub fn is_dirty(&self) -> bool {
        self.0.borrow().history.is_dirty()
    }

    /// Mark the current state as saved (clean).
    pub fn mark_saved(&self) {
        self.0.borrow_mut().history.mark_saved();
    }

    /// Sets the list indent (pixels per level). Default is 20.
    pub fn set_list_indent(&self, indent: f32) {
        self.0.borrow_mut().list_indent = indent;
    }

    /// Returns the current list indent.
    pub fn list_indent(&self) -> f32 {
        self.0.borrow().list_indent
    }

    /// Trigger a layout pass so that geometry queries return up-to-date values.
    ///
    /// In the real app this happens during the widget's `layout()` phase.
    /// Call this in tests before querying visual positions.
    pub fn update_layout(&self, bounds: crate::core::Size)
    where
        <<R as rich_editor::Renderer>::RichEditor as rich_editor::Editor>::Font: Default,
    {
        use crate::core::text::rich_editor::Editor as _;
        use crate::core::text::{LineHeight, Wrapping};
        use crate::core::{Em, Pixels};

        let mut internal = self.0.borrow_mut();
        internal.editor.update(
            bounds,
            Default::default(),
            Pixels(16.0),
            LineHeight::default(),
            Em::ZERO,
            Vec::new(),
            Vec::new(),
            Wrapping::Word,
            None,
            Default::default(),
        );
    }

    /// Returns the visual line geometry for a paragraph line after layout.
    ///
    /// Returns `None` if the line doesn't exist or hasn't been laid out.
    pub fn line_geometry(
        &self,
        line: usize,
    ) -> Option<crate::core::text::rich_editor::paragraph::Geometry> {
        use crate::core::text::rich_editor::Editor as _;
        self.0.borrow().editor.line_geometry(line)
    }

    /// Returns the caret rectangle from the editor's selection state.
    ///
    /// Call `update_layout` first to ensure the layout is current.
    pub fn caret_rect(&self) -> Option<crate::core::Rectangle> {
        use crate::core::text::rich_editor::Editor as _;
        let internal = self.0.borrow();
        match internal.editor.selection() {
            crate::core::text::editor::Selection::Caret(rect) => Some(rect),
            _ => None,
        }
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
            Action::Deselect => {
                if self.editor.copy().is_some() {
                    self.editor.perform(crate::core::text::editor::Action::Move(
                        crate::core::text::editor::Motion::Right,
                    ));
                }
            }
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
                self.sync_paragraph_styles_for_ops(&ops);
                let op = operation::insert(&mut self.editor, c, style);
                ops.push(op);
                self.record_group(ops);
            }
            Edit::Paste(ref text) => {
                let style = self.resolve_style();
                let mut ops = self.delete_selection_if_any();
                self.sync_paragraph_styles_for_ops(&ops);
                let paste_ops = operation::paste(&mut self.editor, text.clone(), style);
                self.sync_paragraph_styles_for_ops(&paste_ops);
                ops.extend(paste_ops);
                self.record_group(ops);
                self.pending_style = None;
            }
            Edit::Enter => {
                // Capture the style at the cursor so the new line inherits it.
                let style = self.resolve_style();
                let mut ops = self.delete_selection_if_any();
                self.sync_paragraph_styles_for_ops(&ops);
                let op = operation::enter(&mut self.editor);
                self.sync_paragraph_styles_for_ops(std::slice::from_ref(&op));
                ops.push(op);
                self.record_group(ops);
                self.pending_style = Some(style);
            }
            Edit::Backspace => {
                let ops = self.backspace_with_list_aware();
                self.sync_paragraph_styles_for_ops(&ops);
                self.record_group(ops);
                self.pending_style = None;
            }
            Edit::Delete => {
                let ops = operation::delete(&mut self.editor);
                self.sync_paragraph_styles_for_ops(&ops);
                self.record_group(ops);
                self.pending_style = None;
            }
            Edit::Format(ref fmt) => {
                let ops = operation::format(&mut self.editor, fmt, &self.paragraph_styles);
                if !ops.is_empty() {
                    self.sync_paragraph_styles_for_ops(&ops);
                    self.record_group(ops);
                } else {
                    self.update_pending_style(fmt);
                    // Persist to the paragraph's span attrs so the style
                    // survives cursor movement. For non-empty lines
                    // set_span_style on 0..0 is a no-op; for empty
                    // paragraphs it writes to the line's default attrs.
                    if let Some(ref style) = self.pending_style {
                        let line = self.editor.cursor().position.line;
                        self.editor.set_span_style(line, 0..0, style);
                    }
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

    fn resolve_style(&self) -> span::Style {
        let mut style = self.pending_style.clone().unwrap_or_else(|| {
            let cursor = self.editor.cursor();
            self.editor.span_style_at(
                cursor.position.line,
                cursor.position.column.saturating_sub(1),
            )
        });
        self.fill_from_defaults(&mut style);
        style
    }

    /// Fill any `None` fields in `style` from `self.default_style`.
    fn fill_from_defaults(&self, style: &mut span::Style) {
        let d = &self.default_style;
        if style.bold.is_none() {
            style.bold = d.bold;
        }
        if style.italic.is_none() {
            style.italic = d.italic;
        }
        if style.underline.is_none() {
            style.underline = d.underline;
        }
        if style.strikethrough.is_none() {
            style.strikethrough = d.strikethrough;
        }
        if style.font.is_none() {
            style.font = d.font;
        }
        if style.size.is_none() {
            style.size = d.size;
        }
        if style.color.is_none() {
            style.color = d.color;
        }
        if style.letter_spacing.is_none() {
            style.letter_spacing = d.letter_spacing;
        }
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

    fn update_pending_style(&mut self, fmt: &Format) {
        let cursor = self.editor.cursor();
        let current = self.pending_style.get_or_insert_with(|| {
            self.editor.span_style_at(
                cursor.position.line,
                cursor.position.column.saturating_sub(1),
            )
        });
        match fmt {
            Format::ToggleBold => current.bold = Some(!current.bold.unwrap_or(false)),
            Format::ToggleItalic => current.italic = Some(!current.italic.unwrap_or(false)),
            Format::ToggleUnderline => {
                current.underline = Some(!current.underline.unwrap_or(false));
            }
            Format::SetFont(font) => current.font = Some(*font),
            Format::SetFontSize(size) => current.size = Some(*size),
            Format::SetColor(color) => current.color = *color,
            Format::SetLetterSpacing(ls) => current.letter_spacing = Some(*ls),
            Format::SetAlignment(_)
            | Format::SetList(_)
            | Format::IndentList
            | Format::DedentList
            | Format::SetLineHeight(_)
            | Format::SetLineSpacing(_) => {}
        }
    }

    /// Sync paragraph_styles for a batch of ops that were just applied to the editor.
    fn sync_paragraph_styles_for_ops(&mut self, ops: &[Op]) {
        for op in ops {
            match op {
                Op::SplitLine { line, .. } => self.sync_paragraph_split(*line),
                Op::MergeLine { line, .. } => self.sync_paragraph_merge(*line),
                Op::DeleteRange {
                    start_line,
                    end_line,
                    ..
                } => self.sync_paragraph_delete_range(*start_line, *end_line),
                Op::InsertRange {
                    start_line, lines, ..
                } => self.sync_paragraph_insert_range(*start_line, lines.len()),
                Op::SetParagraphStyle { line, style, .. } => {
                    self.set_paragraph_style(*line, style.clone());
                }
                Op::SetAlignment {
                    line, alignment, ..
                } => {
                    self.sync_paragraph_alignment(*line, Some(alignment.to_iced()));
                }
                Op::SetLineHeight {
                    line, line_height, ..
                } => {
                    self.sync_paragraph_line_height(*line, *line_height);
                }
                _ => {}
            }
        }
    }

    /// Set the paragraph style for a given line, growing the vec if needed.
    ///
    /// Also syncs the editor's `margin_left` for the line based on the style.
    fn set_paragraph_style(&mut self, line: usize, style: paragraph::Style) {
        if line >= self.paragraph_styles.len() {
            self.paragraph_styles
                .resize(line + 1, paragraph::Style::default());
        }
        let margin = list::compute_margin(&style, self.list_indent);
        self.paragraph_styles[line] = style;
        self.editor.set_margin_left(line, margin);
    }

    /// Get the paragraph style for a given line, defaulting if out of bounds.
    pub(crate) fn paragraph_style(&self, line: usize) -> &paragraph::Style {
        static DEFAULT: std::sync::LazyLock<paragraph::Style> =
            std::sync::LazyLock::new(paragraph::Style::default);
        self.paragraph_styles.get(line).unwrap_or(&DEFAULT)
    }

    /// Sync paragraph_styles after a SplitLine: clone the style at `line` and
    /// insert it after, then sync margins and paragraph style for the new line.
    fn sync_paragraph_split(&mut self, line: usize) {
        let style = self.paragraph_style(line).clone();
        if line + 1 > self.paragraph_styles.len() {
            self.paragraph_styles
                .resize(line + 1, paragraph::Style::default());
        }
        let margin = list::compute_margin(&style, self.list_indent);
        self.paragraph_styles.insert(line + 1, style.clone());
        self.editor.set_margin_left(line + 1, margin);
        self.editor.set_paragraph_style(line + 1, &style);
    }

    /// Sync paragraph_styles after a MergeLine: remove the style at `line + 1`
    /// and sync the surviving line's margin.
    fn sync_paragraph_merge(&mut self, line: usize) {
        if line + 1 < self.paragraph_styles.len() {
            self.paragraph_styles.remove(line + 1);
        }
        let margin = list::compute_margin(self.paragraph_style(line), self.list_indent);
        self.editor.set_margin_left(line, margin);
    }

    /// Sync paragraph_styles after a DeleteRange: remove styles for deleted lines.
    fn sync_paragraph_delete_range(&mut self, start_line: usize, end_line: usize) {
        if start_line < end_line {
            let remove_start = (start_line + 1).min(self.paragraph_styles.len());
            let remove_end = (end_line + 1).min(self.paragraph_styles.len());
            if remove_start < remove_end {
                self.paragraph_styles.drain(remove_start..remove_end);
            }
        }
    }

    /// Sync paragraph_styles after an InsertRange: insert default styles for new lines.
    fn sync_paragraph_insert_range(&mut self, start_line: usize, line_count: usize) {
        if line_count > 1 {
            let insert_at = (start_line + 1).min(self.paragraph_styles.len());
            let new_styles = vec![paragraph::Style::default(); line_count - 1];
            self.paragraph_styles
                .splice(insert_at..insert_at, new_styles);
        }
    }

    /// Update alignment in paragraph_styles for a given line.
    fn sync_paragraph_alignment(
        &mut self,
        line: usize,
        alignment: Option<iced_core::text::Alignment>,
    ) {
        if line >= self.paragraph_styles.len() {
            self.paragraph_styles
                .resize(line + 1, paragraph::Style::default());
        }
        self.paragraph_styles[line].alignment = alignment;
    }

    /// Update line_height in paragraph_styles for a given line.
    fn sync_paragraph_line_height(
        &mut self,
        line: usize,
        line_height: Option<iced_core::text::LineHeight>,
    ) {
        if line >= self.paragraph_styles.len() {
            self.paragraph_styles
                .resize(line + 1, paragraph::Style::default());
        }
        self.paragraph_styles[line].line_height = line_height;
    }

    /// Backspace that is list-aware: at column 0 with no selection, if the
    /// current line has a list style or indent level, dedent/remove list
    /// first instead of merging with the previous line.
    fn backspace_with_list_aware(&mut self) -> Vec<Op> {
        let cursor = self.editor.cursor();
        if cursor.selection.is_none() && cursor.position.column == 0 {
            let line = cursor.position.line;
            let style = self.paragraph_style(line).clone();
            if style.list.is_some() || style.level > 0 {
                let old_style = style.clone();
                let mut new_style = style;
                if new_style.list.is_some() {
                    if new_style.level > 1 {
                        // Nested list — promote one level.
                        new_style.level -= 1;
                        match &mut new_style.list {
                            Some(paragraph::List::Bullet(b)) => {
                                *b = list::bullet_for_level(new_style.level.saturating_sub(1));
                            }
                            Some(paragraph::List::Ordered(n)) => {
                                *n = list::number_for_level(new_style.level.saturating_sub(1));
                            }
                            _ => {}
                        }
                    } else {
                        // Base list level — remove list entirely.
                        new_style.list = None;
                        new_style.level = 0;
                    }
                } else {
                    // Plain indented text — dedent.
                    new_style.level -= 1;
                }
                return vec![Op::SetParagraphStyle {
                    line,
                    style: new_style,
                    old_style,
                }];
            }
        }
        operation::backspace(&mut self.editor)
    }

    fn perform_undo(&mut self) {
        let Some(group) = self.history.undo() else {
            return;
        };

        let mut redo_ops = Vec::new();
        for op in group.into_iter().rev() {
            for inv_op in op.inverse() {
                let captured = operation::capture_op_state(&self.editor, &inv_op);
                operation::apply_op(&mut self.editor, &captured, &self.paragraph_styles);
                self.sync_paragraph_styles_for_ops(std::slice::from_ref(&captured));
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
                operation::apply_op(&mut self.editor, &captured, &self.paragraph_styles);
                self.sync_paragraph_styles_for_ops(std::slice::from_ref(&captured));
                undo_ops.push(captured);
            }
        }

        self.history.push_undo(undo_ops);
        self.pending_style = None;
    }
}
