//! Popup overlay types for the rich editor.

use std::ops::Range;

use crate::core::{Background, Border};

/// A region of text that triggers a popup overlay when the cursor is near it.
///
/// Owns its `value` and `placeholder` strings so the application can store
/// a `Vec<Span>` on its own state without lifetime self-referencing.
#[derive(Clone, Debug)]
pub struct Span {
    pub line: usize,
    pub range: Range<usize>,
    /// The current text shown in the popup's text input.
    pub value: String,
    pub placeholder: String,
    pub background: Option<Background>,
    pub border: Border,
}

/// Identifies a popup span by line and range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanRef {
    pub line: usize,
    pub range: Range<usize>,
}

impl From<&Span> for SpanRef {
    fn from(span: &Span) -> Self {
        Self {
            line: span.line,
            range: span.range.clone(),
        }
    }
}

/// An action produced by the popup overlay.
#[derive(Clone, Debug)]
pub enum Action {
    /// The text input value changed.
    Input { span: SpanRef, value: String },
    /// User pressed Enter to confirm.
    Confirm { span: SpanRef },
    /// User pressed Escape to dismiss; `original` is the value when the
    /// popup first opened, for revert.
    Dismiss { span: SpanRef, original: String },
}

/// Widget ID for the popup's text input.
pub const INPUT_ID: &str = "markright-popup-input";
