use markright::widget::rich_editor::computed_spans;

use crate::parser::{self, Segment, Segments};

pub struct Adapter;

impl computed_spans::Source for Adapter {
    fn parse(&self, source: &str) -> computed_spans::Result {
        let mut display = String::new();
        let mut spans = Vec::new();
        let mut source_offset = 0;

        for segment in Segments::new(source) {
            match segment {
                Segment::Text(t) => {
                    display.push_str(t);
                    source_offset += t.len();
                }
                Segment::Formula { expr, raw } => {
                    let value = parser::eval(expr);
                    let display_start = display.len();
                    display.push_str(&value);
                    let display_end = display.len();

                    spans.push(computed_spans::Span {
                        id: source_offset as u64,
                        line: 0,
                        display_range: display_start..display_end,
                        source_range: source_offset..source_offset + raw.len(),
                        source_value: raw.to_string(),
                        display_value: value,
                        placeholder: "{=expr}".to_string(),
                        background: Some(iced::Background::Color(iced::color!(0xFAF9F5))),
                        border: iced::Border {
                            color: iced::color!(0xE5E4DC),
                            width: 1.0,
                            radius: 3.0.into(),
                        },
                        atomic: true,
                    });

                    source_offset += raw.len();
                }
            }
        }

        computed_spans::Result {
            lines: vec![markright_core::StyledLine {
                text: display,
                runs: vec![],
                paragraph: markright_core::Paragraph::default(),
            }],
            spans,
        }
    }
}
