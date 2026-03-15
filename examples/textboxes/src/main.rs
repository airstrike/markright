use std::collections::HashMap;

use iced::alignment::{self, Vertical::*};
use iced::widget::operation::focus;
use iced::widget::{button, container, row};
use iced::{Color, Element, Length, Point, Rectangle, Size, Task, color};

use function::*;
use markright::widget::rich_editor::{self, Action, Alignment, Content, Format};

mod function;
mod icon;
mod workspace;

use workspace::Id;

const BASE_SIZE: f32 = 16.0;
const TOOLBAR_H: f32 = 32.0;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Textboxes")
        .font(icon::FONT)
        .run()
}

struct App {
    state: workspace::State,
    content: HashMap<Id, Content<iced::Renderer>>,
}

#[derive(Debug, Clone)]
enum Message {
    EditStarted(#[allow(dead_code)] Id),
    EditExited(Id),
    Editor(Action),
    ToggleBold,
    ToggleItalic,
    ToggleUnderline,
    SetAlignment(Alignment),
    SetVAlign(alignment::Vertical),
    SetColor(Option<Color>),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let mut state = workspace::State::new();
        let mut content = HashMap::new();

        let id = state.insert(
            Rectangle::new(Point::new(50.0, 80.0), Size::new(280.0, 160.0)),
            Top,
        );
        content.insert(
            id,
            Content::with_text(
                "Hello, world!\n\nThis is a floating textbox. Double-click to edit.",
            ),
        );

        let id = state.insert(
            Rectangle::new(Point::new(400.0, 120.0), Size::new(260.0, 140.0)),
            Center,
        );
        content.insert(
            id,
            Content::with_text("A second textbox.\n\nDrag me around!"),
        );

        let id = state.insert(
            Rectangle::new(Point::new(200.0, 340.0), Size::new(320.0, 120.0)),
            Bottom,
        );
        content.insert(
            id,
            Content::with_text("Bottom-aligned text in a wider box."),
        );

        (Self { state, content }, Task::none())
    }

    /// Perform an action on the currently-editing content, if any.
    fn perform(&mut self, action: impl Into<Action>) {
        if let Some(content) = self.editing_content() {
            content.perform(action);
        }
    }

    /// Returns a mutable reference to the content being edited, if any.
    fn editing_content(&mut self) -> Option<&mut Content<iced::Renderer>> {
        let id = self.state.editing()?;
        self.content.get_mut(&id)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EditStarted(_) => focus("editor"),
            Message::EditExited(id) => {
                if let Some(content) = self.content.get(&id) {
                    content.perform(Action::Deselect);
                }
                Task::none()
            }
            Message::Editor(action) => {
                self.perform(action);
                Task::none()
            }
            Message::ToggleBold => {
                self.perform(Format::ToggleBold);
                focus("editor")
            }
            Message::ToggleItalic => {
                self.perform(Format::ToggleItalic);
                focus("editor")
            }
            Message::ToggleUnderline => {
                self.perform(Format::ToggleUnderline);
                focus("editor")
            }
            Message::SetAlignment(a) => {
                self.perform(Format::SetAlignment(a));
                focus("editor")
            }
            Message::SetVAlign(v) => {
                if let Some(id) = self.state.editing() {
                    self.state.set_v_align(id, v);
                }
                focus("editor")
            }
            Message::SetColor(color) => {
                self.perform(Format::SetColor(color));
                focus("editor")
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let mut ws = workspace::workspace(&self.state, |id, bx| {
            let content = &self.content[&id];

            let mut editor = rich_editor::rich_editor(content)
                .style(theme::editor::style)
                .padding(8)
                .height(Length::Fill)
                .align_y(bx.v_align())
                .size(BASE_SIZE);

            if bx.is_editing() {
                editor = editor.id("editor").on_action(Message::Editor);
            }

            let box_style = if bx.is_editing() {
                theme::textbox::active
            } else {
                theme::textbox::idle
            };

            container(editor)
                .width(bx.bounds().width)
                .height(bx.bounds().height)
                .style(box_style)
                .into()
        })
        .on_edit(Message::EditStarted)
        .on_edit_exit(Message::EditExited);

        // Mini-toolbar above the active box, centered on it.
        if let Some(id) = self.state.editing() {
            let bounds = self.state.bounds(id);
            let ctx = self.content[&id].cursor_context();
            let center_x = bounds.x + bounds.width / 2.0;
            ws = ws.push(
                Point::new(center_x, bounds.y - TOOLBAR_H - 4.0),
                mini_toolbar(&ctx, self.state.v_align(id)),
            );
        }

        ws.into()
    }
}

const COLOR_SWATCHES: &[(Option<Color>, &str)] = &[
    (None, "Default"),
    (Some(Color::BLACK), "Black"),
    (Some(color!(0xCC2626)), "Red"),
    (Some(color!(0x1A80E6)), "Blue"),
    (Some(color!(0x26A640)), "Green"),
    (Some(color!(0xB36600)), "Orange"),
];

fn btn<'a>(label: iced::widget::Text<'a>, active: bool) -> iced::widget::Button<'a, Message> {
    button(label.size(14))
        .style(theme::toolbar::button.with(active))
        .padding([4, 6])
}

fn mini_toolbar(
    cursor: &rich_editor::cursor::Context,
    v_align: alignment::Vertical,
) -> Element<'static, Message> {
    let bold_active = cursor.character.bold;
    let italic_active = cursor.character.italic;
    let underline_active = cursor.character.underline;
    let h_align = cursor.paragraph.alignment;
    let current_color = cursor.character.color;

    let bold_btn = btn(icon::bold(), bold_active).on_press(Message::ToggleBold);
    let italic_btn = btn(icon::italic(), italic_active).on_press(Message::ToggleItalic);
    let underline_btn = btn(icon::underline(), underline_active).on_press(Message::ToggleUnderline);

    let align_left = btn(icon::text_align_start(), h_align == Alignment::Left)
        .on_press(Message::SetAlignment(Alignment::Left));
    let align_center = btn(icon::text_align_center(), h_align == Alignment::Center)
        .on_press(Message::SetAlignment(Alignment::Center));
    let align_right = btn(icon::text_align_end(), h_align == Alignment::Right)
        .on_press(Message::SetAlignment(Alignment::Right));
    let align_justify = btn(icon::text_align_justify(), h_align == Alignment::Justified)
        .on_press(Message::SetAlignment(Alignment::Justified));

    let v_top = btn(icon::align_v_top(), v_align == Top).on_press(Message::SetVAlign(Top));
    let v_mid = btn(icon::align_v_center(), v_align == Center).on_press(Message::SetVAlign(Center));
    let v_bot = btn(icon::align_v_bottom(), v_align == Bottom).on_press(Message::SetVAlign(Bottom));

    let mut toolbar_row = row![
        bold_btn,
        italic_btn,
        underline_btn,
        align_left,
        align_center,
        align_right,
        align_justify,
        v_top,
        v_mid,
        v_bot,
    ]
    .spacing(2)
    .align_y(Center);

    for &(color, _label) in COLOR_SWATCHES {
        let active = current_color == color;
        toolbar_row = toolbar_row.push(
            button(
                container("")
                    .width(10)
                    .height(10)
                    .style(move |_| theme::toolbar::swatch(color, active)),
            )
            .on_press(Message::SetColor(color))
            .style(theme::toolbar::button.with(active))
            .padding([4, 3]),
        );
    }

    container(toolbar_row)
        .padding([2, 6])
        .style(theme::toolbar::group)
        .into()
}

mod theme {
    use iced::widget::{button, container};
    use iced::{Background, Border};
    use markright::widget::rich_editor;

    pub mod editor {
        use super::*;

        pub fn style(theme: &iced::Theme, status: rich_editor::Status) -> rich_editor::Style {
            let palette = theme.palette();
            let selection = if matches!(status, rich_editor::Status::Focused { .. }) {
                palette.primary.base.color.scale_alpha(0.4)
            } else {
                palette.primary.base.color.scale_alpha(0.2)
            };
            rich_editor::Style {
                background: Background::Color(iced::Color::TRANSPARENT),
                border: Border::default(),
                placeholder: palette.background.strong.color,
                value: palette.background.base.text,
                selection,
            }
        }
    }

    pub mod textbox {
        use super::*;

        pub fn idle(theme: &iced::Theme) -> container::Style {
            let palette = theme.palette();
            container::Style {
                background: Some(Background::Color(palette.background.weak.color)),
                border: Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            }
        }

        pub fn active(theme: &iced::Theme) -> container::Style {
            let palette = theme.palette();
            container::Style {
                background: Some(Background::Color(palette.background.base.color)),
                border: Border {
                    color: palette.primary.base.color.scale_alpha(0.6),
                    width: 2.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            }
        }
    }

    pub mod toolbar {
        use super::*;
        use iced::Color;

        pub fn group(theme: &iced::Theme) -> container::Style {
            let palette = theme.palette();
            container::Style {
                background: Some(Background::Color(palette.background.weak.color)),
                border: Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        }

        pub fn swatch(color: Option<Color>, active: bool) -> container::Style {
            container::Style {
                background: Some(Background::Color(
                    color.unwrap_or(Color::from_rgb(0.5, 0.5, 0.5)),
                )),
                border: Border {
                    color: if active {
                        Color::WHITE
                    } else {
                        Color::TRANSPARENT
                    },
                    width: if active { 1.5 } else { 0.0 },
                    radius: 2.0.into(),
                },
                ..Default::default()
            }
        }

        pub fn button(theme: &iced::Theme, status: button::Status, active: bool) -> button::Style {
            let palette = theme.palette();
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
                        background: Some(Background::Color(
                            palette.background.base.text.scale_alpha(0.1),
                        )),
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
}
