mod icon;
mod workspace;

use std::collections::HashMap;

use iced::alignment;
use iced::widget::operation::focus;
use iced::widget::{button, container, row};
use iced::{Background, Border, Element, Length, Point, Rectangle, Size, Task};

use markright::widget::rich_editor::{self, Action, Content, Edit, FormatAction};

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
    EditExited,
    Editor(Action),
    ToggleBold,
    ToggleItalic,
    ToggleUnderline,
    SetVAlign(alignment::Vertical),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let mut state = workspace::State::new();
        let mut content = HashMap::new();

        let id = state.insert(
            Rectangle::new(Point::new(50.0, 80.0), Size::new(280.0, 160.0)),
            alignment::Vertical::Top,
        );
        content.insert(
            id,
            Content::with_text(
                "Hello, world!\n\nThis is a floating textbox. Double-click to edit.",
            ),
        );

        let id = state.insert(
            Rectangle::new(Point::new(400.0, 120.0), Size::new(260.0, 140.0)),
            alignment::Vertical::Center,
        );
        content.insert(
            id,
            Content::with_text("A second textbox.\n\nDrag me around!"),
        );

        let id = state.insert(
            Rectangle::new(Point::new(200.0, 340.0), Size::new(320.0, 120.0)),
            alignment::Vertical::Bottom,
        );
        content.insert(
            id,
            Content::with_text("Bottom-aligned text in a wider box."),
        );

        (Self { state, content }, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EditStarted(_) => focus("editor"),
            Message::EditExited => Task::none(),
            Message::Editor(action) => {
                if let Some(id) = self.state.editing() {
                    self.content.get_mut(&id).unwrap().perform(action);
                }
                Task::none()
            }
            Message::ToggleBold => {
                if let Some(id) = self.state.editing() {
                    self.content
                        .get_mut(&id)
                        .unwrap()
                        .perform(Action::Edit(Edit::Format(FormatAction::ToggleBold)));
                }
                focus("editor")
            }
            Message::ToggleItalic => {
                if let Some(id) = self.state.editing() {
                    self.content
                        .get_mut(&id)
                        .unwrap()
                        .perform(Action::Edit(Edit::Format(FormatAction::ToggleItalic)));
                }
                focus("editor")
            }
            Message::ToggleUnderline => {
                if let Some(id) = self.state.editing() {
                    self.content
                        .get_mut(&id)
                        .unwrap()
                        .perform(Action::Edit(Edit::Format(FormatAction::ToggleUnderline)));
                }
                focus("editor")
            }
            Message::SetVAlign(v) => {
                if let Some(id) = self.state.editing() {
                    self.state.set_v_align(id, v);
                }
                focus("editor")
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let mut ws = workspace::workspace(&self.state, |id, bx| {
            let content = &self.content[&id];

            let editor: Element<'_, Message> = if bx.is_editing() {
                rich_editor::rich_editor(content)
                    .id("editor")
                    .on_action(Message::Editor)
                    .style(editor_style)
                    .padding(8)
                    .height(Length::Shrink)
                    .size(BASE_SIZE)
                    .into()
            } else {
                rich_editor::rich_editor::<Message, _, _>(content)
                    .style(editor_style)
                    .padding(8)
                    .height(Length::Shrink)
                    .size(BASE_SIZE)
                    .into()
            };

            let box_style = if bx.is_editing() {
                textbox_style_active
            } else {
                textbox_style_idle
            };

            container(editor)
                .align_y(bx.v_align())
                .width(bx.bounds().width)
                .height(bx.bounds().height)
                .style(box_style)
                .into()
        })
        .on_edit(Message::EditStarted)
        .on_edit_exit(Message::EditExited);

        // Mini-toolbar above the active box.
        if let Some(id) = self.state.editing() {
            let bounds = self.state.bounds(id);
            let ctx = self.content[&id].cursor_context();
            ws = ws.push(
                Point::new(bounds.x, bounds.y - TOOLBAR_H - 4.0),
                Size::new(bounds.width, TOOLBAR_H),
                mini_toolbar(&ctx, self.state.v_align(id)),
            );
        }

        ws.into()
    }
}

fn mini_toolbar(
    cursor: &rich_editor::cursor::Context,
    v_align: alignment::Vertical,
) -> Element<'static, Message> {
    let bold_active = cursor.character.bold;
    let italic_active = cursor.character.italic;
    let underline_active = cursor.character.underline;

    let bold_btn = button(icon::bold().size(14))
        .on_press(Message::ToggleBold)
        .style(move |theme, status| toggle_btn_style(theme, status, bold_active))
        .padding([4, 6]);

    let italic_btn = button(icon::italic().size(14))
        .on_press(Message::ToggleItalic)
        .style(move |theme, status| toggle_btn_style(theme, status, italic_active))
        .padding([4, 6]);

    let underline_btn = button(icon::underline().size(14))
        .on_press(Message::ToggleUnderline)
        .style(move |theme, status| toggle_btn_style(theme, status, underline_active))
        .padding([4, 6]);

    let v_top = button(icon::align_v_top().size(14))
        .on_press(Message::SetVAlign(alignment::Vertical::Top))
        .style(move |theme, status| {
            toggle_btn_style(theme, status, v_align == alignment::Vertical::Top)
        })
        .padding([4, 6]);

    let v_mid = button(icon::align_v_center().size(14))
        .on_press(Message::SetVAlign(alignment::Vertical::Center))
        .style(move |theme, status| {
            toggle_btn_style(theme, status, v_align == alignment::Vertical::Center)
        })
        .padding([4, 6]);

    let v_bot = button(icon::align_v_bottom().size(14))
        .on_press(Message::SetVAlign(alignment::Vertical::Bottom))
        .style(move |theme, status| {
            toggle_btn_style(theme, status, v_align == alignment::Vertical::Bottom)
        })
        .padding([4, 6]);

    container(
        row![bold_btn, italic_btn, underline_btn, v_top, v_mid, v_bot]
            .spacing(2)
            .align_y(alignment::Vertical::Center),
    )
    .padding([2, 6])
    .style(toolbar_container_style)
    .into()
}

// --- Styles ---

fn editor_style(theme: &iced::Theme, status: rich_editor::Status) -> rich_editor::Style {
    let palette = theme.extended_palette();
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

fn textbox_style_idle(theme: &iced::Theme) -> container::Style {
    let palette = theme.extended_palette();
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

fn textbox_style_active(theme: &iced::Theme) -> container::Style {
    let palette = theme.extended_palette();
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

fn toolbar_container_style(theme: &iced::Theme) -> container::Style {
    let palette = theme.extended_palette();
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

fn toggle_btn_style(theme: &iced::Theme, status: button::Status, active: bool) -> button::Style {
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
