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
