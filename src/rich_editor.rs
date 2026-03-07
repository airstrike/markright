//! Rich text editor widget with integrated formatting model.
//!
//! This module provides a rich text editor that wraps a `rich_editor::Renderer`
//! backed by cosmic-text. All formatting and text editing go through
//! [`Content::perform`].

mod action;
mod binding;
mod content;
pub mod cursor;
pub mod style;
pub mod widget;

pub use action::{
    Action, Cursor, Edit, FormatAction, Line, LineEnding, Motion, Position, Selection,
};
pub use binding::{Binding, KeyPress};
pub use content::Content;
pub use iced_core::text::Alignment;
pub use style::{Catalog, Style, StyleFn};
pub use widget::{RichEditor, Status, rich_editor};
