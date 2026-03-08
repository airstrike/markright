//! Formatting operations — bold, italic, underline, alignment, font, size.

use crate::core::text::rich_editor::{Editor, Style as RichStyle};
use markright_document::{self as document, Alignment, Op, SpanAttr};
use std::ops::Range;

use super::super::action::FormatAction;
use super::{Cursor, Position, ordered_positions};

/// Apply a format action to the editor.
///
/// Returns ops for selection-based formatting (and SetAlignment which always
/// applies). Returns an empty vec when there's no selection — the caller should
/// update pending_style instead.
pub fn format<E: Editor>(editor: &mut E, fmt: &FormatAction) -> Vec<Op> {
    let cursor = editor.cursor();
    let has_selection = cursor.selection.is_some();

    match fmt {
        FormatAction::ToggleBold => {
            if !has_selection {
                return vec![];
            }
            let is_bold = style_at_selection_start(editor, &cursor)
                .bold
                .unwrap_or(false);
            set_attr_in_selection(editor, SpanAttr::Bold(Some(!is_bold)))
        }
        FormatAction::ToggleItalic => {
            if !has_selection {
                return vec![];
            }
            let is_italic = style_at_selection_start(editor, &cursor)
                .italic
                .unwrap_or(false);
            set_attr_in_selection(editor, SpanAttr::Italic(Some(!is_italic)))
        }
        FormatAction::ToggleUnderline => {
            if !has_selection {
                return vec![];
            }
            let is_underline = style_at_selection_start(editor, &cursor)
                .underline
                .unwrap_or(false);
            set_attr_in_selection(editor, SpanAttr::Underline(Some(!is_underline)))
        }
        FormatAction::SetAlignment(alignment) => set_alignment(editor, *alignment),
        FormatAction::SetFont(font) => {
            if !has_selection {
                return vec![];
            }
            set_attr_in_selection(editor, SpanAttr::Font(Some(*font)))
        }
        FormatAction::SetFontSize(size) => {
            if !has_selection {
                return vec![];
            }
            set_attr_in_selection(editor, SpanAttr::Size(Some(*size)))
        }
    }
}

/// Set a single span attribute across the current selection.
fn set_attr_in_selection<E: Editor>(editor: &mut E, attr: SpanAttr) -> Vec<Op> {
    let cursor = editor.cursor();
    let Some(ref sel) = cursor.selection else {
        return vec![];
    };
    let (start, end) = ordered_positions(&cursor.position, sel);
    set_attr_range(editor, start, end, &attr)
}

/// Set a single span attribute across a multi-line range.
fn set_attr_range<E: Editor>(
    editor: &mut E,
    start: &Position,
    end: &Position,
    attr: &SpanAttr,
) -> Vec<Op> {
    let mut ops = Vec::new();

    for line in start.line..=end.line {
        let col_start = if line == start.line { start.column } else { 0 };
        let col_end = if line == end.line {
            end.column
        } else {
            editor.line(line).map(|l| l.text.len()).unwrap_or(0)
        };

        if col_start < col_end {
            ops.push(set_attr_on_line(editor, line, col_start..col_end, attr));
        }
    }

    ops
}

/// Set a single span attribute on one line range.
///
/// Reads existing styles per-run, applies only the one attribute via
/// read-modify-write, and returns a `SetSpanAttr` op with old values.
fn set_attr_on_line<E: Editor>(
    editor: &mut E,
    line: usize,
    range: Range<usize>,
    attr: &SpanAttr,
) -> Op {
    let runs = document::read_style_runs(editor, line, range.clone());

    // Collect old values of just this attribute, compressed into runs.
    let mut old_values: Vec<(Range<usize>, SpanAttr)> = Vec::new();
    for run in &runs {
        let old_attr = SpanAttr::from_style(&run.style, attr);
        match old_values.last_mut() {
            Some((last_range, last_attr))
                if *last_attr == old_attr && last_range.end == run.range.start =>
            {
                last_range.end = run.range.end;
            }
            _ => {
                old_values.push((run.range.clone(), old_attr));
            }
        }
    }

    // Apply: read-modify-write each run.
    if runs.is_empty() {
        let style = attr.apply_to(&Default::default());
        editor.set_span_style(line, range.clone(), &style);
    } else {
        for run in &runs {
            let merged = attr.apply_to(&run.style);
            editor.set_span_style(line, run.range.clone(), &merged);
        }
    }

    Op::SetSpanAttr {
        line,
        range,
        attr: attr.clone(),
        old_values,
    }
}

/// Set alignment on lines covered by the current cursor/selection.
fn set_alignment<E: Editor>(editor: &mut E, alignment: Alignment) -> Vec<Op> {
    let cursor = editor.cursor();
    let lines = if let Some(ref sel) = cursor.selection {
        let (start, end) = ordered_positions(&cursor.position, sel);
        start.line..=end.line
    } else {
        cursor.position.line..=cursor.position.line
    };
    lines
        .map(|line| {
            let old_alignment = Alignment::from_iced(editor.paragraph_style(line).alignment);
            editor.set_alignment(line, alignment.to_iced());
            Op::SetAlignment {
                line,
                alignment,
                old_alignment,
            }
        })
        .collect()
}

/// Returns the style at the first non-empty character in the selection.
///
/// Skips blank lines at the start so that the toggle state reflects actual
/// content, not unformatted newlines.
fn style_at_selection_start<E: Editor>(editor: &E, cursor: &Cursor) -> RichStyle {
    let (start, end) = match &cursor.selection {
        Some(sel) => ordered_positions(&cursor.position, sel),
        None => {
            return editor.style_at(cursor.position.line, cursor.position.column);
        }
    };

    for line in start.line..=end.line {
        let col_start = if line == start.line { start.column } else { 0 };
        let col_end = if line == end.line {
            end.column
        } else {
            editor.line(line).map(|l| l.text.len()).unwrap_or(0)
        };
        if col_start < col_end {
            return editor.style_at(line, col_start);
        }
    }

    editor.style_at(start.line, start.column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::text::editor::{Action, Line, LineEnding, Selection};
    use crate::core::text::rich_editor::{ParagraphStyle, Style};
    use crate::core::text::{LineHeight, Wrapping};
    use crate::core::{Em, Font, Pixels, Size};
    use std::borrow::Cow;

    /// Mock editor that tracks per-column styles, paragraph styles, and
    /// cursor/selection state. Handles Insert, Delete, Backspace, Paste,
    /// Enter, and SelectAll for integration-style tests.
    struct MockEditor {
        lines: Vec<String>,
        styles: Vec<Vec<Style>>,
        para_styles: Vec<ParagraphStyle>,
        cursor: Cursor,
    }

    impl MockEditor {
        fn new(text: &str) -> Self {
            let lines: Vec<String> = if text.is_empty() {
                vec![String::new()]
            } else {
                text.lines().map(|l| l.to_string()).collect()
            };
            let styles: Vec<Vec<Style>> = lines
                .iter()
                .map(|l| vec![Style::default(); l.len()])
                .collect();
            let para_styles = vec![ParagraphStyle::default(); lines.len()];
            let last_line = lines.len().saturating_sub(1);
            let last_col = lines.last().map(|l| l.len()).unwrap_or(0);
            Self {
                lines,
                styles,
                para_styles,
                cursor: Cursor {
                    position: Position {
                        line: last_line,
                        column: last_col,
                    },
                    selection: None,
                },
            }
        }

        fn select_all(&mut self) {
            let last_line = self.lines.len().saturating_sub(1);
            let last_col = self.lines.last().map(|l| l.len()).unwrap_or(0);
            self.cursor = Cursor {
                position: Position { line: 0, column: 0 },
                selection: Some(Position {
                    line: last_line,
                    column: last_col,
                }),
            };
        }

        fn delete_selection(&mut self) {
            let Some(sel) = self.cursor.selection else {
                return;
            };
            let (start, end) = ordered_positions(&self.cursor.position, &sel);
            let (sl, sc) = (start.line, start.column);
            let (el, ec) = (end.line, end.column);

            if sl == el {
                self.lines[sl].replace_range(sc..ec, "");
                self.styles[sl].drain(sc..ec);
            } else {
                let tail = self.lines[el][ec..].to_string();
                let tail_styles: Vec<Style> = self.styles[el][ec..].to_vec();
                self.lines[sl].truncate(sc);
                self.styles[sl].truncate(sc);
                self.lines[sl].push_str(&tail);
                self.styles[sl].extend(tail_styles);
                for _ in (sl + 1..=el).rev() {
                    self.lines.remove(sl + 1);
                    self.styles.remove(sl + 1);
                    self.para_styles.remove(sl + 1);
                }
            }
            self.cursor = Cursor {
                position: Position {
                    line: sl,
                    column: sc,
                },
                selection: None,
            };
        }

        fn insert_char(&mut self, c: char) {
            if self.cursor.selection.is_some() {
                self.delete_selection();
            }
            let line = self.cursor.position.line;
            let col = self.cursor.position.column;
            let s = c.to_string();
            self.lines[line].insert_str(col, &s);
            self.styles[line].insert(col, Style::default());
            self.cursor.position.column += s.len();
        }
    }

    impl Default for MockEditor {
        fn default() -> Self {
            Self::new("")
        }
    }

    impl Editor for MockEditor {
        type Font = Font;

        fn with_text(text: &str) -> Self {
            Self::new(text)
        }

        fn is_empty(&self) -> bool {
            self.lines.len() == 1 && self.lines[0].is_empty()
        }

        fn cursor(&self) -> Cursor {
            self.cursor
        }

        fn selection(&self) -> Selection {
            Selection::Caret(crate::core::Point::ORIGIN)
        }

        fn copy(&self) -> Option<String> {
            None
        }

        fn line(&self, index: usize) -> Option<Line<'_>> {
            self.lines.get(index).map(|text| Line {
                text: Cow::Borrowed(text.as_str()),
                ending: LineEnding::None,
            })
        }

        fn line_count(&self) -> usize {
            self.lines.len()
        }

        fn perform(&mut self, action: Action) {
            match action {
                Action::SelectAll => self.select_all(),
                Action::Edit(edit) => match edit {
                    crate::core::text::editor::Edit::Insert(c) => self.insert_char(c),
                    crate::core::text::editor::Edit::Delete
                    | crate::core::text::editor::Edit::Backspace => {
                        if self.cursor.selection.is_some() {
                            self.delete_selection();
                        }
                    }
                    crate::core::text::editor::Edit::Paste(ref text) => {
                        if self.cursor.selection.is_some() {
                            self.delete_selection();
                        }
                        let line = self.cursor.position.line;
                        let col = self.cursor.position.column;
                        self.lines[line].insert_str(col, text);
                        let new_styles = vec![Style::default(); text.len()];
                        let rest = self.styles[line].split_off(col);
                        self.styles[line].extend(new_styles);
                        self.styles[line].extend(rest);
                        self.cursor.position.column += text.len();
                    }
                    crate::core::text::editor::Edit::Indent
                    | crate::core::text::editor::Edit::Unindent => {}
                    crate::core::text::editor::Edit::Enter => {
                        let line = self.cursor.position.line;
                        let col = self.cursor.position.column;
                        let tail = self.lines[line][col..].to_string();
                        let tail_styles = self.styles[line][col..].to_vec();
                        self.lines[line].truncate(col);
                        self.styles[line].truncate(col);
                        self.lines.insert(line + 1, tail);
                        self.styles.insert(line + 1, tail_styles);
                        self.para_styles.insert(line + 1, ParagraphStyle::default());
                        self.cursor = Cursor {
                            position: Position {
                                line: line + 1,
                                column: 0,
                            },
                            selection: None,
                        };
                    }
                },
                _ => {}
            }
        }

        fn move_to(&mut self, cursor: Cursor) {
            self.cursor = cursor;
        }

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
            _new_font_features: Vec<crate::core::font::Feature>,
            _new_wrapping: Wrapping,
            _new_hint_factor: Option<f32>,
        ) {
        }

        fn set_span_style(&mut self, line: usize, range: Range<usize>, style: &Style) {
            // Full replace — matches real iced editor behavior.
            if let Some(line_styles) = self.styles.get_mut(line) {
                for col in range {
                    if let Some(cell) = line_styles.get_mut(col) {
                        *cell = style.clone();
                    }
                }
            }
        }

        fn set_paragraph_style(&mut self, line: usize, style: &ParagraphStyle) {
            if let Some(ps) = self.para_styles.get_mut(line) {
                *ps = style.clone();
            }
        }

        fn set_alignment(&mut self, line: usize, alignment: crate::core::text::Alignment) {
            if let Some(ps) = self.para_styles.get_mut(line) {
                ps.alignment = Some(alignment);
            }
        }

        fn style_at(&self, line: usize, column: usize) -> Style {
            self.styles
                .get(line)
                .and_then(|cols| cols.get(column))
                .cloned()
                .unwrap_or_default()
        }

        fn paragraph_style(&self, line: usize) -> ParagraphStyle {
            self.para_styles.get(line).cloned().unwrap_or_default()
        }
    }

    /// Apply format ops then undo them by applying inverses via apply_op.
    fn undo_ops<E: Editor>(editor: &mut E, ops: &[Op]) {
        for op in ops.iter().rev() {
            for inv in op.inverse() {
                super::super::apply_op(editor, &inv);
            }
        }
    }

    #[test]
    fn select_all_bold_italic_underline_then_undo() {
        let mut editor = MockEditor::new("hello");
        editor.select_all();

        // Apply bold
        let bold_ops = format(&mut editor, &FormatAction::ToggleBold);
        assert!(
            !bold_ops.is_empty(),
            "bold should produce ops with selection"
        );
        for col in 0..5 {
            let s = editor.style_at(0, col);
            assert_eq!(s.bold, Some(true), "col {col} should be bold");
            assert_ne!(s.italic, Some(true), "col {col} should not be italic yet");
        }

        // Apply italic — should NOT clear bold
        editor.select_all();
        let italic_ops = format(&mut editor, &FormatAction::ToggleItalic);
        for col in 0..5 {
            let s = editor.style_at(0, col);
            assert_eq!(s.bold, Some(true), "col {col} should still be bold");
            assert_eq!(s.italic, Some(true), "col {col} should be italic");
        }

        // Apply underline — should NOT clear bold or italic
        editor.select_all();
        let underline_ops = format(&mut editor, &FormatAction::ToggleUnderline);
        for col in 0..5 {
            let s = editor.style_at(0, col);
            assert_eq!(s.bold, Some(true), "col {col} should still be bold");
            assert_eq!(s.italic, Some(true), "col {col} should still be italic");
            assert_eq!(s.underline, Some(true), "col {col} should be underlined");
        }

        // Undo underline
        undo_ops(&mut editor, &underline_ops);
        for col in 0..5 {
            let s = editor.style_at(0, col);
            assert_eq!(s.bold, Some(true), "col {col} bold after undo underline");
            assert_eq!(
                s.italic,
                Some(true),
                "col {col} italic after undo underline"
            );
            assert_ne!(
                s.underline,
                Some(true),
                "col {col} underline should be gone"
            );
        }

        // Undo italic
        undo_ops(&mut editor, &italic_ops);
        for col in 0..5 {
            let s = editor.style_at(0, col);
            assert_eq!(s.bold, Some(true), "col {col} bold after undo italic");
            assert_ne!(s.italic, Some(true), "col {col} italic should be gone");
        }

        // Undo bold
        undo_ops(&mut editor, &bold_ops);
        for col in 0..5 {
            let s = editor.style_at(0, col);
            assert_ne!(s.bold, Some(true), "col {col} bold should be gone");
            assert_ne!(s.italic, Some(true), "col {col} italic should be gone");
            assert_ne!(
                s.underline,
                Some(true),
                "col {col} underline should be gone"
            );
        }
    }
}
