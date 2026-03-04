// Document model wrapping cosmic-text Editor for WYSIWYG markdown editing.
//
// The active line (where the cursor sits) shows raw markdown text with
// syntax-highlighted attributes. All other lines show formatted display
// text with markdown markers hidden.

use cosmic_text::Edit as _;
use iced_graphics::text::cosmic_text;

use crate::parse;

/// Default font size used for display attribute scaling.
const DEFAULT_FONT_SIZE: f32 = 16.0;

/// The document model, backed by a cosmic-text `Editor`.
///
/// Maintains parallel `raw_lines` with the original markdown text.
/// The cosmic-text buffer holds either display text (for non-active
/// lines) or raw text (for the active line).
pub struct Content {
    editor: cosmic_text::Editor<'static>,
    raw_lines: Vec<String>,
    active_line: usize,
    parsed_cache: Vec<Option<parse::Line>>,
    code_block_state: Vec<bool>,
}

/// Action that can be performed on the document.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    /// Insert a character at the cursor.
    Insert(char),
    /// Delete forward (like the Delete key).
    Delete,
    /// Delete backward (like the Backspace key).
    Backspace,
    /// Split the current line at the cursor (Enter key).
    Enter,
    /// Move the cursor.
    Move(Motion),
    /// Set cursor to a specific position, clamped to valid range.
    Click { line: usize, offset: usize },
}

/// Cursor motion direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

impl Content {
    /// Create a new empty document with one empty line.
    pub fn new() -> Self {
        let buffer = cosmic_text::Buffer::new_empty(cosmic_text::Metrics {
            font_size: DEFAULT_FONT_SIZE,
            line_height: DEFAULT_FONT_SIZE * 1.2,
        });

        let mut editor = cosmic_text::Editor::new(buffer);

        // Initialize with one empty line
        {
            let mut fs = font_system_lock();
            editor.with_buffer_mut(|buffer| {
                buffer.set_text(
                    fs.raw(),
                    "",
                    &cosmic_text::Attrs::new(),
                    cosmic_text::Shaping::Advanced,
                    None,
                );
            });
        }

        Self {
            editor,
            raw_lines: vec![String::new()],
            active_line: 0,
            parsed_cache: vec![None],
            code_block_state: vec![false],
        }
    }

    /// Create a document initialized with the given markdown text.
    ///
    /// All lines start as display text except line 0 which is active
    /// and shows raw text.
    pub fn with_text(text: &str) -> Self {
        let raw_lines: Vec<String> = text.split('\n').map(String::from).collect();
        let line_count = raw_lines.len();

        // Compute code block state for all lines
        let code_block_state = compute_code_block_state(&raw_lines);

        // Parse all non-active lines for display
        let mut parsed_cache: Vec<Option<parse::Line>> = Vec::with_capacity(line_count);
        {
            let mut in_code_block = false;
            for (i, raw) in raw_lines.iter().enumerate() {
                if i == 0 {
                    // Active line: skip parse for display, but still track code block state
                    parse::parse_line(raw, &mut in_code_block);
                    parsed_cache.push(None);
                } else {
                    let parsed = parse::parse_line(raw, &mut in_code_block);
                    parsed_cache.push(Some(parsed));
                }
            }
        }

        // Build the cosmic-text buffer with display text for non-active lines
        // and raw text for the active line (line 0)
        let buffer = cosmic_text::Buffer::new_empty(cosmic_text::Metrics {
            font_size: DEFAULT_FONT_SIZE,
            line_height: DEFAULT_FONT_SIZE * 1.2,
        });
        let mut editor = cosmic_text::Editor::new(buffer);

        {
            let mut fs = font_system_lock();
            let font_system = fs.raw();

            // First, set the full text as raw to create the right number of lines
            editor.with_buffer_mut(|buffer| {
                buffer.set_text(
                    font_system,
                    text,
                    &cosmic_text::Attrs::new(),
                    cosmic_text::Shaping::Advanced,
                    None,
                );
            });

            // Now update each line with the correct text and attrs
            editor.with_buffer_mut(|buffer| {
                for (i, raw) in raw_lines.iter().enumerate() {
                    if i >= buffer.lines.len() {
                        break;
                    }
                    if i == 0 {
                        // Active line: show raw text with syntax attrs
                        let mut in_code = false;
                        let parsed = parse::parse_line(raw, &mut in_code);
                        let syntax_attrs =
                            parse::to_syntax_attrs(raw, &parsed.spans, &parsed.offset_map);
                        buffer.lines[i].set_text(raw, cosmic_text::LineEnding::None, syntax_attrs);
                    } else if let Some(Some(parsed)) = parsed_cache.get(i) {
                        // Non-active line: show display text with display attrs
                        let display_attrs =
                            parse::to_display_attrs(&parsed.spans, DEFAULT_FONT_SIZE);
                        buffer.lines[i].set_text(
                            &parsed.display,
                            cosmic_text::LineEnding::None,
                            display_attrs,
                        );
                    }
                }
            });
        }

        Self {
            editor,
            raw_lines,
            active_line: 0,
            parsed_cache,
            code_block_state,
        }
    }

    /// Perform an action on the document.
    pub fn perform(&mut self, action: Action) {
        let old_active = self.active_line;

        match action {
            Action::Insert(ch) => {
                if ch == '\n' {
                    self.perform_enter();
                } else {
                    self.perform_insert(ch);
                }
            }
            Action::Delete => self.perform_delete(),
            Action::Backspace => self.perform_backspace(),
            Action::Enter => self.perform_enter(),
            Action::Move(motion) => self.perform_move(motion),
            Action::Click { line, offset } => self.perform_click(line, offset),
        }

        // Check if cursor moved to a different line
        let new_line = self.editor_cursor_line();
        if new_line != old_active {
            self.swap_active_line(old_active, new_line);
        }
    }

    /// Return the current cursor position as (line, byte_offset).
    pub fn cursor(&self) -> (usize, usize) {
        let cursor = cosmic_text::Edit::cursor(&self.editor);
        (cursor.line, cursor.index)
    }

    /// Return the index of the currently active (editable) line.
    pub fn active_line(&self) -> usize {
        self.active_line
    }

    /// Return the total number of lines.
    pub fn line_count(&self) -> usize {
        self.raw_lines.len()
    }

    /// Reconstruct the full markdown text from raw lines.
    ///
    /// Updates the active line from the editor buffer first,
    /// then joins all raw lines with newlines.
    pub fn raw_text(&mut self) -> String {
        self.sync_active_line_to_raw();
        self.raw_lines.join("\n")
    }

    /// Return the raw line text at the given index.
    pub fn line(&self, index: usize) -> Option<&str> {
        self.raw_lines.get(index).map(|s| s.as_str())
    }

    /// Expose the underlying cosmic-text buffer for the editor widget.
    pub fn buffer(&self) -> &cosmic_text::Buffer {
        buffer_from_editor(&self.editor)
    }

    /// Expose the editor for advanced operations.
    pub fn editor(&self) -> &cosmic_text::Editor<'static> {
        &self.editor
    }

    /// Expose the editor mutably for shaping and layout.
    pub fn editor_mut(&mut self) -> &mut cosmic_text::Editor<'static> {
        &mut self.editor
    }

    // --- Private helpers ---

    /// Get the current line from the cosmic-text cursor.
    fn editor_cursor_line(&self) -> usize {
        cosmic_text::Edit::cursor(&self.editor).line
    }

    /// Read the current text of the active line from the editor buffer
    /// and save it back to raw_lines.
    fn sync_active_line_to_raw(&mut self) {
        let active = self.active_line;
        let buffer = buffer_from_editor(&self.editor);
        if let Some(buffer_line) = buffer.lines.get(active)
            && active < self.raw_lines.len()
        {
            self.raw_lines[active] = buffer_line.text().to_string();
        }
    }

    /// Swap the active line: old line gets display text, new line gets raw text.
    fn swap_active_line(&mut self, old: usize, new: usize) {
        // Step 1: Read current text from editor buffer for old active line -> save to raw_lines
        self.sync_active_line_to_raw();

        // Step 2: Recompute code block state (lines may have changed)
        self.code_block_state = compute_code_block_state(&self.raw_lines);

        // Step 3: Parse old line and set it to display text in the buffer
        // Step 4: Set new line to raw text with syntax attrs
        {
            let mut fs = font_system_lock();
            let _font_system = fs.raw();

            // Parse old line for display
            let old_in_code = self.code_block_state.get(old).copied().unwrap_or(false);
            if old < self.raw_lines.len() {
                let mut in_code = old_in_code;
                let parsed = parse::parse_line(&self.raw_lines[old], &mut in_code);
                let display_attrs = parse::to_display_attrs(&parsed.spans, DEFAULT_FONT_SIZE);

                self.editor.with_buffer_mut(|buffer| {
                    if let Some(line) = buffer.lines.get_mut(old) {
                        line.set_text(
                            &parsed.display,
                            cosmic_text::LineEnding::None,
                            display_attrs,
                        );
                    }
                });

                // Update parsed cache
                if old < self.parsed_cache.len() {
                    self.parsed_cache[old] = Some(parsed);
                }
            }

            // Set new active line to raw text with syntax attrs
            let new_in_code = self.code_block_state.get(new).copied().unwrap_or(false);
            if new < self.raw_lines.len() {
                let mut in_code = new_in_code;
                let raw = &self.raw_lines[new];
                let parsed = parse::parse_line(raw, &mut in_code);
                let syntax_attrs = parse::to_syntax_attrs(raw, &parsed.spans, &parsed.offset_map);

                self.editor.with_buffer_mut(|buffer| {
                    if let Some(line) = buffer.lines.get_mut(new) {
                        line.set_text(raw, cosmic_text::LineEnding::None, syntax_attrs);
                    }
                });

                // Clear parsed cache for active line
                if new < self.parsed_cache.len() {
                    self.parsed_cache[new] = None;
                }
            }
        }

        self.active_line = new;
    }

    // --- Action implementations ---

    fn perform_insert(&mut self, ch: char) {
        let mut fs = font_system_lock();
        self.editor
            .action(fs.raw(), cosmic_text::Action::Insert(ch));
        self.sync_raw_lines_structure();
    }

    fn perform_delete(&mut self) {
        let line_count_before = buffer_from_editor(&self.editor).lines.len();
        {
            let mut fs = font_system_lock();
            self.editor.action(fs.raw(), cosmic_text::Action::Delete);
        }
        let line_count_after = buffer_from_editor(&self.editor).lines.len();

        if line_count_after < line_count_before {
            self.sync_raw_lines_structure();
        } else {
            // Character deleted within same line - just sync text
            self.sync_active_line_to_raw();
        }
    }

    fn perform_backspace(&mut self) {
        let line_count_before = buffer_from_editor(&self.editor).lines.len();
        {
            let mut fs = font_system_lock();
            self.editor.action(fs.raw(), cosmic_text::Action::Backspace);
        }
        let line_count_after = buffer_from_editor(&self.editor).lines.len();

        if line_count_after < line_count_before {
            self.sync_raw_lines_structure();
        } else {
            // Character deleted within same line - just sync text
            self.sync_active_line_to_raw();
        }
    }

    fn perform_enter(&mut self) {
        {
            let mut fs = font_system_lock();
            self.editor.action(fs.raw(), cosmic_text::Action::Enter);
        }
        self.sync_raw_lines_structure();
    }

    fn perform_move(&mut self, motion: Motion) {
        let cosmic_motion = match motion {
            Motion::Left => cosmic_text::Motion::Left,
            Motion::Right => cosmic_text::Motion::Right,
            Motion::Up => cosmic_text::Motion::Up,
            Motion::Down => cosmic_text::Motion::Down,
            Motion::Home => cosmic_text::Motion::Home,
            Motion::End => cosmic_text::Motion::End,
        };

        let mut fs = font_system_lock();
        self.editor
            .action(fs.raw(), cosmic_text::Action::Motion(cosmic_motion));
    }

    fn perform_click(&mut self, line: usize, offset: usize) {
        let clamped_line = line.min(self.raw_lines.len().saturating_sub(1));

        // If clicking on a non-active line, we need to convert from raw offset
        // to the offset in whatever text is currently displayed in the buffer.
        // But the plan says Click takes raw offsets, so if the line is not active,
        // we need to convert raw->display offset for the click to land correctly.
        // Actually, let's first swap the active line, then set the cursor.

        let old_active = self.active_line;
        if clamped_line != old_active {
            self.swap_active_line(old_active, clamped_line);
        }

        // Now the target line shows raw text. Clamp offset to line length.
        let raw_len = self.raw_lines.get(clamped_line).map_or(0, |l| l.len());
        let clamped_offset = offset.min(raw_len);

        // Set cursor directly via cosmic-text
        self.editor
            .set_cursor(cosmic_text::Cursor::new(clamped_line, clamped_offset));
    }

    /// Synchronize raw_lines structure with the editor buffer after edits
    /// that may have changed line count (Enter, Delete, Backspace joining).
    fn sync_raw_lines_structure(&mut self) {
        let buffer = buffer_from_editor(&self.editor);
        let buffer_line_count = buffer.lines.len();
        let active = self.editor_cursor_line();

        let old_count = self.raw_lines.len();

        if buffer_line_count > old_count {
            // Lines were added (Enter was pressed) - split the active raw line
            let lines_added = buffer_line_count - old_count;

            // Read the new line texts from buffer around the active area
            let mut new_texts: Vec<String> = Vec::with_capacity(lines_added + 1);
            for i in 0..=lines_added {
                let idx = active - lines_added + i;
                if let Some(bl) = buffer.lines.get(idx) {
                    new_texts.push(bl.text().to_string());
                }
            }

            // Replace the old active line with the split lines
            let old_active_idx = active - lines_added;
            if old_active_idx < self.raw_lines.len() {
                self.raw_lines.remove(old_active_idx);
                for (i, text) in new_texts.into_iter().enumerate() {
                    self.raw_lines.insert(old_active_idx + i, text);
                }
            }
        } else if buffer_line_count < old_count {
            // Lines were removed (join via Delete/Backspace)
            let lines_removed = old_count - buffer_line_count;

            // Read the current active line text from buffer
            let active_text = buffer
                .lines
                .get(active)
                .map(|bl| bl.text().to_string())
                .unwrap_or_default();

            // The join happened at the active line - remove the extra lines
            // and update the active line text
            let remove_start = active;
            let remove_end = (active + lines_removed + 1).min(self.raw_lines.len());
            if remove_start < self.raw_lines.len() {
                // Remove all the lines that got joined
                for _ in 0..((remove_end - remove_start).min(self.raw_lines.len() - remove_start)) {
                    self.raw_lines.remove(remove_start);
                }
                self.raw_lines.insert(remove_start, active_text);
            }
        } else {
            // Same number of lines - just update the active line text
            let buffer = buffer_from_editor(&self.editor);
            if let Some(bl) = buffer.lines.get(active)
                && active < self.raw_lines.len()
            {
                self.raw_lines[active] = bl.text().to_string();
            }
        }

        // Keep caches in sync
        self.parsed_cache.resize(self.raw_lines.len(), None);
        self.code_block_state = compute_code_block_state(&self.raw_lines);
        self.active_line = active;
    }
}

impl Default for Content {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract a `&Buffer` from a cosmic-text `Editor` regardless of `BufferRef` variant.
fn buffer_from_editor<'a, 'b>(editor: &'a cosmic_text::Editor<'b>) -> &'a cosmic_text::Buffer
where
    'b: 'a,
{
    match editor.buffer_ref() {
        cosmic_text::BufferRef::Owned(buffer) => buffer,
        cosmic_text::BufferRef::Borrowed(buffer) => buffer,
        cosmic_text::BufferRef::Arc(buffer) => buffer,
    }
}

/// Lock the global font system (module-level convenience).
fn font_system_lock() -> std::sync::RwLockWriteGuard<'static, iced_graphics::text::FontSystem> {
    iced_graphics::text::font_system()
        .write()
        .expect("font system lock should not be poisoned")
}

/// Compute the code-block-state-before-each-line vector.
///
/// `code_block_state[i]` is `true` if line `i` starts inside a fenced
/// code block (i.e. an odd number of ``` fences appeared before it).
fn compute_code_block_state(raw_lines: &[String]) -> Vec<bool> {
    let mut state = Vec::with_capacity(raw_lines.len());
    let mut in_code = false;
    for raw in raw_lines {
        state.push(in_code);
        // Track code fences
        let trimmed = raw.trim();
        if trimmed == "```" || (trimmed.starts_with("```") && !trimmed[3..].contains('`')) {
            in_code = !in_code;
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Construction ---

    #[test]
    fn new_has_one_empty_line() {
        let c = Content::new();
        assert_eq!(c.line_count(), 1);
        assert_eq!(c.line(0), Some(""));
    }

    #[test]
    fn new_cursor_at_origin() {
        let c = Content::new();
        assert_eq!(c.cursor(), (0, 0));
    }

    #[test]
    fn with_text_splits_on_newlines() {
        let c = Content::with_text("hello\nworld\nfoo");
        assert_eq!(c.line_count(), 3);
        assert_eq!(c.line(0), Some("hello"));
        assert_eq!(c.line(1), Some("world"));
        assert_eq!(c.line(2), Some("foo"));
    }

    #[test]
    fn with_text_trailing_newline() {
        let c = Content::with_text("hello\n");
        assert_eq!(c.line_count(), 2);
        assert_eq!(c.line(0), Some("hello"));
        assert_eq!(c.line(1), Some(""));
    }

    #[test]
    fn with_text_empty_string() {
        let c = Content::with_text("");
        assert_eq!(c.line_count(), 1);
        assert_eq!(c.line(0), Some(""));
    }

    // --- Insert ---

    #[test]
    fn insert_char_advances_cursor() {
        let mut c = Content::new();
        c.perform(Action::Insert('a'));
        assert_eq!(c.cursor().0, 0);
        // After insert, cursor should be past the inserted char
        assert_eq!(c.line(0), Some("a"));
    }

    #[test]
    fn insert_multiple_chars() {
        let mut c = Content::new();
        c.perform(Action::Insert('h'));
        c.perform(Action::Insert('i'));
        assert_eq!(c.line(0), Some("hi"));
    }

    // --- Delete ---

    #[test]
    fn delete_at_middle_of_line() {
        let mut c = Content::with_text("abc");
        c.perform(Action::Click { line: 0, offset: 1 });
        c.perform(Action::Delete);
        assert_eq!(c.line(0), Some("ac"));
    }

    #[test]
    fn delete_at_end_of_line_joins_lines() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Click { line: 0, offset: 5 });
        c.perform(Action::Delete);
        assert_eq!(c.line_count(), 1);
        assert_eq!(c.line(0), Some("helloworld"));
    }

    // --- Backspace ---

    #[test]
    fn backspace_at_middle_of_line() {
        let mut c = Content::with_text("abc");
        c.perform(Action::Click { line: 0, offset: 2 });
        c.perform(Action::Backspace);
        assert_eq!(c.line(0), Some("ac"));
    }

    #[test]
    fn backspace_at_start_of_line_joins_with_previous() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Click { line: 1, offset: 0 });
        c.perform(Action::Backspace);
        assert_eq!(c.line_count(), 1);
        assert_eq!(c.line(0), Some("helloworld"));
    }

    // --- Enter ---

    #[test]
    fn enter_splits_line_at_cursor() {
        let mut c = Content::with_text("helloworld");
        c.perform(Action::Click { line: 0, offset: 5 });
        c.perform(Action::Enter);
        assert_eq!(c.line_count(), 2);
        assert_eq!(c.line(0), Some("hello"));
        assert_eq!(c.line(1), Some("world"));
    }

    // --- Cursor movement ---

    #[test]
    fn move_down_changes_line() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Move(Motion::Down));
        assert_eq!(c.cursor().0, 1);
    }

    #[test]
    fn move_up_changes_line() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Click { line: 1, offset: 0 });
        c.perform(Action::Move(Motion::Up));
        assert_eq!(c.cursor().0, 0);
    }

    // --- Line change triggers text swap ---

    #[test]
    fn click_on_different_line_updates_active_line() {
        let mut c = Content::with_text("hello\nworld");
        assert_eq!(c.active_line(), 0);
        c.perform(Action::Click { line: 1, offset: 0 });
        assert_eq!(c.active_line(), 1);
    }

    // --- raw_text round-trip ---

    #[test]
    fn raw_text_round_trips_single_line() {
        let mut c = Content::with_text("hello");
        assert_eq!(c.raw_text(), "hello");
    }

    #[test]
    fn raw_text_round_trips_multi_line() {
        let input = "hello\nworld\nfoo";
        let mut c = Content::with_text(input);
        assert_eq!(c.raw_text(), input);
    }

    #[test]
    fn raw_text_round_trips_markdown() {
        let input = "# Heading\n**bold** text\n`code`";
        let mut c = Content::with_text(input);
        assert_eq!(c.raw_text(), input);
    }

    #[test]
    fn raw_text_after_edits() {
        let mut c = Content::with_text("hello");
        c.perform(Action::Click { line: 0, offset: 5 });
        c.perform(Action::Enter);
        c.perform(Action::Insert('w'));
        c.perform(Action::Insert('o'));
        c.perform(Action::Insert('r'));
        c.perform(Action::Insert('l'));
        c.perform(Action::Insert('d'));
        assert_eq!(c.raw_text(), "hello\nworld");
    }

    // --- Code block state ---

    #[test]
    fn code_block_state_tracking() {
        let c = Content::with_text("normal\n```\ncode line\n```\nafter");
        assert!(!c.code_block_state[0]); // normal line
        assert!(!c.code_block_state[1]); // ``` fence (starts outside)
        assert!(c.code_block_state[2]); // code line (inside)
        assert!(c.code_block_state[3]); // ``` fence (inside, will toggle)
        assert!(!c.code_block_state[4]); // after (outside again)
    }

    // --- line() out of bounds ---

    #[test]
    fn line_out_of_bounds_returns_none() {
        let c = Content::with_text("hello");
        assert_eq!(c.line(1), None);
    }

    // --- Default trait ---

    #[test]
    fn default_is_same_as_new() {
        let c: Content = Content::default();
        assert_eq!(c.line_count(), 1);
        assert_eq!(c.line(0), Some(""));
        assert_eq!(c.cursor(), (0, 0));
    }
}
