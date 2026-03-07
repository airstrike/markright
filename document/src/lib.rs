//! # Document
//!
//! Operation-based document model for markright's rich text editor.
//!
//! Provides atomic operations ([`Op`]) for all document mutations and
//! an undo/redo [`History`] built on those operations. Designed so a
//! future `crdt` subcrate can consume/produce the same operations.

pub mod history;
pub mod op;

pub use history::{History, UndoGroup};
pub use op::{Op, StyleRun, StyledText};
