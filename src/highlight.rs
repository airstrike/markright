use iced_core::text::highlighter::{self, Highlighter};
use iced_core::{Color, Font, Pixels};

use std::ops::Range;

/// Settings for the [`MarkdownHighlighter`].
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// The base text font (normal weight, normal style).
    pub font: Font,
    /// The monospace font for inline code and code blocks.
    pub mono_font: Font,
    /// The base font size in pixels.
    pub base_size: f32,
    /// The background color, used to hide markdown markers.
    pub background_color: Color,
}

/// The output highlight type, embedding all info needed for `to_format`.
#[derive(Debug, Clone, Copy)]
pub enum Highlight {
    /// Normal text, no special formatting.
    Normal,
    /// A marker that should be hidden (colored to match background).
    HiddenMarker(Color),
    /// A heading (includes the font with appropriate weight and the scaled size).
    Heading { font: Font, size: f32 },
    /// Bold text.
    Bold(Font),
    /// Italic text.
    Italic(Font),
    /// Bold italic text.
    BoldItalic(Font),
    /// Inline code or code block content.
    Code { font: Font },
    /// A code fence line (```), hidden.
    CodeFence(Color),
}

impl Highlight {
    /// Convert this highlight into an iced `Format<Font>`.
    pub fn to_format(&self) -> highlighter::Format<Font> {
        match self {
            Self::Normal => highlighter::Format::default(),
            Self::HiddenMarker(bg) => highlighter::Format {
                color: Some(*bg),
                ..Default::default()
            },
            Self::Heading { font, size } => highlighter::Format {
                font: Some(*font),
                size: Some(Pixels(*size)),
                ..Default::default()
            },
            Self::Bold(font) => highlighter::Format {
                font: Some(*font),
                ..Default::default()
            },
            Self::Italic(font) => highlighter::Format {
                font: Some(*font),
                ..Default::default()
            },
            Self::BoldItalic(font) => highlighter::Format {
                font: Some(*font),
                ..Default::default()
            },
            Self::Code { font } => highlighter::Format {
                font: Some(*font),
                color: Some(Color::from_rgb(0.6, 0.2, 0.2)),
                ..Default::default()
            },
            Self::CodeFence(bg) => highlighter::Format {
                color: Some(*bg),
                ..Default::default()
            },
        }
    }
}

/// A markdown highlighter for iced's `TextEditor`.
///
/// Parses markdown syntax per-line and returns [`Highlight`] spans that
/// format text as headings, bold, italic, code, etc. Markdown markers
/// (like `#`, `**`, `` ` ``) are hidden by coloring them to match the
/// background.
pub struct MarkdownHighlighter {
    settings: Settings,
    /// Whether we're currently inside a fenced code block.
    in_code_block: bool,
    /// The current line index being processed.
    current_line: usize,
}

impl Highlighter for MarkdownHighlighter {
    type Settings = Settings;
    type Highlight = Highlight;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Highlight)>;

    fn new(settings: &Settings) -> Self {
        Self {
            settings: settings.clone(),
            in_code_block: false,
            current_line: 0,
        }
    }

    fn update(&mut self, new_settings: &Settings) {
        self.settings = new_settings.clone();
        self.in_code_block = false;
        self.current_line = 0;
    }

    fn change_line(&mut self, line: usize) {
        // Reset to the changed line so we re-parse from there
        // (code block state may have changed).
        if line < self.current_line {
            self.current_line = line;
        }
        // We must also reset code block state since we'll re-scan from this line.
        // The highlight() method re-calls highlight_line from current_line forward,
        // so we reset code block state. The simplest correct approach: always reset
        // to line 0 for code block tracking.
        self.in_code_block = false;
        self.current_line = 0;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let spans = self.parse_line(line);
        self.current_line += 1;
        spans.into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

impl MarkdownHighlighter {
    fn parse_line(&mut self, line: &str) -> Vec<(Range<usize>, Highlight)> {
        let bg = self.settings.background_color;

        // Check for code fence toggle
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            self.in_code_block = !self.in_code_block;
            // Hide entire fence line
            return vec![(0..line.len(), Highlight::CodeFence(bg))];
        }

        // If inside a code block, entire line is code
        if self.in_code_block {
            let font = self.settings.mono_font;
            return vec![(0..line.len(), Highlight::Code { font })];
        }

        // Check for thematic break (---, ***, ___)
        if is_thematic_break(line) {
            return vec![(0..line.len(), Highlight::Normal)];
        }

        // Check for ATX heading
        if let Some((level, marker_len)) = parse_heading_prefix(line) {
            let size = heading_size(self.settings.base_size, level);
            let mut font = self.settings.font;
            font.weight = iced_core::font::Weight::Bold;

            let mut spans = Vec::new();
            // Hide the marker (e.g., "## ")
            spans.push((0..marker_len, Highlight::HiddenMarker(bg)));
            // Format the rest as heading text with inline parsing
            if marker_len < line.len() {
                let content = &line[marker_len..];
                let inline_spans = parse_inline(content, marker_len, &self.settings);
                for (range, highlight) in inline_spans {
                    // Override with heading size for non-marker spans
                    let heading_highlight = match highlight {
                        Highlight::Normal => Highlight::Heading { font, size },
                        Highlight::Bold(f) => Highlight::Heading { font: f, size },
                        Highlight::Italic(_) => Highlight::Heading {
                            font: Font {
                                style: iced_core::font::Style::Italic,
                                ..font
                            },
                            size,
                        },
                        Highlight::BoldItalic(_) => Highlight::Heading {
                            font: Font {
                                style: iced_core::font::Style::Italic,
                                ..font
                            },
                            size,
                        },
                        Highlight::Code { font: mono } => Highlight::Heading { font: mono, size },
                        // Keep hidden markers as-is
                        other => other,
                    };
                    spans.push((range, heading_highlight));
                }
            }
            return spans;
        }

        // Regular paragraph: parse inline formatting
        parse_inline(line, 0, &self.settings)
    }
}

/// Check if line is a thematic break: 3+ of the same char (-, *, _)
/// with optional spaces, nothing else.
fn is_thematic_break(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    let chars_no_space: Vec<char> = trimmed.chars().filter(|c| *c != ' ').collect();
    if chars_no_space.len() < 3 {
        return false;
    }
    let first = chars_no_space[0];
    (first == '-' || first == '*' || first == '_') && chars_no_space.iter().all(|c| *c == first)
}

/// Parse an ATX heading prefix. Returns (level, marker_byte_length) if valid.
/// A valid heading is 1-6 `#` chars followed by a space (or end of line).
fn parse_heading_prefix(line: &str) -> Option<(usize, usize)> {
    let bytes = line.as_bytes();
    let mut level = 0;
    while level < bytes.len() && level < 6 && bytes[level] == b'#' {
        level += 1;
    }
    if level == 0 {
        return None;
    }
    // Must be followed by space or be end of line
    if level < bytes.len() && bytes[level] != b' ' {
        return None;
    }
    // marker_len includes the space after the #s
    let marker_len = if level < bytes.len() {
        level + 1 // include the space
    } else {
        level
    };
    Some((level, marker_len))
}

/// Heading size based on level (1-6).
fn heading_size(base: f32, level: usize) -> f32 {
    match level {
        1 => base * 2.0,
        2 => base * 1.5,
        3 => base * 1.25,
        4 => base * 1.125,
        5 => base * 1.0,
        6 => base * 0.875,
        _ => base,
    }
}

/// Parse inline markdown formatting within a text span.
/// `offset` is the byte offset to add to all ranges (for heading content).
fn parse_inline(text: &str, offset: usize, settings: &Settings) -> Vec<(Range<usize>, Highlight)> {
    let bg = settings.background_color;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut spans: Vec<(Range<usize>, Highlight)> = Vec::new();
    let mut pos = 0;
    let mut normal_start = 0;

    let bold_font = Font {
        weight: iced_core::font::Weight::Bold,
        ..settings.font
    };
    let italic_font = Font {
        style: iced_core::font::Style::Italic,
        ..settings.font
    };
    let bold_italic_font = Font {
        weight: iced_core::font::Weight::Bold,
        style: iced_core::font::Style::Italic,
        ..settings.font
    };

    while pos < len {
        // Inline code: `...`
        if bytes[pos] == b'`'
            && let Some((code_start, code_end, close_end)) = find_code_span(bytes, pos)
        {
            // Flush normal text before this
            if normal_start < pos {
                spans.push((offset + normal_start..offset + pos, Highlight::Normal));
            }
            // Opening backtick(s) — hidden
            spans.push((
                offset + pos..offset + code_start,
                Highlight::HiddenMarker(bg),
            ));
            // Code content
            spans.push((
                offset + code_start..offset + code_end,
                Highlight::Code {
                    font: settings.mono_font,
                },
            ));
            // Closing backtick(s) — hidden
            spans.push((
                offset + code_end..offset + close_end,
                Highlight::HiddenMarker(bg),
            ));
            pos = close_end;
            normal_start = pos;
            continue;
        }

        // Bold/italic: ***, **, *
        if bytes[pos] == b'*' {
            let star_count = count_char(bytes, pos, b'*');

            if star_count >= 3 {
                // Try bold+italic: ***...***
                if let Some(close) = find_closing(bytes, pos + 3, b'*', 3) {
                    flush_normal(&mut spans, normal_start, pos, offset);
                    spans.push((offset + pos..offset + pos + 3, Highlight::HiddenMarker(bg)));
                    spans.push((
                        offset + pos + 3..offset + close,
                        Highlight::BoldItalic(bold_italic_font),
                    ));
                    spans.push((
                        offset + close..offset + close + 3,
                        Highlight::HiddenMarker(bg),
                    ));
                    pos = close + 3;
                    normal_start = pos;
                    continue;
                }
            }

            if star_count >= 2 {
                // Try bold: **...**
                if let Some(close) = find_closing(bytes, pos + 2, b'*', 2) {
                    flush_normal(&mut spans, normal_start, pos, offset);
                    spans.push((offset + pos..offset + pos + 2, Highlight::HiddenMarker(bg)));
                    spans.push((offset + pos + 2..offset + close, Highlight::Bold(bold_font)));
                    spans.push((
                        offset + close..offset + close + 2,
                        Highlight::HiddenMarker(bg),
                    ));
                    pos = close + 2;
                    normal_start = pos;
                    continue;
                }
            }

            // Try italic: *...*
            if let Some(close) = find_closing(bytes, pos + 1, b'*', 1) {
                flush_normal(&mut spans, normal_start, pos, offset);
                spans.push((offset + pos..offset + pos + 1, Highlight::HiddenMarker(bg)));
                spans.push((
                    offset + pos + 1..offset + close,
                    Highlight::Italic(italic_font),
                ));
                spans.push((
                    offset + close..offset + close + 1,
                    Highlight::HiddenMarker(bg),
                ));
                pos = close + 1;
                normal_start = pos;
                continue;
            }
        }

        pos += 1;
    }

    // Flush remaining normal text
    if normal_start < len {
        spans.push((offset + normal_start..offset + len, Highlight::Normal));
    }

    // If no spans were produced for a non-empty line, emit a single Normal span
    if spans.is_empty() && !text.is_empty() {
        spans.push((offset..offset + len, Highlight::Normal));
    }

    spans
}

/// Flush accumulated normal text into spans.
fn flush_normal(
    spans: &mut Vec<(Range<usize>, Highlight)>,
    normal_start: usize,
    pos: usize,
    offset: usize,
) {
    if normal_start < pos {
        spans.push((offset + normal_start..offset + pos, Highlight::Normal));
    }
}

/// Count consecutive occurrences of `ch` starting at `pos`.
fn count_char(bytes: &[u8], pos: usize, ch: u8) -> usize {
    let mut count = 0;
    while pos + count < bytes.len() && bytes[pos + count] == ch {
        count += 1;
    }
    count
}

/// Find a closing sequence of `count` consecutive `ch` chars, starting search at `from`.
/// Returns the byte index where the closing sequence starts.
fn find_closing(bytes: &[u8], from: usize, ch: u8, count: usize) -> Option<usize> {
    let mut i = from;
    while i + count <= bytes.len() {
        if bytes[i] == ch && count_char(bytes, i, ch) >= count {
            // Make sure we're not matching an empty span
            if i > from {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Find a code span starting at `pos` (which points to a backtick).
/// Returns (content_start, content_end, full_close_end).
fn find_code_span(bytes: &[u8], pos: usize) -> Option<(usize, usize, usize)> {
    let open_count = count_char(bytes, pos, b'`');
    let content_start = pos + open_count;

    // Search for matching closing backticks
    let mut i = content_start;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let close_count = count_char(bytes, i, b'`');
            if close_count == open_count {
                return Some((content_start, i, i + close_count));
            }
            i += close_count;
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_core::font::{Family, Style, Weight};

    fn test_settings() -> Settings {
        Settings {
            font: Font {
                family: Family::SansSerif,
                weight: Weight::Normal,
                stretch: iced_core::font::Stretch::Normal,
                style: Style::Normal,
            },
            mono_font: Font {
                family: Family::Monospace,
                weight: Weight::Normal,
                stretch: iced_core::font::Stretch::Normal,
                style: Style::Normal,
            },
            base_size: 16.0,
            background_color: Color::WHITE,
        }
    }

    fn highlight(lines: &[&str]) -> Vec<Vec<(Range<usize>, Highlight)>> {
        let settings = test_settings();
        let mut h = MarkdownHighlighter::new(&settings);
        lines
            .iter()
            .map(|line| {
                let spans: Vec<_> = h.highlight_line(line).collect();
                spans
            })
            .collect()
    }

    fn highlight_one(line: &str) -> Vec<(Range<usize>, Highlight)> {
        highlight(&[line]).into_iter().next().unwrap()
    }

    #[test]
    fn plain_text_is_normal() {
        let spans = highlight_one("Hello, world!");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, 0..13);
        assert!(matches!(spans[0].1, Highlight::Normal));
    }

    #[test]
    fn heading_level_1() {
        let spans = highlight_one("# Hello");
        // Should have: hidden marker "# ", heading content "Hello"
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].0, 0..2); // "# "
        assert!(matches!(spans[0].1, Highlight::HiddenMarker(_)));
        assert_eq!(spans[1].0, 2..7); // "Hello"
        assert!(
            matches!(spans[1].1, Highlight::Heading { size, .. } if (size - 32.0).abs() < 0.01)
        );
    }

    #[test]
    fn heading_level_3() {
        let spans = highlight_one("### Hello");
        assert_eq!(spans[0].0, 0..4); // "### "
        assert!(matches!(spans[0].1, Highlight::HiddenMarker(_)));
        assert_eq!(spans[1].0, 4..9); // "Hello"
        assert!(
            matches!(spans[1].1, Highlight::Heading { size, .. } if (size - 20.0).abs() < 0.01)
        );
    }

    #[test]
    fn not_a_heading_without_space() {
        let spans = highlight_one("#NotAHeading");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].1, Highlight::Normal));
    }

    #[test]
    fn bold_text() {
        let spans = highlight_one("some **bold** text");
        // "some " Normal, "**" hidden, "bold" Bold, "**" hidden, " text" Normal
        assert_eq!(spans.len(), 5);
        assert!(matches!(spans[0].1, Highlight::Normal));
        assert!(matches!(spans[1].1, Highlight::HiddenMarker(_)));
        assert!(matches!(spans[2].1, Highlight::Bold(_)));
        assert_eq!(spans[2].0, 7..11); // "bold"
        assert!(matches!(spans[3].1, Highlight::HiddenMarker(_)));
        assert!(matches!(spans[4].1, Highlight::Normal));
    }

    #[test]
    fn italic_text() {
        let spans = highlight_one("some *italic* text");
        assert_eq!(spans.len(), 5);
        assert!(matches!(spans[2].1, Highlight::Italic(_)));
        assert_eq!(spans[2].0, 6..12); // "italic"
    }

    #[test]
    fn bold_italic_text() {
        let spans = highlight_one("some ***bold italic*** text");
        assert_eq!(spans.len(), 5);
        assert!(matches!(spans[2].1, Highlight::BoldItalic(_)));
        assert_eq!(spans[2].0, 8..19); // "bold italic"
    }

    #[test]
    fn inline_code() {
        let spans = highlight_one("use `code` here");
        assert_eq!(spans.len(), 5);
        assert!(matches!(spans[0].1, Highlight::Normal));
        assert!(matches!(spans[1].1, Highlight::HiddenMarker(_)));
        assert!(matches!(spans[2].1, Highlight::Code { .. }));
        assert_eq!(spans[2].0, 5..9); // "code"
        assert!(matches!(spans[3].1, Highlight::HiddenMarker(_)));
        assert!(matches!(spans[4].1, Highlight::Normal));
    }

    #[test]
    fn code_block() {
        let results = highlight(&["```rust", "fn main() {}", "```"]);
        // First line: code fence (hidden)
        assert_eq!(results[0].len(), 1);
        assert!(matches!(results[0][0].1, Highlight::CodeFence(_)));
        // Second line: code content
        assert_eq!(results[1].len(), 1);
        assert!(matches!(results[1][0].1, Highlight::Code { .. }));
        // Third line: closing fence (hidden)
        assert_eq!(results[2].len(), 1);
        assert!(matches!(results[2][0].1, Highlight::CodeFence(_)));
    }

    #[test]
    fn thematic_break() {
        let spans = highlight_one("---");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].1, Highlight::Normal));
    }

    #[test]
    fn unmatched_stars_shown_as_normal() {
        let spans = highlight_one("some *unmatched text");
        // No closing *, so entire line is normal
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].1, Highlight::Normal));
    }

    #[test]
    fn heading_with_bold() {
        let spans = highlight_one("## **Bold** heading");
        // "## " hidden, "**" hidden, "Bold" heading+bold, "**" hidden, " heading" heading
        assert!(spans.len() >= 4);
        assert!(matches!(spans[0].1, Highlight::HiddenMarker(_)));
        // The bold markers should be hidden
        assert!(matches!(spans[1].1, Highlight::HiddenMarker(_)));
        // The bold content should be a heading (with bold font)
        assert!(matches!(spans[2].1, Highlight::Heading { .. }));
    }

    #[test]
    fn empty_line() {
        let spans = highlight_one("");
        assert!(spans.is_empty());
    }

    #[test]
    fn hash_only_is_heading() {
        // "# " with nothing after is still a valid heading
        let spans = highlight_one("# ");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].1, Highlight::HiddenMarker(_)));
    }

    #[test]
    fn multiple_inline_formats() {
        let spans = highlight_one("**bold** and *italic*");
        // "**" hidden, "bold" bold, "**" hidden, " and " normal, "*" hidden, "italic" italic, "*" hidden
        assert_eq!(spans.len(), 7);
        assert!(matches!(spans[1].1, Highlight::Bold(_)));
        assert!(matches!(spans[3].1, Highlight::Normal));
        assert!(matches!(spans[5].1, Highlight::Italic(_)));
    }
}
