use iced::color;
use iced::theme::Palette;

/// Theme selection for the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    Light,
    #[default]
    Dark,
}

impl Theme {
    /// Build an iced Theme from this choice.
    pub fn to_theme(self) -> iced::Theme {
        iced::Theme::custom(self.name().to_string(), self.palette())
    }

    fn name(self) -> &'static str {
        match self {
            Self::Light => "Paper Light",
            Self::Dark => "Paper Dark",
        }
    }

    fn palette(self) -> Palette {
        match self {
            Self::Light => Palette {
                background: color!(0xf2eede),
                text: color!(0x555555),
                primary: color!(0x1a1a1a), // Dark gray
                success: color!(0x1e6fcc), // Blue
                warning: color!(0x216609), // Green
                danger: color!(0xcc3e28),  // Red-orange
            },
            Self::Dark => Palette {
                background: color!(0x1f1e1a), // Warm dark background
                text: color!(0xd4c8b0),       // Warm muted paper color
                primary: color!(0xe8dcc0),    // Warm light paper color
                success: color!(0x1e6fcc),    // Blue
                warning: color!(0x216609),    // Green
                danger: color!(0xcc3e28),     // Red-orange
            },
        }
    }

    /// Toggle between light and dark.
    pub fn toggle(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }

    /// Whether this is the dark theme.
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}

pub mod button {
    use iced::widget::button;
    use iced::{Background, Border, Theme};

    /// Toolbar toggle button -- highlighted when active.
    pub fn toolbar_toggle(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
        move |theme, status| {
            let palette = theme.extended_palette();
            if active {
                button::Style {
                    background: Some(Background::Color(palette.primary.base.color)),
                    text_color: palette.primary.base.text,
                    border: Border {
                        radius: 4.0.into(),
                        ..Border::default()
                    },
                    ..Default::default()
                }
            } else {
                match status {
                    button::Status::Hovered => button::Style {
                        background: Some(Background::Color(palette.background.weak.color)),
                        text_color: palette.background.base.text,
                        border: Border {
                            radius: 4.0.into(),
                            ..Border::default()
                        },
                        ..Default::default()
                    },
                    _ => button::Style {
                        background: None,
                        text_color: palette.background.base.text,
                        border: Border {
                            radius: 4.0.into(),
                            ..Border::default()
                        },
                        ..Default::default()
                    },
                }
            }
        }
    }

    /// Icon-only button (transparent background).
    pub fn icon(theme: &Theme, status: button::Status) -> button::Style {
        let palette = theme.extended_palette();
        let text_color = match status {
            button::Status::Disabled => palette.background.weak.text,
            button::Status::Hovered => palette.primary.base.color,
            _ => palette.background.base.text,
        };
        button::Style {
            background: None,
            text_color,
            ..Default::default()
        }
    }
}

pub mod container {
    use iced::widget::container;
    use iced::{Background, Border, Theme};

    /// Toolbar container with subtle background.
    pub fn toolbar(theme: &Theme) -> container::Style {
        let palette = theme.extended_palette();
        container::Style {
            background: Some(Background::Color(palette.background.weak.color)),
            border: Border {
                color: palette.background.strong.color,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        }
    }
}

pub mod text {
    use iced::Theme;
    use iced::widget::text;

    /// Muted text color for the status bar.
    pub fn status_bar(theme: &Theme) -> text::Style {
        let palette = theme.extended_palette();
        text::Style {
            color: Some(palette.background.weak.text),
        }
    }
}

pub mod text_editor {
    use iced::{Background, Border, Theme};
    use markright::widget::rich_editor::{Status, Style};

    /// Editor with no focus border.
    pub fn borderless(theme: &Theme, status: Status) -> Style {
        let palette = theme.extended_palette();
        let selection = if matches!(status, Status::Focused { .. }) {
            palette.primary.strong.color
        } else {
            palette.primary.weak.color
        };
        Style {
            background: Background::Color(palette.background.base.color),
            border: Border::default(),
            placeholder: palette.background.strong.color,
            value: palette.background.base.text,
            selection,
        }
    }
}
