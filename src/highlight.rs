use iced_core::text::highlighter::{self, Highlighter};
use iced_core::{Color, Font, Pixels};

use std::ops::Range;
use std::sync::{Arc, RwLock};

use crate::document::{RichDocument, SpanFormat};

/// Settings for the [`RichTextHighlighter`].
pub struct Settings {
    /// The base text font (normal weight, normal style).
    pub font: Font,
    /// The base font size in pixels.
    pub base_size: f32,
    /// The shared document model.
    pub document: Arc<RwLock<RichDocument>>,
    /// Version counter -- bumped on any formatting change.
    pub version: u64,
}

impl PartialEq for Settings {
    fn eq(&self, other: &Self) -> bool {
        self.font == other.font
            && (self.base_size - other.base_size).abs() < f32::EPSILON
            && self.version == other.version
    }
}

impl Clone for Settings {
    fn clone(&self) -> Self {
        Self {
            font: self.font,
            base_size: self.base_size,
            document: Arc::clone(&self.document),
            version: self.version,
        }
    }
}

/// The output highlight type, embedding all info needed for `to_format`.
#[derive(Debug, Clone, Copy)]
pub enum Highlight {
    /// Normal text, no special formatting.
    Normal,
    /// Formatted text with embedded rendering info.
    Formatted {
        font: Option<Font>,
        size: Option<f32>,
        color: Option<Color>,
    },
}

impl Highlight {
    /// Convert this highlight into an iced `Format<Font>`.
    pub fn to_format(&self) -> highlighter::Format<Font> {
        match self {
            Self::Normal => highlighter::Format::default(),
            Self::Formatted { font, size, color } => highlighter::Format {
                color: *color,
                font: *font,
                size: size.map(Pixels),
            },
        }
    }
}

/// A rich-text highlighter that reads formatting from a [`RichDocument`].
///
/// Instead of parsing markdown syntax, this highlighter reads per-line
/// formatting spans from the shared document model and converts them
/// into [`Highlight`] values for iced's `TextEditor`.
pub struct RichTextHighlighter {
    settings: Settings,
    current_line: usize,
}

impl Highlighter for RichTextHighlighter {
    type Settings = Settings;
    type Highlight = Highlight;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Highlight)>;

    fn new(settings: &Settings) -> Self {
        Self {
            settings: settings.clone(),
            current_line: 0,
        }
    }

    fn update(&mut self, new_settings: &Settings) {
        self.settings = new_settings.clone();
        self.current_line = 0;
    }

    fn change_line(&mut self, line: usize) {
        if line < self.current_line {
            self.current_line = line;
        }
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let line_idx = self.current_line;
        self.current_line += 1;

        let doc = self
            .settings
            .document
            .read()
            .expect("RichDocument lock poisoned");

        if line_idx >= doc.line_count() || line.is_empty() {
            return vec![].into_iter();
        }

        let base_font = self.settings.font;
        let base_size = self.settings.base_size;
        let line_format = doc.line_format(line_idx);
        let spans = doc.spans(line_idx);

        // Determine heading size if applicable.
        let effective_size = line_format
            .heading_level
            .map(|level| heading_size(base_size, level as usize));

        if spans.is_empty() {
            // No formatting spans -- but might have a heading.
            if effective_size.is_some() {
                let mut font = base_font;
                font.weight = iced_core::font::Weight::Bold;
                return vec![(
                    0..line.len(),
                    Highlight::Formatted {
                        font: Some(font),
                        size: effective_size,
                        color: None,
                    },
                )]
                .into_iter();
            }
            return vec![(0..line.len(), Highlight::Normal)].into_iter();
        }

        let mut result = Vec::new();
        for (range, span_fmt) in spans {
            let highlight =
                span_format_to_highlight(span_fmt, &base_font, base_size, effective_size);
            // Clamp range to actual line length.
            let clamped = range.start.min(line.len())..range.end.min(line.len());
            if !clamped.is_empty() {
                result.push((clamped, highlight));
            }
        }

        // Fill gaps with Normal or heading-formatted.
        // (The document should have complete coverage, but be safe.)
        if result.is_empty() && !line.is_empty() {
            result.push((0..line.len(), Highlight::Normal));
        }

        result.into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

/// Convert a [`SpanFormat`] into a [`Highlight`], incorporating heading size
/// and base font information.
fn span_format_to_highlight(
    span: &SpanFormat,
    base_font: &Font,
    _base_size: f32,
    heading_size: Option<f32>,
) -> Highlight {
    if span.is_default() && heading_size.is_none() {
        return Highlight::Normal;
    }

    let font = {
        let mut f = span.font.unwrap_or(*base_font);
        if span.bold {
            f.weight = iced_core::font::Weight::Bold;
        }
        if span.italic {
            f.style = iced_core::font::Style::Italic;
        }
        Some(f)
    };

    let size = span.size.or(heading_size);
    let color = span.color;

    // Only emit Formatted if something differs from default.
    if font.is_some() || size.is_some() || color.is_some() {
        Highlight::Formatted { font, size, color }
    } else {
        Highlight::Normal
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use iced_core::font::{Family, Style, Weight};
    fn test_font() -> Font {
        Font {
            family: Family::SansSerif,
            weight: Weight::Normal,
            stretch: iced_core::font::Stretch::Normal,
            style: Style::Normal,
        }
    }

    fn test_settings(doc: Arc<RwLock<RichDocument>>, version: u64) -> Settings {
        Settings {
            font: test_font(),
            base_size: 16.0,
            document: doc,
            version,
        }
    }

    #[test]
    fn normal_text_produces_normal_highlight() {
        let doc = Arc::new(RwLock::new(RichDocument::with_lines(1)));
        let settings = test_settings(doc, 0);
        let mut h = RichTextHighlighter::new(&settings);

        let spans: Vec<_> = h.highlight_line("Hello, world!").collect();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, 0..13);
        assert!(matches!(spans[0].1, Highlight::Normal));
    }

    #[test]
    fn bold_span_produces_formatted_highlight() {
        let mut rich_doc = RichDocument::with_lines(1);
        rich_doc.toggle_bold(0, 0..5);
        let doc = Arc::new(RwLock::new(rich_doc));

        let settings = test_settings(doc, 1);
        let mut h = RichTextHighlighter::new(&settings);

        let spans: Vec<_> = h.highlight_line("Hello world").collect();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, 0..5);
        match spans[0].1 {
            Highlight::Formatted { font, .. } => {
                let f = font.unwrap();
                assert_eq!(f.weight, Weight::Bold);
            }
            Highlight::Normal => panic!("Expected Formatted, got Normal"),
        }
    }

    #[test]
    fn heading_produces_sized_highlight() {
        let mut rich_doc = RichDocument::with_lines(1);
        rich_doc.line_format_mut(0).heading_level = Some(1);
        let doc = Arc::new(RwLock::new(rich_doc));

        let settings = test_settings(doc, 1);
        let mut h = RichTextHighlighter::new(&settings);

        let spans: Vec<_> = h.highlight_line("Hello").collect();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, 0..5);
        match spans[0].1 {
            Highlight::Formatted { font, size, .. } => {
                let f = font.unwrap();
                assert_eq!(f.weight, Weight::Bold);
                let s = size.unwrap();
                assert!((s - 32.0).abs() < 0.01, "Expected 32.0, got {s}");
            }
            Highlight::Normal => panic!("Expected Formatted heading, got Normal"),
        }
    }

    #[test]
    fn empty_line_produces_empty_iterator() {
        let doc = Arc::new(RwLock::new(RichDocument::with_lines(1)));
        let settings = test_settings(doc, 0);
        let mut h = RichTextHighlighter::new(&settings);

        let spans: Vec<_> = h.highlight_line("").collect();
        assert!(spans.is_empty());
    }

    #[test]
    fn line_beyond_document_produces_empty() {
        let doc = Arc::new(RwLock::new(RichDocument::with_lines(1)));
        let settings = test_settings(doc, 0);
        let mut h = RichTextHighlighter::new(&settings);

        // Advance past line 0.
        let _ = h.highlight_line("first line");
        // Now line 1 is beyond the document's single line.
        let spans: Vec<_> = h.highlight_line("second line").collect();
        assert!(spans.is_empty());
    }
}
