//! Popup overlay types for the rich editor.

/// An action produced by the popup overlay.
#[derive(Debug, Clone)]
pub enum Action {
    /// The text input value changed.
    Input(String),
    /// The user pressed Enter to confirm.
    Confirm,
    /// The user pressed Escape to dismiss.
    Dismiss,
}

/// Widget ID for the popup's text input.
///
/// Use with `iced::widget::operation::focus` to programmatically
/// focus the popup input (e.g., in response to a Tab key binding).
pub const INPUT_ID: &str = "markright-popup-input";
