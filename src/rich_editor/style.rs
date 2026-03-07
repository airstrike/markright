use crate::core::theme;
use crate::core::{Background, Border, Color, Theme};
use crate::rich_editor::Status;

/// The appearance of a [`RichEditor`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The background.
    pub background: Background,
    /// The border.
    pub border: Border,
    /// The placeholder color.
    pub placeholder: Color,
    /// The value color.
    pub value: Color,
    /// The selection color.
    pub selection: Color,
}

/// The theme catalog for a [`RichEditor`].
pub trait Catalog: theme::Base {
    /// The item class.
    type Class<'a>;

    /// The default class.
    fn default<'a>() -> Self::Class<'a>;

    /// The style for a class and status.
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

/// A styling function for a [`RichEditor`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

/// The default style.
pub fn default(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    let active = Style {
        background: Background::Color(palette.background.base.color),
        border: Border {
            radius: 2.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        placeholder: palette.secondary.base.color,
        value: palette.background.base.text,
        selection: palette.primary.weak.color,
    };

    match status {
        Status::Active => active,
        Status::Hovered => Style {
            border: Border {
                color: palette.background.base.text,
                ..active.border
            },
            ..active
        },
        Status::Focused { .. } => Style {
            border: Border {
                color: palette.primary.strong.color,
                ..active.border
            },
            ..active
        },
        Status::Disabled => Style {
            background: Background::Color(palette.background.weak.color),
            value: active.placeholder,
            placeholder: palette.background.strongest.color,
            ..active
        },
    }
}
