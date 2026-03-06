use crate::document::Alignment;
use iced_core::{Color, Font};

/// Formatting context at the current cursor position.
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub character: Character,
    pub paragraph: Paragraph,
    pub position: Position,
}

/// Per-character formatting at cursor.
#[derive(Debug, Clone, Default)]
pub struct Character {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub font: Option<Font>,
    pub size: Option<f32>,
    pub color: Option<Color>,
}

/// Per-paragraph formatting at cursor.
#[derive(Debug, Clone)]
pub struct Paragraph {
    pub alignment: Alignment,
    pub heading: Option<u8>,
    pub spacing_after: f32,
}

impl Default for Paragraph {
    fn default() -> Self {
        Self {
            alignment: Alignment::Left,
            heading: None,
            spacing_after: 0.0,
        }
    }
}

/// Cursor position in the document.
#[derive(Debug, Clone, Copy, Default)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}
