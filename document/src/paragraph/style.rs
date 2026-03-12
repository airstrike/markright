use super::indent;
use super::list;
use super::spacing;

/// Paragraph-level formatting properties.
///
/// Character-level defaults (font, size, color) remain on iced's
/// `ParagraphStyle.style` field. This struct captures layout and
/// list properties that iced doesn't model.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    pub line_spacing: Option<spacing::Spacing>,
    /// Space before paragraph in points.
    pub space_before: Option<f32>,
    /// Space after paragraph in points.
    pub space_after: Option<f32>,
    pub indent: indent::Indent,
    /// Nesting depth (0-8).
    pub level: u8,
    pub list: Option<list::List>,
}
