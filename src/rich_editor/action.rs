use crate::document::Alignment;

use iced_core::{Font, Point};

use std::sync::Arc;

/// Top-level editor action — navigation, selection, and edits.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Action {
    /// Apply an [`Edit`] to the document.
    Edit(Edit),
    /// Apply a [`Motion`].
    Move(Motion),
    /// Select text with a given [`Motion`].
    Select(Motion),
    /// Select the word at the current cursor.
    SelectWord,
    /// Select the line at the current cursor.
    SelectLine,
    /// Select the entire buffer.
    SelectAll,
    /// Click the editor at the given [`Point`].
    Click(Point),
    /// Drag the mouse to the given [`Point`].
    Drag(Point),
    /// Scroll the editor by a number of lines.
    Scroll {
        /// The number of lines to scroll.
        lines: i32,
    },
}

impl Action {
    /// Returns whether the action modifies the document.
    pub fn is_edit(&self) -> bool {
        matches!(self, Self::Edit(_))
    }
}

/// Buffer-modifying operations. Tracks dirty state.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Edit {
    /// Insert a character at the cursor.
    Insert(char),
    /// Paste text at the cursor.
    Paste(Arc<String>),
    /// Break the current line (Enter key).
    Enter,
    /// Delete the previous character.
    Backspace,
    /// Delete the next character.
    Delete,
    /// Apply a formatting change at the current cursor/selection.
    Format(FormatAction),
}

/// A formatting change applied at the current cursor or selection.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FormatAction {
    /// Toggle bold on the selection.
    ToggleBold,
    /// Toggle italic on the selection.
    ToggleItalic,
    /// Toggle underline on the selection.
    ToggleUnderline,
    /// Set or clear the heading level for the current line.
    SetHeadingLevel(Option<u8>),
    /// Set the text alignment for the current line.
    SetAlignment(Alignment),
    /// Set the font for the selection.
    SetFont(Font),
    /// Set the font size for the selection.
    SetFontSize(f32),
}

/// Convert our [`Action`] to an iced [`text::editor::Action`] when possible.
///
/// Returns `None` for formatting actions that have no iced equivalent.
pub(crate) fn to_iced_action(action: &Action) -> Option<iced_core::text::editor::Action> {
    use iced_core::text::editor;

    match action {
        Action::Edit(edit) => {
            let iced_edit = match edit {
                Edit::Insert(c) => editor::Edit::Insert(*c),
                Edit::Paste(s) => editor::Edit::Paste(Arc::clone(s)),
                Edit::Enter => editor::Edit::Enter,
                Edit::Backspace => editor::Edit::Backspace,
                Edit::Delete => editor::Edit::Delete,
                Edit::Format(_) => return None,
            };
            Some(editor::Action::Edit(iced_edit))
        }
        Action::Move(m) => Some(editor::Action::Move(*m)),
        Action::Select(m) => Some(editor::Action::Select(*m)),
        Action::SelectWord => Some(editor::Action::SelectWord),
        Action::SelectLine => Some(editor::Action::SelectLine),
        Action::SelectAll => Some(editor::Action::SelectAll),
        Action::Click(p) => Some(editor::Action::Click(*p)),
        Action::Drag(p) => Some(editor::Action::Drag(*p)),
        Action::Scroll { lines } => Some(editor::Action::Scroll { lines: *lines }),
    }
}

// Re-export iced types that are part of our public API.
pub use iced_core::text::editor::{Cursor, Line, LineEnding, Motion, Position, Selection};
