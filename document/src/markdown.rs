//! Markdown format adapter for markright's rich text editor.
//!
//! Parses markdown into a [`Vec<StyledLine>`] using [pulldown_cmark].
//! The conversion is lossy: markdown semantics that can't be expressed
//! through character styling (links' URLs, images, tables) are
//! approximated or dropped.
//!
//! # What maps to what
//!
//! - `**bold**` → span with `bold: Some(true)`
//! - `*italic*` → span with `italic: Some(true)`
//! - `~~strike~~` → span with `strikethrough: Some(true)`
//! - `` `code` `` → span with monospace font
//! - `# H1`..`###### H6` → font size scaled by level, bold
//! - `- item` / `1. item` → paragraph list + level
//! - ` ``` fenced ``` ` → each code-block line gets monospace font
//! - `> quote` → paragraph indent
//! - `---` → blank paragraph
//! - soft breaks → single space, hard breaks → new `StyledLine`
//!
//! Serialization is best-effort: a [`StyledLine`] with bullet list
//! style becomes `- text`, bold spans become `**text**`, etc.

use std::fmt::Write;

use iced_core::text::rich_editor::paragraph::{self, Bullet, Indent, List, Number};
use iced_core::text::rich_editor::span;
use iced_core::{Font, font};
use markright_core::{Format, StyleRun, StyledLine};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Default monospace font for inline code and code blocks.
fn monospace_font() -> Font {
    Font {
        family: font::Family::Monospace,
        ..Font::DEFAULT
    }
}

/// Font size (relative to base) for each heading level.
fn heading_size(level: HeadingLevel) -> f32 {
    match level {
        HeadingLevel::H1 => 32.0,
        HeadingLevel::H2 => 28.0,
        HeadingLevel::H3 => 24.0,
        HeadingLevel::H4 => 20.0,
        HeadingLevel::H5 => 18.0,
        HeadingLevel::H6 => 16.0,
    }
}

/// The Markdown document format.
pub struct Markdown;

impl Format for Markdown {
    type Error = std::convert::Infallible;

    fn parse(input: &str) -> Result<Vec<StyledLine>, Self::Error> {
        Ok(parse_markdown(input))
    }

    fn serialize(lines: &[StyledLine]) -> String {
        serialize_markdown(lines)
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Inline styling flags tracked as we walk the event stream.
#[derive(Default, Clone, Copy)]
struct Inline {
    bold: bool,
    italic: bool,
    strikethrough: bool,
    code: bool,
}

impl Inline {
    /// Produce a `span::Style` reflecting the current inline flags.
    fn to_span_style(self) -> span::Style {
        span::Style {
            bold: self.bold.then_some(true),
            italic: self.italic.then_some(true),
            strikethrough: self.strikethrough.then_some(true),
            font: self.code.then(monospace_font),
            ..Default::default()
        }
    }
}

/// Context for the block currently being assembled.
#[derive(Clone)]
struct Block {
    /// Accumulated text for this line.
    text: String,
    /// Style runs relative to `text`.
    runs: Vec<StyleRun>,
    /// Paragraph-level formatting.
    paragraph_style: paragraph::Style,
}

impl Block {
    fn new(paragraph_style: paragraph::Style) -> Self {
        Self {
            text: String::new(),
            runs: Vec::new(),
            paragraph_style,
        }
    }

    /// Append text with the current inline styling, extending the last
    /// run if the style matches.
    fn push_text(&mut self, text: &str, inline: Inline) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        let end = self.text.len();
        let style = inline.to_span_style();

        if let Some(last) = self.runs.last_mut()
            && last.range.end == start
            && last.style == style
        {
            last.range.end = end;
            return;
        }
        self.runs.push(StyleRun {
            range: start..end,
            style,
        });
    }

    fn into_styled_line(self) -> StyledLine {
        StyledLine {
            text: self.text,
            runs: self.runs,
            paragraph_style: self.paragraph_style,
        }
    }
}

/// Where we are in the block structure.
#[derive(Clone)]
enum ListKind {
    Bullet,
    Ordered,
}

/// Parse markdown text into styled lines.
fn parse_markdown(input: &str) -> Vec<StyledLine> {
    let parser = Parser::new_ext(
        input,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS,
    );

    let mut out: Vec<StyledLine> = Vec::new();
    let mut block: Option<Block> = None;
    let mut inline = Inline::default();
    let mut list_stack: Vec<ListKind> = Vec::new();
    let mut quote_depth: u8 = 0;
    let mut heading_level: Option<HeadingLevel> = None;
    let mut in_code_block = false;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    block = Some(Block::new(paragraph_style_for(
                        &list_stack,
                        quote_depth,
                        None,
                    )));
                }
                Tag::Heading { level, .. } => {
                    heading_level = Some(level);
                    let size = heading_size(level);
                    let mut ps = paragraph_style_for(&list_stack, quote_depth, Some(level));
                    ps.style = span::Style {
                        bold: Some(true),
                        size: Some(size),
                        ..ps.style
                    };
                    block = Some(Block::new(ps));
                    inline.bold = true;
                }
                Tag::List(Some(_)) => list_stack.push(ListKind::Ordered),
                Tag::List(None) => list_stack.push(ListKind::Bullet),
                Tag::Item => {
                    block = Some(Block::new(paragraph_style_for(
                        &list_stack,
                        quote_depth,
                        None,
                    )));
                }
                Tag::CodeBlock(_) => {
                    in_code_block = true;
                    inline.code = true;
                }
                Tag::BlockQuote(_) => quote_depth = quote_depth.saturating_add(1),
                Tag::Emphasis => inline.italic = true,
                Tag::Strong => inline.bold = true,
                Tag::Strikethrough => inline.strikethrough = true,
                Tag::Link { .. } | Tag::Image { .. } => {
                    // Links/images: we keep the text but drop the URL.
                }
                _ => {}
            },
            Event::End(end) => match end {
                TagEnd::Paragraph | TagEnd::Item => {
                    if let Some(b) = block.take() {
                        out.push(b.into_styled_line());
                    }
                }
                TagEnd::Heading(_) => {
                    if let Some(b) = block.take() {
                        out.push(b.into_styled_line());
                    }
                    inline.bold = false;
                    heading_level = None;
                }
                TagEnd::List(_) => {
                    list_stack.pop();
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    inline.code = false;
                }
                TagEnd::BlockQuote(_) => {
                    quote_depth = quote_depth.saturating_sub(1);
                }
                TagEnd::Emphasis => inline.italic = false,
                TagEnd::Strong => {
                    // Preserve outer bold if this event is inside a heading.
                    inline.bold = heading_level.is_some();
                }
                TagEnd::Strikethrough => inline.strikethrough = false,
                _ => {}
            },
            Event::Text(text) => {
                if in_code_block {
                    // Code blocks emit raw text with newlines preserved.
                    // Split into one StyledLine per source line.
                    let mut first = true;
                    for line in text.split('\n') {
                        if !first {
                            // Flush current block and start a new one.
                            if let Some(b) = block.take() {
                                out.push(b.into_styled_line());
                            }
                        }
                        first = false;
                        let blk =
                            block.get_or_insert_with(|| Block::new(code_block_paragraph_style()));
                        blk.push_text(line, inline);
                    }
                } else {
                    let blk = block.get_or_insert_with(|| {
                        Block::new(paragraph_style_for(&list_stack, quote_depth, None))
                    });
                    blk.push_text(&text, inline);
                }
            }
            Event::Code(code) => {
                let inline_code = Inline {
                    code: true,
                    ..inline
                };
                let blk = block.get_or_insert_with(|| {
                    Block::new(paragraph_style_for(&list_stack, quote_depth, None))
                });
                blk.push_text(&code, inline_code);
            }
            Event::SoftBreak => {
                if let Some(blk) = block.as_mut() {
                    blk.push_text(" ", inline);
                }
            }
            Event::HardBreak => {
                // Flush the current line and start a new one with the
                // same paragraph style.
                if let Some(b) = block.take() {
                    let style = b.paragraph_style.clone();
                    out.push(b.into_styled_line());
                    block = Some(Block::new(style));
                }
            }
            Event::Rule => {
                // Flush any in-progress block first.
                if let Some(b) = block.take() {
                    out.push(b.into_styled_line());
                }
                out.push(StyledLine {
                    text: String::new(),
                    runs: Vec::new(),
                    paragraph_style: paragraph::Style::default(),
                });
            }
            Event::TaskListMarker(_)
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_)
            | Event::FootnoteReference(_) => {}
        }
    }

    // Flush any trailing block.
    if let Some(b) = block.take() {
        out.push(b.into_styled_line());
    }

    if out.is_empty() {
        out.push(StyledLine {
            text: String::new(),
            runs: Vec::new(),
            paragraph_style: paragraph::Style::default(),
        });
    }

    out
}

/// Build a `paragraph::Style` based on current block context.
fn paragraph_style_for(
    list_stack: &[ListKind],
    quote_depth: u8,
    _heading: Option<HeadingLevel>,
) -> paragraph::Style {
    let mut ps = paragraph::Style::default();
    if let Some(last) = list_stack.last() {
        let level = list_stack.len() as u8;
        ps.level = level;
        ps.list = Some(match last {
            ListKind::Bullet => List::Bullet(Bullet::Disc),
            ListKind::Ordered => List::Ordered(Number::Arabic),
        });
    }
    if quote_depth > 0 {
        ps.indent = Indent {
            left: f32::from(quote_depth) * 24.0,
            hanging: 0.0,
        };
    }
    ps
}

fn code_block_paragraph_style() -> paragraph::Style {
    paragraph::Style {
        style: span::Style {
            font: Some(monospace_font()),
            ..Default::default()
        },
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

fn serialize_markdown(lines: &[StyledLine]) -> String {
    let mut out = String::new();
    let mut prev_was_list = false;

    for (i, line) in lines.iter().enumerate() {
        let is_list = line.paragraph_style.list.is_some();

        // Blank line separator between paragraphs (but not between list items).
        if i > 0 && !(prev_was_list && is_list) {
            out.push('\n');
            if !(is_list || prev_was_list) {
                out.push('\n');
            }
        }

        serialize_line(&mut out, line);
        prev_was_list = is_list;
    }

    out
}

fn serialize_line(out: &mut String, line: &StyledLine) {
    // List prefix
    if let Some(list) = &line.paragraph_style.list {
        let level = line.paragraph_style.level.max(1) as usize;
        for _ in 0..(level.saturating_sub(1)) {
            out.push_str("  ");
        }
        match list {
            List::Bullet(_) => out.push_str("- "),
            List::Ordered(_) => out.push_str("1. "),
        }
    }

    // Heading prefix (detect from font size + bold on paragraph defaults).
    let size = line.paragraph_style.style.size;
    let is_bold_heading = line.paragraph_style.style.bold == Some(true) && size.is_some();
    if is_bold_heading && line.paragraph_style.list.is_none() {
        let level = heading_level_from_size(size.unwrap_or(16.0));
        for _ in 0..level {
            out.push('#');
        }
        out.push(' ');
    }

    // Emit runs with markdown delimiters. Skip styling that matches
    // paragraph-level defaults (it's already conveyed by the heading/list
    // prefix).
    let defaults = &line.paragraph_style.style;
    for run in &line.runs {
        let piece = &line.text[run.range.clone()];
        write_styled(out, piece, &run.style, defaults, is_bold_heading);
    }
    // If there were no runs, emit the raw text.
    if line.runs.is_empty() {
        out.push_str(&line.text);
    } else {
        // Emit any text past the last run.
        let last_end = line.runs.last().map(|r| r.range.end).unwrap_or(0);
        if last_end < line.text.len() {
            out.push_str(&line.text[last_end..]);
        }
    }
}

fn heading_level_from_size(size: f32) -> u8 {
    if size >= 30.0 {
        1
    } else if size >= 26.0 {
        2
    } else if size >= 22.0 {
        3
    } else if size >= 19.0 {
        4
    } else if size >= 17.0 {
        5
    } else {
        6
    }
}

fn write_styled(
    out: &mut String,
    text: &str,
    style: &span::Style,
    defaults: &span::Style,
    in_heading: bool,
) {
    // Bold: only emit delimiters if not already provided by heading.
    let bold = style.bold == Some(true) && !in_heading && defaults.bold != Some(true);
    let italic = style.italic == Some(true) && defaults.italic != Some(true);
    let strike = style.strikethrough == Some(true) && defaults.strikethrough != Some(true);
    let code = style.font.map(|f| f.family) == Some(font::Family::Monospace)
        && defaults.font.map(|f| f.family) != Some(font::Family::Monospace);

    if code {
        // Inline code is exclusive with other inline styles in markdown.
        let _ = write!(out, "`{text}`");
        return;
    }
    if strike {
        out.push_str("~~");
    }
    if bold {
        out.push_str("**");
    }
    if italic {
        out.push('*');
    }
    out.push_str(text);
    if italic {
        out.push('*');
    }
    if bold {
        out.push_str("**");
    }
    if strike {
        out.push_str("~~");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Vec<StyledLine> {
        parse_markdown(input)
    }

    #[test]
    fn plain_paragraph_becomes_single_line() {
        let lines = parse("hello world");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "hello world");
    }

    #[test]
    fn bold_run_has_bold_style() {
        let lines = parse("a **bold** word");
        assert_eq!(lines[0].text, "a bold word");
        let bold_run = lines[0]
            .runs
            .iter()
            .find(|r| r.style.bold == Some(true))
            .expect("expected bold run");
        assert_eq!(&lines[0].text[bold_run.range.clone()], "bold");
    }

    #[test]
    fn italic_and_strikethrough_runs() {
        let lines = parse("*italic* and ~~gone~~");
        let line = &lines[0];
        assert!(line.runs.iter().any(|r| r.style.italic == Some(true)));
        assert!(
            line.runs
                .iter()
                .any(|r| r.style.strikethrough == Some(true))
        );
    }

    #[test]
    fn heading_gets_large_size_and_bold() {
        let lines = parse("# Title");
        assert_eq!(lines[0].text, "Title");
        assert_eq!(lines[0].paragraph_style.style.bold, Some(true));
        assert_eq!(lines[0].paragraph_style.style.size, Some(32.0));
    }

    #[test]
    fn unordered_list_items_have_bullet_style() {
        let lines = parse("- one\n- two");
        assert_eq!(lines.len(), 2);
        for line in &lines {
            assert!(matches!(line.paragraph_style.list, Some(List::Bullet(_))));
            assert_eq!(line.paragraph_style.level, 1);
        }
    }

    #[test]
    fn ordered_list_items_have_ordered_style() {
        let lines = parse("1. first\n2. second");
        assert_eq!(lines.len(), 2);
        for line in &lines {
            assert!(matches!(line.paragraph_style.list, Some(List::Ordered(_))));
        }
    }

    #[test]
    fn inline_code_uses_monospace_font() {
        let lines = parse("use `foo` here");
        let code_run = lines[0]
            .runs
            .iter()
            .find(|r| r.style.font.is_some())
            .expect("expected code run");
        assert_eq!(&lines[0].text[code_run.range.clone()], "foo");
    }

    #[test]
    fn fenced_code_block_preserves_lines() {
        let lines = parse("```\nfn main() {}\nlet x = 1;\n```");
        let code_lines: Vec<&StyledLine> = lines
            .iter()
            .filter(|l| l.paragraph_style.style.font.is_some())
            .collect();
        assert!(code_lines.len() >= 2);
        assert!(code_lines[0].text.contains("fn main"));
    }

    #[test]
    fn multiple_paragraphs_separate_lines() {
        let lines = parse("first\n\nsecond");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "first");
        assert_eq!(lines[1].text, "second");
    }

    #[test]
    fn soft_break_becomes_space() {
        let lines = parse("one\ntwo");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "one two");
    }

    #[test]
    fn serialize_plain_text_roundtrips() {
        let lines = parse("hello world");
        let out = serialize_markdown(&lines);
        assert!(out.contains("hello world"));
    }

    #[test]
    fn serialize_bold_roundtrips() {
        let lines = parse("a **bold** word");
        let out = serialize_markdown(&lines);
        assert!(out.contains("**bold**"));
    }

    #[test]
    fn serialize_heading() {
        let lines = parse("# Title");
        let out = serialize_markdown(&lines);
        assert!(out.starts_with("# Title"));
    }

    #[test]
    fn serialize_bullet_list() {
        let lines = parse("- one\n- two");
        let out = serialize_markdown(&lines);
        assert!(out.contains("- one"));
        assert!(out.contains("- two"));
    }
}
