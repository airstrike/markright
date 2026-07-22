//! Capture module — reads styled text from the editor before applying
//! operations so that undo can reconstruct prior state.

use std::ops::Range;

use iced_core::text::rich_editor::Editor;

use super::op::{StyleRun, StyledLine, StyledText};
use crate::paragraph::Paragraph;

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

/// Read a line's content with 0-based style runs and paragraph formatting.
///
/// Unlike [`read_style_runs`] which returns absolute column positions,
/// the runs here are normalized to start at 0 relative to the captured text.
///
/// `paragraph` is the authoritative paragraph for the line — the editor
/// only holds visual style, not names or override flags. Its character
/// defaults are refreshed from the editor so line-default edits (e.g. a
/// color set on an empty line) are captured too. `col_range` is clamped
/// to the line length.
pub fn read_styled_line<E: Editor>(
    editor: &E,
    line: usize,
    col_range: Range<usize>,
    paragraph: &Paragraph,
) -> StyledLine {
    let len = editor.line(line).map(|l| l.text.len()).unwrap_or(0);
    let start = col_range.start.min(len);
    let end = col_range.end.min(len);

    let text = editor
        .line(line)
        .map(|l| l.text[start..end].to_string())
        .unwrap_or_default();
    let abs_runs = read_style_runs(editor, line, start..end);
    let runs = abs_runs
        .into_iter()
        .map(|r| StyleRun {
            range: (r.range.start - start)..(r.range.end - start),
            style: r.style,
        })
        .collect();

    let mut paragraph = paragraph.clone();
    paragraph.style.style = editor.paragraph_style_at(line).style;

    StyledLine {
        text,
        runs,
        paragraph,
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
