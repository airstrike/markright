//! # Document
//!
//! The `.mr` format adapter for markright's rich text editor.
//!
//! Implements [`Format`] as [`Markright`](format::Markright) for the `.mr`
//! serialization format. Re-exports everything from [`markright_core`] for
//! convenience.

pub mod format;

#[cfg(feature = "markdown")]
pub mod markdown;

pub use markright_core::*;
pub use markright_core::{capture, history, op};
