/// Popup example — demonstrates the ideal API for computed spans.
///
/// This is the TARGET API we're building toward. It may not compile yet.
/// The goal: the app provides a source string, a computed-source adapter,
/// and a popup build closure. Everything else — parsing, rendering,
/// edit interception, cursor management, popup lifecycle — is internal
/// to the widget.
use iced::widget::operation::focus;
use iced::widget::{center, column, container, row, text};
use iced::{Element, Shrink, Task};

use markright::rich_editor::computed_spans;
use markright::rich_editor::popup;
use markright::rich_editor::{self, Content, Instruction};

mod adapter;
mod parser;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Popup Example")
        .run()
}

struct App {
    content: Content<iced::Renderer>,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(rich_editor::Action),
    Span(computed_spans::Action),
    Instruction(Instruction),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let source = "Tim had {=1+3} apples, then picked up {=2*6} more at the store.";
        let content = Content::from_computed(source, adapter::Adapter);

        (Self { content }, focus("popup-editor"))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => {
                self.content.perform(action);
                Task::none()
            }
            Message::Span(action) => {
                self.content.apply_span_action(action);
                Task::none()
            }
            Message::Instruction(Instruction::Focus(id)) => focus(id),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let editor = rich_editor::rich_editor(&self.content)
            .id("popup-editor")
            .on_action(Message::Editor)
            .on_instruction(Message::Instruction)
            .computed_popup(Message::Span, |span, on_input| {
                let preview = parser::eval(
                    span.source_value
                        .strip_prefix("{=")
                        .and_then(|s| s.strip_suffix('}'))
                        .unwrap_or(""),
                );
                container(
                    column![
                        popup::input(&span.placeholder, &span.source_value)
                            .style(theme::input)
                            .on_input(on_input)
                            .size(14),
                        row![
                            text("= ").size(12),    // .color(color!(0x94A3B8)),
                            text(preview).size(12), // .color(color!(0x334155)),
                        ]
                    ]
                    .spacing(4)
                    .padding(8),
                )
                .width(Shrink)
                .style(theme::popup)
                .into()
            })
            .width(300);

        center(editor).padding(32).into()
    }
}

pub mod theme {
    use iced::widget::{container, text_input};
    use markright::rich_editor::popup;

    pub fn chip(theme: &iced::Theme) -> popup::SpanStyle {
        let palette = theme.palette();

        popup::SpanStyle {
            background: Some(palette.background.weak.color.into()),
            border: iced::Border {
                color: palette.background.strong.color,
                width: 1.0,
                radius: 3.0.into(),
            },
        }
    }

    pub fn input(theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
        let palette = theme.palette();

        let active = text_input::Style {
            background: palette.background.strongest.color.into(),
            value: palette.background.strongest.text,
            border: iced::Border {
                color: palette.background.strong.color,
                width: 1.0,
                radius: 4.0.into(),
            },
            icon: palette.background.weak.text,
            placeholder: palette.secondary.base.color,
            selection: palette.primary.weak.color,
        };

        match status {
            text_input::Status::Active => active,
            text_input::Status::Focused { .. } => active,
            text_input::Status::Hovered => active,
            text_input::Status::Disabled => active,
        }
    }

    pub fn popup(theme: &iced::Theme) -> container::Style {
        let palette = theme.palette();

        container::Style {
            background: Some(palette.background.weakest.color.into()),
            border: iced::Border {
                color: palette.background.strong.color,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: iced::Shadow {
                color: iced::Color::BLACK.scale_alpha(0.1),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            text_color: Some(palette.background.weakest.text),
            ..Default::default()
        }
    }
}
