//! Rich text editor content — wraps the editor and manages pending style
//! and undo/redo history. All edits flow through [`Content::perform`].

use crate::core::text::editor::Position;
use crate::core::text::rich_editor::{self, Editor as _, paragraph, span};
use markright_core::{History, Op, Paragraph, StyledLine as DocStyledLine, Theme};

use std::borrow::Cow;
use std::cell::RefCell;

use super::action::{self, Action, Edit, Format};
use super::cursor;
use super::list;
use super::operation;

pub use crate::core::text::editor::{Cursor, Line, LineEnding};
pub use markright_core::{StyleRun, StyledLine};

/// Returns the style at the first non-empty character in a selection.
///
/// Skips blank lines so the reported style reflects actual content.
fn style_at_selection<E: rich_editor::Editor>(
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
    /// Per-line paragraphs (name, style, overrides).
    /// Kept in sync with the editor's line count.
    pub(crate) paragraphs: Vec<Paragraph>,
    /// Pixels per indent level for list items.
    pub(crate) list_indent: f32,
    /// Theme mapping paragraph names to default styles.
    pub(crate) theme: Theme,
    /// Optional computed-source state (formula chips, mentions, etc.).
    #[cfg(feature = "computed_spans")]
    pub(crate) computed: Option<Computed>,
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
            paragraphs: vec![Paragraph::default()],
            list_indent: list::DEFAULT_LIST_INDENT,
            theme: Theme::default(),
            #[cfg(feature = "computed_spans")]
            computed: None,
        }))
    }

    /// Set the theme used for paragraph name → style mapping.
    pub fn with_theme(self, theme: Theme) -> Self {
        self.0.borrow_mut().theme = theme;
        self
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
                if line.paragraph.style != paragraph::Style::default() {
                    internal
                        .editor
                        .set_paragraph_style(i, &line.paragraph.style);
                }
                // Wire spacing to cosmic-text margins
                let sb = line.paragraph.style.space_before.unwrap_or(0.0);
                let sa = line.paragraph.style.spacing_after.unwrap_or(0.0);
                if sb != 0.0 || sa != 0.0 {
                    internal.editor.set_paragraph_spacing(i, sb, sa);
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

            // Set paragraphs vector
            internal.paragraphs = lines.iter().map(|l| l.paragraph.clone()).collect();

            // Sync margins for list items
            let margins: Vec<f32> = internal
                .paragraphs
                .iter()
                .map(|p| list::compute_margin(&p.style, internal.list_indent))
                .collect();
            for (i, margin) in margins.into_iter().enumerate() {
                internal.editor.set_margin_left(i, margin);
            }
        }
        content
    }

    /// Create a [`Content`] managed by a computed-source adapter.
    ///
    /// The adapter parses the source string into display content and
    /// computed spans. The Content rebuilds automatically when the
    /// source changes via [`apply_span_action`] or plain-text edits.
    ///
    /// [`apply_span_action`]: Self::apply_span_action
    #[cfg(feature = "computed_spans")]
    pub fn from_computed(source: &str, adapter: impl super::computed_spans::Source) -> Self {
        let result = adapter.parse(source);
        let content = Self::from_styled_lines(&result.lines);
        {
            let mut internal = content.0.borrow_mut();
            internal.computed = Some(Computed {
                source: source.to_string(),
                adapter: Box::new(adapter),
                spans: result.spans,
            });
        }
        content
    }

    /// Returns the computed spans, if this content was created via
    /// [`from_computed`].
    ///
    /// [`from_computed`]: Self::from_computed
    #[cfg(feature = "computed_spans")]
    pub fn computed_spans(&self) -> Vec<super::computed_spans::Span> {
        self.0
            .borrow()
            .computed
            .as_ref()
            .map(|c| c.spans.clone())
            .unwrap_or_default()
    }

    /// Apply a computed-span action (from popup interaction).
    ///
    /// - `Input`: updates the span's source_value and re-evaluates via
    ///   the adapter, rebuilding the display content.
    /// - `Confirm`: no-op (the value was already applied via Input).
    /// - `Dismiss`: reverts the span to its original value.
    /// - `Delete`: removes the span from the source.
    #[cfg(feature = "computed_spans")]
    pub fn apply_span_action(&self, action: super::computed_spans::Action) {
        use super::computed_spans::Action;

        let mut internal = self.0.borrow_mut();
        if internal.computed.is_none() {
            return;
        }

        match action {
            Action::Input { id, value } => {
                // Remember which boundary of the edited chip the main
                // editor cursor is pinned to, so we can re-pin after the
                // rebuild. Without this, popup edits that shrink the
                // chip's display (e.g. `{=1+3}` → `{=1}` shortening 3
                // chars back to 1) can leave the cursor past the new
                // chip.end — outside any span — which makes
                // `computed_popup()` fail to build the popup element
                // and dismisses the popup mid-edit.
                let anchor = {
                    let cursor = internal.editor.cursor();
                    let computed = internal.computed.as_ref().expect("checked above");
                    computed.spans.iter().find(|s| s.id == id).and_then(|span| {
                        if cursor.position.line != span.line {
                            None
                        } else if cursor.position.column == span.display_range.start {
                            Some(ChipAnchor::Start)
                        } else if cursor.position.column == span.display_range.end {
                            Some(ChipAnchor::End)
                        } else {
                            None
                        }
                    })
                };

                {
                    let computed = internal.computed.as_mut().expect("checked above");
                    if let Some(span) = computed.spans.iter_mut().find(|s| s.id == id) {
                        span.source_value = value;
                    }
                }
                Self::rebuild_from_computed(&mut internal);

                if let Some(anchor) = anchor {
                    let column_opt = internal
                        .computed
                        .as_ref()
                        .and_then(|c| c.spans.iter().find(|s| s.id == id))
                        .map(|span| match anchor {
                            ChipAnchor::Start => (span.line, span.display_range.start),
                            ChipAnchor::End => (span.line, span.display_range.end),
                        });
                    if let Some((line, column)) = column_opt {
                        use crate::core::text::rich_editor::Editor as _;
                        internal.editor.move_to(Cursor {
                            position: Position { line, column },
                            selection: None,
                        });
                    }
                }
            }
            Action::Confirm { .. } => {
                // Already applied via Input; nothing to do.
            }
            Action::Dismiss { id, original } => {
                let computed = internal.computed.as_mut().expect("checked above");
                if let Some(span) = computed.spans.iter_mut().find(|s| s.id == id) {
                    span.source_value = original;
                }
                Self::rebuild_from_computed(&mut internal);
            }
            Action::Delete { id } => {
                let computed = internal.computed.as_mut().expect("checked above");
                if let Some(span) = computed.spans.iter().find(|s| s.id == id) {
                    let range = span.source_range.clone();
                    computed.source.replace_range(range, "");
                }
                Self::rebuild_from_computed(&mut internal);
            }
        }
    }

    /// Rebuild the editor content from the current computed source.
    #[cfg(feature = "computed_spans")]
    fn rebuild_from_computed(internal: &mut Internal<R>) {
        let computed = internal
            .computed
            .as_mut()
            .expect("called only when computed is Some");

        // Reconstruct the source from spans.
        let mut new_source = String::new();
        let mut last_source_end = 0;
        for span in &computed.spans {
            // Append text between spans
            if span.source_range.start > last_source_end {
                new_source.push_str(&computed.source[last_source_end..span.source_range.start]);
            }
            new_source.push_str(&span.source_value);
            last_source_end = span.source_range.end;
        }
        // Append trailing text
        if last_source_end < computed.source.len() {
            new_source.push_str(&computed.source[last_source_end..]);
        }
        computed.source = new_source;

        // Re-parse with the adapter.
        let result = computed.adapter.parse(&computed.source);

        Self::apply_parse_result(internal, result);
    }

    /// Apply a computed-span parse result: rebuild the editor with the new
    /// display text, reapply paragraph and span styles, replace stored
    /// spans and paragraphs, and restore the cursor (snapping out of any
    /// newly-formed chip).
    #[cfg(feature = "computed_spans")]
    fn apply_parse_result(internal: &mut Internal<R>, result: super::computed_spans::Result) {
        // Preserve cursor across the editor rebuild. Without this, typing
        // in a popup resets the editor cursor to (0,0), which moves it out
        // of the active span and dismisses the popup on next render.
        let saved_cursor = internal.editor.cursor();

        // Rebuild the editor with new display content.
        let plain: String =
            result
                .lines
                .iter()
                .enumerate()
                .fold(String::new(), |mut acc, (i, line)| {
                    if i > 0 {
                        acc.push('\n');
                    }
                    acc.push_str(&line.text);
                    acc
                });
        internal.editor = R::RichEditor::with_text(&plain);

        // Clamp the saved cursor to the new buffer's bounds before
        // restoring. The display may have shrunk (e.g. typing `}` to
        // close `{=expr}` collapses N source chars into a shorter
        // display value). cosmic-text uses byte offsets and asserts
        // char-boundary on subsequent edits, so an out-of-bounds or
        // mid-codepoint position triggers a panic on the next keystroke.
        let line_count = internal.editor.line_count();
        let clamped_line = saved_cursor.position.line.min(line_count.saturating_sub(1));
        let line_len = internal
            .editor
            .line(clamped_line)
            .map(|l| l.text.len())
            .unwrap_or(0);
        let mut clamped_col = saved_cursor.position.column.min(line_len);
        if let Some(text) = internal.editor.line(clamped_line).map(|l| l.text) {
            while clamped_col > 0 && !text.is_char_boundary(clamped_col) {
                clamped_col -= 1;
            }
        }
        internal.editor.move_to(Cursor {
            position: Position {
                line: clamped_line,
                column: clamped_col,
            },
            selection: saved_cursor.selection,
        });

        // Apply span styles from the new lines.
        let default_style = span::Style::default();
        for (i, line) in result.lines.iter().enumerate() {
            if line.paragraph.style != paragraph::Style::default() {
                internal
                    .editor
                    .set_paragraph_style(i, &line.paragraph.style);
            }
            for run in &line.runs {
                if run.style != default_style {
                    internal
                        .editor
                        .set_span_style(i, run.range.clone(), &run.style);
                }
            }
        }

        // Update paragraphs.
        internal.paragraphs = result.lines.iter().map(|l| l.paragraph.clone()).collect();

        // Update spans.
        let new_spans = result.spans;

        // Snap cursor out of any newly-formed chip. If the cursor landed
        // strictly inside a span's display_range (e.g., typing `}` to close
        // a `{=expr}` pattern collapsed 6 source chars into a shorter
        // display value), move it to the end of that chip. Exact boundary
        // positions stay put — those are valid plain-text positions.
        let cursor = internal.editor.cursor();
        if let Some(span) = new_spans.iter().find(|s| {
            s.line == cursor.position.line
                && cursor.position.column > s.display_range.start
                && cursor.position.column < s.display_range.end
        }) {
            internal.editor.move_to(Cursor {
                position: Position {
                    line: span.line,
                    column: span.display_range.end,
                },
                selection: None,
            });
        }

        if let Some(computed) = internal.computed.as_mut() {
            computed.spans = new_spans;
        }
    }

    /// Returns the current source text, if this is a computed-source content.
    #[cfg(feature = "computed_spans")]
    pub fn source(&self) -> Option<String> {
        self.0.borrow().computed.as_ref().map(|c| c.source.clone())
    }

    /// Export all lines as styled lines for serialization.
    pub fn styled_lines(&self) -> Vec<DocStyledLine> {
        let internal = self.0.borrow();
        let count = internal.editor.line_count();
        (0..count)
            .map(|i| {
                let line = internal.editor.line(i);
                let len = line.as_ref().map(|l| l.text.len()).unwrap_or(0);
                let mut styled = markright_core::read_styled_line(&internal.editor, i, 0..len);
                styled.paragraph = internal.paragraph(i).clone();
                styled
            })
            .collect()
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

    /// Moves the cursor to the given line and column.
    pub fn move_to(&self, line: usize, column: usize) {
        use crate::core::text::rich_editor::Editor as _;
        let cursor = Cursor {
            position: Position { line, column },
            selection: None,
        };
        self.0.borrow_mut().editor.move_to(cursor);
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
            style_at_selection(&internal.editor, &editor_cursor.position, sel)
        } else {
            // No selection: bias-left
            let line = editor_cursor.position.line;
            let col = editor_cursor.position.column;
            internal.editor.span_style_at(line, col.saturating_sub(1))
        };
        internal.fill_from_defaults(&mut char_style);

        let line = editor_cursor.position.line;
        let para = internal.paragraph(line).clone();

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
                name: para.name,
                alignment: super::Alignment::from_iced(para.style.alignment),
                spacing_after: para.style.spacing_after.unwrap_or(0.0),
                line_height: para.style.line_height,
                style: para.style,
            },
            position: cursor::Position {
                line: editor_cursor.position.line,
                column: editor_cursor.position.column,
            },
        }
    }

    /// Returns per-line styled content for debugging/inspection.
    pub fn styled_line(&self, index: usize) -> Option<markright_core::StyledLine> {
        let internal = self.0.borrow();
        let line = internal.editor.line(index)?;
        let len = line.text.len();
        let mut styled = markright_core::read_styled_line(&internal.editor, index, 0..len);
        styled.paragraph = internal.paragraph(index).clone();
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

    /// Strip all per-span overrides of a given attribute across the entire
    /// document. Also clears the attribute from paragraph character defaults.
    ///
    /// Recorded as one undo group so the user can restore everything with Cmd+Z.
    pub fn strip_attr(&self, attr: markright_core::SpanAttr) {
        self.0.borrow_mut().strip_attr(attr);
    }

    /// Set the document's default color and strip all per-span and
    /// paragraph-default color overrides so everything renders in `color`.
    pub fn set_color(&self, color: crate::core::Color) {
        let mut internal = self.0.borrow_mut();
        internal.default_style.color = Some(color);
        internal.strip_attr(markright_core::SpanAttr::Color(None));
    }

    /// Set the document's default font and strip all per-span font overrides.
    pub fn set_font(&self, font: crate::core::Font) {
        let mut internal = self.0.borrow_mut();
        internal.default_style.font = Some(font);
        internal.strip_attr(markright_core::SpanAttr::Font(None));
    }

    /// Set the document's default font size and strip all per-span size overrides.
    pub fn set_font_size(&self, size: f32) {
        let mut internal = self.0.borrow_mut();
        internal.default_style.size = Some(size);
        internal.strip_attr(markright_core::SpanAttr::Size(None));
    }

    /// Set the document's default letter spacing and strip all per-span overrides.
    pub fn set_letter_spacing(&self, spacing: f32) {
        let mut internal = self.0.borrow_mut();
        internal.default_style.letter_spacing = Some(spacing);
        internal.strip_attr(markright_core::SpanAttr::LetterSpacing(None));
    }

    /// Set alignment on every paragraph. Recorded as one undo group.
    pub fn set_alignment(&self, alignment: super::Alignment) {
        let mut internal = self.0.borrow_mut();
        internal.set_alignment_all(alignment);
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
        let default_style = internal.default_style.clone();
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
            default_style,
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

/// Which side of the edited chip the cursor was pinned to before a
/// popup edit, so we can re-pin to the same side after the rebuild.
#[cfg(feature = "computed_spans")]
enum ChipAnchor {
    Start,
    End,
}

#[cfg(feature = "computed_spans")]
pub(crate) struct Computed {
    source: String,
    adapter: Box<dyn super::computed_spans::Source>,
    spans: Vec<super::computed_spans::Span>,
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
                #[cfg(feature = "computed_spans")]
                let had_selection = self.editor.cursor().selection.is_some();
                #[cfg(feature = "computed_spans")]
                let cursor_before = self.editor.cursor();
                let mut ops = self.drain_selection();
                self.sync_paragraphs(&ops);
                let op = operation::insert(&mut self.editor, c, style);
                ops.push(op);
                self.record_group(ops);
                #[cfg(feature = "computed_spans")]
                if !had_selection {
                    self.sync_computed_source(Some((
                        cursor_before.position.line,
                        cursor_before.position.column,
                        c.len_utf8() as isize,
                    )));
                }
            }
            Edit::Paste(ref text) => {
                let style = self.resolve_style();
                #[cfg(feature = "computed_spans")]
                let had_selection = self.editor.cursor().selection.is_some();
                #[cfg(feature = "computed_spans")]
                let cursor_before = self.editor.cursor();
                let mut ops = self.drain_selection();
                self.sync_paragraphs(&ops);
                let paste_ops = operation::paste(&mut self.editor, text.clone(), style);
                self.sync_paragraphs(&paste_ops);
                ops.extend(paste_ops);
                self.record_group(ops);
                self.pending_style = None;
                #[cfg(feature = "computed_spans")]
                if !had_selection && !text.contains('\n') {
                    self.sync_computed_source(Some((
                        cursor_before.position.line,
                        cursor_before.position.column,
                        text.len() as isize,
                    )));
                }
            }
            Edit::Enter { inherit } => {
                // Capture the style at the cursor so the new line inherits it.
                let style = self.resolve_style();
                let mut ops = self.drain_selection();
                self.sync_paragraphs(&ops);
                let op = operation::enter(&mut self.editor);
                if let Op::SplitLine { line, .. } = &op {
                    self.sync_paragraph_split(*line, inherit);
                }
                ops.push(op);
                self.record_group(ops);
                self.pending_style = Some(style);
                // Enter splits a line — multi-line shift is non-trivial.
                // Skip sync for now; chips remain at pre-Enter positions
                // until the next sync-friendly edit.
            }
            Edit::Backspace => {
                #[cfg(feature = "computed_spans")]
                let had_selection = self.editor.cursor().selection.is_some();
                #[cfg(feature = "computed_spans")]
                let cursor_before = self.editor.cursor();
                let ops = self.backspace_list_aware();
                self.sync_paragraphs(&ops);
                self.record_group(ops);
                self.pending_style = None;
                #[cfg(feature = "computed_spans")]
                if !had_selection && cursor_before.position.column > 0 {
                    self.sync_computed_source(Some((
                        cursor_before.position.line,
                        cursor_before.position.column,
                        -1,
                    )));
                }
            }
            Edit::Delete => {
                #[cfg(feature = "computed_spans")]
                let had_selection = self.editor.cursor().selection.is_some();
                #[cfg(feature = "computed_spans")]
                let cursor_before = self.editor.cursor();
                let ops = operation::delete(&mut self.editor);
                self.sync_paragraphs(&ops);
                self.record_group(ops);
                self.pending_style = None;
                #[cfg(feature = "computed_spans")]
                if !had_selection {
                    // Delete removes the char AT cursor; content from
                    // (cursor + 1) onward shifts left by 1.
                    self.sync_computed_source(Some((
                        cursor_before.position.line,
                        cursor_before.position.column + 1,
                        -1,
                    )));
                }
            }
            Edit::Format(ref fmt) => {
                let ops = operation::format(&mut self.editor, fmt, &self.paragraphs, &self.theme);
                if !ops.is_empty() {
                    self.sync_paragraphs(&ops);
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

    /// Reconstruct the computed source from the current display + spans and
    /// re-run the adapter. Called after plain-text edits so typing a
    /// pattern like `{=3*4}` in a plain region materializes a new chip.
    ///
    /// The reconstruction is: for each line, walk the chip spans in order
    /// by `display_range.start` and interleave the plain-text display
    /// between chips with each chip's `source_value`. Each chip's
    /// `display_value` appears verbatim in the post-edit display (chips
    /// are atomic at the binding layer), so its column range is still
    /// valid — we just swap the chip's display text for its source text.
    #[cfg(feature = "computed_spans")]
    fn sync_computed_source(&mut self, shift: Option<(usize, usize, isize)>) {
        if self.computed.is_none() {
            return;
        }

        // Apply per-edit shift to existing spans BEFORE walking the new
        // display: the post-edit display has chips at shifted positions,
        // and the recorded display_ranges are still in pre-edit coords.
        // Without this, walking the new display with stale ranges
        // mis-aligns chip boundaries — the reconstruction swallows newly
        // inserted plain-text characters or duplicates chip-display chars.
        if let Some((line, threshold, delta)) = shift
            && let Some(computed) = self.computed.as_mut()
        {
            for span in &mut computed.spans {
                if span.line == line && span.display_range.start >= threshold {
                    let start = (span.display_range.start as isize + delta).max(0) as usize;
                    let end = (span.display_range.end as isize + delta).max(0) as usize;
                    span.display_range = start..end;
                }
            }
        }

        let line_count = self.editor.line_count();

        // Collect chip spans grouped per line, sorted by display_range.start.
        // Clone out so we're free of the `self.computed` borrow while we
        // read editor lines below.
        let spans: Vec<super::computed_spans::Span> =
            self.computed.as_ref().expect("checked above").spans.clone();

        let mut new_source = String::new();
        for line_idx in 0..line_count {
            let display_text: String = self
                .editor
                .line(line_idx)
                .map(|l| l.text.into_owned())
                .unwrap_or_default();

            let mut line_spans: Vec<&super::computed_spans::Span> =
                spans.iter().filter(|s| s.line == line_idx).collect();
            line_spans.sort_by_key(|s| s.display_range.start);

            let mut last_end = 0usize;
            for span in line_spans {
                // The stored display_range may be stale (e.g. edits
                // deleted the chip's display characters) or shifted
                // past the post-edit display length. Clamp to actual
                // bounds and to char boundaries; skip spans whose
                // content has been deleted.
                let mut start = span.display_range.start.min(display_text.len());
                let mut end = span.display_range.end.min(display_text.len());
                while start > 0 && !display_text.is_char_boundary(start) {
                    start -= 1;
                }
                while end > start && !display_text.is_char_boundary(end) {
                    end -= 1;
                }
                if start < last_end || start >= display_text.len() {
                    continue;
                }
                if start > last_end {
                    new_source.push_str(&display_text[last_end..start]);
                }
                new_source.push_str(&span.source_value);
                last_end = end;
            }
            if last_end < display_text.len() {
                new_source.push_str(&display_text[last_end..]);
            }
            if line_idx + 1 < line_count {
                new_source.push('\n');
            }
        }

        // Run the adapter on the reconstructed source, then update the
        // stored source. The `computed` borrow is taken twice (once for
        // the adapter call, once for the source update) but neither
        // overlaps with the editor reads above.
        let result = self
            .computed
            .as_ref()
            .expect("checked above")
            .adapter
            .parse(&new_source);
        self.computed.as_mut().expect("checked above").source = new_source;

        Content::<R>::apply_parse_result(self, result);
    }

    /// Delete the current selection (if any) and return the ops.
    ///
    /// After this call the cursor is at the start of where the selection was,
    /// with no selection — ready for an insert or enter.
    fn drain_selection(&mut self) -> Vec<Op> {
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
            | Format::SetLineSpacing(_)
            | Format::SetName(_)
            | Format::SetSpaceBefore(_)
            | Format::SetSpaceAfter(_) => {}
        }
    }

    /// Strip all per-span overrides of `attr` and clear it from paragraph
    /// character defaults. Recorded as one undo group.
    fn strip_attr(&mut self, attr: markright_core::SpanAttr) {
        use markright_core::SpanAttr;

        // First: clear paragraph character defaults and rebuild ALL line
        // defaults. This must happen BEFORE reading/stripping spans, because
        // update() may have baked the old default_style value into line
        // defaults — spans inherit from those defaults, so we need them
        // clean before we read and clear span overrides.
        let count = self.editor.line_count();
        // Ensure paragraphs covers all lines.
        if self.paragraphs.len() < count {
            self.paragraphs.resize(count, Paragraph::default());
        }
        for line in 0..count {
            attr.clear_in(&mut self.paragraphs[line].style.style);
            let ps = self.paragraphs[line].style.clone();
            self.editor.set_paragraph_style(line, &ps);
        }

        // Now strip span-level overrides.
        let mut ops = Vec::new();
        let count = self.editor.line_count();

        for line in 0..count {
            let len = self.editor.line(line).map(|l| l.text.len()).unwrap_or(0);
            if len == 0 {
                continue;
            }
            let range = 0..len;
            let runs = markright_core::read_style_runs(&self.editor, line, range.clone());
            let old_values: Vec<(std::ops::Range<usize>, SpanAttr)> = runs
                .iter()
                .filter(|r| attr.is_set_in(&r.style))
                .map(|r| (r.range.clone(), SpanAttr::from_style(&r.style, &attr)))
                .collect();

            if old_values.is_empty() {
                continue;
            }

            let op = Op::SetSpanAttr {
                line,
                range,
                attr: attr.clone(),
                old_values,
            };
            operation::apply_op(&mut self.editor, &op, &self.paragraphs);
            ops.push(op);
        }

        self.record_group(ops);
    }

    /// Set alignment on every paragraph. Recorded as one undo group.
    fn set_alignment_all(&mut self, alignment: super::Alignment) {
        let mut ops = Vec::new();
        let count = self.editor.line_count();

        for line in 0..count {
            let current = markright_core::Alignment::from_iced(
                self.paragraphs.get(line).and_then(|p| p.style.alignment),
            );
            if current == alignment {
                continue;
            }
            let old = self.paragraphs.get(line).cloned().unwrap_or_default();
            let mut new = old.clone();
            new.style.alignment = Some(alignment.to_iced());
            new.set_override(markright_core::OverrideSet::ALIGNMENT);
            let op = Op::SetParagraph {
                line,
                paragraph: Box::new(new),
                old_paragraph: Box::new(old),
            };
            operation::apply_op(&mut self.editor, &op, &self.paragraphs);
            self.sync_paragraphs(std::slice::from_ref(&op));
            ops.push(op);
        }

        self.record_group(ops);
    }

    /// Sync paragraphs for a batch of ops that were just applied to the editor.
    fn sync_paragraphs(&mut self, ops: &[Op]) {
        for op in ops {
            match op {
                Op::SplitLine { line, .. } => self.sync_paragraph_split(*line, true),
                Op::MergeLine { line, .. } => self.sync_paragraph_merge(*line),
                Op::DeleteRange {
                    start_line,
                    end_line,
                    ..
                } => self.sync_paragraph_delete(*start_line, *end_line),
                Op::InsertRange {
                    start_line, lines, ..
                } => self.sync_paragraph_insert(*start_line, lines.len()),
                Op::SetParagraph {
                    line, paragraph, ..
                } => {
                    self.set_paragraph(*line, *paragraph.clone());
                }
                _ => {}
            }
        }
    }

    /// Set the paragraph for a given line, growing the vec if needed.
    ///
    /// Also syncs the editor's `margin_left` and `paragraph_style` for the line.
    fn set_paragraph(&mut self, line: usize, paragraph: Paragraph) {
        if line >= self.paragraphs.len() {
            self.paragraphs.resize(line + 1, Paragraph::default());
        }
        let margin = list::compute_margin(&paragraph.style, self.list_indent);
        self.editor.set_paragraph_style(line, &paragraph.style);
        self.editor.set_paragraph_spacing(
            line,
            paragraph.style.space_before.unwrap_or(0.0),
            paragraph.style.spacing_after.unwrap_or(0.0),
        );
        self.paragraphs[line] = paragraph;
        self.editor.set_margin_left(line, margin);
    }

    /// Get the paragraph for a given line, defaulting if out of bounds.
    pub(crate) fn paragraph(&self, line: usize) -> &Paragraph {
        static DEFAULT: std::sync::LazyLock<Paragraph> =
            std::sync::LazyLock::new(Paragraph::default);
        self.paragraphs.get(line).unwrap_or(&DEFAULT)
    }

    /// Sync paragraphs after a SplitLine: clone the paragraph at `line` and
    /// insert it after, then optionally demote headings to BODY.
    fn sync_paragraph_split(&mut self, line: usize, inherit: bool) {
        let parent = self.paragraph(line).clone();
        let mut new_para = parent.clone();

        // Demote to BODY if:
        // 1. Not inheriting (no Shift held)
        // 2. Paragraph is a heading
        // 3. Cursor was at end of line (the new line has empty text)
        if !inherit && new_para.name.heading_level().is_some() {
            let new_line_empty = self
                .editor
                .line(line + 1)
                .map(|l| l.text.is_empty())
                .unwrap_or(true);
            if new_line_empty {
                self.theme.apply(&mut new_para, markright_core::Name::BODY);
                new_para.clear_overrides();
            }
        }

        if line + 1 > self.paragraphs.len() {
            self.paragraphs.resize(line + 1, Paragraph::default());
        }
        let margin = list::compute_margin(&new_para.style, self.list_indent);
        self.paragraphs.insert(line + 1, new_para.clone());
        self.editor.set_margin_left(line + 1, margin);
        self.editor.set_paragraph_style(line + 1, &new_para.style);
        self.editor.set_paragraph_spacing(
            line + 1,
            new_para.style.space_before.unwrap_or(0.0),
            new_para.style.spacing_after.unwrap_or(0.0),
        );
    }

    /// Sync paragraphs after a MergeLine: remove the paragraph at `line + 1`
    /// and sync the surviving line's margin.
    fn sync_paragraph_merge(&mut self, line: usize) {
        if line + 1 < self.paragraphs.len() {
            self.paragraphs.remove(line + 1);
        }
        let sb = self.paragraph(line).style.space_before.unwrap_or(0.0);
        let sa = self.paragraph(line).style.spacing_after.unwrap_or(0.0);
        let margin = list::compute_margin(&self.paragraph(line).style, self.list_indent);
        self.editor.set_paragraph_spacing(line, sb, sa);
        self.editor.set_margin_left(line, margin);
    }

    /// Sync paragraphs after a DeleteRange: remove paragraphs for deleted lines.
    fn sync_paragraph_delete(&mut self, start_line: usize, end_line: usize) {
        if start_line < end_line {
            let remove_start = (start_line + 1).min(self.paragraphs.len());
            let remove_end = (end_line + 1).min(self.paragraphs.len());
            if remove_start < remove_end {
                self.paragraphs.drain(remove_start..remove_end);
            }
        }
    }

    /// Sync paragraphs after an InsertRange: insert default paragraphs for new lines.
    fn sync_paragraph_insert(&mut self, start_line: usize, line_count: usize) {
        if line_count > 1 {
            let insert_at = (start_line + 1).min(self.paragraphs.len());
            let new_paras = vec![Paragraph::default(); line_count - 1];
            self.paragraphs.splice(insert_at..insert_at, new_paras);
        }
    }

    /// Backspace that is list-aware: at column 0 with no selection, if the
    /// current line has a list style or indent level, dedent/remove list
    /// first instead of merging with the previous line.
    fn backspace_list_aware(&mut self) -> Vec<Op> {
        let cursor = self.editor.cursor();
        if cursor.selection.is_none() && cursor.position.column == 0 {
            let line = cursor.position.line;
            let para = self.paragraph(line).clone();
            if para.style.list.is_some() || para.style.level > 0 {
                let old = para.clone();
                let mut new = para;
                if new.style.list.is_some() {
                    if new.style.level > 1 {
                        // Nested list — promote one level.
                        new.style.level -= 1;
                        match &mut new.style.list {
                            Some(paragraph::List::Bullet(b)) => {
                                *b = list::bullet_for_level(new.style.level.saturating_sub(1));
                            }
                            Some(paragraph::List::Ordered(n)) => {
                                *n = list::number_for_level(new.style.level.saturating_sub(1));
                            }
                            _ => {}
                        }
                    } else {
                        // Base list level — remove list entirely.
                        new.style.list = None;
                        new.style.level = 0;
                    }
                } else {
                    // Plain indented text — dedent.
                    new.style.level -= 1;
                }
                return vec![Op::SetParagraph {
                    line,
                    paragraph: Box::new(new),
                    old_paragraph: Box::new(old),
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
                operation::apply_op(&mut self.editor, &captured, &self.paragraphs);
                self.sync_paragraphs(std::slice::from_ref(&captured));
                redo_ops.push(captured);
            }
        }

        self.history.push_redo(redo_ops);
        self.pending_style = None;
        // Undo/redo can apply arbitrary ops including SplitLine/MergeLine.
        // Without per-op shift info we can't reconstruct correctly with
        // stale spans; skip sync. Spans rebuild on the next sync-friendly
        // edit, and chip styling for the restored state is unchanged
        // (the editor text itself is correct via op replay).
    }

    fn perform_redo(&mut self) {
        let Some(group) = self.history.redo() else {
            return;
        };

        let mut undo_ops = Vec::new();
        for op in group.into_iter().rev() {
            for inv_op in op.inverse() {
                let captured = operation::capture_op_state(&self.editor, &inv_op);
                operation::apply_op(&mut self.editor, &captured, &self.paragraphs);
                self.sync_paragraphs(std::slice::from_ref(&captured));
                undo_ops.push(captured);
            }
        }

        self.history.push_undo(undo_ops);
        self.pending_style = None;
        // Undo/redo can apply arbitrary ops including SplitLine/MergeLine.
        // Without per-op shift info we can't reconstruct correctly with
        // stale spans; skip sync. Spans rebuild on the next sync-friendly
        // edit, and chip styling for the restored state is unchanged
        // (the editor text itself is correct via op replay).
    }
}
