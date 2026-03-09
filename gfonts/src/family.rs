use std::fmt;

/// A Google Fonts family entry.
#[derive(Debug, Clone)]
pub struct Family {
    pub name: String,
    pub category: Category,
    pub popularity: u32,
    pub variants: Variants,
}

/// Whether a family uses variable or static font files.
#[derive(Debug, Clone)]
pub enum Variants {
    /// Variable font — a single file covers weight/italic axes.
    Variable { axes: Vec<Axis> },
    /// Static font — separate file per weight/style combination.
    Static { keys: Vec<String> },
}

/// An axis of a variable font (e.g. weight, width, italic).
#[derive(Debug, Clone)]
pub struct Axis {
    pub tag: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

/// Google Fonts family category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    SansSerif,
    Serif,
    Display,
    Handwriting,
    Monospace,
}

impl Category {
    pub(crate) fn from_metadata(s: &str) -> Self {
        match s {
            "SANS_SERIF" => Self::SansSerif,
            "SERIF" => Self::Serif,
            "DISPLAY" => Self::Display,
            "HANDWRITING" => Self::Handwriting,
            "MONOSPACE" => Self::Monospace,
            _ => Self::SansSerif,
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SansSerif => write!(f, "Sans Serif"),
            Self::Serif => write!(f, "Serif"),
            Self::Display => write!(f, "Display"),
            Self::Handwriting => write!(f, "Handwriting"),
            Self::Monospace => write!(f, "Monospace"),
        }
    }
}
