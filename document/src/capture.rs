//! Capture module — reads styled text from the editor before applying
//! operations so that undo can reconstruct prior state.

use std::ops::Range;

use iced_core::text::rich_editor::Editor;

use super::op::{StyleRun, StyledLine, StyledText};

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
        let style = editor.span_style_at(line, col);

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

/// Read a line's content with 0-based style runs and paragraph style.
///
/// Unlike [`read_style_runs`] which returns absolute column positions,
/// the runs here are normalized to start at 0 relative to the captured text.
pub fn read_styled_line<E: Editor>(editor: &E, line: usize, col_range: Range<usize>) -> StyledLine {
    let text = editor
        .line(line)
        .map(|l| l.text[col_range.start..col_range.end.min(l.text.len())].to_string())
        .unwrap_or_default();
    let abs_runs = read_style_runs(editor, line, col_range.clone());
    let offset = col_range.start;
    let runs = abs_runs
        .into_iter()
        .map(|r| StyleRun {
            range: (r.range.start - offset)..(r.range.end - offset),
            style: r.style,
        })
        .collect();
    StyledLine {
        text,
        runs,
        paragraph_style: editor.paragraph_style_at(line),
    }
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
