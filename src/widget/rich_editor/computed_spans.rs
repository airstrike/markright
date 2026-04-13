//! Computed spans — regions where displayed text differs from the
//! underlying source value, with popup-based editing.
//!
//! This module is gated behind the `computed_spans` cargo feature.
//!
//! # Overview
//!
//! A [`ComputedSpan`] represents an "atomic chip" in the editor: a
//! region whose displayed text is computed from a source value. The
//! user cannot type directly into the displayed text; instead, a popup
//! overlay provides an editable text input for the source value.
//!
//! Use cases include:
//! - **Formulas**: `{=2+3}` displays as `5`, popup edits the expression
//! - **Mentions**: `@alice` displays as `Alice Smith`, popup edits the handle
//! - **Spell suggestions**: misspelled word displays as-is, popup offers corrections
//! - **AI rewrites**: original text displays, popup shows suggested improvement
//!
//! # Usage
//!
//! ```ignore
//! rich_editor(&content)
//!     .computed_spans(&spans, Message::Computed, |input, span| {
//!         container(column![
//!             input.size(14),
//!             text(format!("= {}", evaluate(&span.source_value))),
//!         ])
//!         .into()
//!     })
//! ```

use std::ops::Range;

use crate::core::{Background, Border};

/// A region where the displayed text differs from the underlying source.
///
/// The editor renders `display_range` as a styled, atomic chip. The
/// popup overlay lets the user edit `source_value`; the application
/// re-evaluates and provides a new `display_value` on the next render.
#[derive(Clone, Debug)]
pub struct ComputedSpan {
    /// Stable identifier assigned by the application.
    pub id: u64,
    /// Line in the displayed content.
    pub line: usize,
    /// Column range in the displayed content.
    pub display_range: Range<usize>,
    /// The value the popup's text input edits (e.g., `"{=2+3}"`).
    pub source_value: String,
    /// What's shown in the editor (e.g., `"5"`). Must match the actual
    /// text at `display_range` in the editor's content.
    pub display_value: String,
    /// Placeholder shown when the popup input is empty.
    pub placeholder: String,
    /// Background fill drawn behind the chip.
    pub background: Option<Background>,
    /// Border drawn around the chip.
    pub border: Border,
    /// When `true`, the editor blocks character insertion inside the
    /// chip and treats Backspace/Delete at the boundary as whole-chip
    /// deletion.
    pub atomic: bool,
}

/// An action produced by computed-span interaction.
#[derive(Clone, Debug)]
pub enum Action {
    /// The user changed the source value in the popup.
    Input { id: u64, value: String },
    /// The user confirmed the edit (Enter in popup).
    Confirm { id: u64 },
    /// The user dismissed the popup (Escape). `original` is the source
    /// value when the popup first opened, for revert.
    Dismiss { id: u64, original: String },
    /// The user deleted the entire span (Backspace/Delete at boundary).
    Delete { id: u64 },
}
