//! # Document
//!
//! The document model for markright's rich text editor.
//!
//! For now, `Document` is a type alias for `cosmic_text::Editor`. Over time
//! this will evolve into a full document model with paragraph styles, block-level
//! formatting, embedded objects, undo/redo, and rope-based storage.
//!
//! The `rich_editor::Editor` trait insulates the widget layer from changes
//! here -- when Document evolves, the trait stays stable.

/// The document buffer. Currently backed by cosmic-text's Editor, which
/// owns the text (Buffer) and per-character formatting (AttrsList per line).
///
/// cosmic-text handles span range adjustments on insert/delete automatically
/// via `BufferLine::split_off()` + `BufferLine::append()`.
pub type Document = cosmic_text::Editor<'static>;
