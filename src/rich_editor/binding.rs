use crate::core::SmolStr;
use crate::core::keyboard;
use crate::core::keyboard::key;
use crate::rich_editor::{FormatAction, Motion, Status};
use std::ops;

// A binding to an action in the [`RichEditor`].
#[derive(Debug, Clone, PartialEq)]
pub enum Binding<Message> {
    /// Unfocus the editor.
    Unfocus,
    /// Copy the selection.
    Copy,
    /// Cut the selection.
    Cut,
    /// Paste from clipboard.
    Paste,
    /// Apply a [`Motion`].
    Move(Motion),
    /// Select text with a [`Motion`].
    Select(Motion),
    /// Select the word at cursor.
    SelectWord,
    /// Select the current line.
    SelectLine,
    /// Select all text.
    SelectAll,
    /// Insert a character.
    Insert(char),
    /// Break the line (Enter).
    Enter,
    /// Delete previous character.
    Backspace,
    /// Delete next character.
    Delete,
    /// Apply a formatting action (built-in shortcuts like Cmd+B).
    Format(FormatAction),
    /// A sequence of bindings.
    Sequence(Vec<Self>),
    /// A custom message.
    Custom(Message),
}

/// A key press event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPress {
    /// The key pressed.
    pub key: keyboard::Key,
    /// The key with modifiers applied.
    pub modified_key: keyboard::Key,
    /// The physical key.
    pub physical_key: keyboard::key::Physical,
    /// Keyboard modifiers.
    pub modifiers: keyboard::Modifiers,
    /// Text produced by the key press.
    pub text: Option<SmolStr>,
    /// Current editor status.
    pub status: Status,
}

impl<Message> Binding<Message> {
    /// Returns the default binding for the given key press, including
    /// built-in formatting shortcuts (Cmd+B, Cmd+I, Cmd+U).
    pub fn from_key_press(event: KeyPress) -> Option<Self> {
        let KeyPress {
            key,
            modified_key,
            physical_key,
            modifiers,
            text,
            status,
        } = event;

        if !matches!(status, Status::Focused { .. }) {
            return None;
        }

        // Command combinations.
        let combination = match key.to_latin(physical_key) {
            Some('c') if modifiers.command() => Some(Self::Copy),
            Some('x') if modifiers.command() => Some(Self::Cut),
            Some('v') if modifiers.command() && !modifiers.alt() => Some(Self::Paste),
            Some('a') if modifiers.command() => Some(Self::SelectAll),
            // Built-in formatting shortcuts.
            Some('b') if modifiers.command() => Some(Self::Format(FormatAction::ToggleBold)),
            Some('i') if modifiers.command() => Some(Self::Format(FormatAction::ToggleItalic)),
            Some('u') if modifiers.command() => Some(Self::Format(FormatAction::ToggleUnderline)),
            _ => None,
        };

        if let Some(binding) = combination {
            return Some(binding);
        }

        #[cfg(target_os = "macos")]
        let modified_key = convert_macos_shortcut(&key, modifiers).unwrap_or(modified_key);

        match modified_key.as_ref() {
            keyboard::Key::Named(key::Named::Enter) => Some(Self::Enter),
            keyboard::Key::Named(key::Named::Backspace) => Some(Self::Backspace),
            keyboard::Key::Named(key::Named::Delete)
                if text.is_none() || text.as_deref() == Some("\u{7f}") =>
            {
                Some(Self::Delete)
            }
            keyboard::Key::Named(key::Named::Escape) => Some(Self::Unfocus),
            _ => {
                if let Some(text) = text {
                    let c = text.chars().find(|c| !c.is_control())?;
                    Some(Self::Insert(c))
                } else if let keyboard::Key::Named(named_key) = key.as_ref() {
                    let motion = motion(named_key)?;

                    let motion = if modifiers.macos_command() {
                        match motion {
                            Motion::Left => Motion::Home,
                            Motion::Right => Motion::End,
                            _ => motion,
                        }
                    } else {
                        motion
                    };

                    let motion = if modifiers.jump() {
                        motion.widen()
                    } else {
                        motion
                    };

                    Some(if modifiers.shift() {
                        Self::Select(motion)
                    } else {
                        Self::Move(motion)
                    })
                } else {
                    None
                }
            }
        }
    }
}

fn motion(key: key::Named) -> Option<Motion> {
    match key {
        key::Named::ArrowLeft => Some(Motion::Left),
        key::Named::ArrowRight => Some(Motion::Right),
        key::Named::ArrowUp => Some(Motion::Up),
        key::Named::ArrowDown => Some(Motion::Down),
        key::Named::Home => Some(Motion::Home),
        key::Named::End => Some(Motion::End),
        key::Named::PageUp => Some(Motion::PageUp),
        key::Named::PageDown => Some(Motion::PageDown),
        _ => None,
    }
}

pub(crate) enum Ime {
    Toggle(bool),
    Preedit {
        content: String,
        selection: Option<ops::Range<usize>>,
    },
    Commit(String),
}

#[cfg(target_os = "macos")]
pub(crate) fn convert_macos_shortcut(
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<keyboard::Key> {
    if modifiers != keyboard::Modifiers::CTRL {
        return None;
    }

    let key = match key.as_ref() {
        keyboard::Key::Character("b") => key::Named::ArrowLeft,
        keyboard::Key::Character("f") => key::Named::ArrowRight,
        keyboard::Key::Character("a") => key::Named::Home,
        keyboard::Key::Character("e") => key::Named::End,
        keyboard::Key::Character("h") => key::Named::Backspace,
        keyboard::Key::Character("d") => key::Named::Delete,
        _ => return None,
    };

    Some(keyboard::Key::Named(key))
}
