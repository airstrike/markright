use std::ops::Range;

use iced_core::text::rich_editor::{ParagraphStyle, Style};

/// A single run of uniform styling within a line.
#[derive(Debug, Clone)]
pub struct StyleRun {
    pub range: Range<usize>,
    pub style: Style,
}

/// Text with associated style runs.
#[derive(Debug, Clone)]
pub struct StyledText {
    pub text: String,
    /// Non-overlapping, sorted runs covering `0..text.len()`.
    pub runs: Vec<StyleRun>,
}

/// An atomic document operation.
///
/// Each variant carries enough data to compute its own inverse for undo/redo.
/// `old_*` fields are populated by the capture step before application.
#[derive(Debug, Clone)]
pub enum Op {
    /// Insert styled text at a position.
    InsertText {
        line: usize,
        col: usize,
        content: StyledText,
    },
    /// Delete styled text at a position (content records what was deleted).
    DeleteText {
        line: usize,
        col: usize,
        content: StyledText,
    },
    /// Split a line at the given column (Enter key).
    SplitLine { line: usize, col: usize },
    /// Merge the next line into this one at the given column.
    /// `col` is the length of the line before the merge (the join point).
    MergeLine { line: usize, col: usize },
    /// Set character formatting on a range. `old_runs` captures prior state.
    SetSpanStyle {
        line: usize,
        range: Range<usize>,
        style: Style,
        old_runs: Vec<StyleRun>,
    },
    /// Set paragraph formatting. `old_style` captures prior state.
    SetParagraphStyle {
        line: usize,
        style: ParagraphStyle,
        old_style: ParagraphStyle,
    },
}

impl Op {
    /// Return the inverse operations that undo this operation.
    ///
    /// Most operations invert to a single op, but `SetSpanStyle` may produce
    /// multiple ops (one per old run to restore).
    pub fn inverse(&self) -> Vec<Op> {
        match self {
            Op::InsertText { line, col, content } => {
                vec![Op::DeleteText {
                    line: *line,
                    col: *col,
                    content: content.clone(),
                }]
            }
            Op::DeleteText { line, col, content } => {
                vec![Op::InsertText {
                    line: *line,
                    col: *col,
                    content: content.clone(),
                }]
            }
            Op::SplitLine { line, col } => {
                vec![Op::MergeLine {
                    line: *line,
                    col: *col,
                }]
            }
            Op::MergeLine { line, col } => {
                vec![Op::SplitLine {
                    line: *line,
                    col: *col,
                }]
            }
            Op::SetSpanStyle {
                line,
                range,
                old_runs,
                ..
            } => {
                if old_runs.is_empty() {
                    vec![Op::SetSpanStyle {
                        line: *line,
                        range: range.clone(),
                        style: Style::default(),
                        old_runs: Vec::new(),
                    }]
                } else {
                    old_runs
                        .iter()
                        .map(|run| Op::SetSpanStyle {
                            line: *line,
                            range: run.range.clone(),
                            style: run.style.clone(),
                            old_runs: Vec::new(),
                        })
                        .collect()
                }
            }
            Op::SetParagraphStyle {
                line,
                style,
                old_style,
            } => {
                vec![Op::SetParagraphStyle {
                    line: *line,
                    style: old_style.clone(),
                    old_style: style.clone(),
                }]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_styled_text() -> StyledText {
        StyledText {
            text: "hello".to_string(),
            runs: vec![StyleRun {
                range: 0..5,
                style: Style {
                    bold: Some(true),
                    ..Style::default()
                },
            }],
        }
    }

    #[test]
    fn insert_text_inverse_is_delete_text() {
        let op = Op::InsertText {
            line: 2,
            col: 5,
            content: sample_styled_text(),
        };
        let inv = op.inverse();
        assert_eq!(inv.len(), 1);
        match &inv[0] {
            Op::DeleteText { line, col, content } => {
                assert_eq!(*line, 2);
                assert_eq!(*col, 5);
                assert_eq!(content.text, "hello");
            }
            other => panic!("expected DeleteText, got {other:?}"),
        }
    }

    #[test]
    fn delete_text_inverse_is_insert_text() {
        let op = Op::DeleteText {
            line: 3,
            col: 10,
            content: sample_styled_text(),
        };
        let inv = op.inverse();
        assert_eq!(inv.len(), 1);
        match &inv[0] {
            Op::InsertText { line, col, content } => {
                assert_eq!(*line, 3);
                assert_eq!(*col, 10);
                assert_eq!(content.text, "hello");
            }
            other => panic!("expected InsertText, got {other:?}"),
        }
    }

    #[test]
    fn split_line_inverse_is_merge_line() {
        let op = Op::SplitLine { line: 1, col: 8 };
        let inv = op.inverse();
        assert_eq!(inv.len(), 1);
        match &inv[0] {
            Op::MergeLine { line, col } => {
                assert_eq!(*line, 1);
                assert_eq!(*col, 8);
            }
            other => panic!("expected MergeLine, got {other:?}"),
        }

        // And back again
        let round_trip = inv[0].inverse();
        assert_eq!(round_trip.len(), 1);
        match &round_trip[0] {
            Op::SplitLine { line, col } => {
                assert_eq!(*line, 1);
                assert_eq!(*col, 8);
            }
            other => panic!("expected SplitLine, got {other:?}"),
        }
    }

    #[test]
    fn set_span_style_inverse_restores_old_runs() {
        let op = Op::SetSpanStyle {
            line: 0,
            range: 0..10,
            style: Style {
                bold: Some(true),
                ..Style::default()
            },
            old_runs: vec![
                StyleRun {
                    range: 0..5,
                    style: Style {
                        italic: Some(true),
                        ..Style::default()
                    },
                },
                StyleRun {
                    range: 5..10,
                    style: Style::default(),
                },
            ],
        };
        let inv = op.inverse();
        assert_eq!(inv.len(), 2);

        match &inv[0] {
            Op::SetSpanStyle {
                line,
                range,
                style,
                old_runs,
            } => {
                assert_eq!(*line, 0);
                assert_eq!(*range, 0..5);
                assert_eq!(style.italic, Some(true));
                assert!(old_runs.is_empty());
            }
            other => panic!("expected SetSpanStyle, got {other:?}"),
        }

        match &inv[1] {
            Op::SetSpanStyle {
                line,
                range,
                style,
                old_runs,
            } => {
                assert_eq!(*line, 0);
                assert_eq!(*range, 5..10);
                assert_eq!(*style, Style::default());
                assert!(old_runs.is_empty());
            }
            other => panic!("expected SetSpanStyle, got {other:?}"),
        }
    }

    #[test]
    fn set_span_style_inverse_with_empty_old_runs() {
        let op = Op::SetSpanStyle {
            line: 1,
            range: 3..7,
            style: Style {
                underline: Some(true),
                ..Style::default()
            },
            old_runs: Vec::new(),
        };
        let inv = op.inverse();
        assert_eq!(inv.len(), 1);
        match &inv[0] {
            Op::SetSpanStyle {
                line,
                range,
                style,
                old_runs,
            } => {
                assert_eq!(*line, 1);
                assert_eq!(*range, 3..7);
                assert_eq!(*style, Style::default());
                assert!(old_runs.is_empty());
            }
            other => panic!("expected SetSpanStyle, got {other:?}"),
        }
    }

    #[test]
    fn set_paragraph_style_inverse_swaps_styles() {
        let new_style = ParagraphStyle {
            alignment: Some(iced_core::text::Alignment::Center),
            ..ParagraphStyle::default()
        };
        let old_style = ParagraphStyle {
            alignment: Some(iced_core::text::Alignment::Left),
            ..ParagraphStyle::default()
        };
        let op = Op::SetParagraphStyle {
            line: 0,
            style: new_style.clone(),
            old_style: old_style.clone(),
        };
        let inv = op.inverse();
        assert_eq!(inv.len(), 1);
        match &inv[0] {
            Op::SetParagraphStyle {
                line,
                style,
                old_style: inv_old,
            } => {
                assert_eq!(*line, 0);
                assert_eq!(*style, old_style);
                assert_eq!(*inv_old, new_style);
            }
            other => panic!("expected SetParagraphStyle, got {other:?}"),
        }
    }

    #[test]
    fn double_inverse_matches_original() {
        let original = Op::InsertText {
            line: 4,
            col: 2,
            content: sample_styled_text(),
        };
        let inv = original.inverse();
        assert_eq!(inv.len(), 1);
        let double_inv = inv[0].inverse();
        assert_eq!(double_inv.len(), 1);

        match (&original, &double_inv[0]) {
            (
                Op::InsertText {
                    line: l1,
                    col: c1,
                    content: ct1,
                },
                Op::InsertText {
                    line: l2,
                    col: c2,
                    content: ct2,
                },
            ) => {
                assert_eq!(l1, l2);
                assert_eq!(c1, c2);
                assert_eq!(ct1.text, ct2.text);
                assert_eq!(ct1.runs.len(), ct2.runs.len());
            }
            (orig, result) => {
                panic!("expected matching InsertText variants, got {orig:?} and {result:?}")
            }
        }
    }
}
