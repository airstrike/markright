//! Popup example — demonstrates the ideal API for computed spans.
//!
//! This is the TARGET API we're building toward. It may not compile yet.
//! The goal: the app provides a source string, a computed-source adapter,
//! and a popup build closure. Everything else — parsing, rendering,
//! edit interception, cursor management, popup lifecycle — is internal
//! to the widget.

use iced::widget::operation::focus;
use iced::widget::{column, container, row, text};
use iced::{Element, Length, Task, color};

use markright::widget::rich_editor::computed_spans;
use markright::widget::rich_editor::{self, Content, Instruction};

// ── the adapter ───────────────────────────────────────────────────────
//
// The app defines how to parse source text into display content +
// computed spans. This is the only formula-specific code.

struct FormulaAdapter;

impl computed_spans::Source for FormulaAdapter {
    fn parse(&self, source: &str) -> computed_spans::ParseResult {
        let mut display = String::new();
        let mut spans = Vec::new();
        let mut source_offset = 0;

        for segment in FormulaSegments::new(source) {
            match segment {
                Segment::Text(t) => {
                    display.push_str(t);
                    source_offset += t.len();
                }
                Segment::Formula { expr, raw } => {
                    let value = eval(expr);
                    let display_start = display.len();
                    display.push_str(&value);
                    let display_end = display.len();

                    spans.push(computed_spans::ComputedSpan {
                        id: source_offset as u64,
                        line: 0,
                        display_range: display_start..display_end,
                        source_range: source_offset..source_offset + raw.len(),
                        source_value: raw.to_string(),
                        display_value: value,
                        placeholder: "{=expr}".to_string(),
                        background: Some(iced::Background::Color(color!(0xFAF9F5))),
                        border: iced::Border {
                            color: color!(0xE5E4DC),
                            width: 1.0,
                            radius: 3.0.into(),
                        },
                        atomic: true,
                    });

                    source_offset += raw.len();
                }
            }
        }

        computed_spans::ParseResult {
            lines: vec![markright_core::StyledLine {
                text: display,
                runs: vec![],
                paragraph: markright_core::Paragraph::default(),
            }],
            spans,
        }
    }
}

// ── app ───────────────────────────────────────────────────────────────

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
        let content = Content::from_computed(source, FormulaAdapter);

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
            .id("formula-editor")
            .on_action(Message::Editor)
            .on_instruction(Message::Instruction)
            .on_computed_action(Message::Span)
            .computed_popup(|input, span| {
                let preview = eval(
                    span.source_value
                        .strip_prefix("{=")
                        .and_then(|s| s.strip_suffix('}'))
                        .unwrap_or(""),
                );
                container(
                    column![
                        input.size(14),
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

// ── formula parsing (minimal, app-specific) ───────────────────────────

enum Segment<'a> {
    Text(&'a str),
    Formula { expr: &'a str, raw: &'a str },
}

struct FormulaSegments<'a> {
    remaining: &'a str,
}

impl<'a> FormulaSegments<'a> {
    fn new(source: &'a str) -> Self {
        Self { remaining: source }
    }
}

impl<'a> Iterator for FormulaSegments<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        if let Some(pos) = self.remaining.find("{=") {
            if pos > 0 {
                let text = &self.remaining[..pos];
                self.remaining = &self.remaining[pos..];
                return Some(Segment::Text(text));
            }

            if let Some(end) = self.remaining[2..].find('}') {
                let raw = &self.remaining[..end + 3];
                let expr = &self.remaining[2..end + 2];
                self.remaining = &self.remaining[end + 3..];
                return Some(Segment::Formula { expr, raw });
            }
        }

        let text = self.remaining;
        self.remaining = "";
        Some(Segment::Text(text))
    }
}

fn eval(expr: &str) -> String {
    // Trivial evaluator for the example
    let expr = expr.trim();
    if let Some((a, b)) = expr.split_once('+') {
        if let (Ok(a), Ok(b)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
            return format!("{}", a + b);
        }
    }
    if let Some((a, b)) = expr.split_once('*') {
        if let (Ok(a), Ok(b)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
            return format!("{}", a * b);
        }
    }
    if let Ok(n) = expr.parse::<f64>() {
        return format!("{n}");
    }
    format!("?{expr}")
}
