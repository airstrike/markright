// Document model — lines + edit operations

/// The document model, holding lines of raw markdown text.
#[derive(Debug, Clone)]
pub struct Content {
    lines: Vec<String>,
    cursor: Cursor,
}

/// Cursor position in raw text coordinates (byte offsets).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub line: usize,
    pub offset: usize,
}

/// Action that can be performed on the document.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
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
    Click {
        line: usize,
        offset: usize,
    },
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
        Self {
            lines: vec![String::new()],
            cursor: Cursor { line: 0, offset: 0 },
        }
    }

    /// Parse text into lines, splitting on newlines.
    pub fn with_text(text: &str) -> Self {
        let lines: Vec<String> = text.split('\n').map(String::from).collect();
        Self {
            lines,
            cursor: Cursor { line: 0, offset: 0 },
        }
    }

    /// Perform an action on the document.
    pub fn perform(&mut self, action: Action) {
        match action {
            Action::Insert(ch) => {
                if ch == '\n' {
                    self.action_enter();
                } else {
                    self.action_insert(ch);
                }
            }
            Action::Delete => self.action_delete(),
            Action::Backspace => self.action_backspace(),
            Action::Enter => self.action_enter(),
            Action::Move(motion) => self.action_move(motion),
            Action::Click { line, offset } => self.action_click(line, offset),
        }
    }

    /// Return the current cursor position.
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Return the content of a line by index.
    pub fn line(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(|s| s.as_str())
    }

    /// Return the total number of lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Join all lines with `\n` to produce the full document text.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    // --- Private action implementations ---

    fn action_insert(&mut self, ch: char) {
        let line = &mut self.lines[self.cursor.line];
        line.insert(self.cursor.offset, ch);
        self.cursor.offset += ch.len_utf8();
    }

    fn action_delete(&mut self) {
        let line_idx = self.cursor.line;
        let offset = self.cursor.offset;
        let line_len = self.lines[line_idx].len();

        if offset < line_len {
            // Delete the character at cursor position.
            // String::remove handles multi-byte chars correctly.
            self.lines[line_idx].remove(offset);
        } else if line_idx + 1 < self.lines.len() {
            // At end of line — join with next line
            let next_line = self.lines.remove(line_idx + 1);
            self.lines[line_idx].push_str(&next_line);
        }
    }

    fn action_backspace(&mut self) {
        let line_idx = self.cursor.line;
        let offset = self.cursor.offset;

        if offset > 0 {
            // Find the previous character boundary
            let prev_char_start = self.lines[line_idx][..offset]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .expect("offset > 0 implies at least one char");
            self.lines[line_idx].remove(prev_char_start);
            self.cursor.offset = prev_char_start;
        } else if line_idx > 0 {
            // At start of line — join with previous line
            let current_line = self.lines.remove(line_idx);
            self.cursor.line -= 1;
            self.cursor.offset = self.lines[self.cursor.line].len();
            self.lines[self.cursor.line].push_str(&current_line);
        }
    }

    fn action_enter(&mut self) {
        let line_idx = self.cursor.line;
        let offset = self.cursor.offset;

        let remainder = self.lines[line_idx][offset..].to_string();
        self.lines[line_idx].truncate(offset);
        self.lines.insert(line_idx + 1, remainder);

        self.cursor.line += 1;
        self.cursor.offset = 0;
    }

    fn action_move(&mut self, motion: Motion) {
        match motion {
            Motion::Left => {
                if self.cursor.offset > 0 {
                    // Move back one character
                    let prev = self.lines[self.cursor.line][..self.cursor.offset]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .expect("offset > 0 implies at least one char");
                    self.cursor.offset = prev;
                } else if self.cursor.line > 0 {
                    // Move to end of previous line
                    self.cursor.line -= 1;
                    self.cursor.offset = self.lines[self.cursor.line].len();
                }
            }
            Motion::Right => {
                let line_len = self.lines[self.cursor.line].len();
                if self.cursor.offset < line_len {
                    // Move forward one character
                    let ch = self.lines[self.cursor.line][self.cursor.offset..]
                        .chars()
                        .next()
                        .expect("offset is within line bounds");
                    self.cursor.offset += ch.len_utf8();
                } else if self.cursor.line + 1 < self.lines.len() {
                    // Move to start of next line
                    self.cursor.line += 1;
                    self.cursor.offset = 0;
                }
            }
            Motion::Up => {
                if self.cursor.line > 0 {
                    self.cursor.line -= 1;
                    let line_len = self.lines[self.cursor.line].len();
                    self.cursor.offset = self.cursor.offset.min(line_len);
                }
            }
            Motion::Down => {
                if self.cursor.line + 1 < self.lines.len() {
                    self.cursor.line += 1;
                    let line_len = self.lines[self.cursor.line].len();
                    self.cursor.offset = self.cursor.offset.min(line_len);
                }
            }
            Motion::Home => {
                self.cursor.offset = 0;
            }
            Motion::End => {
                self.cursor.offset = self.lines[self.cursor.line].len();
            }
        }
    }

    fn action_click(&mut self, line: usize, offset: usize) {
        let clamped_line = line.min(self.lines.len() - 1);
        let clamped_offset = offset.min(self.lines[clamped_line].len());
        self.cursor.line = clamped_line;
        self.cursor.offset = clamped_offset;
    }
}

impl Default for Content {
    fn default() -> Self {
        Self::new()
    }
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
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 0 });
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
        assert_eq!(c.line(0), Some("a"));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 1 });
    }

    #[test]
    fn insert_multiple_chars() {
        let mut c = Content::new();
        c.perform(Action::Insert('h'));
        c.perform(Action::Insert('i'));
        assert_eq!(c.line(0), Some("hi"));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 2 });
    }

    #[test]
    fn insert_at_middle_of_line() {
        let mut c = Content::with_text("ac");
        c.perform(Action::Click { line: 0, offset: 1 });
        c.perform(Action::Insert('b'));
        assert_eq!(c.line(0), Some("abc"));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 2 });
    }

    #[test]
    fn insert_newline_acts_as_enter() {
        let mut c = Content::with_text("hello");
        c.perform(Action::Click { line: 0, offset: 5 });
        c.perform(Action::Insert('\n'));
        assert_eq!(c.line_count(), 2);
        assert_eq!(c.line(0), Some("hello"));
        assert_eq!(c.line(1), Some(""));
        assert_eq!(c.cursor(), Cursor { line: 1, offset: 0 });
    }

    #[test]
    fn insert_unicode_char() {
        let mut c = Content::new();
        c.perform(Action::Insert('\u{00e9}')); // e-acute (2 bytes)
        assert_eq!(c.line(0), Some("\u{00e9}"));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 2 });
    }

    // --- Delete ---

    #[test]
    fn delete_at_middle_of_line() {
        let mut c = Content::with_text("abc");
        c.perform(Action::Click { line: 0, offset: 1 });
        c.perform(Action::Delete);
        assert_eq!(c.line(0), Some("ac"));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 1 });
    }

    #[test]
    fn delete_at_end_of_line_joins_lines() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Click { line: 0, offset: 5 });
        c.perform(Action::Delete);
        assert_eq!(c.line_count(), 1);
        assert_eq!(c.line(0), Some("helloworld"));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 5 });
    }

    #[test]
    fn delete_at_end_of_last_line_is_noop() {
        let mut c = Content::with_text("hello");
        c.perform(Action::Click { line: 0, offset: 5 });
        c.perform(Action::Delete);
        assert_eq!(c.line(0), Some("hello"));
    }

    // --- Backspace ---

    #[test]
    fn backspace_at_middle_of_line() {
        let mut c = Content::with_text("abc");
        c.perform(Action::Click { line: 0, offset: 2 });
        c.perform(Action::Backspace);
        assert_eq!(c.line(0), Some("ac"));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 1 });
    }

    #[test]
    fn backspace_at_start_of_line_joins_with_previous() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Click { line: 1, offset: 0 });
        c.perform(Action::Backspace);
        assert_eq!(c.line_count(), 1);
        assert_eq!(c.line(0), Some("helloworld"));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 5 });
    }

    #[test]
    fn backspace_at_start_of_first_line_is_noop() {
        let mut c = Content::with_text("hello");
        c.perform(Action::Backspace);
        assert_eq!(c.line(0), Some("hello"));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 0 });
    }

    #[test]
    fn backspace_unicode_char() {
        let mut c = Content::with_text("caf\u{00e9}");
        c.perform(Action::Click { line: 0, offset: 5 }); // after the e-acute (2 bytes)
        c.perform(Action::Backspace);
        assert_eq!(c.line(0), Some("caf"));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 3 });
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
        assert_eq!(c.cursor(), Cursor { line: 1, offset: 0 });
    }

    #[test]
    fn enter_at_start_of_line() {
        let mut c = Content::with_text("hello");
        c.perform(Action::Enter);
        assert_eq!(c.line_count(), 2);
        assert_eq!(c.line(0), Some(""));
        assert_eq!(c.line(1), Some("hello"));
        assert_eq!(c.cursor(), Cursor { line: 1, offset: 0 });
    }

    #[test]
    fn enter_at_end_of_line() {
        let mut c = Content::with_text("hello");
        c.perform(Action::Click { line: 0, offset: 5 });
        c.perform(Action::Enter);
        assert_eq!(c.line_count(), 2);
        assert_eq!(c.line(0), Some("hello"));
        assert_eq!(c.line(1), Some(""));
        assert_eq!(c.cursor(), Cursor { line: 1, offset: 0 });
    }

    // --- Move Left ---

    #[test]
    fn move_left_within_line() {
        let mut c = Content::with_text("abc");
        c.perform(Action::Click { line: 0, offset: 2 });
        c.perform(Action::Move(Motion::Left));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 1 });
    }

    #[test]
    fn move_left_wraps_to_previous_line() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Click { line: 1, offset: 0 });
        c.perform(Action::Move(Motion::Left));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 5 });
    }

    #[test]
    fn move_left_at_document_start_is_noop() {
        let mut c = Content::with_text("hello");
        c.perform(Action::Move(Motion::Left));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 0 });
    }

    // --- Move Right ---

    #[test]
    fn move_right_within_line() {
        let mut c = Content::with_text("abc");
        c.perform(Action::Move(Motion::Right));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 1 });
    }

    #[test]
    fn move_right_wraps_to_next_line() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Click { line: 0, offset: 5 });
        c.perform(Action::Move(Motion::Right));
        assert_eq!(c.cursor(), Cursor { line: 1, offset: 0 });
    }

    #[test]
    fn move_right_at_document_end_is_noop() {
        let mut c = Content::with_text("hello");
        c.perform(Action::Click { line: 0, offset: 5 });
        c.perform(Action::Move(Motion::Right));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 5 });
    }

    // --- Move Up ---

    #[test]
    fn move_up_preserves_offset() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Click { line: 1, offset: 3 });
        c.perform(Action::Move(Motion::Up));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 3 });
    }

    #[test]
    fn move_up_clamps_offset_to_shorter_line() {
        let mut c = Content::with_text("hi\nhello");
        c.perform(Action::Click { line: 1, offset: 4 });
        c.perform(Action::Move(Motion::Up));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 2 }); // "hi" has len 2
    }

    #[test]
    fn move_up_at_first_line_is_noop() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Click { line: 0, offset: 3 });
        c.perform(Action::Move(Motion::Up));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 3 });
    }

    // --- Move Down ---

    #[test]
    fn move_down_preserves_offset() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Click { line: 0, offset: 3 });
        c.perform(Action::Move(Motion::Down));
        assert_eq!(c.cursor(), Cursor { line: 1, offset: 3 });
    }

    #[test]
    fn move_down_clamps_offset_to_shorter_line() {
        let mut c = Content::with_text("hello\nhi");
        c.perform(Action::Click { line: 0, offset: 4 });
        c.perform(Action::Move(Motion::Down));
        assert_eq!(c.cursor(), Cursor { line: 1, offset: 2 }); // "hi" has len 2
    }

    #[test]
    fn move_down_at_last_line_is_noop() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Click { line: 1, offset: 3 });
        c.perform(Action::Move(Motion::Down));
        assert_eq!(c.cursor(), Cursor { line: 1, offset: 3 });
    }

    // --- Home / End ---

    #[test]
    fn home_moves_to_start_of_line() {
        let mut c = Content::with_text("hello");
        c.perform(Action::Click { line: 0, offset: 3 });
        c.perform(Action::Move(Motion::Home));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 0 });
    }

    #[test]
    fn end_moves_to_end_of_line() {
        let mut c = Content::with_text("hello");
        c.perform(Action::Move(Motion::End));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 5 });
    }

    // --- Click ---

    #[test]
    fn click_sets_cursor() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Click { line: 1, offset: 3 });
        assert_eq!(c.cursor(), Cursor { line: 1, offset: 3 });
    }

    #[test]
    fn click_clamps_line_to_last() {
        let mut c = Content::with_text("hello\nworld");
        c.perform(Action::Click {
            line: 99,
            offset: 0,
        });
        assert_eq!(c.cursor(), Cursor { line: 1, offset: 0 });
    }

    #[test]
    fn click_clamps_offset_to_line_length() {
        let mut c = Content::with_text("hi");
        c.perform(Action::Click {
            line: 0,
            offset: 99,
        });
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 2 });
    }

    #[test]
    fn click_clamps_both_line_and_offset() {
        let mut c = Content::with_text("ab\ncd");
        c.perform(Action::Click {
            line: 99,
            offset: 99,
        });
        assert_eq!(c.cursor(), Cursor { line: 1, offset: 2 });
    }

    // --- text() round-trip ---

    #[test]
    fn text_round_trips_single_line() {
        let c = Content::with_text("hello");
        assert_eq!(c.text(), "hello");
    }

    #[test]
    fn text_round_trips_multi_line() {
        let input = "hello\nworld\nfoo";
        let c = Content::with_text(input);
        assert_eq!(c.text(), input);
    }

    #[test]
    fn text_after_edits() {
        let mut c = Content::with_text("hello");
        c.perform(Action::Click { line: 0, offset: 5 });
        c.perform(Action::Enter);
        c.perform(Action::Insert('w'));
        c.perform(Action::Insert('o'));
        c.perform(Action::Insert('r'));
        c.perform(Action::Insert('l'));
        c.perform(Action::Insert('d'));
        assert_eq!(c.text(), "hello\nworld");
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
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 0 });
    }

    // --- Unicode movement ---

    #[test]
    fn move_right_over_multibyte_char() {
        let mut c = Content::with_text("caf\u{00e9}!");
        c.perform(Action::Click { line: 0, offset: 3 }); // before e-acute
        c.perform(Action::Move(Motion::Right));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 5 }); // e-acute is 2 bytes
    }

    #[test]
    fn move_left_over_multibyte_char() {
        let mut c = Content::with_text("caf\u{00e9}!");
        c.perform(Action::Click { line: 0, offset: 5 }); // after e-acute
        c.perform(Action::Move(Motion::Left));
        assert_eq!(c.cursor(), Cursor { line: 0, offset: 3 }); // before e-acute
    }
}
