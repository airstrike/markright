// Markdown line parser — spans + offset map

use std::ops::Range;

/// Result of parsing a single markdown line.
#[derive(Debug, Clone)]
pub struct Line {
    /// Text with markers removed, for display when line is not active.
    pub display: String,
    /// Styled spans for the display text.
    pub spans: Vec<Span>,
    /// Bidirectional offset map between raw and display coordinates.
    pub offset_map: OffsetMap,
}

/// A styled region of the display text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// Byte range in display text.
    pub range: Range<usize>,
    /// Visual style for this span.
    pub style: Style,
}

/// Visual style for a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub heading: Option<u8>,
}

/// Bidirectional offset map between raw and display byte positions.
///
/// Stores a sorted vec of `(raw_pos, display_pos)` entries at each marker
/// boundary. Interpolates linearly between entries (text between markers
/// maps 1:1).
#[derive(Debug, Clone)]
pub struct OffsetMap {
    /// Sorted entries of (raw_pos, display_pos) at marker boundaries.
    entries: Vec<(usize, usize)>,
}

impl OffsetMap {
    /// Create a new offset map from sorted boundary entries.
    pub fn new(entries: Vec<(usize, usize)>) -> Self {
        Self { entries }
    }

    /// Create an identity offset map (raw == display) for a given length.
    pub fn identity(len: usize) -> Self {
        Self {
            entries: vec![(0, 0), (len, len)],
        }
    }

    /// Given a byte offset in raw text, return the corresponding display offset.
    pub fn raw_to_display(&self, raw_offset: usize) -> usize {
        self.interpolate_forward(raw_offset)
    }

    /// Given a byte offset in display text, return the corresponding raw offset.
    pub fn display_to_raw(&self, display_offset: usize) -> usize {
        self.interpolate_reverse(display_offset)
    }

    fn interpolate_forward(&self, raw: usize) -> usize {
        if self.entries.is_empty() {
            return raw;
        }

        // Find the segment containing this raw offset
        for window in self.entries.windows(2) {
            let (r0, d0) = window[0];
            let (r1, d1) = window[1];
            if raw >= r0 && raw <= r1 {
                // Linear interpolation: text between markers maps 1:1
                let delta = raw - r0;
                let max_delta = d1 - d0;
                return d0 + delta.min(max_delta);
            }
        }

        // Past the last entry — extrapolate 1:1 from the last entry
        if let Some(&(r_last, d_last)) = self.entries.last()
            && raw >= r_last
        {
            return d_last + (raw - r_last);
        }

        raw
    }

    fn interpolate_reverse(&self, display: usize) -> usize {
        if self.entries.is_empty() {
            return display;
        }

        // Find the best segment containing this display offset.
        // Prefer text regions (non-zero display delta) over marker regions
        // (zero display delta). Among text regions, prefer the last one
        // that contains the display offset, so that boundary values map
        // to the text side rather than the marker side.
        let mut best_text: Option<usize> = None;
        let mut best_any: Option<usize> = None;
        for (i, window) in self.entries.windows(2).enumerate() {
            let (_r0, d0) = window[0];
            let (_r1, d1) = window[1];
            if display >= d0 && display <= d1 {
                if best_any.is_none() {
                    best_any = Some(i);
                }
                // Text region: display range is non-zero
                if d1 > d0 {
                    best_text = Some(i);
                }
            }
        }

        let best = best_text.or(best_any);

        if let Some(i) = best {
            let (r0, d0) = self.entries[i];
            let (r1, _d1) = self.entries[i + 1];
            let delta = display - d0;
            let max_delta = r1 - r0;
            return r0 + delta.min(max_delta);
        }

        // Past the last entry — extrapolate 1:1 from the last entry
        if let Some(&(r_last, d_last)) = self.entries.last()
            && display >= d_last
        {
            return r_last + (display - d_last);
        }

        display
    }
}

/// Parse a single line of markdown.
///
/// `in_code_block` tracks whether we're inside a fenced code block
/// (``` delimiters). It's mutated: toggled when a fence line is encountered.
pub fn parse_line(raw: &str, in_code_block: &mut bool) -> Line {
    // Check for code fence toggle
    let trimmed = raw.trim();
    if trimmed == "```" || (trimmed.starts_with("```") && !trimmed[3..].contains('`')) {
        *in_code_block = !*in_code_block;

        // Fence lines are displayed as-is with code style
        return Line {
            display: raw.to_string(),
            spans: vec![Span {
                range: 0..raw.len(),
                style: Style {
                    code: true,
                    ..Style::default()
                },
            }],
            offset_map: OffsetMap::identity(raw.len()),
        };
    }

    // Inside code block: no inline parsing, whole line gets code style
    if *in_code_block {
        return Line {
            display: raw.to_string(),
            spans: vec![Span {
                range: 0..raw.len(),
                style: Style {
                    code: true,
                    ..Style::default()
                },
            }],
            offset_map: OffsetMap::identity(raw.len()),
        };
    }

    // Check for heading prefix
    let (heading_level, content_start) = parse_heading_prefix(raw);

    let content = &raw[content_start..];

    // Parse inline formatting on the content portion
    let (display_content, spans_content, map_entries) = parse_inline(content);

    // Build the full display string and adjust spans/map for heading prefix removal
    let prefix_stripped = content_start; // bytes removed from raw
    let display = display_content;
    let spans = if let Some(level) = heading_level {
        // Apply heading style to the entire display line
        // Also preserve any inline styles by merging
        spans_content
            .into_iter()
            .map(|mut s| {
                s.style.heading = Some(level);
                s
            })
            .collect()
    } else {
        spans_content
    };

    // Adjust offset map entries: raw positions need the heading prefix offset added back
    let entries: Vec<(usize, usize)> = map_entries
        .into_iter()
        .map(|(r, d)| (r + prefix_stripped, d))
        .collect();

    let offset_map = OffsetMap::new(entries);

    Line {
        display,
        spans,
        offset_map,
    }
}

/// Check if a line starts with a heading prefix (1-6 `#` chars followed by a space).
/// Returns (heading_level, byte_offset_of_content_after_prefix).
fn parse_heading_prefix(raw: &str) -> (Option<u8>, usize) {
    let bytes = raw.as_bytes();
    let mut count = 0;
    while count < bytes.len() && count < 6 && bytes[count] == b'#' {
        count += 1;
    }
    if count > 0 && count < bytes.len() && bytes[count] == b' ' {
        (Some(count as u8), count + 1) // skip "# "
    } else {
        (None, 0)
    }
}

/// Parse inline formatting from a string, returning:
/// - display text (markers removed)
/// - spans (ranges in display text with styles)
/// - offset map entries (raw_pos, display_pos) at marker boundaries
fn parse_inline(content: &str) -> (String, Vec<Span>, Vec<(usize, usize)>) {
    let bytes = content.as_bytes();
    let len = bytes.len();

    let mut display = String::with_capacity(len);
    let mut spans: Vec<Span> = Vec::new();
    let mut entries: Vec<(usize, usize)> = Vec::new();

    // Start entry
    entries.push((0, 0));

    let mut raw_pos = 0;

    while raw_pos < len {
        if bytes[raw_pos] == b'`' {
            // Inline code: find matching closing backtick
            if let Some(close) = find_closing_backtick(bytes, raw_pos + 1) {
                let open_marker_len = 1;
                let close_marker_len = 1;
                let inner = &content[raw_pos + open_marker_len..close];

                // Record entry for opening marker removal
                let display_start = display.len();
                entries.push((raw_pos, display_start));
                // Skip opening backtick
                entries.push((raw_pos + open_marker_len, display_start));

                // Append inner text
                display.push_str(inner);
                let display_end = display.len();

                // Record entry for closing marker removal
                entries.push((close, display_end));
                entries.push((close + close_marker_len, display_end));

                spans.push(Span {
                    range: display_start..display_end,
                    style: Style {
                        code: true,
                        ..Style::default()
                    },
                });

                raw_pos = close + close_marker_len;
                continue;
            }
        }

        if bytes[raw_pos] == b'*' {
            // Count consecutive asterisks
            let star_count = count_chars(bytes, raw_pos, b'*');
            let mut matched = false;

            if star_count >= 3 {
                // Try bold+italic: ***text***
                if let Some(close) = find_closing_delimiter(bytes, raw_pos + 3, b'*', 3) {
                    let inner = &content[raw_pos + 3..close];
                    let display_start = display.len();
                    entries.push((raw_pos, display_start));
                    entries.push((raw_pos + 3, display_start));

                    display.push_str(inner);
                    let display_end = display.len();

                    entries.push((close, display_end));
                    entries.push((close + 3, display_end));

                    spans.push(Span {
                        range: display_start..display_end,
                        style: Style {
                            bold: true,
                            italic: true,
                            ..Style::default()
                        },
                    });

                    raw_pos = close + 3;
                    matched = true;
                }
            }

            if !matched && star_count >= 2 {
                // Try bold: **text**
                if let Some(close) = find_closing_delimiter(bytes, raw_pos + 2, b'*', 2) {
                    let inner = &content[raw_pos + 2..close];
                    let display_start = display.len();
                    entries.push((raw_pos, display_start));
                    entries.push((raw_pos + 2, display_start));

                    display.push_str(inner);
                    let display_end = display.len();

                    entries.push((close, display_end));
                    entries.push((close + 2, display_end));

                    spans.push(Span {
                        range: display_start..display_end,
                        style: Style {
                            bold: true,
                            ..Style::default()
                        },
                    });

                    raw_pos = close + 2;
                    matched = true;
                }
            }

            if !matched && star_count == 1 {
                // Try italic: *text* — only when exactly 1 star
                if let Some(close) = find_closing_delimiter(bytes, raw_pos + 1, b'*', 1) {
                    let inner = &content[raw_pos + 1..close];
                    let display_start = display.len();
                    entries.push((raw_pos, display_start));
                    entries.push((raw_pos + 1, display_start));

                    display.push_str(inner);
                    let display_end = display.len();

                    entries.push((close, display_end));
                    entries.push((close + 1, display_end));

                    spans.push(Span {
                        range: display_start..display_end,
                        style: Style {
                            italic: true,
                            ..Style::default()
                        },
                    });

                    raw_pos = close + 1;
                    matched = true;
                }
            }

            if matched {
                continue;
            }

            // Unmatched — treat all consecutive stars as literal text
            for _ in 0..star_count {
                display.push('*');
            }
            raw_pos += star_count;
            continue;
        }

        // Regular character — copy to display, handling multi-byte UTF-8
        let ch = content[raw_pos..]
            .chars()
            .next()
            .expect("valid char at position");
        display.push(ch);
        raw_pos += ch.len_utf8();
    }

    // End entry
    entries.push((len, display.len()));

    (display, spans, entries)
}

/// Find the position of a closing backtick starting from `start`.
fn find_closing_backtick(bytes: &[u8], start: usize) -> Option<usize> {
    let mut pos = start;
    while pos < bytes.len() {
        if bytes[pos] == b'`' {
            return Some(pos);
        }
        pos += 1;
    }
    None
}

/// Count consecutive occurrences of `ch` starting at `pos`.
fn count_chars(bytes: &[u8], pos: usize, ch: u8) -> usize {
    let mut count = 0;
    while pos + count < bytes.len() && bytes[pos + count] == ch {
        count += 1;
    }
    count
}

/// Find closing delimiter of exactly `count` consecutive `ch` characters,
/// starting search from `start`. The closing delimiter must be exactly `count`
/// characters (not more).
fn find_closing_delimiter(bytes: &[u8], start: usize, ch: u8, count: usize) -> Option<usize> {
    let mut pos = start;
    while pos + count <= bytes.len() {
        if bytes[pos] == ch {
            let found = count_chars(bytes, pos, ch);
            if found == count {
                // Make sure the character before (if any) is not the same delimiter
                // and the character after (if any) is not the same delimiter
                let before_ok = pos == start || bytes[pos - 1] != ch;
                let after_ok = pos + count >= bytes.len() || bytes[pos + count] != ch;
                if before_ok && after_ok {
                    return Some(pos);
                }
            }
            pos += found;
        } else {
            pos += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Plain text ---

    #[test]
    fn plain_text_display_equals_raw() {
        let mut in_code = false;
        let line = parse_line("hello world", &mut in_code);
        assert_eq!(line.display, "hello world");
        assert!(line.spans.is_empty());
    }

    #[test]
    fn plain_text_identity_offset_map() {
        let mut in_code = false;
        let line = parse_line("hello", &mut in_code);
        for i in 0..=5 {
            assert_eq!(line.offset_map.raw_to_display(i), i);
            assert_eq!(line.offset_map.display_to_raw(i), i);
        }
    }

    #[test]
    fn empty_line() {
        let mut in_code = false;
        let line = parse_line("", &mut in_code);
        assert_eq!(line.display, "");
        assert!(line.spans.is_empty());
    }

    // --- Headings ---

    #[test]
    fn heading_1_strips_prefix() {
        let mut in_code = false;
        let line = parse_line("# Title", &mut in_code);
        assert_eq!(line.display, "Title");
        assert_eq!(line.spans.len(), 0);
    }

    #[test]
    fn heading_1_with_inline_formatting() {
        let mut in_code = false;
        let line = parse_line("# **Bold Title**", &mut in_code);
        assert_eq!(line.display, "Bold Title");
        assert_eq!(line.spans.len(), 1);
        assert!(line.spans[0].style.bold);
        assert_eq!(line.spans[0].style.heading, Some(1));
    }

    #[test]
    fn heading_levels_1_through_6() {
        for level in 1..=6u8 {
            let mut in_code = false;
            let prefix = "#".repeat(level as usize);
            let raw = format!("{prefix} Heading");
            let line = parse_line(&raw, &mut in_code);
            assert_eq!(line.display, "Heading");
        }
    }

    #[test]
    fn seven_hashes_is_not_a_heading() {
        let mut in_code = false;
        let line = parse_line("####### Not a heading", &mut in_code);
        assert_eq!(line.display, "####### Not a heading");
    }

    #[test]
    fn hash_without_space_is_not_a_heading() {
        let mut in_code = false;
        let line = parse_line("#NoSpace", &mut in_code);
        assert_eq!(line.display, "#NoSpace");
    }

    #[test]
    fn heading_offset_map_accounts_for_prefix() {
        let mut in_code = false;
        // raw: "# Hello"  (# = 0, space = 1, H = 2, e = 3, ...)
        // display: "Hello" (H = 0, e = 1, ...)
        let line = parse_line("# Hello", &mut in_code);
        assert_eq!(line.display, "Hello");
        // raw offset 2 ("H") -> display offset 0
        assert_eq!(line.offset_map.raw_to_display(2), 0);
        // display offset 0 ("H") -> raw offset 2
        assert_eq!(line.offset_map.display_to_raw(0), 2);
        // raw offset 7 (end) -> display offset 5 (end)
        assert_eq!(line.offset_map.raw_to_display(7), 5);
    }

    // --- Bold ---

    #[test]
    fn bold_markers_stripped() {
        let mut in_code = false;
        let line = parse_line("**bold**", &mut in_code);
        assert_eq!(line.display, "bold");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].range, 0..4);
        assert!(line.spans[0].style.bold);
        assert!(!line.spans[0].style.italic);
    }

    #[test]
    fn bold_with_surrounding_text() {
        let mut in_code = false;
        let line = parse_line("a **b** c", &mut in_code);
        assert_eq!(line.display, "a b c");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].range, 2..3); // "b" in display
        assert!(line.spans[0].style.bold);
    }

    // --- Italic ---

    #[test]
    fn italic_markers_stripped() {
        let mut in_code = false;
        let line = parse_line("*italic*", &mut in_code);
        assert_eq!(line.display, "italic");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].range, 0..6);
        assert!(line.spans[0].style.italic);
        assert!(!line.spans[0].style.bold);
    }

    // --- Bold+Italic ---

    #[test]
    fn bold_italic_markers_stripped() {
        let mut in_code = false;
        let line = parse_line("***both***", &mut in_code);
        assert_eq!(line.display, "both");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].range, 0..4);
        assert!(line.spans[0].style.bold);
        assert!(line.spans[0].style.italic);
    }

    // --- Inline code ---

    #[test]
    fn inline_code_markers_stripped() {
        let mut in_code = false;
        let line = parse_line("`code`", &mut in_code);
        assert_eq!(line.display, "code");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].range, 0..4);
        assert!(line.spans[0].style.code);
    }

    #[test]
    fn inline_code_no_nested_formatting() {
        let mut in_code = false;
        // Stars inside backticks should not be parsed as bold
        let line = parse_line("`**not bold**`", &mut in_code);
        assert_eq!(line.display, "**not bold**");
        assert_eq!(line.spans.len(), 1);
        assert!(line.spans[0].style.code);
        assert!(!line.spans[0].style.bold);
    }

    // --- Mixed formatting ---

    #[test]
    fn mixed_bold_and_italic() {
        let mut in_code = false;
        let line = parse_line("hello **world** and *more*", &mut in_code);
        assert_eq!(line.display, "hello world and more");
        assert_eq!(line.spans.len(), 2);

        // "world" bold
        assert_eq!(line.spans[0].range, 6..11);
        assert!(line.spans[0].style.bold);

        // "more" italic
        assert_eq!(line.spans[1].range, 16..20);
        assert!(line.spans[1].style.italic);
    }

    #[test]
    fn multiple_code_spans() {
        let mut in_code = false;
        let line = parse_line("`a` and `b`", &mut in_code);
        assert_eq!(line.display, "a and b");
        assert_eq!(line.spans.len(), 2);
        assert!(line.spans[0].style.code);
        assert!(line.spans[1].style.code);
        assert_eq!(line.spans[0].range, 0..1); // "a"
        assert_eq!(line.spans[1].range, 6..7); // "b"
    }

    // --- Unmatched delimiters ---

    #[test]
    fn unmatched_bold_treated_as_literal() {
        let mut in_code = false;
        let line = parse_line("**no close", &mut in_code);
        assert_eq!(line.display, "**no close");
        assert!(line.spans.is_empty());
    }

    #[test]
    fn unmatched_italic_treated_as_literal() {
        let mut in_code = false;
        let line = parse_line("*no close", &mut in_code);
        assert_eq!(line.display, "*no close");
        assert!(line.spans.is_empty());
    }

    #[test]
    fn unmatched_backtick_treated_as_literal() {
        let mut in_code = false;
        let line = parse_line("`no close", &mut in_code);
        assert_eq!(line.display, "`no close");
        assert!(line.spans.is_empty());
    }

    // --- Code fences ---

    #[test]
    fn code_fence_toggles_state() {
        let mut in_code = false;
        let _line = parse_line("```", &mut in_code);
        assert!(in_code);
        let _line = parse_line("```", &mut in_code);
        assert!(!in_code);
    }

    #[test]
    fn code_fence_with_language_tag() {
        let mut in_code = false;
        let line = parse_line("```rust", &mut in_code);
        assert!(in_code);
        assert_eq!(line.display, "```rust");
        assert!(line.spans[0].style.code);
    }

    #[test]
    fn lines_inside_code_block_get_code_style() {
        let mut in_code = true;
        let line = parse_line("let x = **not bold**;", &mut in_code);
        assert_eq!(line.display, "let x = **not bold**;");
        assert_eq!(line.spans.len(), 1);
        assert!(line.spans[0].style.code);
        assert!(!line.spans[0].style.bold);
    }

    #[test]
    fn code_block_no_inline_parsing() {
        let mut in_code = false;

        // Open fence
        let _ = parse_line("```", &mut in_code);
        assert!(in_code);

        // Line inside - should not parse inline formatting
        let line = parse_line("**bold** *italic* `code`", &mut in_code);
        assert_eq!(line.display, "**bold** *italic* `code`");
        assert_eq!(line.spans.len(), 1);
        assert!(line.spans[0].style.code);

        // Close fence
        let _ = parse_line("```", &mut in_code);
        assert!(!in_code);

        // After close, inline parsing resumes
        let line = parse_line("**bold**", &mut in_code);
        assert_eq!(line.display, "bold");
        assert!(line.spans[0].style.bold);
    }

    // --- Offset map round-tripping ---

    #[test]
    fn offset_map_round_trip_bold() {
        let mut in_code = false;
        // raw: "**bold**"  (8 bytes)
        // display: "bold"  (4 bytes)
        let line = parse_line("**bold**", &mut in_code);

        // raw 0 (first *) -> display 0
        assert_eq!(line.offset_map.raw_to_display(0), 0);
        // raw 2 (b) -> display 0
        assert_eq!(line.offset_map.raw_to_display(2), 0);
        // raw 3 (o) -> display 1
        assert_eq!(line.offset_map.raw_to_display(3), 1);
        // raw 6 (closing **) -> display 4
        assert_eq!(line.offset_map.raw_to_display(6), 4);
        // raw 8 (end) -> display 4
        assert_eq!(line.offset_map.raw_to_display(8), 4);

        // display 0 (b) -> raw 2
        assert_eq!(line.offset_map.display_to_raw(0), 2);
        // display 4 (end) -> raw 6
        assert_eq!(line.offset_map.display_to_raw(4), 6);
    }

    #[test]
    fn offset_map_round_trip_mixed() {
        let mut in_code = false;
        // raw:     "a **b** c"  (a=0, space=1, *=2, *=3, b=4, *=5, *=6, space=7, c=8)
        // display: "a b c"      (a=0, space=1, b=2, space=3, c=4)
        let line = parse_line("a **b** c", &mut in_code);

        // Characters before the bold marker
        assert_eq!(line.offset_map.raw_to_display(0), 0); // "a"
        assert_eq!(line.offset_map.raw_to_display(1), 1); // " "

        // The bold content
        assert_eq!(line.offset_map.raw_to_display(4), 2); // "b"

        // After bold closing marker
        assert_eq!(line.offset_map.raw_to_display(7), 3); // " "
        assert_eq!(line.offset_map.raw_to_display(8), 4); // "c"

        // Round-trip: display -> raw
        assert_eq!(line.offset_map.display_to_raw(0), 0); // "a"
        assert_eq!(line.offset_map.display_to_raw(2), 4); // "b"
        assert_eq!(line.offset_map.display_to_raw(4), 8); // "c"
    }

    #[test]
    fn offset_map_identity_for_plain_text() {
        let map = OffsetMap::identity(10);
        for i in 0..=10 {
            assert_eq!(map.raw_to_display(i), i);
            assert_eq!(map.display_to_raw(i), i);
        }
    }

    // --- UTF-8 handling ---

    #[test]
    fn unicode_plain_text() {
        let mut in_code = false;
        let line = parse_line("cafe\u{0301}", &mut in_code); // "café" with combining accent
        assert_eq!(line.display, "cafe\u{0301}");
    }

    #[test]
    fn unicode_with_bold() {
        let mut in_code = false;
        let line = parse_line("**caf\u{00e9}**", &mut in_code);
        assert_eq!(line.display, "caf\u{00e9}");
        assert!(line.spans[0].style.bold);
    }

    // --- Edge cases ---

    #[test]
    fn adjacent_bold_spans() {
        let mut in_code = false;
        // **a****b** is ambiguous. Our parser matches the outermost **...**
        // and treats the inner **** as literal since it can't find exactly 2
        // closing stars at the 4-star boundary.
        let line = parse_line("**a****b**", &mut in_code);
        assert_eq!(line.display, "a****b");
        assert_eq!(line.spans.len(), 1);
        assert!(line.spans[0].style.bold);
    }

    #[test]
    fn fence_line_gets_code_style() {
        let mut in_code = false;
        let line = parse_line("```", &mut in_code);
        assert_eq!(line.spans.len(), 1);
        assert!(line.spans[0].style.code);
    }
}
