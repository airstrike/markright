use iced::widget::{Space, button, container, row};
use iced::{Element, Length};

use markright::widget::rich_editor::{Action, Alignment, Edit, FormatAction, cursor};

use crate::icon;
use crate::theme;

/// Build the toolbar view with icon buttons.
///
/// The toolbar emits [`Action`] values directly -- no intermediate
/// `ToolbarAction` type needed.
pub fn view<'a, Message>(
    ctx: &cursor::Context,
    is_dark: bool,
    can_undo: bool,
    can_redo: bool,
    on_action: impl Fn(Action) -> Message + 'a,
    on_toggle_theme: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let fmt = |f: FormatAction| Action::Edit(Edit::Format(f));

    let msg_undo = on_action(Action::Undo);
    let msg_redo = on_action(Action::Redo);

    let mut undo_btn = button(icon::undo().size(16))
        .padding([4, 8])
        .style(theme::button::icon);
    if can_undo {
        undo_btn = undo_btn.on_press(msg_undo);
    }

    let mut redo_btn = button(icon::redo().size(16))
        .padding([4, 8])
        .style(theme::button::icon);
    if can_redo {
        redo_btn = redo_btn.on_press(msg_redo);
    }

    let msg_bold = on_action(fmt(FormatAction::ToggleBold));
    let msg_italic = on_action(fmt(FormatAction::ToggleItalic));
    let msg_underline = on_action(fmt(FormatAction::ToggleUnderline));
    let msg_align_left = on_action(fmt(FormatAction::SetAlignment(Alignment::Left)));
    let msg_align_center = on_action(fmt(FormatAction::SetAlignment(Alignment::Center)));
    let msg_align_right = on_action(fmt(FormatAction::SetAlignment(Alignment::Right)));
    let msg_align_justify = on_action(fmt(FormatAction::SetAlignment(Alignment::Justified)));

    let bold_btn = button(icon::bold().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(ctx.character.bold))
        .on_press(msg_bold);

    let italic_btn = button(icon::italic().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(ctx.character.italic))
        .on_press(msg_italic);

    let underline_btn = button(icon::underline().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(ctx.character.underline))
        .on_press(msg_underline);

    let is_left = matches!(
        ctx.paragraph.alignment,
        Alignment::Left | Alignment::Default
    );
    let is_center = ctx.paragraph.alignment == Alignment::Center;
    let is_right = ctx.paragraph.alignment == Alignment::Right;
    let is_justify = ctx.paragraph.alignment == Alignment::Justified;

    let align_left_btn = button(icon::text_align_start().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(is_left))
        .on_press(msg_align_left);

    let align_center_btn = button(icon::text_align_center().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(is_center))
        .on_press(msg_align_center);

    let align_right_btn = button(icon::text_align_end().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(is_right))
        .on_press(msg_align_right);

    let align_justify_btn = button(icon::text_align_justify().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(is_justify))
        .on_press(msg_align_justify);

    let theme_icon = if is_dark {
        icon::sun().size(16)
    } else {
        icon::moon().size(16)
    };
    let theme_toggle = button(theme_icon)
        .padding([4, 8])
        .style(theme::button::icon)
        .on_press(on_toggle_theme);

    let spacer = Space::new().width(Length::Fill);

    container(
        row![
            undo_btn,
            redo_btn,
            Space::new().width(8),
            bold_btn,
            italic_btn,
            underline_btn,
            align_left_btn,
            align_center_btn,
            align_right_btn,
            align_justify_btn,
            spacer,
            theme_toggle,
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .style(theme::container::toolbar)
    .padding([8, 16])
    .width(Length::Fill)
    .into()
}
