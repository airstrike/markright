use std::ops::Range;

use iced_core::font::OpticalSize;
use iced_core::text::LineHeight;
use iced_core::text::rich_editor::{paragraph, span};
use iced_core::{Color, Font};

/// Text alignment for paragraphs.
///
/// Unlike iced's `text::Alignment`, this enum has no `Default` variant —
/// alignment is always explicit. This ensures undo always restores a
/// concrete value rather than a `None` that the editor ignores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Center,
    Right,
    Justified,
}

impl Alignment {
    /// Convert from iced's optional alignment. `None` and `Default` map to `Left`.
    pub fn from_iced(alignment: Option<iced_core::text::Alignment>) -> Self {
        match alignment {
            Some(iced_core::text::Alignment::Center) => Self::Center,
            Some(iced_core::text::Alignment::Right) => Self::Right,
            Some(iced_core::text::Alignment::Justified) => Self::Justified,
            _ => Self::Left,
        }
    }

    /// Convert to iced's alignment (always `Some`).
    pub fn to_iced(self) -> iced_core::text::Alignment {
        match self {
            Self::Left => iced_core::text::Alignment::Left,
            Self::Center => iced_core::text::Alignment::Center,
            Self::Right => iced_core::text::Alignment::Right,
            Self::Justified => iced_core::text::Alignment::Justified,
        }
    }
}

/// A single character-level attribute change.
///
/// Each variant carries one attribute value (or `None` to unset).
/// Used by [`Op::SetSpanAttr`] for additive per-attribute formatting.
#[derive(Debug, Clone, PartialEq)]
pub enum SpanAttr {
    Bold(Option<bool>),
    Italic(Option<bool>),
    Underline(Option<bool>),
    Strikethrough(Option<bool>),
    Font(Option<Font>),
    Size(Option<f32>),
    Color(Option<Color>),
    LetterSpacing(Option<f32>),
    OpticalSize(Option<OpticalSize>),
}

impl SpanAttr {
    /// Extract this attribute's value from a full `span::Style`.
    pub fn from_style(style: &span::Style, template: &SpanAttr) -> SpanAttr {
        match template {
            SpanAttr::Bold(_) => SpanAttr::Bold(style.bold),
            SpanAttr::Italic(_) => SpanAttr::Italic(style.italic),
            SpanAttr::Underline(_) => SpanAttr::Underline(style.underline),
            SpanAttr::Strikethrough(_) => SpanAttr::Strikethrough(style.strikethrough),
            SpanAttr::Font(_) => SpanAttr::Font(style.font),
            SpanAttr::Size(_) => SpanAttr::Size(style.size),
            SpanAttr::Color(_) => SpanAttr::Color(style.color),
            SpanAttr::LetterSpacing(_) => SpanAttr::LetterSpacing(style.letter_spacing),
            SpanAttr::OpticalSize(_) => SpanAttr::OpticalSize(style.optical_size),
        }
    }

    /// Returns `true` if this attribute's corresponding field in `style` is `Some`.
    pub fn is_set_in(&self, style: &span::Style) -> bool {
        match self {
            SpanAttr::Bold(_) => style.bold.is_some(),
            SpanAttr::Italic(_) => style.italic.is_some(),
            SpanAttr::Underline(_) => style.underline.is_some(),
            SpanAttr::Strikethrough(_) => style.strikethrough.is_some(),
            SpanAttr::Font(_) => style.font.is_some(),
            SpanAttr::Size(_) => style.size.is_some(),
            SpanAttr::Color(_) => style.color.is_some(),
            SpanAttr::LetterSpacing(_) => style.letter_spacing.is_some(),
            SpanAttr::OpticalSize(_) => style.optical_size.is_some(),
        }
    }

    /// Clear this attribute's corresponding field in `style` (set to `None`).
    pub fn clear_in(&self, style: &mut span::Style) {
        match self {
            SpanAttr::Bold(_) => style.bold = None,
            SpanAttr::Italic(_) => style.italic = None,
            SpanAttr::Underline(_) => style.underline = None,
            SpanAttr::Strikethrough(_) => style.strikethrough = None,
            SpanAttr::Font(_) => style.font = None,
            SpanAttr::Size(_) => style.size = None,
            SpanAttr::Color(_) => style.color = None,
            SpanAttr::LetterSpacing(_) => style.letter_spacing = None,
            SpanAttr::OpticalSize(_) => style.optical_size = None,
        }
    }

    /// Apply this attribute onto a full `span::Style`, returning the modified style.
    pub fn apply_to(&self, style: &span::Style) -> span::Style {
        let mut result = style.clone();
        match self {
            SpanAttr::Bold(v) => result.bold = *v,
            SpanAttr::Italic(v) => result.italic = *v,
            SpanAttr::Underline(v) => result.underline = *v,
            SpanAttr::Strikethrough(v) => result.strikethrough = *v,
            SpanAttr::Font(v) => result.font = *v,
            SpanAttr::Size(v) => result.size = *v,
            SpanAttr::Color(v) => result.color = *v,
            SpanAttr::LetterSpacing(v) => result.letter_spacing = *v,
            SpanAttr::OpticalSize(v) => result.optical_size = *v,
        }
        result
    }
}

/// A single run of uniform styling within a line.
#[derive(Debug, Clone)]
pub struct StyleRun {
    pub range: Range<usize>,
    pub style: span::Style,
}

/// Text with associated style runs.
#[derive(Debug, Clone)]
pub struct StyledText {
    pub text: String,
    /// Non-overlapping, sorted runs covering `0..text.len()`.
    pub runs: Vec<StyleRun>,
}

/// A single line's content with text, style runs, and paragraph formatting.
///
/// Style runs are 0-based relative to `text`.
#[derive(Debug, Clone)]
pub struct StyledLine {
    pub text: String,
    pub runs: Vec<StyleRun>,
    /// Unified paragraph style (alignment, character defaults, spacing, indent, level, list).
    pub paragraph_style: paragraph::Style,
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
    /// Set a single character attribute on a range.
    ///
    /// `old_values` captures per-run previous values of just this attribute
    /// for undo. Each entry is a (range, old_attr) pair.
    SetSpanAttr {
        line: usize,
        range: Range<usize>,
        attr: SpanAttr,
        old_values: Vec<(Range<usize>, SpanAttr)>,
    },
    /// Set paragraph alignment on a line.
    SetAlignment {
        line: usize,
        alignment: Alignment,
        old_alignment: Alignment,
    },
    /// Delete a multi-line selection. Self-contained for undo.
    DeleteRange {
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
        lines: Vec<StyledLine>,
    },
    /// Insert a multi-line range. Self-contained for redo.
    InsertRange {
        start_line: usize,
        start_col: usize,
        lines: Vec<StyledLine>,
    },
    /// Set the paragraph style (spacing, indent, level, list) on a line.
    SetParagraphStyle {
        line: usize,
        style: Box<paragraph::Style>,
        old_style: Box<paragraph::Style>,
    },
    /// Set line height on a line.
    SetLineHeight {
        line: usize,
        line_height: Option<LineHeight>,
        old_line_height: Option<LineHeight>,
    },
}

impl Op {
    /// Return the inverse operations that undo this operation.
    ///
    /// Most operations invert to a single op, but `SetSpanAttr` may produce
    /// multiple ops (one per old_value run to restore).
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
            Op::SetSpanAttr {
                line,
                range,
                attr,
                old_values,
            } => {
                if old_values.is_empty() {
                    // Inverse of an inverse — restore the attr on the full range.
                    // old_values will be filled by capture_op_state at apply time.
                    vec![Op::SetSpanAttr {
                        line: *line,
                        range: range.clone(),
                        attr: attr.clone(),
                        old_values: Vec::new(),
                    }]
                } else {
                    old_values
                        .iter()
                        .map(|(run_range, old_attr)| Op::SetSpanAttr {
                            line: *line,
                            range: run_range.clone(),
                            attr: old_attr.clone(),
                            old_values: Vec::new(),
                        })
                        .collect()
                }
            }
            Op::SetAlignment {
                line,
                alignment,
                old_alignment,
            } => {
                vec![Op::SetAlignment {
                    line: *line,
                    alignment: *old_alignment,
                    old_alignment: *alignment,
                }]
            }
            Op::DeleteRange {
                start_line,
                start_col,
                lines,
                ..
            } => {
                vec![Op::InsertRange {
                    start_line: *start_line,
                    start_col: *start_col,
                    lines: lines.clone(),
                }]
            }
            Op::InsertRange {
                start_line,
                start_col,
                lines,
            } => {
                let (end_line, end_col) = end_position(*start_line, *start_col, lines);
                vec![Op::DeleteRange {
                    start_line: *start_line,
                    start_col: *start_col,
                    end_line,
                    end_col,
                    lines: lines.clone(),
                }]
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
            Op::SetLineHeight {
                line,
                line_height,
                old_line_height,
            } => {
                vec![Op::SetLineHeight {
                    line: *line,
                    line_height: *old_line_height,
                    old_line_height: *line_height,
                }]
            }
        }
    }
}

/// Compute the end position from a start position and a set of styled lines.
fn end_position(start_line: usize, start_col: usize, lines: &[StyledLine]) -> (usize, usize) {
    if lines.len() == 1 {
        (start_line, start_col + lines[0].text.len())
    } else {
        (
            start_line + lines.len() - 1,
            lines.last().map(|l| l.text.len()).unwrap_or(0),
        )
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
                style: span::Style {
                    bold: Some(true),
                    ..span::Style::default()
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
    fn set_span_attr_inverse_restores_old_values() {
        let op = Op::SetSpanAttr {
            line: 0,
            range: 0..10,
            attr: SpanAttr::Bold(Some(true)),
            old_values: vec![
                (0..5, SpanAttr::Bold(None)),
                (5..10, SpanAttr::Bold(Some(false))),
            ],
        };
        let inv = op.inverse();
        assert_eq!(inv.len(), 2);

        match &inv[0] {
            Op::SetSpanAttr {
                line,
                range,
                attr,
                old_values,
            } => {
                assert_eq!(*line, 0);
                assert_eq!(*range, 0..5);
                assert_eq!(*attr, SpanAttr::Bold(None));
                assert!(old_values.is_empty());
            }
            other => panic!("expected SetSpanAttr, got {other:?}"),
        }

        match &inv[1] {
            Op::SetSpanAttr {
                line,
                range,
                attr,
                old_values,
            } => {
                assert_eq!(*line, 0);
                assert_eq!(*range, 5..10);
                assert_eq!(*attr, SpanAttr::Bold(Some(false)));
                assert!(old_values.is_empty());
            }
            other => panic!("expected SetSpanAttr, got {other:?}"),
        }
    }

    #[test]
    fn set_span_attr_inverse_with_empty_old_values() {
        let op = Op::SetSpanAttr {
            line: 1,
            range: 3..7,
            attr: SpanAttr::Underline(Some(true)),
            old_values: Vec::new(),
        };
        let inv = op.inverse();
        assert_eq!(inv.len(), 1);
        match &inv[0] {
            Op::SetSpanAttr {
                line,
                range,
                attr,
                old_values,
            } => {
                assert_eq!(*line, 1);
                assert_eq!(*range, 3..7);
                assert_eq!(*attr, SpanAttr::Underline(Some(true)));
                assert!(old_values.is_empty());
            }
            other => panic!("expected SetSpanAttr, got {other:?}"),
        }
    }

    #[test]
    fn set_alignment_inverse_swaps() {
        let op = Op::SetAlignment {
            line: 0,
            alignment: Alignment::Center,
            old_alignment: Alignment::Left,
        };
        let inv = op.inverse();
        assert_eq!(inv.len(), 1);
        match &inv[0] {
            Op::SetAlignment {
                line,
                alignment,
                old_alignment,
            } => {
                assert_eq!(*line, 0);
                assert_eq!(*alignment, Alignment::Left);
                assert_eq!(*old_alignment, Alignment::Center);
            }
            other => panic!("expected SetAlignment, got {other:?}"),
        }
    }

    fn sample_styled_line(text: &str) -> StyledLine {
        StyledLine {
            text: text.to_string(),
            runs: vec![StyleRun {
                range: 0..text.len(),
                style: span::Style::default(),
            }],
            paragraph_style: paragraph::Style::default(),
        }
    }

    #[test]
    fn delete_range_inverse_is_insert_range() {
        let op = Op::DeleteRange {
            start_line: 1,
            start_col: 5,
            end_line: 3,
            end_col: 10,
            lines: vec![
                sample_styled_line("tail"),
                sample_styled_line("middle line"),
                sample_styled_line("head part"),
            ],
        };
        let inv = op.inverse();
        assert_eq!(inv.len(), 1);
        match &inv[0] {
            Op::InsertRange {
                start_line,
                start_col,
                lines,
            } => {
                assert_eq!(*start_line, 1);
                assert_eq!(*start_col, 5);
                assert_eq!(lines.len(), 3);
                assert_eq!(lines[0].text, "tail");
                assert_eq!(lines[2].text, "head part");
            }
            other => panic!("expected InsertRange, got {other:?}"),
        }
    }

    #[test]
    fn insert_range_inverse_is_delete_range() {
        let op = Op::InsertRange {
            start_line: 2,
            start_col: 3,
            lines: vec![sample_styled_line("abc"), sample_styled_line("defgh")],
        };
        let inv = op.inverse();
        assert_eq!(inv.len(), 1);
        match &inv[0] {
            Op::DeleteRange {
                start_line,
                start_col,
                end_line,
                end_col,
                lines,
            } => {
                assert_eq!(*start_line, 2);
                assert_eq!(*start_col, 3);
                assert_eq!(*end_line, 3);
                assert_eq!(*end_col, 5); // "defgh".len()
                assert_eq!(lines.len(), 2);
            }
            other => panic!("expected DeleteRange, got {other:?}"),
        }
    }

    #[test]
    fn insert_range_single_line_end_position() {
        let op = Op::InsertRange {
            start_line: 0,
            start_col: 10,
            lines: vec![sample_styled_line("hello")],
        };
        let inv = op.inverse();
        match &inv[0] {
            Op::DeleteRange {
                end_line, end_col, ..
            } => {
                assert_eq!(*end_line, 0);
                assert_eq!(*end_col, 15); // 10 + 5
            }
            other => panic!("expected DeleteRange, got {other:?}"),
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

    #[test]
    fn span_attr_from_style_extracts_correct_field() {
        let style = span::Style {
            bold: Some(true),
            italic: Some(false),
            size: Some(16.0),
            ..span::Style::default()
        };
        assert_eq!(
            SpanAttr::from_style(&style, &SpanAttr::Bold(None)),
            SpanAttr::Bold(Some(true))
        );
        assert_eq!(
            SpanAttr::from_style(&style, &SpanAttr::Italic(None)),
            SpanAttr::Italic(Some(false))
        );
        assert_eq!(
            SpanAttr::from_style(&style, &SpanAttr::Size(None)),
            SpanAttr::Size(Some(16.0))
        );
        assert_eq!(
            SpanAttr::from_style(&style, &SpanAttr::Underline(None)),
            SpanAttr::Underline(None)
        );
    }

    #[test]
    fn span_attr_apply_to_sets_only_one_field() {
        let style = span::Style {
            bold: Some(true),
            italic: Some(true),
            ..span::Style::default()
        };
        let modified = SpanAttr::Bold(Some(false)).apply_to(&style);
        assert_eq!(modified.bold, Some(false));
        assert_eq!(modified.italic, Some(true)); // preserved
    }

    #[test]
    fn set_paragraph_style_inverse_swaps() {
        let old = paragraph::Style {
            level: 0,
            ..Default::default()
        };
        let new = paragraph::Style {
            level: 1,
            list: Some(paragraph::List::Bullet(paragraph::Bullet::Disc)),
            indent: paragraph::Indent {
                left: 36.0,
                hanging: 18.0,
            },
            ..Default::default()
        };
        let op = Op::SetParagraphStyle {
            line: 2,
            style: Box::new(new.clone()),
            old_style: Box::new(old.clone()),
        };
        let inv = op.inverse();
        assert_eq!(inv.len(), 1);
        match &inv[0] {
            Op::SetParagraphStyle {
                line,
                style,
                old_style,
            } => {
                assert_eq!(*line, 2);
                assert_eq!(**style, old);
                assert_eq!(**old_style, new);
            }
            other => panic!("expected SetParagraphStyle, got {other:?}"),
        }
    }

    #[test]
    fn set_paragraph_style_double_inverse_matches_original() {
        let style = paragraph::Style {
            level: 2,
            line_spacing: Some(paragraph::Spacing::Multiple(1.5)),
            space_before: Some(12.0),
            ..Default::default()
        };
        let op = Op::SetParagraphStyle {
            line: 0,
            style: Box::new(style.clone()),
            old_style: Box::new(paragraph::Style::default()),
        };
        let inv = op.inverse();
        let double_inv = inv[0].inverse();
        assert_eq!(double_inv.len(), 1);
        match &double_inv[0] {
            Op::SetParagraphStyle {
                line,
                style: s,
                old_style,
            } => {
                assert_eq!(*line, 0);
                assert_eq!(**s, style);
                assert_eq!(**old_style, paragraph::Style::default());
            }
            other => panic!("expected SetParagraphStyle, got {other:?}"),
        }
    }
}
