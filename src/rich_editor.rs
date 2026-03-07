//! Rich text editor widget with integrated formatting model.
//!
//! This module provides a rich text editor that wraps a `rich_editor::Renderer`
//! backed by cosmic-text. All formatting and text editing go through
//! [`Content::perform`].

mod action;
mod content;
pub mod cursor;
pub mod widget;

pub use action::{
    Action, Cursor, Edit, FormatAction, Line, LineEnding, Motion, Position, Selection,
};
pub use content::Content;
pub use widget::{Binding, Catalog, KeyPress, RichEditor, Status, Style, StyleFn, rich_editor};
