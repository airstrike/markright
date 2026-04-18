use markright::rich_editor::computed_spans;

use crate::parser::{self, Segment, Segments};
use crate::theme;

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
                        style: std::rc::Rc::new(theme::chip),
                        atomic: true,
                    });

                    source_offset += raw.len();
                }
            }
        }

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
