//! `.mr` serialization format for rich text documents.
//!
//! Styled text uses `{{attrs} text}` syntax. Paragraph properties use `>|key=val|`.
//! Unstyled text is identical to plain `.txt`.

use std::fmt::Write;

use iced_core::text::LineHeight;
use iced_core::text::rich_editor::{ParagraphStyle, Style};
use iced_core::{Color, Font, Pixels, font};

use crate::paragraph::{self, Bullet, List, Number, Spacing};
use crate::{StyleRun, StyledLine};

/// Serialize styled lines to `.mr` format.
pub fn serialize(lines: &[StyledLine]) -> String {
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let before = out.len();
        serialize_paragraph_header(&mut out, line);
        let has_header = out.len() > before;
        serialize_line_content(&mut out, line, !has_header);
    }
    out
}

/// Parse `.mr` format into styled lines.
pub fn parse(input: &str) -> Result<Vec<StyledLine>, ParseError> {
    let mut lines = Vec::new();
    let mut raw_lines = input.split('\n').peekable();

    while let Some(raw) = raw_lines.next() {
        if let Some(props) = raw.strip_prefix(">|") {
            // Paragraph property line — content is on the next line
            let (paragraph_style, paragraph) = parse_paragraph_header(props)?;
            let content_line = raw_lines.next().unwrap_or("");
            let (text, runs) = parse_line_content(content_line)?;
            lines.push(StyledLine {
                text,
                runs,
                paragraph_style,
                paragraph,
            });
        } else {
            let (text, runs) = parse_line_content(raw)?;
            lines.push(StyledLine {
                text,
                runs,
                paragraph_style: ParagraphStyle::default(),
                paragraph: paragraph::Style::default(),
            });
        }
    }

    // Ensure at least one line
    if lines.is_empty() {
        lines.push(StyledLine {
            text: String::new(),
            runs: Vec::new(),
            paragraph_style: ParagraphStyle::default(),
            paragraph: paragraph::Style::default(),
        });
    }

    Ok(lines)
}

/// A parse error with position information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error at offset {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for ParseError {}

// ---------------------------------------------------------------------------
// Serializer internals
// ---------------------------------------------------------------------------

fn serialize_paragraph_header(out: &mut String, line: &StyledLine) {
    let para = &line.paragraph;
    let ps = &line.paragraph_style;

    let mut props = Vec::new();

    // Alignment (from ParagraphStyle)
    if let Some(align) = ps.alignment {
        let s = match align {
            iced_core::text::Alignment::Left | iced_core::text::Alignment::Default => "left",
            iced_core::text::Alignment::Center => "center",
            iced_core::text::Alignment::Right => "right",
            iced_core::text::Alignment::Justified => "justify",
        };
        // Only emit if non-default
        if !matches!(
            align,
            iced_core::text::Alignment::Left | iced_core::text::Alignment::Default
        ) {
            props.push(format!("align={s}"));
        }
    }

    // Line height (from ParagraphStyle)
    if let Some(lh) = ps.line_height {
        match lh {
            LineHeight::Relative(r) => props.push(format!("lh={}", format_float(r))),
            LineHeight::Absolute(px) => props.push(format!("lh={}px", format_float(px.0))),
        }
    }

    // Line spacing (from paragraph::Style)
    if let Some(spacing) = &para.line_spacing {
        match spacing {
            Spacing::Multiple(m) => props.push(format!("ls={}x", format_float(*m))),
            Spacing::Exact(px) => props.push(format!("ls={}px", format_float(*px))),
        }
    }

    // Space before/after
    if let Some(sb) = para.space_before {
        props.push(format!("sb={}", format_float(sb)));
    }
    if let Some(sa) = para.space_after {
        props.push(format!("sa={}", format_float(sa)));
    }
    // Also check ParagraphStyle.spacing_after
    if let Some(sa) = ps.spacing_after
        && para.space_after.is_none()
    {
        props.push(format!("sa={}", format_float(sa)));
    }

    // Level
    if para.level > 0 {
        props.push(format!("level={}", para.level));
    }

    // List
    if let Some(list) = &para.list {
        let s = match list {
            List::Bullet(Bullet::Disc) => "bullet",
            List::Bullet(Bullet::Circle) => "circle",
            List::Bullet(Bullet::Square) => "square",
            List::Ordered(Number::Arabic) => "1",
            List::Ordered(Number::LowerAlpha) => "a",
            List::Ordered(Number::UpperAlpha) => "A",
            List::Ordered(Number::LowerRoman) => "i",
            List::Ordered(Number::UpperRoman) => "I",
            _ => "bullet",
        };
        props.push(format!("list={s}"));
    }

    // Paragraph character defaults (d: prefix)
    serialize_default_style_attrs(&mut props, &ps.style);

    if !props.is_empty() {
        writeln!(out, ">|{}|", props.join(" ")).unwrap();
    }
}

/// Serialize paragraph character defaults with `d:` prefix.
fn serialize_default_style_attrs(props: &mut Vec<String>, style: &Style) {
    if style.bold == Some(true) {
        props.push("d:b".to_string());
    }
    if style.italic == Some(true) {
        props.push("d:i".to_string());
    }
    if style.underline == Some(true) {
        props.push("d:u".to_string());
    }
    if style.strikethrough == Some(true) {
        props.push("d:s".to_string());
    }
    if let Some(font) = style.font
        && let font::Family::Name(name) = font.family
    {
        props.push(format!("d:f={}", name.replace(' ', "_")));
    }
    if let Some(sz) = style.size {
        props.push(format!("d:sz={}", format_float(sz)));
    }
    if let Some(color) = style.color {
        props.push(format!("d:c={}", format_color(color)));
    }
    if let Some(sp) = style.letter_spacing {
        props.push(format!("d:sp={}", format_float(sp)));
    }
}

fn serialize_line_content(out: &mut String, line: &StyledLine, escape_prop_prefix: bool) {
    let text = &line.text;
    let runs = &line.runs;

    // Escape >| at line start when there's no paragraph header above
    if escape_prop_prefix && text.starts_with(">|") {
        out.push('\\');
    }

    if runs.is_empty() {
        escape_text(out, text);
        return;
    }

    // Walk through the text, emitting styled spans
    let mut pos = 0;
    for run in runs {
        // Gap before this run (shouldn't happen with well-formed runs, but be safe)
        if run.range.start > pos {
            escape_text(out, &text[pos..run.range.start]);
        }

        let run_text = &text[run.range.clone()];
        let attrs = style_to_attrs(&run.style);

        if attrs.is_empty() {
            escape_text(out, run_text);
        } else {
            out.push_str("{{");
            out.push_str(&attrs);
            out.push_str("} ");
            escape_text(out, run_text);
            out.push('}');
        }

        pos = run.range.end;
    }

    // Trailing text after last run
    if pos < text.len() {
        escape_text(out, &text[pos..]);
    }
}

/// Convert a Style to space-separated attribute tokens.
fn style_to_attrs(style: &Style) -> String {
    let mut tokens = Vec::new();

    if style.bold == Some(true) {
        tokens.push("b".to_string());
    }
    if style.italic == Some(true) {
        tokens.push("i".to_string());
    }
    if style.underline == Some(true) {
        tokens.push("u".to_string());
    }
    if style.strikethrough == Some(true) {
        tokens.push("s".to_string());
    }
    if let Some(font) = style.font
        && let font::Family::Name(name) = font.family
    {
        tokens.push(format!("f={}", name.replace(' ', "_")));
    }
    if let Some(sz) = style.size {
        tokens.push(format!("sz={}", format_float(sz)));
    }
    if let Some(color) = style.color {
        tokens.push(format!("c={}", format_color(color)));
    }
    if let Some(sp) = style.letter_spacing {
        tokens.push(format!("sp={}", format_float(sp)));
    }

    tokens.join(" ")
}

fn escape_text(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
}

fn format_color(c: Color) -> String {
    let r = (c.r * 255.0).round() as u8;
    let g = (c.g * 255.0).round() as u8;
    let b = (c.b * 255.0).round() as u8;
    if c.a < 1.0 {
        let a = (c.a * 255.0).round() as u8;
        format!("{r:02x}{g:02x}{b:02x}{a:02x}")
    } else {
        format!("{r:02x}{g:02x}{b:02x}")
    }
}

fn format_float(v: f32) -> String {
    // Remove trailing zeros: 1.50 → 1.5, 24.0 → 24
    let s = format!("{v}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// Parser internals
// ---------------------------------------------------------------------------

fn parse_paragraph_header(header: &str) -> Result<(ParagraphStyle, paragraph::Style), ParseError> {
    // Strip trailing `|`
    let content = header
        .strip_suffix('|')
        .ok_or_else(|| ParseError {
            message: "paragraph header missing closing '|'".into(),
            offset: 0,
        })?
        .trim();

    let mut ps = ParagraphStyle::default();
    let mut para = paragraph::Style::default();

    for token in content.split_whitespace() {
        if let Some(rest) = token.strip_prefix("d:") {
            // Paragraph character default
            parse_default_attr(rest, &mut ps.style)?;
        } else if let Some(val) = token.strip_prefix("align=") {
            ps.alignment = Some(match val {
                "left" => iced_core::text::Alignment::Left,
                "center" => iced_core::text::Alignment::Center,
                "right" => iced_core::text::Alignment::Right,
                "justify" => iced_core::text::Alignment::Justified,
                _ => {
                    return Err(ParseError {
                        message: format!("unknown alignment: {val}"),
                        offset: 0,
                    });
                }
            });
        } else if let Some(val) = token.strip_prefix("lh=") {
            ps.line_height = Some(parse_line_height(val)?);
        } else if let Some(val) = token.strip_prefix("ls=") {
            para.line_spacing = Some(parse_line_spacing(val)?);
        } else if let Some(val) = token.strip_prefix("sb=") {
            para.space_before = Some(parse_f32(val)?);
        } else if let Some(val) = token.strip_prefix("sa=") {
            para.space_after = Some(parse_f32(val)?);
        } else if let Some(val) = token.strip_prefix("level=") {
            para.level = val.parse::<u8>().map_err(|_| ParseError {
                message: format!("invalid level: {val}"),
                offset: 0,
            })?;
        } else if let Some(val) = token.strip_prefix("list=") {
            para.list = Some(parse_list(val)?);
        } else {
            return Err(ParseError {
                message: format!("unknown paragraph property: {token}"),
                offset: 0,
            });
        }
    }

    Ok((ps, para))
}

fn parse_default_attr(attr: &str, style: &mut Style) -> Result<(), ParseError> {
    match attr {
        "b" => style.bold = Some(true),
        "i" => style.italic = Some(true),
        "u" => style.underline = Some(true),
        "s" => style.strikethrough = Some(true),
        _ if attr.starts_with("f=") => {
            let name = &attr[2..];
            style.font = Some(make_font(name));
        }
        _ if attr.starts_with("sz=") => {
            style.size = Some(parse_f32(&attr[3..])?);
        }
        _ if attr.starts_with("c=") => {
            style.color = Some(parse_color(&attr[2..])?);
        }
        _ if attr.starts_with("sp=") => {
            style.letter_spacing = Some(parse_f32(&attr[3..])?);
        }
        _ => {
            return Err(ParseError {
                message: format!("unknown default attribute: d:{attr}"),
                offset: 0,
            });
        }
    }
    Ok(())
}

fn parse_line_height(val: &str) -> Result<LineHeight, ParseError> {
    if let Some(px) = val.strip_suffix("px") {
        Ok(LineHeight::Absolute(Pixels(parse_f32(px)?)))
    } else {
        Ok(LineHeight::Relative(parse_f32(val)?))
    }
}

fn parse_line_spacing(val: &str) -> Result<Spacing, ParseError> {
    if let Some(px) = val.strip_suffix("px") {
        Ok(Spacing::Exact(parse_f32(px)?))
    } else if let Some(mult) = val.strip_suffix('x') {
        Ok(Spacing::Multiple(parse_f32(mult)?))
    } else {
        Err(ParseError {
            message: format!("line spacing needs 'x' or 'px' suffix: {val}"),
            offset: 0,
        })
    }
}

fn parse_list(val: &str) -> Result<List, ParseError> {
    match val {
        "bullet" => Ok(List::Bullet(Bullet::Disc)),
        "circle" => Ok(List::Bullet(Bullet::Circle)),
        "square" => Ok(List::Bullet(Bullet::Square)),
        "1" => Ok(List::Ordered(Number::Arabic)),
        "a" => Ok(List::Ordered(Number::LowerAlpha)),
        "A" => Ok(List::Ordered(Number::UpperAlpha)),
        "i" => Ok(List::Ordered(Number::LowerRoman)),
        "I" => Ok(List::Ordered(Number::UpperRoman)),
        _ => Err(ParseError {
            message: format!("unknown list style: {val}"),
            offset: 0,
        }),
    }
}

fn parse_f32(val: &str) -> Result<f32, ParseError> {
    val.parse::<f32>().map_err(|_| ParseError {
        message: format!("invalid number: {val}"),
        offset: 0,
    })
}

fn parse_color(hex: &str) -> Result<Color, ParseError> {
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| ParseError {
                message: format!("invalid hex color: {hex}"),
                offset: 0,
            })
        })
        .collect::<Result<_, _>>()?;

    match bytes.len() {
        3 => Ok(Color::from_rgb8(bytes[0], bytes[1], bytes[2])),
        4 => Ok(Color::from_rgba8(
            bytes[0],
            bytes[1],
            bytes[2],
            f32::from(bytes[3]) / 255.0,
        )),
        _ => Err(ParseError {
            message: format!("hex color must be 6 or 8 chars: {hex}"),
            offset: 0,
        }),
    }
}

/// Create a `Font` from a name (underscores → spaces), leaking the string for `'static`.
fn make_font(name: &str) -> Font {
    let family_name: &'static str = Box::leak(name.replace('_', " ").into_boxed_str());
    Font {
        family: font::Family::Name(family_name),
        ..Font::DEFAULT
    }
}

/// Parse inline content: `{attrs text}` spans and escaped characters.
fn parse_line_content(input: &str) -> Result<(String, Vec<StyleRun>), ParseError> {
    // Handle escaped >| at line start
    let input = if let Some(rest) = input.strip_prefix("\\>|") {
        // Put back the >| without escape
        let mut s = String::with_capacity(rest.len() + 2);
        s.push_str(">|");
        s.push_str(rest);
        return parse_inline(&s);
    } else {
        input
    };
    parse_inline(input)
}

/// Core inline parser. Processes `{{attrs} text}` spans with nesting.
fn parse_inline(input: &str) -> Result<(String, Vec<StyleRun>), ParseError> {
    let mut text = String::new();
    let mut runs: Vec<StyleRun> = Vec::new();
    let mut style_stack: Vec<(Style, usize)> = Vec::new(); // (style, text_start)
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '\\' => {
                // Escape sequence
                i += 1;
                if i < chars.len() {
                    text.push(chars[i]);
                    i += 1;
                }
            }
            '{' if i + 1 < chars.len() && chars[i + 1] == '{' => {
                // Styled span: {{attrs} text}
                i += 2; // skip {{
                let (style, consumed) = parse_bracketed_attrs(&chars, i)?;
                i += consumed; // now past the closing }
                // Skip optional space after attr block
                if i < chars.len() && chars[i] == ' ' {
                    i += 1;
                }

                // The current style is the combination of parent styles + this span
                let parent_style = style_stack
                    .last()
                    .map(|(s, _)| s.clone())
                    .unwrap_or_default();
                let merged = merge_styles(&parent_style, &style);
                style_stack.push((merged, text.len()));
            }
            '}' if !style_stack.is_empty() => {
                // Close the innermost styled span
                let (style, start) = style_stack.pop().unwrap();
                let end = text.len();
                if end > start {
                    runs.push(StyleRun {
                        range: start..end,
                        style,
                    });
                }
                i += 1;
            }
            _ => {
                // All other characters (including bare { and }) are literal text
                text.push(chars[i]);
                i += 1;
            }
        }
    }

    // If there's text not covered by any run, fill gaps with default style
    let runs = fill_gaps(runs, text.len());

    Ok((text, runs))
}

/// Parse attributes inside `{...}` (the inner braces of `{{attrs} text}`).
/// Returns (style, chars consumed including the closing `}`).
fn parse_bracketed_attrs(chars: &[char], start: usize) -> Result<(Style, usize), ParseError> {
    let mut style = Style::default();
    let mut i = start;

    loop {
        // Skip spaces
        while i < chars.len() && chars[i] == ' ' {
            i += 1;
        }

        if i >= chars.len() {
            return Err(ParseError {
                message: "unclosed attribute block".into(),
                offset: start,
            });
        }

        if chars[i] == '}' {
            i += 1; // consume closing }
            return Ok((style, i - start));
        }

        // Read the next token (up to space or })
        let token_start = i;
        while i < chars.len() && chars[i] != ' ' && chars[i] != '}' {
            i += 1;
        }
        let token: String = chars[token_start..i].iter().collect();

        if let Some(attr) = try_parse_attr(&token) {
            apply_attr(&mut style, attr);
        } else {
            return Err(ParseError {
                message: format!("unknown attribute in span: {token}"),
                offset: token_start,
            });
        }
    }
}

/// Try to parse a single attribute token. Returns None if not recognized.
fn try_parse_attr(token: &str) -> Option<AttrToken> {
    match token {
        "b" => Some(AttrToken::Bold),
        "i" => Some(AttrToken::Italic),
        "u" => Some(AttrToken::Underline),
        "s" => Some(AttrToken::Strikethrough),
        _ => {
            if let Some(val) = token.strip_prefix("f=") {
                Some(AttrToken::Font(val.to_string()))
            } else if let Some(val) = token.strip_prefix("sz=") {
                val.parse::<f32>().ok().map(AttrToken::Size)
            } else if let Some(val) = token.strip_prefix("c=") {
                parse_color(val).ok().map(AttrToken::Color)
            } else if let Some(val) = token.strip_prefix("sp=") {
                val.parse::<f32>().ok().map(AttrToken::LetterSpacing)
            } else {
                None
            }
        }
    }
}

enum AttrToken {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Font(String),
    Size(f32),
    Color(Color),
    LetterSpacing(f32),
}

fn apply_attr(style: &mut Style, attr: AttrToken) {
    match attr {
        AttrToken::Bold => style.bold = Some(true),
        AttrToken::Italic => style.italic = Some(true),
        AttrToken::Underline => style.underline = Some(true),
        AttrToken::Strikethrough => style.strikethrough = Some(true),
        AttrToken::Font(name) => style.font = Some(make_font(&name)),
        AttrToken::Size(sz) => style.size = Some(sz),
        AttrToken::Color(c) => style.color = Some(c),
        AttrToken::LetterSpacing(sp) => style.letter_spacing = Some(sp),
    }
}

/// Merge child style onto parent: child values override parent values.
fn merge_styles(parent: &Style, child: &Style) -> Style {
    Style {
        bold: child.bold.or(parent.bold),
        italic: child.italic.or(parent.italic),
        underline: child.underline.or(parent.underline),
        strikethrough: child.strikethrough.or(parent.strikethrough),
        font: child.font.or(parent.font),
        size: child.size.or(parent.size),
        color: child.color.or(parent.color),
        letter_spacing: child.letter_spacing.or(parent.letter_spacing),
    }
}

/// Fill gaps in runs with default-styled runs so runs cover 0..len.
fn fill_gaps(mut runs: Vec<StyleRun>, len: usize) -> Vec<StyleRun> {
    if len == 0 {
        return Vec::new();
    }

    // Sort by start position
    runs.sort_by_key(|r| r.range.start);

    let mut filled = Vec::new();
    let mut pos = 0;

    for run in runs {
        if run.range.start > pos {
            filled.push(StyleRun {
                range: pos..run.range.start,
                style: Style::default(),
            });
        }
        pos = run.range.end;
        filled.push(run);
    }

    if pos < len {
        filled.push(StyleRun {
            range: pos..len,
            style: Style::default(),
        });
    }

    filled
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_line(text: &str) -> StyledLine {
        let runs = if text.is_empty() {
            Vec::new()
        } else {
            vec![StyleRun {
                range: 0..text.len(),
                style: Style::default(),
            }]
        };
        StyledLine {
            text: text.to_string(),
            runs,
            paragraph_style: ParagraphStyle::default(),
            paragraph: paragraph::Style::default(),
        }
    }

    fn assert_round_trip(lines: &[StyledLine]) {
        let serialized = serialize(lines);
        let parsed = parse(&serialized).expect("parse failed");
        assert_eq!(
            parsed.len(),
            lines.len(),
            "line count mismatch.\nSerialized:\n{serialized}"
        );
        for (i, (orig, rt)) in lines.iter().zip(parsed.iter()).enumerate() {
            assert_eq!(orig.text, rt.text, "text mismatch on line {i}");
            assert_eq!(
                orig.runs.len(),
                rt.runs.len(),
                "run count mismatch on line {i}.\nOrig runs: {:?}\nParsed runs: {:?}\nSerialized:\n{serialized}",
                orig.runs,
                rt.runs,
            );
            for (j, (or, pr)) in orig.runs.iter().zip(rt.runs.iter()).enumerate() {
                assert_eq!(or.range, pr.range, "run {j} range mismatch on line {i}");
                assert_styles_eq(&or.style, &pr.style, i, j);
            }
            assert_eq!(
                orig.paragraph, rt.paragraph,
                "paragraph style mismatch on line {i}"
            );
            assert_eq!(
                orig.paragraph_style.alignment, rt.paragraph_style.alignment,
                "alignment mismatch on line {i}"
            );
            assert_eq!(
                orig.paragraph_style.line_height, rt.paragraph_style.line_height,
                "line_height mismatch on line {i}"
            );
        }
    }

    fn assert_styles_eq(a: &Style, b: &Style, line: usize, run: usize) {
        assert_eq!(a.bold, b.bold, "bold mismatch at line {line} run {run}");
        assert_eq!(
            a.italic, b.italic,
            "italic mismatch at line {line} run {run}"
        );
        assert_eq!(
            a.underline, b.underline,
            "underline mismatch at line {line} run {run}"
        );
        assert_eq!(
            a.strikethrough, b.strikethrough,
            "strikethrough mismatch at line {line} run {run}"
        );
        assert_eq!(a.font, b.font, "font mismatch at line {line} run {run}");
        assert_eq!(a.size, b.size, "size mismatch at line {line} run {run}");
        assert_eq!(
            a.letter_spacing, b.letter_spacing,
            "letter_spacing mismatch at line {line} run {run}"
        );
        // Compare colors with tolerance for float rounding
        match (a.color, b.color) {
            (Some(ca), Some(cb)) => {
                assert!(
                    (ca.r - cb.r).abs() < 0.01
                        && (ca.g - cb.g).abs() < 0.01
                        && (ca.b - cb.b).abs() < 0.01
                        && (ca.a - cb.a).abs() < 0.01,
                    "color mismatch at line {line} run {run}: {ca:?} vs {cb:?}"
                );
            }
            (None, None) => {}
            _ => panic!("color presence mismatch at line {line} run {run}"),
        }
    }

    #[test]
    fn plain_text_round_trips() {
        let lines = vec![default_line("Hello, world!"), default_line("Second line.")];
        assert_round_trip(&lines);

        // Verify it looks like plain text
        let s = serialize(&lines);
        assert_eq!(s, "Hello, world!\nSecond line.");
    }

    #[test]
    fn bold_italic_underline_strikethrough() {
        let lines = vec![StyledLine {
            text: "bold italic underline strike".to_string(),
            runs: vec![
                StyleRun {
                    range: 0..4,
                    style: Style {
                        bold: Some(true),
                        ..Style::default()
                    },
                },
                StyleRun {
                    range: 4..5,
                    style: Style::default(),
                },
                StyleRun {
                    range: 5..11,
                    style: Style {
                        italic: Some(true),
                        ..Style::default()
                    },
                },
                StyleRun {
                    range: 11..12,
                    style: Style::default(),
                },
                StyleRun {
                    range: 12..21,
                    style: Style {
                        underline: Some(true),
                        ..Style::default()
                    },
                },
                StyleRun {
                    range: 21..22,
                    style: Style::default(),
                },
                StyleRun {
                    range: 22..28,
                    style: Style {
                        strikethrough: Some(true),
                        ..Style::default()
                    },
                },
            ],
            paragraph_style: ParagraphStyle::default(),
            paragraph: paragraph::Style::default(),
        }];
        assert_round_trip(&lines);
    }

    #[test]
    fn color_font_size_letter_spacing() {
        let lines = vec![StyledLine {
            text: "styled".to_string(),
            runs: vec![StyleRun {
                range: 0..6,
                style: Style {
                    color: Some(Color::from_rgb8(255, 0, 0)),
                    font: Some(Font {
                        family: font::Family::Name(Box::leak(
                            "Georgia".to_string().into_boxed_str(),
                        )),
                        ..Font::DEFAULT
                    }),
                    size: Some(20.0),
                    letter_spacing: Some(2.0),
                    ..Style::default()
                },
            }],
            paragraph_style: ParagraphStyle::default(),
            paragraph: paragraph::Style::default(),
        }];
        assert_round_trip(&lines);

        let s = serialize(&lines);
        assert_eq!(s, "{{f=Georgia sz=20 c=ff0000 sp=2} styled}");
    }

    #[test]
    fn nested_styles() {
        // "bold bold-italic bold" — uses nesting
        let input = "{{b} bold {{i} bold-italic} bold}";
        let (text, runs) = parse_inline(input).unwrap();
        assert_eq!(text, "bold bold-italic bold");

        // Check that "bold-italic" portion has both bold and italic
        let bi_run = runs
            .iter()
            .find(|r| r.range.start == 5 && r.range.end == 16);
        assert!(bi_run.is_some(), "expected bold-italic run");
        let bi = bi_run.unwrap();
        assert_eq!(bi.style.bold, Some(true));
        assert_eq!(bi.style.italic, Some(true));
    }

    #[test]
    fn paragraph_alignment() {
        let lines = vec![StyledLine {
            text: "centered".to_string(),
            runs: vec![StyleRun {
                range: 0..8,
                style: Style::default(),
            }],
            paragraph_style: ParagraphStyle {
                alignment: Some(iced_core::text::Alignment::Center),
                ..ParagraphStyle::default()
            },
            paragraph: paragraph::Style::default(),
        }];
        assert_round_trip(&lines);

        let s = serialize(&lines);
        assert!(s.starts_with(">|align=center|"));
    }

    #[test]
    fn paragraph_line_height_relative_and_absolute() {
        let lines = vec![
            StyledLine {
                text: "relative".to_string(),
                runs: vec![StyleRun {
                    range: 0..8,
                    style: Style::default(),
                }],
                paragraph_style: ParagraphStyle {
                    line_height: Some(LineHeight::Relative(1.5)),
                    ..ParagraphStyle::default()
                },
                paragraph: paragraph::Style::default(),
            },
            StyledLine {
                text: "absolute".to_string(),
                runs: vec![StyleRun {
                    range: 0..8,
                    style: Style::default(),
                }],
                paragraph_style: ParagraphStyle {
                    line_height: Some(LineHeight::Absolute(Pixels(24.0))),
                    ..ParagraphStyle::default()
                },
                paragraph: paragraph::Style::default(),
            },
        ];
        assert_round_trip(&lines);

        let s = serialize(&lines);
        assert!(s.contains("lh=1.5"));
        assert!(s.contains("lh=24px"));
    }

    #[test]
    fn paragraph_list_bullet_and_ordered() {
        let lines = vec![
            StyledLine {
                text: "bullet item".to_string(),
                runs: vec![StyleRun {
                    range: 0..11,
                    style: Style::default(),
                }],
                paragraph_style: ParagraphStyle::default(),
                paragraph: paragraph::Style {
                    list: Some(List::Bullet(Bullet::Disc)),
                    level: 1,
                    ..Default::default()
                },
            },
            StyledLine {
                text: "numbered item".to_string(),
                runs: vec![StyleRun {
                    range: 0..13,
                    style: Style::default(),
                }],
                paragraph_style: ParagraphStyle::default(),
                paragraph: paragraph::Style {
                    list: Some(List::Ordered(Number::Arabic)),
                    level: 1,
                    ..Default::default()
                },
            },
        ];
        assert_round_trip(&lines);
    }

    #[test]
    fn paragraph_list_nesting() {
        let lines = vec![
            StyledLine {
                text: "level 1".to_string(),
                runs: vec![StyleRun {
                    range: 0..7,
                    style: Style::default(),
                }],
                paragraph_style: ParagraphStyle::default(),
                paragraph: paragraph::Style {
                    list: Some(List::Bullet(Bullet::Disc)),
                    level: 1,
                    ..Default::default()
                },
            },
            StyledLine {
                text: "level 2".to_string(),
                runs: vec![StyleRun {
                    range: 0..7,
                    style: Style::default(),
                }],
                paragraph_style: ParagraphStyle::default(),
                paragraph: paragraph::Style {
                    list: Some(List::Bullet(Bullet::Circle)),
                    level: 2,
                    ..Default::default()
                },
            },
        ];
        assert_round_trip(&lines);

        let s = serialize(&lines);
        assert!(s.contains("level=1"));
        assert!(s.contains("level=2"));
    }

    #[test]
    fn escaping_braces() {
        let lines = vec![default_line("use {braces} and \\backslash")];
        assert_round_trip(&lines);

        let s = serialize(&lines);
        assert_eq!(s, "use \\{braces\\} and \\\\backslash");
    }

    #[test]
    fn escaping_property_line_prefix() {
        let lines = vec![default_line(">|this looks like a prop line|")];
        assert_round_trip(&lines);

        let s = serialize(&lines);
        assert!(s.starts_with("\\>|"));
    }

    #[test]
    fn empty_paragraphs() {
        let lines = vec![
            default_line("before"),
            default_line(""),
            default_line("after"),
        ];
        assert_round_trip(&lines);

        let s = serialize(&lines);
        assert_eq!(s, "before\n\nafter");
    }

    #[test]
    fn complex_mixed_document() {
        let lines = vec![
            // Heading: centered, bold, 24px
            StyledLine {
                text: "My Document".to_string(),
                runs: vec![StyleRun {
                    range: 0..11,
                    style: Style {
                        bold: Some(true),
                        size: Some(24.0),
                        ..Style::default()
                    },
                }],
                paragraph_style: ParagraphStyle {
                    alignment: Some(iced_core::text::Alignment::Center),
                    ..ParagraphStyle::default()
                },
                paragraph: paragraph::Style::default(),
            },
            // Empty line
            StyledLine {
                text: String::new(),
                runs: Vec::new(),
                paragraph_style: ParagraphStyle::default(),
                paragraph: paragraph::Style::default(),
            },
            // Body with mixed styles
            StyledLine {
                text: "Normal bold red end".to_string(),
                runs: vec![
                    StyleRun {
                        range: 0..7,
                        style: Style::default(),
                    },
                    StyleRun {
                        range: 7..11,
                        style: Style {
                            bold: Some(true),
                            ..Style::default()
                        },
                    },
                    StyleRun {
                        range: 11..12,
                        style: Style::default(),
                    },
                    StyleRun {
                        range: 12..15,
                        style: Style {
                            color: Some(Color::from_rgb8(255, 0, 0)),
                            ..Style::default()
                        },
                    },
                    StyleRun {
                        range: 15..19,
                        style: Style::default(),
                    },
                ],
                paragraph_style: ParagraphStyle::default(),
                paragraph: paragraph::Style::default(),
            },
            // Bullet list
            StyledLine {
                text: "First bullet".to_string(),
                runs: vec![StyleRun {
                    range: 0..12,
                    style: Style::default(),
                }],
                paragraph_style: ParagraphStyle::default(),
                paragraph: paragraph::Style {
                    list: Some(List::Bullet(Bullet::Disc)),
                    level: 1,
                    ..Default::default()
                },
            },
        ];
        assert_round_trip(&lines);
    }

    #[test]
    fn paragraph_character_defaults() {
        let lines = vec![StyledLine {
            text: "heading text".to_string(),
            runs: vec![StyleRun {
                range: 0..12,
                style: Style::default(),
            }],
            paragraph_style: ParagraphStyle {
                style: Style {
                    bold: Some(true),
                    size: Some(24.0),
                    ..Style::default()
                },
                ..ParagraphStyle::default()
            },
            paragraph: paragraph::Style::default(),
        }];
        assert_round_trip(&lines);

        let s = serialize(&lines);
        assert!(s.contains("d:b"));
        assert!(s.contains("d:sz=24"));
    }

    #[test]
    fn parse_plain_text() {
        let input = "Hello\nWorld";
        let lines = parse(input).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "Hello");
        assert_eq!(lines[1].text, "World");
    }

    #[test]
    fn parse_empty_input() {
        let lines = parse("").unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "");
    }

    #[test]
    fn parse_styled_span() {
        let input = "before {{b} bold} after";
        let lines = parse(input).unwrap();
        assert_eq!(lines[0].text, "before bold after");
        // Find the bold run
        let bold_run = lines[0].runs.iter().find(|r| r.style.bold == Some(true));
        assert!(bold_run.is_some());
        let br = bold_run.unwrap();
        assert_eq!(br.range, 7..11);
    }

    #[test]
    fn serialize_line_starting_with_property_prefix() {
        // A line whose text starts with >| must be escaped
        let line = StyledLine {
            text: ">|not a property|".to_string(),
            runs: vec![StyleRun {
                range: 0..17,
                style: Style::default(),
            }],
            paragraph_style: ParagraphStyle::default(),
            paragraph: paragraph::Style::default(),
        };
        let s = serialize(&[line]);
        assert!(s.starts_with("\\>|"), "expected escaped prefix, got: {s}");
    }

    #[test]
    fn font_with_spaces() {
        let lines = vec![StyledLine {
            text: "text".to_string(),
            runs: vec![StyleRun {
                range: 0..4,
                style: Style {
                    font: Some(Font {
                        family: font::Family::Name(Box::leak(
                            "IBM Plex Sans".to_string().into_boxed_str(),
                        )),
                        ..Font::DEFAULT
                    }),
                    ..Style::default()
                },
            }],
            paragraph_style: ParagraphStyle::default(),
            paragraph: paragraph::Style::default(),
        }];

        let s = serialize(&lines);
        assert_eq!(s, "{{f=IBM_Plex_Sans} text}");

        // Round-trip: font name should match
        let parsed = parse(&s).unwrap();
        assert_eq!(
            parsed[0].runs[0].style.font.unwrap().family,
            font::Family::Name("IBM Plex Sans")
        );
    }

    #[test]
    fn paragraph_space_before_after() {
        let lines = vec![StyledLine {
            text: "spaced".to_string(),
            runs: vec![StyleRun {
                range: 0..6,
                style: Style::default(),
            }],
            paragraph_style: ParagraphStyle::default(),
            paragraph: paragraph::Style {
                space_before: Some(12.0),
                space_after: Some(8.0),
                ..Default::default()
            },
        }];
        assert_round_trip(&lines);

        let s = serialize(&lines);
        assert!(s.contains("sb=12"));
        assert!(s.contains("sa=8"));
    }

    #[test]
    fn paragraph_line_spacing() {
        let lines = vec![
            StyledLine {
                text: "multiple".to_string(),
                runs: vec![StyleRun {
                    range: 0..8,
                    style: Style::default(),
                }],
                paragraph_style: ParagraphStyle::default(),
                paragraph: paragraph::Style {
                    line_spacing: Some(Spacing::Multiple(1.5)),
                    ..Default::default()
                },
            },
            StyledLine {
                text: "exact".to_string(),
                runs: vec![StyleRun {
                    range: 0..5,
                    style: Style::default(),
                }],
                paragraph_style: ParagraphStyle::default(),
                paragraph: paragraph::Style {
                    line_spacing: Some(Spacing::Exact(18.0)),
                    ..Default::default()
                },
            },
        ];
        assert_round_trip(&lines);

        let s = serialize(&lines);
        assert!(s.contains("ls=1.5x"));
        assert!(s.contains("ls=18px"));
    }

    #[test]
    fn bare_braces_are_literal() {
        // A single { or } not part of {{attrs} text} should be literal text
        let input = "hello { world } end";
        let (text, _runs) = parse_inline(input).unwrap();
        assert_eq!(text, "hello { world } end");
    }

    #[test]
    fn ambiguous_attr_like_text_preserved() {
        // The motivating case: bold text "i foo" must not become italic on round-trip.
        // With {{attrs} text}, the serializer emits {{b} i foo} — the "i" is clearly
        // text, not an attribute, because it's outside the inner {}.
        let lines = vec![StyledLine {
            text: "i foo".to_string(),
            runs: vec![StyleRun {
                range: 0..5,
                style: Style {
                    bold: Some(true),
                    ..Style::default()
                },
            }],
            paragraph_style: ParagraphStyle::default(),
            paragraph: paragraph::Style::default(),
        }];

        let s = serialize(&lines);
        assert_eq!(s, "{{b} i foo}");

        // Round-trip must preserve bold-only (no italic)
        let parsed = parse(&s).unwrap();
        assert_eq!(parsed[0].text, "i foo");
        let bold_run = &parsed[0].runs[0];
        assert_eq!(bold_run.style.bold, Some(true));
        assert_eq!(bold_run.style.italic, None);
    }

    #[test]
    fn attr_like_tokens_in_text_round_trip() {
        // Text containing things that look like attrs: "b", "sz=20", "f=Arial"
        let lines = vec![default_line("set b to true and sz=20 for f=Arial")];
        assert_round_trip(&lines);

        // No styling should be applied — these are plain text
        let s = serialize(&lines);
        assert_eq!(s, "set b to true and sz=20 for f=Arial");
    }

    #[test]
    fn unknown_attr_in_span_is_error() {
        // Inside {{...}}, unknown tokens are errors (not silently treated as text)
        let input = "{{b bogus} text}";
        let result = parse_inline(input);
        assert!(result.is_err());
    }

    #[test]
    fn empty_attr_block() {
        // {{} text} — empty attr block, text is unstyled
        let input = "before {{} middle} after";
        let (text, _runs) = parse_inline(input).unwrap();
        assert_eq!(text, "before middle after");
    }

    #[test]
    fn multiple_attrs_in_block() {
        let input = "{{b i u sz=24 c=ff0000} styled}";
        let (text, runs) = parse_inline(input).unwrap();
        assert_eq!(text, "styled");
        let run = runs.iter().find(|r| r.range == (0..6)).unwrap();
        assert_eq!(run.style.bold, Some(true));
        assert_eq!(run.style.italic, Some(true));
        assert_eq!(run.style.underline, Some(true));
        assert_eq!(run.style.size, Some(24.0));
        assert!(run.style.color.is_some());
    }
}
