#![cfg(feature = "computed_spans")]

use markright::rich_editor::computed_spans;
use markright::rich_editor::popup;
use markright::rich_editor::{Content, Edit};

use iced::advanced::text::rich_editor::span;
use std::rc::Rc;

type C = Content<iced::Renderer>;

struct CodeAdapter;

fn code_style() -> span::Style {
    span::Style {
        font: Some(iced::Font::MONOSPACE),
        size: Some(14.0),
        padding: Some(iced::Padding::new(1.0).left(4).right(4)),
        ..Default::default()
    }
}

impl computed_spans::Source for CodeAdapter {
    fn parse(&self, source: &str) -> computed_spans::Result {
        let mut display = String::new();
        let mut spans = Vec::new();
        let mut source_offset = 0;
        let mut rest = source;

        while let Some(open) = rest.find('`') {
            display.push_str(&rest[..open]);
            source_offset += open;

            if let Some(close) = rest[open + 1..].find('`') {
                let content = &rest[open + 1..open + 1 + close];
                let raw = &rest[open..open + 1 + close + 1];
                let display_start = display.len();
                display.push_str(content);
                let display_end = display.len();

                spans.push(computed_spans::Span {
                    id: source_offset as u64,
                    line: 0,
                    display_range: display_start..display_end,
                    source_range: source_offset..source_offset + raw.len(),
                    source_value: raw.to_string(),
                    display_value: content.to_string(),
                    placeholder: String::new(),
                    style: Rc::new(|_: &iced::Theme| popup::SpanStyle {
                        background: None,
                        border: iced::Border::default(),
                    }),
                    atomic: false,
                    popup: false,
                    text_style: Some(code_style()),
                });

                source_offset += raw.len();
                rest = &rest[open + 1 + close + 1..];
            } else {
                display.push('`');
                source_offset += 1;
                rest = &rest[open + 1..];
            }
        }
        display.push_str(rest);

        computed_spans::Result {
            lines: vec![markright::StyledLine {
                text: display,
                runs: vec![],
                paragraph: markright::Paragraph::default(),
            }],
            spans,
        }
    }
}

#[test]
fn insert_inside_non_popup_span() {
    let source = "hello `metric` world";
    let c = C::from_computed(source, CodeAdapter);

    // Display: "hello metric world"
    assert_eq!(c.text(), "hello metric world");

    // Move cursor to column 7 (after 'm', before 'e' in "metric")
    c.move_to(0, 7);

    // Insert 'Z' five times
    for _ in 0..5 {
        c.perform(Edit::Insert('Z'));
    }

    // Expected: "hello mZZZZZetric world"
    assert_eq!(c.text(), "hello mZZZZZetric world");

    // Source should be: "hello `mZZZZZetric` world"
    assert_eq!(c.source().unwrap(), "hello `mZZZZZetric` world");
}

#[test]
fn backspace_at_span_boundary_keeps_font_and_chip_consistent() {
    let source = "hello `metric` data";
    let c = C::from_computed(source, CodeAdapter);
    assert_eq!(c.text(), "hello metric data");

    // Cursor after the space: "hello metric |data"
    c.move_to(0, 13);
    c.perform(Edit::Backspace);

    // Space deleted — "metric" and "data" are now adjacent
    assert_eq!(c.text(), "hello metricdata");
    assert_eq!(c.source().unwrap(), "hello `metric`data");

    // Font and chip must agree: code style covers ONLY "metric" (6..12)
    let styled = c.styled_line(0).expect("should have line 0");
    let code_runs: Vec<_> = styled
        .runs
        .iter()
        .filter(|r| r.style.font == Some(iced::Font::MONOSPACE))
        .collect();
    assert_eq!(code_runs.len(), 1, "exactly one code-styled run");
    assert_eq!(
        code_runs[0].range,
        6..12,
        "code font covers only 'metric', not 'metricdata'"
    );

    // Chip (computed span) must cover the same range
    let spans = c.computed_spans();
    let code_spans: Vec<_> = spans.iter().filter(|s| !s.popup).collect();
    assert_eq!(code_spans.len(), 1);
    assert_eq!(
        code_spans[0].display_range,
        6..12,
        "chip covers only 'metric'"
    );

    // No monospace run may extend past position 12
    for run in &styled.runs {
        if run.style.font == Some(iced::Font::MONOSPACE) {
            assert!(
                run.range.end <= 12,
                "monospace run {:?} leaks past 'metric' into 'data'",
                run.range,
            );
        }
    }
}
