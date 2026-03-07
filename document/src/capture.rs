//! Capture module — reads styled text from the editor before applying
//! operations so that undo can reconstruct prior state.

use std::ops::Range;

use iced_core::text::rich_editor::Editor;

use super::op::{StyleRun, StyledText};

/// Read character-style runs from the editor over a column range on one line.
///
/// Consecutive columns with equal styles are compressed into a single
/// [`StyleRun`]. Returns an empty vec when `range` is empty.
pub fn read_style_runs<E: Editor>(editor: &E, line: usize, range: Range<usize>) -> Vec<StyleRun> {
    if range.is_empty() {
        return Vec::new();
    }

    let mut runs: Vec<StyleRun> = Vec::new();

    for col in range.clone() {
        let style = editor.style_at(line, col);

        match runs.last_mut() {
            Some(last) if last.style == style => {
                last.range.end = col + 1;
            }
            _ => {
                runs.push(StyleRun {
                    range: col..col + 1,
                    style,
                });
            }
        }
    }

    runs
}

/// Read styled text from the editor on one line over a column range.
///
/// `text` is the caller-provided content string (e.g. from `editor.line()`).
/// Styles are captured via [`read_style_runs`].
pub fn read_styled_text<E: Editor>(
    editor: &E,
    line: usize,
    range: Range<usize>,
    text: &str,
) -> StyledText {
    StyledText {
        text: text.to_string(),
        runs: read_style_runs(editor, line, range),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_core::text::rich_editor::{
        Action, Cursor, Line, ParagraphStyle, Position, Selection, Style,
    };
    use iced_core::text::{LineHeight, Wrapping};
    use iced_core::{Em, Font, Pixels, Size};
    use std::borrow::Cow;

    #[derive(Default)]
    struct MockEditor {
        styles: Vec<Vec<Style>>,
    }

    impl Editor for MockEditor {
        type Font = Font;

        fn with_text(_text: &str) -> Self {
            Self::default()
        }

        fn is_empty(&self) -> bool {
            self.styles.is_empty()
        }

        fn cursor(&self) -> Cursor {
            Cursor {
                position: Position { line: 0, column: 0 },
                selection: None,
            }
        }

        fn selection(&self) -> Selection {
            Selection::Caret(iced_core::Point::ORIGIN)
        }

        fn copy(&self) -> Option<String> {
            None
        }

        fn line(&self, _index: usize) -> Option<Line<'_>> {
            Some(Line {
                text: Cow::Borrowed(""),
                ending: iced_core::text::editor::LineEnding::None,
            })
        }

        fn line_count(&self) -> usize {
            self.styles.len()
        }

        fn perform(&mut self, _action: Action) {}

        fn move_to(&mut self, _cursor: Cursor) {}

        fn bounds(&self) -> Size {
            Size::ZERO
        }

        fn min_bounds(&self) -> Size {
            Size::ZERO
        }

        fn hint_factor(&self) -> Option<f32> {
            None
        }

        fn update(
            &mut self,
            _new_bounds: Size,
            _new_font: Self::Font,
            _new_size: Pixels,
            _new_line_height: LineHeight,
            _new_letter_spacing: Em,
            _new_font_features: Vec<iced_core::font::Feature>,
            _new_wrapping: Wrapping,
            _new_hint_factor: Option<f32>,
        ) {
        }

        fn set_span_style(&mut self, _line: usize, _range: std::ops::Range<usize>, _style: &Style) {
        }

        fn set_paragraph_style(&mut self, _line: usize, _style: &ParagraphStyle) {}

        fn style_at(&self, line: usize, column: usize) -> Style {
            self.styles
                .get(line)
                .and_then(|cols| cols.get(column))
                .cloned()
                .unwrap_or_default()
        }

        fn paragraph_style(&self, _line: usize) -> ParagraphStyle {
            ParagraphStyle::default()
        }
    }

    fn bold_style() -> Style {
        Style {
            bold: Some(true),
            ..Style::default()
        }
    }

    fn italic_style() -> Style {
        Style {
            italic: Some(true),
            ..Style::default()
        }
    }

    #[test]
    fn single_style_single_run() {
        let editor = MockEditor {
            styles: vec![vec![bold_style(); 5]],
        };

        let runs = read_style_runs(&editor, 0, 0..5);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].range, 0..5);
        assert_eq!(runs[0].style, bold_style());
    }

    #[test]
    fn adjacent_equal_styles_merge() {
        // Cols 0..3 bold, cols 3..5 bold — should merge into one run
        let editor = MockEditor {
            styles: vec![vec![bold_style(); 5]],
        };

        let runs = read_style_runs(&editor, 0, 0..5);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].range, 0..5);
    }

    #[test]
    fn different_styles_produce_multiple_runs() {
        let mut line_styles = vec![bold_style(); 2];
        line_styles.extend(vec![italic_style(); 2]);

        let editor = MockEditor {
            styles: vec![line_styles],
        };

        let runs = read_style_runs(&editor, 0, 0..4);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].range, 0..2);
        assert_eq!(runs[0].style, bold_style());
        assert_eq!(runs[1].range, 2..4);
        assert_eq!(runs[1].style, italic_style());
    }

    #[test]
    fn empty_range_returns_empty() {
        let editor = MockEditor {
            styles: vec![vec![Style::default(); 5]],
        };

        let runs = read_style_runs(&editor, 0, 0..0);
        assert!(runs.is_empty());
    }

    #[test]
    fn read_styled_text_captures_text_and_runs() {
        let editor = MockEditor {
            styles: vec![vec![bold_style(); 5]],
        };

        let styled = read_styled_text(&editor, 0, 0..5, "hello");
        assert_eq!(styled.text, "hello");
        assert_eq!(styled.runs.len(), 1);
        assert_eq!(styled.runs[0].range, 0..5);
        assert_eq!(styled.runs[0].style, bold_style());
    }
}
