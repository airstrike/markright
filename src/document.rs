// Block-based document model using pulldown-cmark for markdown parsing.

use std::ops::Range;

/// The block-based document model.
///
/// Maintains an ordered list of content blocks parsed from markdown.
/// One block is "active" (showing raw markdown for editing), while
/// all others show formatted display text with markers hidden.
pub struct Document {
    blocks: Vec<Block>,
    active_block: usize,
    cursor: usize, // byte offset within active block's raw text
}

/// A content block in the document.
pub struct Block {
    pub kind: BlockKind,
    pub raw: String,
    pub display_cache: Option<DisplayCache>,
}

/// The kind of content block.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BlockKind {
    Paragraph,
    Heading(u8),               // level 1-6
    CodeBlock(Option<String>), // optional language tag
    ThematicBreak,             // ---
}

/// Cached display information for an inactive block.
///
/// Contains the display text (markers stripped) and styled spans
/// for rendering formatted text.
pub struct DisplayCache {
    pub display_text: String,
    pub spans: Vec<InlineSpan>,
}

/// A styled region within display text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineSpan {
    pub range: Range<usize>,
    pub style: SpanStyle,
}

/// Visual style for an inline span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpanStyle {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strikethrough: bool,
}

impl Document {
    /// Create a document from markdown text.
    pub fn from_markdown(text: &str) -> Self {
        let mut blocks = parse_blocks(text);

        // Build display caches for all blocks except the active one (index 0).
        for (i, block) in blocks.iter_mut().enumerate() {
            if i != 0 {
                block.display_cache = Some(parse_inline_spans(&block.raw, &block.kind));
            }
        }

        Document {
            blocks,
            active_block: 0,
            cursor: 0,
        }
    }

    /// Reconstruct full markdown from blocks.
    pub fn to_markdown(&self) -> String {
        let mut result = String::new();

        for (i, block) in self.blocks.iter().enumerate() {
            if i > 0 {
                result.push_str("\n\n");
            }
            result.push_str(&block.raw);
        }

        result
    }

    /// Set which block is active (for editing).
    /// Deactivates the old block (builds display cache),
    /// activates the new block (clears display cache).
    pub fn set_active_block(&mut self, index: usize) {
        if index >= self.blocks.len() || index == self.active_block {
            return;
        }

        // Build display cache for the old active block.
        let old = self.active_block;
        let cache = parse_inline_spans(&self.blocks[old].raw, &self.blocks[old].kind);
        self.blocks[old].display_cache = Some(cache);

        // Clear display cache for the new active block.
        self.blocks[index].display_cache = None;

        self.active_block = index;
        self.cursor = 0;
    }

    /// Get the current cursor position as (block_index, byte_offset).
    pub fn cursor(&self) -> (usize, usize) {
        (self.active_block, self.cursor)
    }

    /// Set cursor position within the active block.
    pub fn set_cursor(&mut self, offset: usize) {
        let raw_len = self.blocks[self.active_block].raw.len();
        self.cursor = offset.min(raw_len);
    }

    /// Get the number of blocks.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Get a reference to a block by index.
    pub fn block(&self, index: usize) -> Option<&Block> {
        self.blocks.get(index)
    }

    /// Get the active block index.
    pub fn active_block(&self) -> usize {
        self.active_block
    }

    /// Insert a character at the cursor position in the active block.
    pub fn insert(&mut self, ch: char) {
        let block = &mut self.blocks[self.active_block];
        // Ensure cursor is on a char boundary.
        let offset = self.cursor.min(block.raw.len());
        block.raw.insert(offset, ch);
        self.cursor = offset + ch.len_utf8();
    }

    /// Delete the character after the cursor (Delete key).
    pub fn delete(&mut self) {
        let block = &mut self.blocks[self.active_block];
        if self.cursor < block.raw.len() {
            // Find the next char boundary after cursor.
            let next = next_char_boundary(&block.raw, self.cursor);
            block.raw.drain(self.cursor..next);
        } else if self.active_block + 1 < self.blocks.len() {
            // Merge with the next block.
            self.merge_next_block();
        }
    }

    /// Delete the character before the cursor (Backspace key).
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let block = &mut self.blocks[self.active_block];
            let prev = prev_char_boundary(&block.raw, self.cursor);
            block.raw.drain(prev..self.cursor);
            self.cursor = prev;
        } else if self.active_block > 0 {
            // Merge with the previous block.
            self.merge_with_previous_block();
        }
    }

    /// Split the active block at the cursor (Enter key).
    /// Creates a new block after the current one.
    pub fn enter(&mut self) {
        let block = &self.blocks[self.active_block];
        let offset = self.cursor.min(block.raw.len());

        let after_text = block.raw[offset..].to_string();
        let before_text = block.raw[..offset].to_string();

        self.blocks[self.active_block].raw = before_text;

        // Build display cache for old active block since it's being deactivated.
        let old_kind = self.blocks[self.active_block].kind.clone();
        let old_raw = self.blocks[self.active_block].raw.clone();
        self.blocks[self.active_block].display_cache =
            Some(parse_inline_spans(&old_raw, &old_kind));

        // The new block is always a paragraph (Enter creates a new paragraph).
        let new_block = Block {
            kind: BlockKind::Paragraph,
            raw: after_text,
            display_cache: None,
        };

        let new_index = self.active_block + 1;
        self.blocks.insert(new_index, new_block);
        self.active_block = new_index;
        self.cursor = 0;
    }

    /// Move cursor left by one character within the active block.
    /// If at the start of a block, move to end of previous block.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            let raw = &self.blocks[self.active_block].raw;
            self.cursor = prev_char_boundary(raw, self.cursor);
        } else if self.active_block > 0 {
            let new_block = self.active_block - 1;
            let end_offset = self.blocks[new_block].raw.len();
            self.set_active_block(new_block);
            self.cursor = end_offset;
        }
    }

    /// Move cursor right by one character within the active block.
    /// If at the end of a block, move to start of next block.
    pub fn move_right(&mut self) {
        let raw_len = self.blocks[self.active_block].raw.len();
        if self.cursor < raw_len {
            let raw = &self.blocks[self.active_block].raw;
            self.cursor = next_char_boundary(raw, self.cursor);
        } else if self.active_block + 1 < self.blocks.len() {
            let new_block = self.active_block + 1;
            self.set_active_block(new_block);
            self.cursor = 0;
        }
    }

    /// Move cursor to the start of the active block.
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to the end of the active block.
    pub fn move_end(&mut self) {
        self.cursor = self.blocks[self.active_block].raw.len();
    }

    /// Move cursor up. For now, moves to the previous block (same offset clamped).
    pub fn move_up(&mut self) {
        if self.active_block > 0 {
            let new_block = self.active_block - 1;
            let offset = self.cursor.min(self.blocks[new_block].raw.len());
            self.set_active_block(new_block);
            self.cursor = offset;
        }
    }

    /// Move cursor down. For now, moves to the next block (same offset clamped).
    pub fn move_down(&mut self) {
        if self.active_block + 1 < self.blocks.len() {
            let new_block = self.active_block + 1;
            let offset = self.cursor.min(self.blocks[new_block].raw.len());
            self.set_active_block(new_block);
            self.cursor = offset;
        }
    }

    /// Merge the next block into the current active block.
    fn merge_next_block(&mut self) {
        let next_index = self.active_block + 1;
        if next_index >= self.blocks.len() {
            return;
        }
        let next_block = self.blocks.remove(next_index);
        self.blocks[self.active_block].raw.push_str(&next_block.raw);
    }

    /// Merge the current active block into the previous block.
    fn merge_with_previous_block(&mut self) {
        if self.active_block == 0 {
            return;
        }

        let current = self.blocks.remove(self.active_block);
        let prev_index = self.active_block - 1;

        // Cursor goes to the end of the previous block's text (the join point).
        let join_offset = self.blocks[prev_index].raw.len();
        self.blocks[prev_index].raw.push_str(&current.raw);

        // The previous block becomes active.
        self.blocks[prev_index].display_cache = None;
        self.active_block = prev_index;
        self.cursor = join_offset;
    }
}

/// Parse markdown text into blocks using pulldown-cmark.
fn parse_blocks(markdown: &str) -> Vec<Block> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};

    let parser = Parser::new(markdown).into_offset_iter();
    let mut blocks = Vec::new();

    // Track the start of the current top-level block.
    let mut block_start: Option<usize> = None;
    let mut current_kind = BlockKind::Paragraph;
    let mut depth: usize = 0;

    for (event, range) in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current_kind = BlockKind::Heading(level as u8);
                block_start = Some(range.start);
                depth += 1;
            }
            Event::Start(Tag::Paragraph) => {
                current_kind = BlockKind::Paragraph;
                block_start = Some(range.start);
                depth += 1;
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match &kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        if lang.is_empty() {
                            None
                        } else {
                            Some(lang.to_string())
                        }
                    }
                    pulldown_cmark::CodeBlockKind::Indented => None,
                };
                current_kind = BlockKind::CodeBlock(lang);
                block_start = Some(range.start);
                depth += 1;
            }
            // Handle nested inline tags (strong, emphasis, etc.) - just track depth.
            Event::Start(_) => {
                depth += 1;
            }
            Event::End(TagEnd::Heading(_))
            | Event::End(TagEnd::Paragraph)
            | Event::End(TagEnd::CodeBlock) => {
                depth = depth.saturating_sub(1);
                let start = block_start.unwrap_or(range.start);
                let raw = markdown[start..range.end].to_string();
                // Trim trailing newlines from the raw block text, as the
                // separator between blocks is handled by to_markdown().
                let raw = raw.trim_end_matches('\n').to_string();
                blocks.push(Block {
                    kind: current_kind.clone(),
                    raw,
                    display_cache: None,
                });
                block_start = None;
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
            }
            Event::Rule => {
                let raw = markdown[range.clone()].trim_end_matches('\n').to_string();
                blocks.push(Block {
                    kind: BlockKind::ThematicBreak,
                    raw,
                    display_cache: None,
                });
            }
            // All other events (Text, Code, SoftBreak, etc.) are consumed
            // as part of the block they're inside.
            _ => {}
        }
    }

    if blocks.is_empty() {
        blocks.push(Block {
            kind: BlockKind::Paragraph,
            raw: String::new(),
            display_cache: None,
        });
    }

    blocks
}

/// Parse inline formatting within a block's raw text to produce display text and spans.
fn parse_inline_spans(raw: &str, kind: &BlockKind) -> DisplayCache {
    match kind {
        BlockKind::ThematicBreak => DisplayCache {
            display_text: String::new(),
            spans: Vec::new(),
        },
        BlockKind::CodeBlock(_) => {
            let content = strip_code_fences(raw);
            let len = content.len();
            DisplayCache {
                display_text: content,
                spans: if len > 0 {
                    vec![InlineSpan {
                        range: 0..len,
                        style: SpanStyle {
                            code: true,
                            ..SpanStyle::default()
                        },
                    }]
                } else {
                    Vec::new()
                },
            }
        }
        BlockKind::Heading(_) => {
            let content = strip_heading_prefix(raw);
            parse_inline_content(content)
        }
        BlockKind::Paragraph => parse_inline_content(raw),
    }
}

/// Strip code block fence lines from raw text.
///
/// Input: "```rust\ncode here\nmore code\n```"
/// Output: "code here\nmore code"
fn strip_code_fences(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().collect();
    if lines.len() < 2 {
        return String::new();
    }

    // Check if first line starts with ``` and last line is ```
    let first = lines[0].trim();
    let last = lines[lines.len() - 1].trim();

    if first.starts_with("```") && last.starts_with("```") && lines.len() > 2 {
        lines[1..lines.len() - 1].join("\n")
    } else if first.starts_with("```") && last.starts_with("```") {
        // Only opening and closing fence, no content.
        String::new()
    } else {
        // Not a fenced block (possibly indented code block), return as-is.
        raw.to_string()
    }
}

/// Strip heading prefix from raw text.
///
/// Input: "## Hello" -> "Hello"
fn strip_heading_prefix(raw: &str) -> &str {
    let trimmed = raw.trim_start_matches('#');
    // Skip the space after the last '#'
    trimmed.strip_prefix(' ').unwrap_or(trimmed)
}

/// Parse inline markdown formatting using pulldown-cmark.
///
/// Returns display text with markers stripped and styled spans.
fn parse_inline_content(raw: &str) -> DisplayCache {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};

    let parser = Parser::new(raw);
    let mut display_text = String::new();
    let mut spans = Vec::new();
    let mut style_stack: Vec<SpanStyle> = Vec::new();
    let mut current_style = SpanStyle::default();

    for event in parser {
        match event {
            Event::Start(Tag::Strong) => {
                style_stack.push(current_style);
                current_style.bold = true;
            }
            Event::Start(Tag::Emphasis) => {
                style_stack.push(current_style);
                current_style.italic = true;
            }
            Event::Start(Tag::Strikethrough) => {
                style_stack.push(current_style);
                current_style.strikethrough = true;
            }
            Event::End(TagEnd::Strong)
            | Event::End(TagEnd::Emphasis)
            | Event::End(TagEnd::Strikethrough) => {
                if let Some(prev) = style_stack.pop() {
                    current_style = prev;
                }
            }
            Event::Text(text) => {
                let start = display_text.len();
                display_text.push_str(&text);
                let end = display_text.len();
                if current_style != SpanStyle::default() && end > start {
                    spans.push(InlineSpan {
                        range: start..end,
                        style: current_style,
                    });
                }
            }
            Event::Code(text) => {
                let start = display_text.len();
                display_text.push_str(&text);
                let end = display_text.len();
                spans.push(InlineSpan {
                    range: start..end,
                    style: SpanStyle {
                        code: true,
                        ..current_style
                    },
                });
            }
            Event::SoftBreak => {
                display_text.push(' ');
            }
            Event::HardBreak => {
                display_text.push('\n');
            }
            _ => {}
        }
    }

    let merged_spans = merge_adjacent_spans(spans);

    DisplayCache {
        display_text,
        spans: merged_spans,
    }
}

/// Merge adjacent spans that have the same style.
///
/// pulldown-cmark may split text across multiple events within the same tag,
/// so we merge spans with identical styles when their ranges are contiguous.
fn merge_adjacent_spans(spans: Vec<InlineSpan>) -> Vec<InlineSpan> {
    let mut merged: Vec<InlineSpan> = Vec::new();

    for span in spans {
        if let Some(last) = merged.last_mut()
            && last.style == span.style
            && last.range.end == span.range.start
        {
            last.range.end = span.range.end;
            continue;
        }
        merged.push(span);
    }

    merged
}

/// Find the next character boundary after the given byte offset.
fn next_char_boundary(s: &str, offset: usize) -> usize {
    let mut pos = offset + 1;
    while pos < s.len() && !s.is_char_boundary(pos) {
        pos += 1;
    }
    pos.min(s.len())
}

/// Find the previous character boundary before the given byte offset.
fn prev_char_boundary(s: &str, offset: usize) -> usize {
    if offset == 0 {
        return 0;
    }
    let mut pos = offset - 1;
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_blocks / from_markdown tests ---

    #[test]
    fn from_markdown_parses_paragraph() {
        let doc = Document::from_markdown("Hello world");
        assert_eq!(doc.block_count(), 1);
        assert_eq!(doc.block(0).unwrap().kind, BlockKind::Paragraph);
        assert_eq!(doc.block(0).unwrap().raw, "Hello world");
    }

    #[test]
    fn from_markdown_parses_heading() {
        let doc = Document::from_markdown("# Title");
        assert_eq!(doc.block_count(), 1);
        assert_eq!(doc.block(0).unwrap().kind, BlockKind::Heading(1));
        assert_eq!(doc.block(0).unwrap().raw, "# Title");
    }

    #[test]
    fn from_markdown_parses_multiple_heading_levels() {
        let md = "# H1\n\n## H2\n\n### H3\n\n#### H4\n\n##### H5\n\n###### H6";
        let doc = Document::from_markdown(md);
        assert_eq!(doc.block_count(), 6);
        assert_eq!(doc.block(0).unwrap().kind, BlockKind::Heading(1));
        assert_eq!(doc.block(1).unwrap().kind, BlockKind::Heading(2));
        assert_eq!(doc.block(2).unwrap().kind, BlockKind::Heading(3));
        assert_eq!(doc.block(3).unwrap().kind, BlockKind::Heading(4));
        assert_eq!(doc.block(4).unwrap().kind, BlockKind::Heading(5));
        assert_eq!(doc.block(5).unwrap().kind, BlockKind::Heading(6));
    }

    #[test]
    fn from_markdown_parses_code_block_with_language() {
        let md = "```rust\nfn main() {}\n```";
        let doc = Document::from_markdown(md);
        assert_eq!(doc.block_count(), 1);
        assert_eq!(
            doc.block(0).unwrap().kind,
            BlockKind::CodeBlock(Some("rust".to_string()))
        );
    }

    #[test]
    fn from_markdown_parses_code_block_without_language() {
        let md = "```\nsome code\n```";
        let doc = Document::from_markdown(md);
        assert_eq!(doc.block_count(), 1);
        assert_eq!(doc.block(0).unwrap().kind, BlockKind::CodeBlock(None));
    }

    #[test]
    fn from_markdown_parses_thematic_break() {
        let md = "---";
        let doc = Document::from_markdown(md);
        assert_eq!(doc.block_count(), 1);
        assert_eq!(doc.block(0).unwrap().kind, BlockKind::ThematicBreak);
    }

    #[test]
    fn from_markdown_parses_mixed_blocks() {
        let md = "# Title\n\nSome text\n\n---\n\n```rust\ncode\n```";
        let doc = Document::from_markdown(md);
        assert_eq!(doc.block_count(), 4);
        assert_eq!(doc.block(0).unwrap().kind, BlockKind::Heading(1));
        assert_eq!(doc.block(1).unwrap().kind, BlockKind::Paragraph);
        assert_eq!(doc.block(2).unwrap().kind, BlockKind::ThematicBreak);
        assert_eq!(
            doc.block(3).unwrap().kind,
            BlockKind::CodeBlock(Some("rust".to_string()))
        );
    }

    #[test]
    fn from_markdown_empty_input_creates_one_paragraph() {
        let doc = Document::from_markdown("");
        assert_eq!(doc.block_count(), 1);
        assert_eq!(doc.block(0).unwrap().kind, BlockKind::Paragraph);
        assert_eq!(doc.block(0).unwrap().raw, "");
    }

    #[test]
    fn from_markdown_sets_active_block_to_zero() {
        let doc = Document::from_markdown("# Title\n\nParagraph");
        assert_eq!(doc.active_block(), 0);
        // Active block has no display cache.
        assert!(doc.block(0).unwrap().display_cache.is_none());
        // Inactive block has display cache.
        assert!(doc.block(1).unwrap().display_cache.is_some());
    }

    // --- to_markdown tests ---

    #[test]
    fn to_markdown_round_trip_paragraph() {
        let md = "Hello world";
        let doc = Document::from_markdown(md);
        assert_eq!(doc.to_markdown(), md);
    }

    #[test]
    fn to_markdown_round_trip_heading_and_paragraph() {
        let md = "# Title\n\nSome text";
        let doc = Document::from_markdown(md);
        assert_eq!(doc.to_markdown(), md);
    }

    #[test]
    fn to_markdown_round_trip_with_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let doc = Document::from_markdown(md);
        assert_eq!(doc.to_markdown(), md);
    }

    #[test]
    fn to_markdown_round_trip_mixed() {
        let md = "# Title\n\nSome text\n\n---\n\n```rust\ncode\n```";
        let doc = Document::from_markdown(md);
        assert_eq!(doc.to_markdown(), md);
    }

    // --- set_active_block tests ---

    #[test]
    fn set_active_block_swaps_display_cache() {
        let md = "# Title\n\nParagraph";
        let mut doc = Document::from_markdown(md);

        // Initially block 0 is active (no cache), block 1 has cache.
        assert!(doc.block(0).unwrap().display_cache.is_none());
        assert!(doc.block(1).unwrap().display_cache.is_some());

        doc.set_active_block(1);

        // Now block 0 has cache, block 1 is active (no cache).
        assert!(doc.block(0).unwrap().display_cache.is_some());
        assert!(doc.block(1).unwrap().display_cache.is_none());
        assert_eq!(doc.active_block(), 1);
    }

    #[test]
    fn set_active_block_out_of_bounds_is_noop() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_active_block(99);
        assert_eq!(doc.active_block(), 0);
    }

    #[test]
    fn set_active_block_same_index_is_noop() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_active_block(0);
        assert_eq!(doc.active_block(), 0);
        assert!(doc.block(0).unwrap().display_cache.is_none());
    }

    // --- cursor tests ---

    #[test]
    fn cursor_starts_at_zero() {
        let doc = Document::from_markdown("Hello");
        assert_eq!(doc.cursor(), (0, 0));
    }

    #[test]
    fn set_cursor_clamps_to_length() {
        let mut doc = Document::from_markdown("Hi");
        doc.set_cursor(100);
        assert_eq!(doc.cursor(), (0, 2));
    }

    // --- move_left tests ---

    #[test]
    fn move_left_within_block() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(3);
        doc.move_left();
        assert_eq!(doc.cursor(), (0, 2));
    }

    #[test]
    fn move_left_at_start_of_block_moves_to_previous() {
        let mut doc = Document::from_markdown("Hello\n\nWorld");
        doc.set_active_block(1);
        doc.set_cursor(0);
        doc.move_left();
        assert_eq!(doc.active_block(), 0);
        assert_eq!(doc.cursor(), (0, 5)); // end of "Hello"
    }

    #[test]
    fn move_left_at_start_of_first_block_is_noop() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(0);
        doc.move_left();
        assert_eq!(doc.cursor(), (0, 0));
    }

    #[test]
    fn move_left_with_multibyte_char() {
        let mut doc = Document::from_markdown("a\u{00e9}b"); // a + e-acute(2 bytes) + b
        doc.set_cursor(4); // after 'b'
        doc.move_left();
        assert_eq!(doc.cursor(), (0, 3)); // after e-acute
        doc.move_left();
        assert_eq!(doc.cursor(), (0, 1)); // after 'a'
    }

    // --- move_right tests ---

    #[test]
    fn move_right_within_block() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(0);
        doc.move_right();
        assert_eq!(doc.cursor(), (0, 1));
    }

    #[test]
    fn move_right_at_end_of_block_moves_to_next() {
        let mut doc = Document::from_markdown("Hello\n\nWorld");
        doc.set_cursor(5); // end of "Hello"
        doc.move_right();
        assert_eq!(doc.active_block(), 1);
        assert_eq!(doc.cursor(), (1, 0)); // start of "World"
    }

    #[test]
    fn move_right_at_end_of_last_block_is_noop() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(5);
        doc.move_right();
        assert_eq!(doc.cursor(), (0, 5));
    }

    #[test]
    fn move_right_with_multibyte_char() {
        let mut doc = Document::from_markdown("a\u{00e9}b");
        doc.set_cursor(0);
        doc.move_right();
        assert_eq!(doc.cursor(), (0, 1)); // after 'a'
        doc.move_right();
        assert_eq!(doc.cursor(), (0, 3)); // after e-acute (2 bytes)
    }

    // --- move_home tests ---

    #[test]
    fn move_home_goes_to_start() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(3);
        doc.move_home();
        assert_eq!(doc.cursor(), (0, 0));
    }

    #[test]
    fn move_home_at_start_is_noop() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(0);
        doc.move_home();
        assert_eq!(doc.cursor(), (0, 0));
    }

    // --- move_end tests ---

    #[test]
    fn move_end_goes_to_end() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(0);
        doc.move_end();
        assert_eq!(doc.cursor(), (0, 5));
    }

    #[test]
    fn move_end_at_end_is_noop() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(5);
        doc.move_end();
        assert_eq!(doc.cursor(), (0, 5));
    }

    // --- move_up tests ---

    #[test]
    fn move_up_to_previous_block() {
        let mut doc = Document::from_markdown("Hello\n\nWorld");
        doc.set_active_block(1);
        doc.set_cursor(3);
        doc.move_up();
        assert_eq!(doc.active_block(), 0);
        assert_eq!(doc.cursor(), (0, 3)); // same offset, within "Hello"
    }

    #[test]
    fn move_up_clamps_offset() {
        let mut doc = Document::from_markdown("Hi\n\nLonger text");
        doc.set_active_block(1);
        doc.set_cursor(10);
        doc.move_up();
        assert_eq!(doc.active_block(), 0);
        assert_eq!(doc.cursor(), (0, 2)); // clamped to len of "Hi"
    }

    #[test]
    fn move_up_at_first_block_is_noop() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(3);
        doc.move_up();
        assert_eq!(doc.active_block(), 0);
        assert_eq!(doc.cursor(), (0, 3));
    }

    // --- move_down tests ---

    #[test]
    fn move_down_to_next_block() {
        let mut doc = Document::from_markdown("Hello\n\nWorld");
        doc.set_cursor(3);
        doc.move_down();
        assert_eq!(doc.active_block(), 1);
        assert_eq!(doc.cursor(), (1, 3)); // same offset, within "World"
    }

    #[test]
    fn move_down_clamps_offset() {
        let mut doc = Document::from_markdown("Longer text\n\nHi");
        doc.set_cursor(10);
        doc.move_down();
        assert_eq!(doc.active_block(), 1);
        assert_eq!(doc.cursor(), (1, 2)); // clamped to len of "Hi"
    }

    #[test]
    fn move_down_at_last_block_is_noop() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(3);
        doc.move_down();
        assert_eq!(doc.active_block(), 0);
        assert_eq!(doc.cursor(), (0, 3));
    }

    // --- insert tests ---

    #[test]
    fn insert_character_at_start() {
        let mut doc = Document::from_markdown("ello");
        doc.set_cursor(0);
        doc.insert('H');
        assert_eq!(doc.block(0).unwrap().raw, "Hello");
        assert_eq!(doc.cursor(), (0, 1));
    }

    #[test]
    fn insert_character_at_end() {
        let mut doc = Document::from_markdown("Hell");
        doc.set_cursor(4);
        doc.insert('o');
        assert_eq!(doc.block(0).unwrap().raw, "Hello");
        assert_eq!(doc.cursor(), (0, 5));
    }

    #[test]
    fn insert_multibyte_character() {
        let mut doc = Document::from_markdown("ab");
        doc.set_cursor(1);
        doc.insert('\u{00e9}'); // e-acute, 2 bytes
        assert_eq!(doc.block(0).unwrap().raw, "a\u{00e9}b");
        assert_eq!(doc.cursor(), (0, 3)); // 1 + 2 = 3
    }

    // --- delete tests ---

    #[test]
    fn delete_character_at_start() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(0);
        doc.delete();
        assert_eq!(doc.block(0).unwrap().raw, "ello");
        assert_eq!(doc.cursor(), (0, 0));
    }

    #[test]
    fn delete_at_end_of_block_merges_with_next() {
        let mut doc = Document::from_markdown("Hello\n\nWorld");
        doc.set_cursor(5); // end of "Hello"
        doc.delete();
        assert_eq!(doc.block_count(), 1);
        assert_eq!(doc.block(0).unwrap().raw, "HelloWorld");
    }

    #[test]
    fn delete_at_end_of_last_block_is_noop() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(5);
        doc.delete();
        assert_eq!(doc.block(0).unwrap().raw, "Hello");
    }

    // --- backspace tests ---

    #[test]
    fn backspace_deletes_previous_character() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(5);
        doc.backspace();
        assert_eq!(doc.block(0).unwrap().raw, "Hell");
        assert_eq!(doc.cursor(), (0, 4));
    }

    #[test]
    fn backspace_at_start_of_block_merges_with_previous() {
        let mut doc = Document::from_markdown("Hello\n\nWorld");
        doc.set_active_block(1);
        doc.set_cursor(0);
        doc.backspace();
        assert_eq!(doc.block_count(), 1);
        assert_eq!(doc.block(0).unwrap().raw, "HelloWorld");
        assert_eq!(doc.active_block(), 0);
        assert_eq!(doc.cursor(), (0, 5)); // cursor at join point
    }

    #[test]
    fn backspace_at_start_of_first_block_is_noop() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(0);
        doc.backspace();
        assert_eq!(doc.block(0).unwrap().raw, "Hello");
        assert_eq!(doc.cursor(), (0, 0));
    }

    // --- enter tests ---

    #[test]
    fn enter_splits_block_at_cursor() {
        let mut doc = Document::from_markdown("HelloWorld");
        doc.set_cursor(5);
        doc.enter();
        assert_eq!(doc.block_count(), 2);
        assert_eq!(doc.block(0).unwrap().raw, "Hello");
        assert_eq!(doc.block(1).unwrap().raw, "World");
        assert_eq!(doc.active_block(), 1);
        assert_eq!(doc.cursor(), (1, 0));
    }

    #[test]
    fn enter_at_start_creates_empty_block_before() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(0);
        doc.enter();
        assert_eq!(doc.block_count(), 2);
        assert_eq!(doc.block(0).unwrap().raw, "");
        assert_eq!(doc.block(1).unwrap().raw, "Hello");
        assert_eq!(doc.active_block(), 1);
    }

    #[test]
    fn enter_at_end_creates_empty_block_after() {
        let mut doc = Document::from_markdown("Hello");
        doc.set_cursor(5);
        doc.enter();
        assert_eq!(doc.block_count(), 2);
        assert_eq!(doc.block(0).unwrap().raw, "Hello");
        assert_eq!(doc.block(1).unwrap().raw, "");
        assert_eq!(doc.active_block(), 1);
    }

    #[test]
    fn enter_new_block_is_paragraph() {
        let mut doc = Document::from_markdown("# Title");
        doc.set_cursor(7);
        doc.enter();
        assert_eq!(doc.block(0).unwrap().kind, BlockKind::Heading(1));
        assert_eq!(doc.block(1).unwrap().kind, BlockKind::Paragraph);
    }

    // --- strip_code_fences tests ---

    #[test]
    fn strip_code_fences_basic() {
        assert_eq!(strip_code_fences("```\ncode\n```"), "code");
    }

    #[test]
    fn strip_code_fences_with_language() {
        assert_eq!(
            strip_code_fences("```rust\nfn main() {}\n```"),
            "fn main() {}"
        );
    }

    #[test]
    fn strip_code_fences_multiline() {
        assert_eq!(
            strip_code_fences("```\nline1\nline2\nline3\n```"),
            "line1\nline2\nline3"
        );
    }

    #[test]
    fn strip_code_fences_empty_content() {
        assert_eq!(strip_code_fences("```\n```"), "");
    }

    // --- strip_heading_prefix tests ---

    #[test]
    fn strip_heading_prefix_h1() {
        assert_eq!(strip_heading_prefix("# Hello"), "Hello");
    }

    #[test]
    fn strip_heading_prefix_h3() {
        assert_eq!(strip_heading_prefix("### Hello"), "Hello");
    }

    #[test]
    fn strip_heading_prefix_no_space() {
        // Edge case: heading without space after #
        assert_eq!(strip_heading_prefix("#Hello"), "Hello");
    }

    // --- parse_inline_content tests ---

    #[test]
    fn inline_plain_text() {
        let cache = parse_inline_content("Hello world");
        assert_eq!(cache.display_text, "Hello world");
        assert!(cache.spans.is_empty());
    }

    #[test]
    fn inline_bold_text() {
        let cache = parse_inline_content("Hello **bold** world");
        assert_eq!(cache.display_text, "Hello bold world");
        assert_eq!(cache.spans.len(), 1);
        assert_eq!(cache.spans[0].range, 6..10);
        assert!(cache.spans[0].style.bold);
        assert!(!cache.spans[0].style.italic);
    }

    #[test]
    fn inline_italic_text() {
        let cache = parse_inline_content("Hello *italic* world");
        assert_eq!(cache.display_text, "Hello italic world");
        assert_eq!(cache.spans.len(), 1);
        assert_eq!(cache.spans[0].range, 6..12);
        assert!(cache.spans[0].style.italic);
        assert!(!cache.spans[0].style.bold);
    }

    #[test]
    fn inline_code_text() {
        let cache = parse_inline_content("Hello `code` world");
        assert_eq!(cache.display_text, "Hello code world");
        assert_eq!(cache.spans.len(), 1);
        assert_eq!(cache.spans[0].range, 6..10);
        assert!(cache.spans[0].style.code);
    }

    #[test]
    fn inline_bold_and_italic_nested() {
        let cache = parse_inline_content("***bold italic***");
        assert_eq!(cache.display_text, "bold italic");
        assert_eq!(cache.spans.len(), 1);
        assert!(cache.spans[0].style.bold);
        assert!(cache.spans[0].style.italic);
    }

    #[test]
    fn inline_mixed_formatting() {
        let cache = parse_inline_content("normal **bold** *italic* `code`");
        assert_eq!(cache.display_text, "normal bold italic code");
        assert_eq!(cache.spans.len(), 3);

        // bold span
        assert_eq!(cache.spans[0].range, 7..11);
        assert!(cache.spans[0].style.bold);

        // italic span
        assert_eq!(cache.spans[1].range, 12..18);
        assert!(cache.spans[1].style.italic);

        // code span
        assert_eq!(cache.spans[2].range, 19..23);
        assert!(cache.spans[2].style.code);
    }

    // --- merge_adjacent_spans tests ---

    #[test]
    fn merge_adjacent_spans_combines_contiguous_same_style() {
        let spans = vec![
            InlineSpan {
                range: 0..3,
                style: SpanStyle {
                    bold: true,
                    ..SpanStyle::default()
                },
            },
            InlineSpan {
                range: 3..6,
                style: SpanStyle {
                    bold: true,
                    ..SpanStyle::default()
                },
            },
        ];
        let merged = merge_adjacent_spans(spans);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].range, 0..6);
    }

    #[test]
    fn merge_adjacent_spans_keeps_different_styles_separate() {
        let spans = vec![
            InlineSpan {
                range: 0..3,
                style: SpanStyle {
                    bold: true,
                    ..SpanStyle::default()
                },
            },
            InlineSpan {
                range: 3..6,
                style: SpanStyle {
                    italic: true,
                    ..SpanStyle::default()
                },
            },
        ];
        let merged = merge_adjacent_spans(spans);
        assert_eq!(merged.len(), 2);
    }

    // --- Display cache for heading ---

    #[test]
    fn heading_display_cache_strips_prefix() {
        let md = "# Title\n\nParagraph";
        let doc = Document::from_markdown(md);
        // Block 0 is active, so block 1 (paragraph) has cache.
        // Switch to block 1 to get cache for block 0.
        let mut doc = doc;
        doc.set_active_block(1);
        let cache = doc.block(0).unwrap().display_cache.as_ref().unwrap();
        assert_eq!(cache.display_text, "Title");
    }

    // --- Display cache for code block ---

    #[test]
    fn code_block_display_cache_strips_fences() {
        let md = "```rust\nfn main() {}\n```";
        let doc = Document::from_markdown(md);
        // Block 0 is active, so no display cache. Let's build one manually.
        let block = doc.block(0).unwrap();
        let cache = parse_inline_spans(&block.raw, &block.kind);
        assert_eq!(cache.display_text, "fn main() {}");
        assert_eq!(cache.spans.len(), 1);
        assert!(cache.spans[0].style.code);
    }

    // --- Edge cases ---

    #[test]
    fn block_returns_none_for_out_of_bounds() {
        let doc = Document::from_markdown("Hello");
        assert!(doc.block(99).is_none());
    }

    #[test]
    fn from_markdown_preserves_code_block_raw() {
        let md = "```rust\nfn main() {}\n```";
        let doc = Document::from_markdown(md);
        assert_eq!(doc.block(0).unwrap().raw, "```rust\nfn main() {}\n```");
    }

    #[test]
    fn from_markdown_preserves_heading_raw() {
        let md = "## Hello World";
        let doc = Document::from_markdown(md);
        assert_eq!(doc.block(0).unwrap().raw, "## Hello World");
    }
}
