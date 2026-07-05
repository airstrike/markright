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

const CODE_OPEN: char = '\u{E000}';
const CODE_CLOSE: char = '\u{E001}';

struct FormulaAdapter;

impl computed_spans::Source for FormulaAdapter {
    fn parse(&self, source: &str) -> computed_spans::Result {
        let mut display = String::new();
        let mut spans = Vec::new();
        let mut source_offset = 0;
        let mut rest = source;

        while let Some(pos) = rest.find("{=") {
            display.push_str(&rest[..pos]);
            source_offset += pos;
            rest = &rest[pos..];

            if let Some(end) = rest[2..].find('}') {
                let raw = &rest[..end + 3];
                let expr = &rest[2..end + 2];
                let value = expr.to_string();
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
                    style: Rc::new(|_: &iced::Theme| popup::SpanStyle {
                        background: None,
                        border: iced::Border::default(),
                    }),
                    atomic: true,
                    popup: true,
                });

                source_offset += raw.len();
                rest = &rest[raw.len()..];
            } else {
                display.push_str("{=");
                source_offset += 2;
                rest = &rest[2..];
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

struct EvalFormulaAdapter;

impl computed_spans::Source for EvalFormulaAdapter {
    fn parse(&self, source: &str) -> computed_spans::Result {
        let mut display = String::new();
        let mut spans = Vec::new();
        let mut source_offset = 0;
        let mut rest = source;

        while let Some(pos) = rest.find("{=") {
            display.push_str(&rest[..pos]);
            source_offset += pos;
            rest = &rest[pos..];

            if let Some(end) = rest[2..].find('}') {
                let raw = &rest[..end + 3];
                let expr = &rest[2..end + 2];
                let value = simple_eval(expr);
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
                    style: Rc::new(|_: &iced::Theme| popup::SpanStyle {
                        background: None,
                        border: iced::Border::default(),
                    }),
                    atomic: true,
                    popup: true,
                });

                source_offset += raw.len();
                rest = &rest[raw.len()..];
            } else {
                display.push('{');
                source_offset += 1;
                rest = &rest[1..];
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

fn simple_eval(expr: &str) -> String {
    if let Some((a, b)) = expr.split_once('+')
        && let (Ok(a), Ok(b)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>())
    {
        return format!("{}", a + b);
    }
    if let Some((a, b)) = expr.split_once('*')
        && let (Ok(a), Ok(b)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>())
    {
        return format!("{}", a * b);
    }
    if let Ok(n) = expr.parse::<f64>() {
        return format!("{n}");
    }
    format!("?{expr}")
}

struct MixedAdapter;

impl computed_spans::Source for MixedAdapter {
    fn parse(&self, source: &str) -> computed_spans::Result {
        let mut display = String::new();
        let mut spans = Vec::new();
        let mut runs = Vec::new();
        let mut source_offset = 0;
        let mut rest = source;

        while !rest.is_empty() {
            let formula_pos = rest.find("{=");
            let backtick_pos = rest.find('`');
            let fence_pos = rest.find(CODE_OPEN);

            let next = [formula_pos, backtick_pos, fence_pos]
                .into_iter()
                .flatten()
                .min();

            let Some(pos) = next else {
                display.push_str(&rest.replace([CODE_OPEN, CODE_CLOSE], "`"));
                break;
            };

            if pos > 0 {
                display.push_str(&rest[..pos].replace([CODE_OPEN, CODE_CLOSE], "`"));
                source_offset += pos;
                rest = &rest[pos..];
                continue;
            }

            if rest.starts_with("{=") {
                if let Some(end) = rest[2..].find('}') {
                    let raw = &rest[..end + 3];
                    let expr = &rest[2..end + 2];
                    let value = expr.to_string();
                    let display_start = display.len();
                    display.push_str(&value);
                    let display_end = display.len();

                    runs.push(markright::StyleRun {
                        range: display_start..display_end,
                        style: code_style(),
                    });

                    spans.push(computed_spans::Span {
                        id: source_offset as u64,
                        line: 0,
                        display_range: display_start..display_end,
                        source_range: source_offset..source_offset + raw.len(),
                        source_value: raw.to_string(),
                        display_value: value,
                        placeholder: "{=expr}".to_string(),
                        style: Rc::new(|_: &iced::Theme| popup::SpanStyle {
                            background: None,
                            border: iced::Border::default(),
                        }),
                        atomic: true,
                        popup: true,
                    });

                    source_offset += raw.len();
                    rest = &rest[raw.len()..];
                    continue;
                }
                display.push('{');
                source_offset += 1;
                rest = &rest[1..];
                continue;
            }

            let open_ch = rest.chars().next().unwrap();
            let open_len = open_ch.len_utf8();
            let close_ch = if open_ch == '`' { '`' } else { CODE_CLOSE };

            let inner = &rest[open_len..];
            let close = if open_ch == close_ch {
                inner.find(close_ch)
            } else {
                let closer = inner.find(close_ch);
                let nested = inner.find(open_ch);
                match (closer, nested) {
                    (Some(c), Some(n)) if n < c => None,
                    (c, _) => c,
                }
            };
            if let Some(close) = close {
                let content = &inner[..close];
                let raw_len = open_len + close + close_ch.len_utf8();
                let raw = &rest[..raw_len];
                let display_start = display.len();
                display.push_str(content);
                let display_end = display.len();

                runs.push(markright::StyleRun {
                    range: display_start..display_end,
                    style: code_style(),
                });

                let sentinel_value = format!("{CODE_OPEN}{content}{CODE_CLOSE}");

                spans.push(computed_spans::Span {
                    id: source_offset as u64,
                    line: 0,
                    display_range: display_start..display_end,
                    source_range: source_offset..source_offset + raw.len(),
                    source_value: sentinel_value,
                    display_value: content.to_string(),
                    placeholder: String::new(),
                    style: Rc::new(|_: &iced::Theme| popup::SpanStyle {
                        background: None,
                        border: iced::Border::default(),
                    }),
                    atomic: false,
                    popup: false,
                });

                source_offset += raw_len;
                rest = &rest[raw_len..];
            } else {
                display.push('`');
                source_offset += open_len;
                rest = &rest[open_len..];
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

impl computed_spans::Source for CodeAdapter {
    fn parse(&self, source: &str) -> computed_spans::Result {
        let mut display = String::new();
        let mut spans = Vec::new();
        let mut runs = Vec::new();
        let mut source_offset = 0;
        let mut rest = source;

        while let Some(open) = rest.find(['`', CODE_OPEN]) {
            let open_ch = rest[open..].chars().next().unwrap();
            let open_len = open_ch.len_utf8();
            let close_ch = if open_ch == '`' { '`' } else { CODE_CLOSE };

            // Orphaned sentinels render as backtick
            display.push_str(&rest[..open].replace([CODE_OPEN, CODE_CLOSE], "`"));
            source_offset += open;

            let inner = &rest[open + open_len..];
            // Find closer, but bail if another opener appears first
            let close = if open_ch == close_ch {
                inner.find(close_ch)
            } else {
                let closer = inner.find(close_ch);
                let nested = inner.find(open_ch);
                match (closer, nested) {
                    (Some(c), Some(n)) if n < c => None,
                    (c, _) => c,
                }
            };
            if let Some(close) = close {
                let content = &inner[..close];
                let raw_len = open_len + close + close_ch.len_utf8();
                let raw = &rest[open..open + raw_len];
                let display_start = display.len();
                display.push_str(content);
                let display_end = display.len();

                runs.push(markright::StyleRun {
                    range: display_start..display_end,
                    style: code_style(),
                });

                let sentinel_value = format!("{CODE_OPEN}{content}{CODE_CLOSE}");

                spans.push(computed_spans::Span {
                    id: source_offset as u64,
                    line: 0,
                    display_range: display_start..display_end,
                    source_range: source_offset..source_offset + raw.len(),
                    source_value: sentinel_value,
                    display_value: content.to_string(),
                    placeholder: String::new(),
                    style: Rc::new(|_: &iced::Theme| popup::SpanStyle {
                        background: None,
                        border: iced::Border::default(),
                    }),
                    atomic: false,
                    popup: false,
                });

                source_offset += raw_len;
                rest = &rest[open + raw_len..];
            } else {
                display.push('`');
                source_offset += open_len;
                rest = &rest[open + open_len..];
            }
        }
        display.push_str(&rest.replace([CODE_OPEN, CODE_CLOSE], "`"));

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

fn assert_spans(c: &C, expected: &[(std::ops::Range<usize>, &str)]) {
    let spans = c.computed_spans();
    let actual: Vec<_> = spans
        .iter()
        .filter(|s| !s.popup)
        .map(|s| (s.display_range.clone(), s.display_value.as_str()))
        .collect();
    let expected: Vec<_> = expected.iter().map(|(r, v)| (r.clone(), *v)).collect();
    assert_eq!(
        actual,
        expected,
        "display={:?} source={:?}",
        c.text(),
        c.source()
    );
}

fn cursor_col(c: &C) -> usize {
    c.cursor().position.column
}

#[test]
fn backtick_lifecycle() {
    let c = C::from_computed("", CodeAdapter);
    assert_eq!(c.text(), "");

    // Type ` → source "`", display "`", cursor 1
    c.perform(Edit::Insert('`'));
    assert_eq!(c.text(), "`", "after typing `");
    assert_eq!(cursor_col(&c), 1);
    assert!(c.computed_spans().is_empty(), "unclosed backtick = no span");

    // Type a → source "`a", display "`a", cursor 2
    c.perform(Edit::Insert('a'));
    assert_eq!(c.text(), "`a");
    assert_eq!(cursor_col(&c), 2);

    // Type b → source "`ab", display "`ab", cursor 3
    c.perform(Edit::Insert('b'));
    assert_eq!(c.text(), "`ab");
    assert_eq!(cursor_col(&c), 3);

    // Type c → source "`abc", display "`abc", cursor 4
    c.perform(Edit::Insert('c'));
    assert_eq!(c.text(), "`abc");
    assert_eq!(cursor_col(&c), 4);

    // Type ` → source "`abc`", display "abc" (code span), cursor 3
    // Backticks eaten: 4+1-2 = 3
    c.perform(Edit::Insert('`'));
    assert_eq!(c.source().unwrap(), "`abc`");
    assert_eq!(c.text(), "abc", "closing backtick creates code span");
    assert_eq!(cursor_col(&c), 3, "cursor at end of 'abc'");
    assert_eq!(c.computed_spans().len(), 1);
    assert_eq!(c.computed_spans()[0].display_range, 0..3);

    // Type . → source "`abc`.", display "abc.", cursor 4
    c.perform(Edit::Insert('.'));
    assert_eq!(c.source().unwrap(), "`abc`.");
    assert_eq!(c.text(), "abc.");
    assert_eq!(cursor_col(&c), 4);

    // BKSP → source "`abc`", display "abc", cursor 3
    c.perform(Edit::Backspace);
    assert_eq!(c.source().unwrap(), "`abc`");
    assert_eq!(c.text(), "abc");
    assert_eq!(cursor_col(&c), 3);

    // BKSP → dissolve trailing backtick → source "`abc", display "`abc", cursor 4
    c.perform(Edit::Backspace);
    assert_eq!(c.source().unwrap(), "`abc");
    assert_eq!(c.text(), "`abc", "dissolved span, backtick visible again");
    assert_eq!(cursor_col(&c), 4, "cursor at end of `abc");
    assert!(c.computed_spans().is_empty(), "no span after dissolve");
}

#[test]
fn multi_span_dissolve_does_not_repair() {
    // Two code spans: `abc` d `ef`
    // Display: "abc d ef" with spans [0..3, 5..7]
    let c = C::from_computed("`abc` d `ef`", CodeAdapter);
    assert_eq!(c.text(), "abc d ef");
    assert_spans(&c, &[(0..3, "abc"), (6..8, "ef")]);

    // Dissolve trailing backtick of "abc" (backspace at span[0].end = 3)
    // Expected: "abc" becomes plain text, "ef" stays code
    // Display: "`abc d ef" (backtick visible, "ef" still code)
    //
    // BUG: greedy re-parsing pairs the first ` with the ` before ef,
    // creating code span "abc d " instead. This test documents the
    // DESIRED behavior — dissolving one span must not re-pair with
    // unrelated backticks.
    c.move_to(0, 3);
    c.perform(Edit::Backspace);

    // The opening backtick of "abc" is now visible (unclosed).
    // "ef" should remain a code span.
    assert_eq!(c.text(), "`abc d ef");
    assert_spans(&c, &[(7..9, "ef")]);
    assert_eq!(cursor_col(&c), 4, "cursor after visible `abc");
}

#[test]
fn multi_span_source_ranges_consistent_after_sentinel_conversion() {
    // After from_computed, source_ranges must index correctly into
    // the stored source — even though the adapter converts backtick
    // delimiters to multi-byte sentinels in source_value.
    //
    // If source_ranges still point at the original 1-byte backtick
    // positions but source_values use 3-byte sentinels,
    // rebuild_from_computed will produce garbled source.
    let c = C::from_computed("`abc` d `ef`", CodeAdapter);
    assert_eq!(c.text(), "abc d ef");

    // Dissolve trailing backtick of "abc" WITHOUT any prior edits.
    // This goes through try_dissolve_span_boundary → rebuild_from_computed
    // using the original source_ranges (which point into the literal-backtick
    // source, not the sentinel-based source_values).
    c.move_to(0, 3);
    c.perform(Edit::Backspace);

    // "abc" dissolved, "ef" must remain a code span.
    assert_eq!(c.text(), "`abc d ef");
    assert_spans(&c, &[(7..9, "ef")]);

    // Now type something after "ef" to confirm the second span
    // survived the dissolve and still works for edits.
    c.move_to(0, 9);
    c.perform(Edit::Insert('!'));
    assert_eq!(c.text(), "`abc d ef!");
}

#[test]
fn closing_backtick_cursor_lands_at_span_end() {
    // Start with two code spans: `abc` d `ef`
    let c = C::from_computed("`abc` d `ef`", CodeAdapter);
    assert_eq!(c.text(), "abc d ef");
    assert_spans(&c, &[(0..3, "abc"), (6..8, "ef")]);

    // Dissolve "abc" → display "`abc d ef", "ef" stays code
    c.move_to(0, 3);
    c.perform(Edit::Backspace);
    assert_eq!(c.text(), "`abc d ef");
    assert_spans(&c, &[(7..9, "ef")]);

    // Cursor at col 4 (after 'c' in "`abc"):  "`abc| d ef"
    // The dissolve already put us at 4.
    assert_eq!(cursor_col(&c), 4);

    // Type ` to re-close the span
    c.perform(Edit::Insert('`'));

    // The orphaned sentinel + new backtick should form a matched pair.
    // Display: "abc d ef" (8 chars). "abc" at 0..3, "ef" at 6..8.
    assert_eq!(c.text(), "abc d ef");
    assert_spans(&c, &[(0..3, "abc"), (6..8, "ef")]);

    // Cursor should be at 3 (end of "abc"), not 4 (past the space).
    assert_eq!(cursor_col(&c), 3, "cursor at end of newly-created span");
}

#[test]
fn type_full_sequence_two_spans_and_period() {
    let c = C::from_computed("", CodeAdapter);

    // Type: `abc` d `ef`.
    for ch in "`abc`".chars() {
        c.perform(Edit::Insert(ch));
    }
    assert_eq!(c.text(), "abc");
    assert_eq!(cursor_col(&c), 3);
    assert_spans(&c, &[(0..3, "abc")]);

    for ch in " d ".chars() {
        c.perform(Edit::Insert(ch));
    }
    assert_eq!(c.text(), "abc d ");
    assert_eq!(cursor_col(&c), 6);

    for ch in "`ef`".chars() {
        c.perform(Edit::Insert(ch));
    }
    assert_eq!(c.text(), "abc d ef");
    assert_eq!(cursor_col(&c), 8, "cursor at end of 'ef' span");
    assert_spans(&c, &[(0..3, "abc"), (6..8, "ef")]);

    c.perform(Edit::Insert('.'));
    assert_eq!(c.text(), "abc d ef.");
    assert_eq!(cursor_col(&c), 9, "cursor after period");
}

#[test]
fn insert_inside_atomic_span_is_blocked() {
    // Formula {=123} displays as "123" (atomic, popup).
    // Insert strictly inside must be blocked.
    let c = C::from_computed("x{=123}y", FormulaAdapter);
    assert_eq!(c.text(), "x123y");
    let spans = c.computed_spans();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].display_range, 1..4);
    assert!(spans[0].atomic);

    // Cursor strictly inside the span (col 2)
    c.move_to(0, 2);
    c.perform(Edit::Insert('Z'));

    // Nothing should change — the span is atomic.
    assert_eq!(c.text(), "x123y");
    assert_eq!(cursor_col(&c), 2);
}

#[test]
fn backspace_at_formula_boundary_dissolves_to_text() {
    // "{=10+10} projects" → display "10+10 projects"
    // Cursor at span.end → backspace strips closing `}`
    // → source "{=10+10 projects" → display "{=10+10 projects" (plain text)
    let c = C::from_computed("{=10+10} projects", FormulaAdapter);
    assert_eq!(c.text(), "10+10 projects");
    let spans = c.computed_spans();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].display_range, 0..5);

    // Cursor at span.end (col 5)
    c.move_to(0, 5);
    c.perform(Edit::Backspace);

    // Formula dissolved — closing } stripped, source becomes plain text.
    // Display shows the raw source with {= visible.
    assert_eq!(c.text(), "{=10+10 projects");
    assert!(
        c.computed_spans().is_empty(),
        "no spans after dissolving formula"
    );
    // Cursor after "{=10+10" = position 7 (the { and = are now visible)
    assert_eq!(cursor_col(&c), 7);
}

#[test]
fn backspace_after_typing_formula() {
    // Simulate typing "This input supports {=1+2}" character by character
    let c = C::from_computed("", FormulaAdapter);

    for ch in "This input supports {=1+2}".chars() {
        c.perform(Edit::Insert(ch));
    }

    // After typing "}", the formula span is created.
    // Now backspace — should dissolve the formula, not panic.
    c.perform(Edit::Backspace);

    // Formula should be dissolved — raw source visible.
    let after = c.text();
    assert!(
        after.contains("{=1+2"),
        "dissolved formula should show raw source, got: {after:?}"
    );
}

#[test]
fn dissolved_formula_cursor_position() {
    // "{=1+2} projects" → display "3 projects", formula at 0..1
    // (the EvalFormulaAdapter evaluates 1+2 → "3")
    // Backspace at span.end → "{=1+2 projects", cursor at 5 (after "2")
    let c = C::from_computed("{=1+2} projects", EvalFormulaAdapter);
    assert_eq!(c.text(), "3 projects");
    let spans = c.computed_spans();
    assert_eq!(spans[0].display_range, 0..1);
    assert_eq!(spans[0].display_value, "3");

    c.move_to(0, 1);
    c.perform(Edit::Backspace);

    assert_eq!(c.text(), "{=1+2 projects");
    // Cursor should be at 5: "{=1+2| projects"
    assert_eq!(
        cursor_col(&c),
        5,
        "cursor right after the dissolved content"
    );
}

#[test]
fn backspace_at_formula_in_mixed_source() {
    // Use a source with both formulas and code spans (like the popup example).
    // The adapter handles both {=expr} and `code`.
    let c = C::from_computed("The `users` table has {=1+3} columns.", MixedAdapter);

    // Display: "The users table has 4 columns."
    // Code "users" at some range, formula "4" at some range.
    let spans = c.computed_spans();
    let formula = spans
        .iter()
        .find(|s| s.atomic)
        .expect("should have formula");
    let formula_end = formula.display_range.end;

    // Move to formula end and backspace
    c.move_to(0, formula_end);
    c.perform(Edit::Backspace);

    // Should not panic, formula should dissolve
    let after = c.text();
    assert!(after.contains("{=1+3"), "dissolved formula, got: {after:?}");
}

#[test]
fn backspace_at_span_end_dissolves_boundary() {
    // Source: "`market`data" → display: "marketdata"
    // Cursor at span.end (position 6, between "market" and "data")
    // Backspace should remove the closing backtick, dissolving the span.
    // Source becomes "`marketdata" (unclosed backtick = literal text).
    // Display becomes "`marketdata" (backtick visible, no code styling).
    let c = C::from_computed("`market`data", CodeAdapter);
    assert_eq!(c.text(), "marketdata");

    c.move_to(0, 6);
    c.perform(Edit::Backspace);

    assert_eq!(c.source().unwrap(), "`marketdata");
    assert_eq!(c.text(), "`marketdata");

    // No code spans remain (unclosed backtick = literal text)
    let spans = c.computed_spans();
    assert!(spans.is_empty(), "no code spans after dissolving boundary");
}

#[test]
fn space_at_span_end_inserts_after_delimiter() {
    // Source: "`market`data" → display: "marketdata"
    // Cursor at span.end (position 6)
    // Space should insert AFTER the closing backtick in the source.
    // Source becomes "`market` data", display "market data".
    let c = C::from_computed("`market`data", CodeAdapter);
    assert_eq!(c.text(), "marketdata");

    c.move_to(0, 6);
    c.perform(Edit::Insert(' '));

    assert_eq!(c.source().unwrap(), "`market` data");
    assert_eq!(c.text(), "market data");

    // Code span still exists, covering only "market" (0..6)
    let spans = c.computed_spans();
    let code_spans: Vec<_> = spans.iter().filter(|s| !s.popup).collect();
    assert_eq!(code_spans.len(), 1);
    assert_eq!(code_spans[0].display_range, 0..6);
}
