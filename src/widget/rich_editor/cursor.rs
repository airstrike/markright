//! Cursor context types — formatting state at the current cursor position.

use super::action::Alignment;
use crate::core::{Color, Font};
use markright_document::paragraph;

/// Formatting context at the current cursor position.
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub character: Character,
    pub paragraph: Paragraph,
    pub position: Position,
}

/// Per-character formatting at cursor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Character {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub font: Option<Font>,
    pub size: Option<f32>,
    pub color: Option<Color>,
    pub letter_spacing: Option<f32>,
}

/// Per-paragraph formatting at cursor.
#[derive(Debug, Clone, PartialEq)]
pub struct Paragraph {
    pub alignment: Alignment,
    pub spacing_after: f32,
    /// Document-model paragraph style (spacing, indent, level, list).
    pub style: paragraph::Style,
}

impl Default for Paragraph {
    fn default() -> Self {
        Self {
            alignment: Alignment::Left,
            spacing_after: 0.0,
            style: paragraph::Style::default(),
        }
    }
}

/// Cursor position in the document.
#[derive(Debug, Clone, Copy, Default)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}
