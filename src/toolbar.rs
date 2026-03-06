use iced_widget::button;
use iced_widget::container;
use iced_widget::core::{self, Border, Color, Font, Length, Padding};
use iced_widget::pick_list;
use iced_widget::rule;
use iced_widget::{Renderer, Row, Theme};

use crate::document::{Alignment, LineFormat, SpanFormat};

/// Convenience alias for the Element type used by the toolbar.
type Element<'a, Message> = core::Element<'a, Message, Theme, Renderer>;

/// Messages emitted by the toolbar.
#[derive(Debug, Clone)]
pub enum ToolbarAction {
    ToggleBold,
    ToggleItalic,
    ToggleUnderline,
    SetHeadingLevel(Option<u8>),
    SetAlignment(Alignment),
}

/// Current formatting state at the cursor, used to highlight active buttons.
#[derive(Debug, Clone, Default)]
pub struct ToolbarState {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub heading_level: Option<u8>,
    pub alignment: Alignment,
}

impl ToolbarState {
    /// Build toolbar state from document formatting at a cursor position.
    pub fn from_document(span: &SpanFormat, line: &LineFormat) -> Self {
        Self {
            bold: span.bold,
            italic: span.italic,
            underline: span.underline,
            heading_level: line.heading_level,
            alignment: line.alignment,
        }
    }
}

/// Heading level options for the pick list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingOption {
    Normal,
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

impl HeadingOption {
    /// All heading options for populating a pick list.
    pub const ALL: [HeadingOption; 7] = [
        HeadingOption::Normal,
        HeadingOption::H1,
        HeadingOption::H2,
        HeadingOption::H3,
        HeadingOption::H4,
        HeadingOption::H5,
        HeadingOption::H6,
    ];

    /// Convert from an optional heading level (as stored in the document).
    pub fn from_level(level: Option<u8>) -> Self {
        match level {
            None => Self::Normal,
            Some(1) => Self::H1,
            Some(2) => Self::H2,
            Some(3) => Self::H3,
            Some(4) => Self::H4,
            Some(5) => Self::H5,
            Some(6) => Self::H6,
            Some(_) => Self::Normal,
        }
    }

    /// Convert to an optional heading level.
    pub fn to_level(self) -> Option<u8> {
        match self {
            Self::Normal => None,
            Self::H1 => Some(1),
            Self::H2 => Some(2),
            Self::H3 => Some(3),
            Self::H4 => Some(4),
            Self::H5 => Some(5),
            Self::H6 => Some(6),
        }
    }
}

impl std::fmt::Display for HeadingOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "Normal"),
            Self::H1 => write!(f, "Heading 1"),
            Self::H2 => write!(f, "Heading 2"),
            Self::H3 => write!(f, "Heading 3"),
            Self::H4 => write!(f, "Heading 4"),
            Self::H5 => write!(f, "Heading 5"),
            Self::H6 => write!(f, "Heading 6"),
        }
    }
}

/// Build the toolbar view.
///
/// Returns an `Element` containing a horizontal toolbar with formatting
/// controls (bold, italic, underline, heading level, and alignment).
///
/// The `on_action` closure maps `ToolbarAction` values to your app's message
/// type so the toolbar integrates with any iced application.
pub fn toolbar<'a, Message>(
    state: &ToolbarState,
    on_action: impl Fn(ToolbarAction) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    // Pre-compute messages for each action so we don't need to borrow
    // on_action multiple times.
    let msg_bold = on_action(ToolbarAction::ToggleBold);
    let msg_italic = on_action(ToolbarAction::ToggleItalic);
    let msg_underline = on_action(ToolbarAction::ToggleUnderline);
    let msg_align_left = on_action(ToolbarAction::SetAlignment(Alignment::Left));
    let msg_align_center = on_action(ToolbarAction::SetAlignment(Alignment::Center));
    let msg_align_right = on_action(ToolbarAction::SetAlignment(Alignment::Right));
    let msg_align_justify = on_action(ToolbarAction::SetAlignment(Alignment::Justify));

    let is_bold = state.bold;
    let is_italic = state.italic;
    let is_underline = state.underline;
    let current_alignment = state.alignment;

    let bold_btn = format_button("B", is_bold, msg_bold)
        .style(move |theme, status| toggle_button_style(theme, status, is_bold));

    let italic_btn = format_button("I", is_italic, msg_italic)
        .style(move |theme, status| toggle_button_style(theme, status, is_italic));

    let underline_btn = format_button("U", is_underline, msg_underline)
        .style(move |theme, status| toggle_button_style(theme, status, is_underline));

    let selected_heading = HeadingOption::from_level(state.heading_level);
    let heading_picker = pick_list::PickList::new(
        Some(selected_heading),
        HeadingOption::ALL.as_slice(),
        HeadingOption::to_string,
    )
    .on_select(move |option: HeadingOption| {
        on_action(ToolbarAction::SetHeadingLevel(option.to_level()))
    })
    .text_size(13)
    .padding(Padding::from([4.0, 8.0]));

    let is_left = current_alignment == Alignment::Left;
    let is_center = current_alignment == Alignment::Center;
    let is_right = current_alignment == Alignment::Right;
    let is_justify = current_alignment == Alignment::Justify;

    let align_left = alignment_button("L", msg_align_left)
        .style(move |theme, status| toggle_button_style(theme, status, is_left));

    let align_center = alignment_button("C", msg_align_center)
        .style(move |theme, status| toggle_button_style(theme, status, is_center));

    let align_right = alignment_button("R", msg_align_right)
        .style(move |theme, status| toggle_button_style(theme, status, is_right));

    let align_justify = alignment_button("J", msg_align_justify)
        .style(move |theme, status| toggle_button_style(theme, status, is_justify));

    let separator = || -> Element<'_, Message> { rule::vertical(1).into() };

    let toolbar_row = Row::new()
        .push(bold_btn)
        .push(italic_btn)
        .push(underline_btn)
        .push(separator())
        .push(heading_picker)
        .push(separator())
        .push(align_left)
        .push(align_center)
        .push(align_right)
        .push(align_justify)
        .spacing(4)
        .align_y(core::alignment::Vertical::Center);

    container::Container::new(toolbar_row)
        .width(Length::Fill)
        .padding(Padding::from([6.0, 12.0]))
        .style(toolbar_container_style)
        .into()
}

/// Create a formatting toggle button (B, I, U).
fn format_button<'a, Message: Clone + 'a>(
    label: &'a str,
    active: bool,
    on_press: Message,
) -> button::Button<'a, Message> {
    let font_weight = if active || label == "B" {
        core::font::Weight::Bold
    } else {
        core::font::Weight::Normal
    };

    let font_style = if label == "I" {
        core::font::Style::Italic
    } else {
        core::font::Style::Normal
    };

    let label_widget = iced_widget::text(label)
        .size(14)
        .font(Font {
            weight: font_weight,
            style: font_style,
            ..Font::default()
        })
        .center();

    button::Button::new(label_widget)
        .on_press(on_press)
        .padding(Padding::from([4.0, 10.0]))
}

/// Create an alignment button with a text label.
fn alignment_button<'a, Message: Clone + 'a>(
    label: &'a str,
    on_press: Message,
) -> button::Button<'a, Message> {
    let label_widget = iced_widget::text(label).size(13).center();

    button::Button::new(label_widget)
        .on_press(on_press)
        .padding(Padding::from([4.0, 8.0]))
}

/// Style function for toggle buttons -- provides visual feedback for
/// active/inactive state.
fn toggle_button_style(theme: &Theme, status: button::Status, active: bool) -> button::Style {
    let palette = theme.extended_palette();

    if active {
        let base = button::Style {
            background: Some(core::Background::Color(palette.primary.weak.color)),
            text_color: palette.primary.base.text,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: palette.primary.base.color,
            },
            ..button::Style::default()
        };

        match status {
            button::Status::Active | button::Status::Pressed => base,
            button::Status::Hovered => button::Style {
                background: Some(core::Background::Color(palette.primary.base.color)),
                ..base
            },
            button::Status::Disabled => button::Style {
                text_color: palette.background.strong.color,
                ..base
            },
        }
    } else {
        let base = button::Style {
            background: Some(core::Background::Color(Color::TRANSPARENT)),
            text_color: palette.background.base.text,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: Color::TRANSPARENT,
            },
            ..button::Style::default()
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(core::Background::Color(palette.background.weak.color)),
                border: Border {
                    color: palette.background.strong.color,
                    ..base.border
                },
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(core::Background::Color(palette.background.strong.color)),
                ..base
            },
            button::Status::Disabled => button::Style {
                text_color: palette.background.strong.color,
                ..base
            },
        }
    }
}

/// Style for the toolbar container -- subtle bottom border to separate
/// from editor content.
fn toolbar_container_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(core::Background::Color(palette.background.weak.color)),
        border: Border {
            width: 0.0,
            color: palette.background.strong.color,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_state_default_is_plain() {
        let state = ToolbarState::default();
        assert!(!state.bold);
        assert!(!state.italic);
        assert!(!state.underline);
        assert_eq!(state.heading_level, None);
        assert_eq!(state.alignment, Alignment::Left);
    }

    #[test]
    fn toolbar_state_from_document_captures_formatting() {
        let span = SpanFormat {
            bold: true,
            italic: false,
            underline: true,
            ..SpanFormat::default()
        };
        let line = LineFormat {
            heading_level: Some(2),
            alignment: Alignment::Center,
            ..LineFormat::default()
        };
        let state = ToolbarState::from_document(&span, &line);
        assert!(state.bold);
        assert!(!state.italic);
        assert!(state.underline);
        assert_eq!(state.heading_level, Some(2));
        assert_eq!(state.alignment, Alignment::Center);
    }

    #[test]
    fn heading_option_round_trips() {
        for option in HeadingOption::ALL {
            let level = option.to_level();
            let back = HeadingOption::from_level(level);
            assert_eq!(option, back);
        }
    }

    #[test]
    fn heading_option_display() {
        assert_eq!(HeadingOption::Normal.to_string(), "Normal");
        assert_eq!(HeadingOption::H1.to_string(), "Heading 1");
        assert_eq!(HeadingOption::H6.to_string(), "Heading 6");
    }

    #[test]
    fn heading_option_from_unknown_level_is_normal() {
        assert_eq!(HeadingOption::from_level(Some(7)), HeadingOption::Normal);
        assert_eq!(HeadingOption::from_level(Some(0)), HeadingOption::Normal);
    }
}
