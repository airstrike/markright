use iced_core::text::highlighter::{self, Highlighter};
use iced_core::{Color, Font, Pixels};

use std::ops::Range;

use crate::document::{LineFormat, RichDocument, SpanFormat};

/// The output highlight type for the rich text editor.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Highlight {
    /// Normal text, no special formatting.
    Normal,
    /// Formatted text with embedded rendering info.
    Formatted {
        font: Option<Font>,
        size: Option<f32>,
        color: Option<Color>,
        underline: bool,
    },
}

impl Highlight {
    /// Convert to an iced [`highlighter::Format`].
    pub(crate) fn to_format(self) -> highlighter::Format<Font> {
        match self {
            Self::Normal => highlighter::Format::default(),
            // NOTE: iced graphics layer does not yet render underlines (cosmic-text TODO)
            Self::Formatted {
                font,
                size,
                color,
                underline,
            } => highlighter::Format {
                color,
                font,
                size: size.map(Pixels),
                underline: if underline { Some(true) } else { None },
            },
        }
    }
}

/// Settings for the rich text highlighter.
///
/// Uses a version counter to detect formatting changes without shared state.
#[derive(Debug, Clone)]
pub(crate) struct Settings {
    pub font: Font,
    pub base_size: f32,
    /// Snapshot of line formats from the document.
    pub line_formats: Vec<LineFormat>,
    /// Snapshot of per-line spans from the document.
    pub line_spans: Vec<Vec<(Range<usize>, SpanFormat)>>,
    /// Version counter for change detection.
    pub version: u64,
}

impl Settings {
    /// Create settings from a snapshot of the document.
    pub(crate) fn from_document(
        font: Font,
        base_size: f32,
        doc: &RichDocument,
        version: u64,
    ) -> Self {
        let count = doc.line_count();
        let mut line_formats = Vec::with_capacity(count);
        let mut line_spans = Vec::with_capacity(count);

        for i in 0..count {
            line_formats.push(doc.line_format(i).clone());
            line_spans.push(doc.spans(i).to_vec());
        }

        Self {
            font,
            base_size,
            line_formats,
            line_spans,
            version,
        }
    }
}

impl PartialEq for Settings {
    fn eq(&self, other: &Self) -> bool {
        self.font == other.font
            && (self.base_size - other.base_size).abs() < f32::EPSILON
            && self.version == other.version
    }
}

/// A rich-text highlighter that reads formatting from a snapshot of the
/// [`RichDocument`]. No shared state — the widget takes a snapshot before
/// each highlight pass.
#[derive(Debug)]
pub(crate) struct RichHighlighter {
    settings: Settings,
    current_line: usize,
}

impl Highlighter for RichHighlighter {
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

        if line_idx >= self.settings.line_formats.len() || line.is_empty() {
            return vec![].into_iter();
        }

        let base_font = self.settings.font;
        let base_size = self.settings.base_size;
        let line_format = &self.settings.line_formats[line_idx];
        let spans = &self.settings.line_spans[line_idx];

        let effective_size = line_format
            .heading_level
            .map(|level| heading_size(base_size, level as usize));

        if spans.is_empty() {
            if effective_size.is_some() {
                let mut font = base_font;
                font.weight = iced_core::font::Weight::Bold;
                return vec![(
                    0..line.len(),
                    Highlight::Formatted {
                        font: Some(font),
                        size: effective_size,
                        color: None,
                        underline: false,
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
            let clamped = range.start.min(line.len())..range.end.min(line.len());
            if !clamped.is_empty() {
                result.push((clamped, highlight));
            }
        }

        if result.is_empty() && !line.is_empty() {
            result.push((0..line.len(), Highlight::Normal));
        }

        result.into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

/// Convert a [`SpanFormat`] into a [`Highlight`].
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
    let underline = span.underline;

    if font.is_some() || size.is_some() || color.is_some() || underline {
        Highlight::Formatted {
            font,
            size,
            color,
            underline,
        }
    } else {
        Highlight::Normal
    }
}

/// Heading size based on level (1-6).
pub(crate) fn heading_size(base: f32, level: usize) -> f32 {
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
