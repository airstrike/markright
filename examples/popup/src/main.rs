/// Popup example — demonstrates the ideal API for computed spans.
///
/// This is the TARGET API we're building toward. It may not compile yet.
/// The goal: the app provides a source string, a computed-source adapter,
/// and a popup build closure. Everything else — parsing, rendering,
/// edit interception, cursor management, popup lifecycle — is internal
/// to the widget.
use iced::widget::operation::focus;
use iced::widget::{column, container, row, text};
use iced::{Element, Shrink, Task};

use markright::widget::rich_editor::computed_spans;
use markright::widget::rich_editor::{self, Content, Instruction};

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
            .computed_popup(Message::Span, |input, span| {
                let preview = parser::eval(
                    span.source_value
                        .strip_prefix("{=")
                        .and_then(|s| s.strip_suffix('}'))
                        .unwrap_or(""),
                );
                let chars = span.source_value.len().max(8) as f32;
                let input_width = (chars * 8.5 + 32.0).min(400.0);
                container(
                    column![
                        input.size(14).width(input_width),
                        row![
                            text("= ").size(12).style(theme::equation),
                            text(preview).size(12).style(theme::preview),
                        ]
                    ]
                    .spacing(4)
                    .padding(8),
                )
                .width(Shrink)
                .style(theme::popup)
                .into()
            });

        container(editor).padding(32).max_width(640).into()
    }
}

mod theme {
    use iced::widget::{container, text};

    pub fn popup(theme: &iced::Theme) -> container::Style {
        let p = theme.palette();
        container::Style {
            background: Some(p.background.strongest.color.into()),
            border: iced::Border {
                color: p.background.weak.color,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: iced::Shadow {
                color: iced::Color::BLACK.scale_alpha(0.1),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        }
    }

    pub fn preview(theme: &iced::Theme) -> text::Style {
        text::Style {
            color: Some(theme.palette().background.strong.text),
        }
    }

    pub fn equation(theme: &iced::Theme) -> text::Style {
        text::Style {
            color: Some(theme.palette().background.weak.text),
        }
    }
}
