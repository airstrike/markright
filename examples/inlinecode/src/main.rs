//! Render-only example: a string with `` `code` `` spans → styled text.
//!
//! No editor machinery (no popup, no `computed_spans`, no `Source` trait).
//! Backticks delimit code; doubled `` `` `` escapes a literal backtick.

use std::ops::Range;
use std::rc::Rc;

use iced::advanced::text::rich_editor::span;
use iced::widget::center;
use iced::widget::text::Wrapping;
use iced::{Background, Border, Element, Font, Task, Theme, color, font};

use markright::widget::rich_editor::{self, Content, Highlight, popup};
use markright_core::{Paragraph, StyleRun, StyledLine};

const BODY: Font = Font::new("Geist");
const CODE: Font = Font::new("IBM Plex Mono");
const SRC: &str = "Table references column `metric`, which the source doesn't expose. \
                   Available: `Metric`, `Q1`, `Q2`, `Q3`, `Q4`, `FY 2026`.";

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Inline Code")
        .theme(Theme::Light)
        .centered()
        .default_font(BODY)
        .run()
}

struct App {
    content: Content<iced::Renderer>,
    highlights: Vec<Highlight>,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(rich_editor::Action),
    FontLoaded,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self::build(),
            Task::batch([load_font("IBM Plex Mono"), load_font("Geist")]),
        )
    }

    /// Build the editor content from `SRC`. Called on startup and again each
    /// time a font load resolves — the cosmic-text buffer caches its shaped
    /// lines, so we re-shape after fonts arrive to actually pick them up.
    fn build() -> Self {
        let (line, _ranges) = parse(SRC);
        let content = Content::from_styled_lines(&[line]);
        let highlights = highlights_from(&content);
        Self {
            content,
            highlights,
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(action) => {
                self.content.perform(action);
                self.highlights = highlights_from(&self.content);
            }
            Message::FontLoaded => {
                *self = Self::build();
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        center(
            rich_editor::rich_editor(&self.content)
                .on_action(Message::Edit)
                .wrapping(Wrapping::Glyph)
                .highlights(&self.highlights),
        )
        .padding(32)
        .into()
    }
}

fn parse(src: &str) -> (StyledLine, Vec<Range<usize>>) {
    let code = span::Style {
        font: Some(CODE),
        size: Some(14.0),
        color: Some(color!(0x4B5563)),
        padding: Some(iced::Padding::new(1.0).left(4.0).right(4.0)),
        ..Default::default()
    };

    let mut text = String::new();
    let mut runs = Vec::new();
    let mut ranges = Vec::new();
    let mut rest = src;

    while let Some(pos) = rest.find('`') {
        text.push_str(&rest[..pos]);
        rest = &rest[pos + 1..];
        if let Some(stripped) = rest.strip_prefix('`') {
            text.push('`');
            rest = stripped;
        } else if let Some(close) = rest.find('`') {
            let start = text.len();
            text.push_str(&rest[..close]);
            let end = text.len();
            let range = start..end;
            runs.push(StyleRun {
                range: range.clone(),
                style: code.clone(),
            });
            ranges.push(range);
            rest = &rest[close + 1..];
        } else {
            text.push('`');
        }
    }
    text.push_str(rest);

    (
        StyledLine {
            text,
            runs,
            paragraph: Paragraph::default(),
        },
        ranges,
    )
}

fn highlight_style(_: &Theme) -> popup::SpanStyle {
    popup::SpanStyle {
        background: Some(Background::Color(color!(0xF3F4F6))),
        border: Border {
            color: color!(0xD1D5DB),
            width: 1.0,
            radius: 3.0.into(),
        },
    }
}

fn highlights_from(content: &Content<iced::Renderer>) -> Vec<Highlight> {
    let mut highlights = Vec::new();
    for line_idx in 0..content.line_count() {
        if let Some(styled_line) = content.styled_line(line_idx) {
            for run in &styled_line.runs {
                if run.style.padding.is_some() {
                    highlights.push(Highlight {
                        line: line_idx,
                        range: run.range.clone(),
                        style: Rc::new(highlight_style),
                    });
                }
            }
        }
    }
    highlights
}

fn load_font(name: &'static str) -> Task<Message> {
    Task::future(async move { fount::google::load(name, None).await }).then(|result| match result {
        Ok(bytes_list) => Task::batch(
            bytes_list
                .into_iter()
                .map(|bytes| font::load(bytes).map(|_| Message::FontLoaded)),
        ),
        Err(_) => Task::done(Message::FontLoaded),
    })
}
