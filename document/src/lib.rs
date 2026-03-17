//! # Document
//!
//! Operation-based document model for markright's rich text editor.
//!
//! Provides atomic operations ([`Op`]) for all document mutations and
//! an undo/redo [`History`] built on those operations. Designed so a
//! future `crdt` subcrate can consume/produce the same operations.

pub mod capture;
pub mod format;
pub mod history;
pub mod op;

/// Paragraph-level formatting types, re-exported from `iced_core`.
pub use iced_core::text::rich_editor::paragraph;

pub use capture::{read_style_runs, read_styled_line, read_styled_text};
pub use history::{History, UndoGroup};
pub use op::{Alignment, Op, SpanAttr, StyleRun, StyledLine, StyledText};
