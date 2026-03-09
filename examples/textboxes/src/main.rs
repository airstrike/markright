mod workspace;

use iced::alignment;
use iced::keyboard;
use iced::widget::operation::focus;
use iced::widget::{button, container, mouse_area, row, text};
use iced::{Background, Border, Element, Length, Point, Size, Subscription, Task};

use markright::widget::rich_editor::{self, Action, Content, Edit, FormatAction, Status, Style};

use workspace::{Child, Workspace};

const BASE_SIZE: f32 = 16.0;
const TOOLBAR_H: f32 = 32.0;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Textboxes")
        .subscription(App::subscription)
        .run()
}

struct App {
    boxes: Vec<TextBox>,
    interaction: Interaction,
}

struct TextBox {
    position: Point,
    size: Size,
    content: Content<iced::Renderer>,
    v_align: alignment::Vertical,
}

#[derive(Debug, Clone, Copy, Default)]
enum Interaction {
    #[default]
    Idle,
    Pressed {
        id: usize,
    },
    Dragging {
        id: usize,
        origin: Point,
        box_origin: Point,
    },
    Editing {
        id: usize,
    },
}

#[derive(Debug, Clone)]
enum Message {
    BoxPressed(usize),
    MouseMoved(Point),
    MouseReleased,
    BoxDoubleClicked(usize),
    Editor(Action),
    ExitEdit,
    Ignored,
    ToggleBold,
    ToggleItalic,
    ToggleUnderline,
    SetVAlign(alignment::Vertical),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let boxes = vec![
            TextBox {
                position: Point::new(50.0, 80.0),
                size: Size::new(280.0, 160.0),
                content: Content::with_text(
                    "Hello, world!\n\nThis is a floating textbox. Double-click to edit.",
                ),
                v_align: alignment::Vertical::Top,
            },
            TextBox {
                position: Point::new(400.0, 120.0),
                size: Size::new(260.0, 140.0),
                content: Content::with_text("A second textbox.\n\nDrag me around!"),
                v_align: alignment::Vertical::Center,
            },
            TextBox {
                position: Point::new(200.0, 340.0),
                size: Size::new(320.0, 120.0),
                content: Content::with_text("Bottom-aligned text in a wider box."),
                v_align: alignment::Vertical::Bottom,
            },
        ];

        (
            Self {
                boxes,
                interaction: Interaction::Idle,
            },
            Task::none(),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        keyboard::listen().map(|event| match event {
            keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            } => Message::ExitEdit,
            _ => Message::Ignored,
        })
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ignored => Task::none(),
            Message::BoxPressed(id) => {
                if matches!(self.interaction, Interaction::Editing { id: eid } if eid == id) {
                    return Task::none();
                }
                self.interaction = Interaction::Pressed { id };
                Task::none()
            }
            Message::MouseMoved(point) => {
                match self.interaction {
                    Interaction::Pressed { id } => {
                        self.interaction = Interaction::Dragging {
                            id,
                            origin: point,
                            box_origin: self.boxes[id].position,
                        };
                    }
                    Interaction::Dragging {
                        id,
                        origin,
                        box_origin,
                    } => {
                        let dx = point.x - origin.x;
                        let dy = point.y - origin.y;
                        self.boxes[id].position = Point::new(box_origin.x + dx, box_origin.y + dy);
                    }
                    _ => {}
                }
                Task::none()
            }
            Message::MouseReleased => {
                match self.interaction {
                    Interaction::Pressed { .. } | Interaction::Dragging { .. } => {
                        self.interaction = Interaction::Idle;
                    }
                    _ => {}
                }
                Task::none()
            }
            Message::BoxDoubleClicked(id) => {
                self.interaction = Interaction::Editing { id };
                focus("editor")
            }
            Message::Editor(action) => {
                if let Interaction::Editing { id } = self.interaction {
                    self.boxes[id].content.perform(action);
                }
                Task::none()
            }
            Message::ExitEdit => {
                if matches!(self.interaction, Interaction::Editing { .. }) {
                    self.interaction = Interaction::Idle;
                }
                Task::none()
            }
            Message::ToggleBold => {
                if let Interaction::Editing { id } = self.interaction {
                    self.boxes[id]
                        .content
                        .perform(Action::Edit(Edit::Format(FormatAction::ToggleBold)));
                }
                focus("editor")
            }
            Message::ToggleItalic => {
                if let Interaction::Editing { id } = self.interaction {
                    self.boxes[id]
                        .content
                        .perform(Action::Edit(Edit::Format(FormatAction::ToggleItalic)));
                }
                focus("editor")
            }
            Message::ToggleUnderline => {
                if let Interaction::Editing { id } = self.interaction {
                    self.boxes[id]
                        .content
                        .perform(Action::Edit(Edit::Format(FormatAction::ToggleUnderline)));
                }
                focus("editor")
            }
            Message::SetVAlign(v) => {
                if let Interaction::Editing { id } = self.interaction {
                    self.boxes[id].v_align = v;
                }
                focus("editor")
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let editing_id = match self.interaction {
            Interaction::Editing { id } => Some(id),
            _ => None,
        };

        let children: Vec<Child<'_, Message>> = self
            .boxes
            .iter()
            .enumerate()
            .flat_map(|(i, tb)| {
                let is_editing = editing_id == Some(i);
                let mut items = Vec::new();

                // Mini-toolbar above the editing textbox
                if is_editing {
                    let cursor = tb.content.cursor_context();
                    let toolbar = mini_toolbar(&cursor, tb.v_align);
                    items.push(Child {
                        position: Point::new(tb.position.x, tb.position.y - TOOLBAR_H - 4.0),
                        size: Size::new(tb.size.width, TOOLBAR_H),
                        element: toolbar,
                    });
                }

                let editor: Element<'_, Message> = if is_editing {
                    // Active editor
                    container(
                        rich_editor::rich_editor(&tb.content)
                            .id("editor")
                            .on_action(Message::Editor)
                            .style(editor_style)
                            .padding(8)
                            .height(Length::Shrink)
                            .size(BASE_SIZE),
                    )
                    .align_y(tb.v_align)
                    .width(tb.size.width)
                    .height(tb.size.height)
                    .style(textbox_style_active)
                    .into()
                } else {
                    // Display-only
                    mouse_area(
                        container(
                            rich_editor::rich_editor::<Message, _, _>(&tb.content)
                                .style(editor_style)
                                .padding(8)
                                .height(Length::Shrink)
                                .size(BASE_SIZE),
                        )
                        .align_y(tb.v_align)
                        .width(tb.size.width)
                        .height(tb.size.height)
                        .style(textbox_style_idle),
                    )
                    .interaction(iced::mouse::Interaction::Pointer)
                    .on_press(Message::BoxPressed(i))
                    .on_double_click(Message::BoxDoubleClicked(i))
                    .into()
                };

                items.push(Child {
                    position: tb.position,
                    size: tb.size,
                    element: editor,
                });

                items
            })
            .collect();

        let workspace: Element<'_, Message> = Workspace::new(children).into();

        // Wrap in mouse_area for drag tracking
        mouse_area(workspace)
            .on_move(Message::MouseMoved)
            .on_release(Message::MouseReleased)
            .into()
    }
}

fn mini_toolbar(
    cursor: &rich_editor::cursor::Context,
    v_align: alignment::Vertical,
) -> Element<'static, Message> {
    let bold_active = cursor.character.bold;
    let italic_active = cursor.character.italic;
    let underline_active = cursor.character.underline;

    let bold_btn = button(text("B").size(13))
        .on_press(Message::ToggleBold)
        .style(move |theme, status| toggle_btn_style(theme, status, bold_active))
        .padding([2, 8]);

    let italic_btn = button(text("I").size(13))
        .on_press(Message::ToggleItalic)
        .style(move |theme, status| toggle_btn_style(theme, status, italic_active))
        .padding([2, 8]);

    let underline_btn = button(text("U").size(13))
        .on_press(Message::ToggleUnderline)
        .style(move |theme, status| toggle_btn_style(theme, status, underline_active))
        .padding([2, 8]);

    let v_top = button(text("T").size(11))
        .on_press(Message::SetVAlign(alignment::Vertical::Top))
        .style(move |theme, status| {
            toggle_btn_style(theme, status, v_align == alignment::Vertical::Top)
        })
        .padding([2, 6]);

    let v_mid = button(text("M").size(11))
        .on_press(Message::SetVAlign(alignment::Vertical::Center))
        .style(move |theme, status| {
            toggle_btn_style(theme, status, v_align == alignment::Vertical::Center)
        })
        .padding([2, 6]);

    let v_bot = button(text("B").size(11))
        .on_press(Message::SetVAlign(alignment::Vertical::Bottom))
        .style(move |theme, status| {
            toggle_btn_style(theme, status, v_align == alignment::Vertical::Bottom)
        })
        .padding([2, 6]);

    container(
        row![
            bold_btn,
            italic_btn,
            underline_btn,
            text("  ").size(8),
            v_top,
            v_mid,
            v_bot
        ]
        .spacing(2)
        .align_y(alignment::Vertical::Center),
    )
    .padding([2, 6])
    .style(toolbar_container_style)
    .into()
}

// --- Styles ---

fn editor_style(theme: &iced::Theme, status: Status) -> Style {
    let palette = theme.extended_palette();
    let selection = if matches!(status, Status::Focused { .. }) {
        palette.primary.base.color.scale_alpha(0.4)
    } else {
        palette.primary.base.color.scale_alpha(0.2)
    };
    Style {
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
