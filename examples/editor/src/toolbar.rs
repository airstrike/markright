use iced::widget::{Space, button, combo_box, container, mouse_area, row, text, text_input};
use iced::{Color, Element, Length, Subscription, color, mouse};

use markright::paragraph;
use markright::widget::rich_editor::{self, Action as EditorAction, Alignment, Format, cursor};

use crate::icon;
use crate::pull;
use crate::theme;

const GROUP_SPACING: f32 = 1.0;
const TOOLBAR_SPACING: f32 = 6.0;

const COLOR_SWATCHES: &[Option<Color>] = &[
    None,
    Some(Color::BLACK),
    Some(color!(0xcc3e28)), // red
    Some(color!(0xb36600)), // orange
    Some(color!(0x216609)), // green
    Some(color!(0x1e6fcc)), // blue
    Some(color!(0x5c21a5)), // purple
];

pub struct State {
    font_list: combo_box::State<String>,
    size_list: combo_box::State<String>,
    recent_fonts: Vec<String>,
    letter_spacing_input: String,
    line_height_input: String,
    pull: Option<pull::Pull>,
    show_debug: bool,
}

impl Default for State {
    fn default() -> Self {
        let size_list = combo_box::State::new(
            [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 48, 64, 72]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        Self {
            font_list: combo_box::State::new(vec!["IBM Plex Sans".to_string()]),
            size_list,
            recent_fonts: Vec::new(),
            letter_spacing_input: "0.00".into(),
            line_height_input: "1.3".into(),
            pull: None,
            show_debug: false,
        }
    }
}

impl State {
    pub fn show_debug(&self) -> bool {
        self.show_debug
    }

    /// Update input fields from the current cursor context.
    pub fn sync_from_cursor(&mut self, ctx: &cursor::Context) {
        self.letter_spacing_input = match ctx.character.letter_spacing {
            Some(ls) => format!("{ls:.2}"),
            None => "0.00".into(),
        };
        self.line_height_input = match ctx.paragraph.line_height {
            Some(markright::LineHeight::Relative(r)) => format!("{r:.1}"),
            Some(markright::LineHeight::Absolute(px)) => format!("{:.1}", px.0),
            None => "1.3".into(),
        };
    }

    /// Rebuild the font combo-box: recently-used first, then the rest
    /// alphabetically. If `promote` is non-empty, move it to most-recent.
    pub fn rebuild_font_list(&mut self, fonts: &fount::Fount, promote: &str) {
        if !promote.is_empty() {
            self.recent_fonts.retain(|n| n != promote);
            self.recent_fonts.insert(0, promote.to_string());
        }
        let mut names = fonts.families();
        // Move recent picks to the front, most-recent first.
        for (i, recent) in self.recent_fonts.iter().enumerate() {
            if let Some(pos) = names.iter().position(|n| n == recent) {
                names.remove(pos);
                names.insert(i, recent.clone());
            }
        }
        self.font_list = combo_box::State::new(names);
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Save,
    Format(Format),
    Undo,
    Redo,
    FontSelected(String),
    SizeSelected(String),
    LetterSpacingInput(String),
    LetterSpacingSubmit,
    LineHeightInput(String),
    LineHeightSubmit,
    SetColor(Option<Color>),
    Pull(pull::Message),
    ToggleTheme,
    ToggleDebug,
}

pub enum Action {
    /// Internal state change only.
    None,
    /// Just refocus the editor.
    FocusEditor,
    /// Apply editor action and refocus.
    Editor(rich_editor::Action),
    /// Apply editor action without refocusing (mid-drag).
    Pending(rich_editor::Action),
    /// Font was selected — App handles loading + format application.
    FontSelected(String),
    /// Toggle application theme.
    ToggleTheme,
    /// Toggle debug panel. `opening` = new state.
    ToggleDebug { opening: bool },
    /// Save the document.
    Save,
}

pub fn update(state: &mut State, message: Message) -> Action {
    match message {
        Message::Save => Action::Save,
        Message::Format(f) => Action::Editor(f.into()),
        Message::Undo => Action::Editor(EditorAction::Undo),
        Message::Redo => Action::Editor(EditorAction::Redo),
        Message::FontSelected(name) => Action::FontSelected(name),
        Message::SizeSelected(s) => {
            if let Ok(size) = s.parse::<f32>() {
                Action::Editor(Format::SetFontSize(size).into())
            } else {
                Action::None
            }
        }
        Message::LetterSpacingInput(s) => {
            state.letter_spacing_input = s;
            Action::None
        }
        Message::LetterSpacingSubmit => {
            if let Ok(v) = state.letter_spacing_input.parse::<f32>() {
                Action::Editor(Format::SetLetterSpacing(v).into())
            } else {
                Action::FocusEditor
            }
        }
        Message::LineHeightInput(s) => {
            state.line_height_input = s;
            Action::None
        }
        Message::LineHeightSubmit => {
            if let Ok(v) = state.line_height_input.parse::<f32>() {
                Action::Editor(Format::SetLineHeight(v.into()).into())
            } else {
                Action::FocusEditor
            }
        }
        Message::SetColor(color) => Action::Editor(Format::SetColor(color).into()),
        Message::Pull(msg) => match msg {
            pull::Message::Start(pull::Kind::LetterSpacing) => {
                let current = state.letter_spacing_input.parse::<f32>().unwrap_or(0.0);
                state.pull = Some(pull::Pull::letter_spacing(current));
                Action::None
            }
            pull::Message::Start(pull::Kind::LineHeight) => {
                let current = state.line_height_input.parse::<f32>().unwrap_or(1.3);
                state.pull = Some(pull::Pull::line_height(current));
                Action::None
            }
            pull::Message::Move(position) => {
                if let Some(ref mut p) = state.pull {
                    let value = p.moved(position);
                    match p {
                        pull::Pull::LetterSpacing(_) => {
                            state.letter_spacing_input = format!("{value:.2}");
                            Action::Pending(Format::SetLetterSpacing(value).into())
                        }
                        pull::Pull::LineHeight(_) => {
                            state.line_height_input = format!("{value}");
                            Action::Pending(Format::SetLineHeight(value.into()).into())
                        }
                    }
                } else {
                    Action::None
                }
            }
            pull::Message::End => {
                state.pull.take();
                Action::FocusEditor
            }
        },
        Message::ToggleTheme => Action::ToggleTheme,
        Message::ToggleDebug => {
            state.show_debug = !state.show_debug;
            Action::ToggleDebug {
                opening: state.show_debug,
            }
        }
    }
}

pub fn subscription(state: &State) -> Subscription<Message> {
    pull::subscription(&state.pull).map(Message::Pull)
}

/// Wrap content in a subtle group container with fixed height matching buttons.
fn group<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> container::Container<'a, M> {
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

pub fn view<'a>(
    state: &'a State,
    ctx: &cursor::Context,
    is_dark: bool,
    can_undo: bool,
    can_redo: bool,
) -> Element<'a, Message> {
    let save_btn = button(icon::save().size(16))
        .padding([4, 8])
        .style(theme::button::icon)
        .on_press(Message::Save);

    let mut undo_btn = button(icon::undo().size(16))
        .padding([4, 8])
        .style(theme::button::icon);
    if can_undo {
        undo_btn = undo_btn.on_press(Message::Undo);
    }

    let mut redo_btn = button(icon::redo().size(16))
        .padding([4, 8])
        .style(theme::button::icon);
    if can_redo {
        redo_btn = redo_btn.on_press(Message::Redo);
    }

    let bold_btn = button(icon::bold().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(ctx.character.bold))
        .on_press(Message::Format(Format::ToggleBold));

    let italic_btn = button(icon::italic().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(ctx.character.italic))
        .on_press(Message::Format(Format::ToggleItalic));

    let underline_btn = button(icon::underline().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(ctx.character.underline))
        .on_press(Message::Format(Format::ToggleUnderline));

    let is_left = ctx.paragraph.alignment == Alignment::Left;
    let is_center = ctx.paragraph.alignment == Alignment::Center;
    let is_right = ctx.paragraph.alignment == Alignment::Right;
    let is_justify = ctx.paragraph.alignment == Alignment::Justified;

    let align_left_btn = button(icon::text_align_start().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(is_left))
        .on_press(Message::Format(Format::SetAlignment(Alignment::Left)));

    let align_center_btn = button(icon::text_align_center().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(is_center))
        .on_press(Message::Format(Format::SetAlignment(Alignment::Center)));

    let align_right_btn = button(icon::text_align_end().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(is_right))
        .on_press(Message::Format(Format::SetAlignment(Alignment::Right)));

    let align_justify_btn = button(icon::text_align_justify().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(is_justify))
        .on_press(Message::Format(Format::SetAlignment(Alignment::Justified)));

    let is_bullet = matches!(ctx.paragraph.style.list, Some(paragraph::List::Bullet(_)));
    let is_ordered = matches!(ctx.paragraph.style.list, Some(paragraph::List::Ordered(_)));

    let bullet_btn = button(icon::list().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(is_bullet))
        .on_press(Message::Format(Format::SetList(Some(
            paragraph::List::Bullet(paragraph::Bullet::Disc),
        ))));

    let ordered_btn = button(icon::list_ordered().size(16))
        .padding([4, 8])
        .style(theme::button::toolbar_toggle(is_ordered))
        .on_press(Message::Format(Format::SetList(Some(
            paragraph::List::Ordered(paragraph::Number::Arabic),
        ))));

    let indent_btn = button(icon::indent_increase().size(16))
        .padding([4, 8])
        .style(theme::button::icon)
        .on_press(Message::Format(Format::IndentList));

    let dedent_btn = button(icon::indent_decrease().size(16))
        .padding([4, 8])
        .style(theme::button::icon)
        .on_press(Message::Format(Format::DedentList));

    let theme_icon = if is_dark {
        icon::sun().size(16)
    } else {
        icon::moon().size(16)
    };
    let theme_toggle = button(theme_icon)
        .padding([4, 8])
        .style(theme::button::icon)
        .on_press(Message::ToggleTheme);

    let size = ctx.character.size.unwrap_or(crate::BASE_SIZE);

    let file_group = group(row![save_btn]);
    let history_group = group(row![undo_btn, redo_btn].spacing(GROUP_SPACING));
    let format_group = group(row![bold_btn, italic_btn, underline_btn].spacing(GROUP_SPACING));
    let list_group =
        group(row![bullet_btn, ordered_btn, dedent_btn, indent_btn].spacing(GROUP_SPACING));
    let align_group = group(
        row![
            align_left_btn,
            align_center_btn,
            align_right_btn,
            align_justify_btn,
        ]
        .spacing(GROUP_SPACING),
    );

    let current_font = font_name(ctx.character.font);
    let font_selector = combo_box(
        &state.font_list,
        "Font…",
        Some(&current_font),
        Message::FontSelected,
    )
    .width(140)
    .size(12)
    .input_style(theme::combo_box::toolbar)
    .menu_style(theme::combo_box::toolbar_menu);

    let current_size = format!("{}", size as u32);
    let size_selector = combo_box(
        &state.size_list,
        "Size",
        Some(&current_size),
        Message::SizeSelected,
    )
    .width(50)
    .size(12)
    .align_x(iced::Alignment::End)
    .input_style(theme::combo_box::toolbar)
    .menu_style(theme::combo_box::toolbar_menu);

    let font_group = group(
        row![font_selector, size_selector]
            .spacing(GROUP_SPACING)
            .align_y(iced::Alignment::Center),
    );

    let letter_spacing_label = mouse_area(container(icon::whole_word().size(16)).padding([0, 4]))
        .interaction(mouse::Interaction::ResizingHorizontally)
        .on_press(Message::Pull(pull::Message::Start(
            pull::Kind::LetterSpacing,
        )));

    let letter_spacing_input = text_input("0", &state.letter_spacing_input)
        .on_input(Message::LetterSpacingInput)
        .on_submit(Message::LetterSpacingSubmit)
        .width(48)
        .size(12)
        .align_x(iced::Alignment::End)
        .style(theme::combo_box::toolbar);

    let line_height_label =
        mouse_area(container(icon::list_chevrons_up_down().size(16)).padding([0, 4]))
            .interaction(mouse::Interaction::ResizingVertically)
            .on_press(Message::Pull(pull::Message::Start(pull::Kind::LineHeight)));
    let line_height_input = text_input("1", &state.line_height_input)
        .on_input(Message::LineHeightInput)
        .on_submit(Message::LineHeightSubmit)
        .width(48)
        .size(12)
        .align_x(iced::Alignment::End)
        .style(theme::combo_box::toolbar);

    let spacing_group = group(
        row![
            letter_spacing_label,
            letter_spacing_input,
            line_height_label,
            line_height_input,
        ]
        .spacing(GROUP_SPACING)
        .align_y(iced::Alignment::Center),
    );

    let current_color = ctx.character.color;

    let mut swatch_row = row![]
        .spacing(GROUP_SPACING * 2.0)
        .align_y(iced::Alignment::Center);
    for &color in COLOR_SWATCHES {
        let active = current_color == color;
        swatch_row = swatch_row
            .push(
                button(
                    container("")
                        .width(12)
                        .height(12)
                        .style(move |_| theme::swatch::style(color, active)),
                )
                .on_press(Message::SetColor(color))
                .style(theme::button::icon)
                .padding([4, 2]),
            )
            .padding([0, 2]);
    }
    let color_group = group(swatch_row);

    let debug_btn = button(text("{*}").size(12))
        .padding([4, 8])
        .style(theme::button::icon)
        .on_press(Message::ToggleDebug);

    let mut toolbar_row = row![
        file_group,
        history_group,
        format_group,
        list_group,
        align_group,
        font_group,
        spacing_group,
        color_group,
        Space::new().width(Length::Fill),
        group(row![debug_btn, theme_toggle]),
    ]
    .spacing(TOOLBAR_SPACING)
    .align_y(iced::Alignment::Center);

    if state.show_debug {
        toolbar_row = toolbar_row.push(Space::new().width(crate::debug::PANEL_W - TOOLBAR_SPACING));
    }

    container(toolbar_row)
        .style(theme::container::toolbar)
        .padding([8, 16])
        .width(Length::Fill)
        .into()
}
