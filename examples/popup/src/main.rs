/// Popup example — demonstrates the ideal API for computed spans.
///
/// This is the TARGET API we're building toward. It may not compile yet.
/// The goal: the app provides a source string, a computed-source adapter,
/// and a popup build closure. Everything else — parsing, rendering,
/// edit interception, cursor management, popup lifecycle — is internal
/// to the widget.
use iced::widget::operation::focus;
use iced::widget::{center, column, container, row, slider, text};
use iced::{Center, Element, Fill, Shrink, Task};

use markright::rich_editor;
use markright::rich_editor::{Content, Instruction, computed_spans, popup};

mod adapter;
mod parser;

const SOURCE: &str = "The `users` table has {=1+3} columns. \
                      Query `SELECT *` returns {=2*6} rows.";

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Popup Example")
        .run()
}

struct App {
    content: Content<iced::Renderer>,
    code_v: f32,
    code_h: f32,
    formula_v: f32,
    formula_h: f32,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(rich_editor::Action),
    Span(computed_spans::Action),
    Instruction(Instruction),
    CodeV(f32),
    CodeH(f32),
    FormulaV(f32),
    FormulaH(f32),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let code_v = 1.0;
        let code_h = 6.0;
        let formula_v = 4.0;
        let formula_h = 3.0;
        let content = Content::from_computed(
            SOURCE,
            adapter::Adapter {
                code_style: theme::code_style(code_v, code_h),
                formula_style: theme::formula_style(formula_v, formula_h),
            },
        );

        (
            Self {
                content,
                code_v,
                code_h,
                formula_v,
                formula_h,
            },
            focus("popup-editor"),
        )
    }

    fn rebuild(&mut self) {
        let source = self.content.source().unwrap_or_else(|| SOURCE.to_string());
        self.content = Content::from_computed(
            &source,
            adapter::Adapter {
                code_style: theme::code_style(self.code_v, self.code_h),
                formula_style: theme::formula_style(self.formula_v, self.formula_h),
            },
        );
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => {
                self.content.perform(action);
            }
            Message::Span(action) => {
                self.content.span_perform(action);
            }
            Message::Instruction(Instruction::Focus(id)) => return focus(id),

            Message::CodeV(v) => {
                self.code_v = v;
                self.rebuild();
            }
            Message::CodeH(v) => {
                self.code_h = v;
                self.rebuild();
            }
            Message::FormulaV(v) => {
                self.formula_v = v;
                self.rebuild();
            }
            Message::FormulaH(v) => {
                self.formula_h = v;
                self.rebuild();
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let editor = rich_editor(&self.content)
            .id("popup-editor")
            .size(16)
            .line_height(1.6)
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
                        text!("={preview}").size(12)
                    ]
                    .spacing(4)
                    .padding(8),
                )
                .width(Shrink)
                .style(theme::popup)
                .into()
            })
            .padding(5)
            .width(300);

        let code_controls = column![
            text("Code spans").size(12),
            labeled_slider("V", self.code_v, Message::CodeV),
            labeled_slider("H", self.code_h, Message::CodeH),
        ]
        .spacing(4);

        let formula_controls = column![
            text("Formula spans").size(12),
            labeled_slider("V", self.formula_v, Message::FormulaV),
            labeled_slider("H", self.formula_h, Message::FormulaH),
        ]
        .spacing(4);

        let controls = row![code_controls, formula_controls].spacing(24).padding(8);

        center(column![editor, controls].spacing(16).width(300))
            .padding(32)
            .into()
    }
}

fn labeled_slider<'a>(
    label: &'a str,
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        text!("{label}").size(11).width(16),
        slider(0.0..=12.0, value, on_change).step(0.5).width(Fill),
        text!("{value:.1}").size(11).width(30),
    ]
    .spacing(6)
    .align_y(Center)
    .into()
}

pub mod theme {
    use iced::advanced::text::rich_editor::span;
    use iced::widget::{container, text_input};
    use markright::rich_editor::popup;

    pub fn formula_chip(theme: &iced::Theme) -> popup::SpanStyle {
        let palette = theme.palette();

        popup::SpanStyle {
            background: Some(palette.primary.weak.color.into()),
            border: iced::Border {
                color: palette.primary.base.color,
                width: 1.0,
                radius: 3.0.into(),
            },
        }
    }

    pub fn formula_style(v: f32, h: f32) -> span::Style {
        span::Style {
            bold: Some(true),
            padding: Some(iced::Padding::new(v).left(h).right(h)),
            ..Default::default()
        }
    }

    pub fn code_chip(theme: &iced::Theme) -> popup::SpanStyle {
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

    pub fn code_style(v: f32, h: f32) -> span::Style {
        span::Style {
            font: Some(iced::Font::MONOSPACE),
            size: Some(14.0),
            padding: Some(iced::Padding::new(v).left(h).right(h)),
            ..Default::default()
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
