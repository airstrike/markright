use iced::widget::{Space, button, combo_box, container, row, text};
use iced::{Element, Length};

use markright::paragraph;
use markright::widget::rich_editor::{Action, Alignment, Edit, FormatAction, cursor};

use crate::icon;
use crate::theme;

/// Wrap content in a subtle group container with fixed height matching buttons.
fn group<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> container::Container<'a, Message> {
    container(content)
        .style(theme::container::group)
        .height(28)
        .align_y(iced::Alignment::Center)
}

/// Extract font family name from cursor context, falling back to the default.
fn font_name(font: Option<iced::Font>) -> String {
    match font.map(|f| f.family) {
        Some(iced::font::Family::Name(name)) => name.to_string(),
        _ => "IBM Plex Sans".to_string(),
    }
}

/// Build the toolbar view with grouped icon buttons.
pub fn view<'a, Message>(
    ctx: &cursor::Context,
    font_list: &'a combo_box::State<String>,
    size_list: &'a combo_box::State<String>,
    is_dark: bool,
    can_undo: bool,
    can_redo: bool,
    show_debug: bool,
    on_action: impl Fn(Action) -> Message + 'a,
    on_font_selected: impl Fn(String) -> Message + 'a,
    on_size_selected: impl Fn(String) -> Message + 'a,
    on_toggle_theme: Message,
    on_toggle_debug: Message,
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

    let is_left = ctx.paragraph.alignment == Alignment::Left;
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

    let is_bullet = matches!(ctx.paragraph.style.list, Some(paragraph::List::Bullet(_)));
    let is_ordered = matches!(ctx.paragraph.style.list, Some(paragraph::List::Ordered(_)));

    let msg_bullet = on_action(fmt(FormatAction::SetList(Some(paragraph::List::Bullet(
        paragraph::Bullet::Disc,
    )))));
    let msg_ordered = on_action(fmt(FormatAction::SetList(Some(paragraph::List::Ordered(
        paragraph::Number::Arabic,
    )))));
    let msg_indent = on_action(fmt(FormatAction::IndentList));
    let msg_dedent = on_action(fmt(FormatAction::DedentList));

    let bullet_btn = button(icon::list().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(is_bullet))
        .on_press(msg_bullet);

    let ordered_btn = button(icon::list_ordered().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(is_ordered))
        .on_press(msg_ordered);

    let indent_btn = button(icon::indent_increase().size(16))
        .padding([4, 8])
        .style(theme::button::icon)
        .on_press(msg_indent);

    let dedent_btn = button(icon::indent_decrease().size(16))
        .padding([4, 8])
        .style(theme::button::icon)
        .on_press(msg_dedent);

    let theme_icon = if is_dark {
        icon::sun().size(16)
    } else {
        icon::moon().size(16)
    };
    let theme_toggle = button(theme_icon)
        .padding([4, 8])
        .style(theme::button::icon)
        .on_press(on_toggle_theme);

    let size = ctx.character.size.unwrap_or(crate::BASE_SIZE);

    let history_group = group(row![undo_btn, redo_btn]);
    let format_group = group(row![bold_btn, italic_btn, underline_btn]);
    let list_group = group(row![bullet_btn, ordered_btn, dedent_btn, indent_btn]);
    let align_group = group(row![
        align_left_btn,
        align_center_btn,
        align_right_btn,
        align_justify_btn,
    ]);
    let current_font = font_name(ctx.character.font);
    let font_selector = combo_box(font_list, "Font…", Some(&current_font), on_font_selected)
        .width(140)
        .size(12)
        .input_style(theme::combo_box::toolbar)
        .menu_style(theme::combo_box::toolbar_menu);

    let current_size = format!("{}", size as u32);
    let size_selector = combo_box(size_list, "Size", Some(&current_size), on_size_selected)
        .width(50)
        .size(12)
        .align_x(iced::Alignment::End)
        .input_style(theme::combo_box::toolbar)
        .menu_style(theme::combo_box::toolbar_menu);

    let font_group = group(
        row![font_selector, size_selector]
            .spacing(4)
            .align_y(iced::Alignment::Center),
    );

    let debug_btn = button(text("{*}").size(12))
        .padding([4, 8])
        .style(theme::button::icon)
        .on_press(on_toggle_debug);

    let toolbar_spacing = 6.0;

    let mut toolbar_row = row![
        history_group,
        format_group,
        list_group,
        align_group,
        font_group,
        Space::new().width(Length::Fill),
        group(row![debug_btn, theme_toggle]),
    ]
    .spacing(toolbar_spacing)
    .align_y(iced::Alignment::Center);

    if show_debug {
        toolbar_row = toolbar_row.push(Space::new().width(crate::debug::PANEL_W - toolbar_spacing));
    }

    container(toolbar_row)
        .style(theme::container::toolbar)
        .padding([8, 16])
        .width(Length::Fill)
        .into()
}
