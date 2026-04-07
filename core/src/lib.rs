//! # Core
//!
//! Format-agnostic buffer model for rich text editing.
//!
//! Provides atomic operations ([`Op`]) for all document mutations,
//! an undo/redo [`History`], styled-line capture utilities, and the
//! [`Format`] trait that external format adapters implement.

pub mod capture;
pub mod format;
pub mod history;
pub mod op;
pub mod paragraph;
pub mod theme;

pub use capture::{read_style_runs, read_styled_line, read_styled_text};
pub use format::Format;
pub use history::{History, UndoGroup};
pub use op::{Alignment, Op, SpanAttr, StyleRun, StyledLine, StyledText};
pub use paragraph::{Name, OverrideSet, Paragraph};
pub use theme::Theme;
