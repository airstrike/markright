use std::rc::Rc;

use markright::rich_editor::computed_spans;

use crate::parser::{self, CODE_CLOSE, CODE_OPEN, Segment, Segments};
use crate::theme;

pub struct Adapter;

impl computed_spans::Source for Adapter {
    fn parse(&self, source: &str) -> computed_spans::Result {
        let mut display = String::new();
        let mut spans = Vec::new();
        let mut runs = Vec::new();
        let mut source_offset = 0;

        for segment in Segments::new(source) {
            match segment {
                Segment::Text(t) => {
                    display.push_str(&t.replace(CODE_OPEN, "`").replace(CODE_CLOSE, "`"));
                    source_offset += t.len();
                }
                Segment::Formula { expr, raw } => {
                    let value = parser::eval(expr);
                    let display_start = display.len();
                    display.push_str(&value);
                    let display_end = display.len();

                    runs.push(markright::StyleRun {
                        range: display_start..display_end,
                        style: theme::formula_style(),
                    });

                    spans.push(computed_spans::Span {
                        id: source_offset as u64,
                        line: 0,
                        display_range: display_start..display_end,
                        source_range: source_offset..source_offset + raw.len(),
                        source_value: raw.to_string(),
                        display_value: value,
                        placeholder: "{=expr}".to_string(),
                        style: Rc::new(theme::formula_chip),
                        atomic: true,
                        popup: true,
                    });

                    source_offset += raw.len();
                }
                Segment::Code { content, raw } => {
                    let display_start = display.len();
                    display.push_str(&content);
                    let display_end = display.len();

                    runs.push(markright::StyleRun {
                        range: display_start..display_end,
                        style: theme::code_style(),
                    });

                    // Store source_value with sentinel delimiters so
                    // matched pairs can't re-pair if one is dissolved.
                    let sentinel_value =
                        format!("{CODE_OPEN}{}{CODE_CLOSE}", content.replace('`', "``"));

                    spans.push(computed_spans::Span {
                        id: source_offset as u64,
                        line: 0,
                        display_range: display_start..display_end,
                        source_range: source_offset..source_offset + raw.len(),
                        source_value: sentinel_value,
                        display_value: content,
                        placeholder: String::new(),
                        style: Rc::new(theme::code_chip),
                        atomic: false,
                        popup: false,
                    });

                    source_offset += raw.len();
                }
            }
        }

        computed_spans::Result {
            lines: vec![markright::StyledLine {
                text: display,
                runs,
                paragraph: markright::Paragraph::default(),
            }],
            spans,
        }
    }
}
