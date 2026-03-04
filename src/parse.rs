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

        // Find the segment containing this display offset
        for window in self.entries.windows(2) {
            let (r0, d0) = window[0];
            let (r1, d1) = window[1];
            if display >= d0 && display <= d1 {
                let delta = display - d0;
                let max_delta = r1 - r0;
                return r0 + delta.min(max_delta);
            }
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
                    continue;
                }
            }

            if star_count >= 2 {
                // Try bold: **text**
                if let Some(close) = find_closing_delimiter(bytes, raw_pos + 2, b'*', 2) {
                    let inner = &content[raw_pos + 2..close];
                    let display_start = display.len();
                    entries.push((raw_pos, display_start));
                    entries.push((raw_pos + 2, display_start));

                    // Parse inner text for nested formatting? No — keep it simple
                    // for now. Just append the inner text as bold.
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
                    continue;
                }
            }

            if star_count >= 1 {
                // Try italic: *text*
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
                    continue;
                }
            }
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
