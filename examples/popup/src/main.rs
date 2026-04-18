/// Popup example — demonstrates the ideal API for computed spans.
///
/// This is the TARGET API we're building toward. It may not compile yet.
/// The goal: the app provides a source string, a computed-source adapter,
/// and a popup build closure. Everything else — parsing, rendering,
/// edit interception, cursor management, popup lifecycle — is internal
/// to the widget.
use iced::widget::operation::focus;
use iced::widget::{column, container, row, text};
use iced::{Element, Length, Task, color};

use markright::widget::rich_editor::computed_spans;
use markright::widget::rich_editor::popup;
use markright::widget::rich_editor::{self, Content, Instruction};

mod adapter;
mod parser;

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

        (Self { content }, Task::none())
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
                            .on_input(on_input)
                            .size(14),
                        row![
                            text("= ").size(12).color(color!(0x94A3B8)),
                            text(preview).size(12).color(color!(0x334155)),
                        ]
                    ]
                    .spacing(4)
                    .padding(8),
                )
                .width(Length::Shrink)
                .style(|_theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(color!(0xFFFFFF))),
                    border: iced::Border {
                        color: color!(0xCBD5E1),
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    shadow: iced::Shadow {
                        color: color!(0x000000, 0.1),
                        offset: iced::Vector::new(0.0, 4.0),
                        blur_radius: 12.0,
                    },
                    ..Default::default()
                })
                .into()
            });

        container(editor).padding(32).max_width(640).into()
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Popup Example")
        .run()
}
