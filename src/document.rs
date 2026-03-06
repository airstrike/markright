use iced_core::{Color, Font};
use std::ops::Range;

/// Per-character formatting attributes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpanFormat {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub font: Option<Font>,
    pub size: Option<f32>,
    pub color: Option<Color>,
}

impl SpanFormat {
    /// Returns true if all fields are at their default (plain text) values.
    pub fn is_default(&self) -> bool {
        !self.bold
            && !self.italic
            && !self.underline
            && self.font.is_none()
            && self.size.is_none()
            && self.color.is_none()
    }
}

/// Text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Per-line formatting attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct LineFormat {
    pub alignment: Alignment,
    /// Heading level 1-6, or None for normal text.
    pub heading_level: Option<u8>,
    pub spacing_after: f32,
}

impl Default for LineFormat {
    fn default() -> Self {
        Self {
            alignment: Alignment::Left,
            heading_level: None,
            spacing_after: 0.0,
        }
    }
}

/// A line with its formatting.
#[derive(Debug, Clone)]
pub struct FormattedLine {
    pub format: LineFormat,
    pub spans: Vec<(Range<usize>, SpanFormat)>,
}

impl FormattedLine {
    fn new() -> Self {
        Self {
            format: LineFormat::default(),
            spans: Vec::new(),
        }
    }
}

/// The rich document model -- parallel formatting storage alongside iced's Content.
#[derive(Debug, Clone)]
pub struct RichDocument {
    lines: Vec<FormattedLine>,
}

impl RichDocument {
    /// Create an empty document with one default-formatted line.
    pub fn new() -> Self {
        Self {
            lines: vec![FormattedLine::new()],
        }
    }

    /// Create a document with `n` default-formatted lines.
    pub fn with_lines(n: usize) -> Self {
        let lines = (0..n).map(|_| FormattedLine::new()).collect();
        Self { lines }
    }

    /// The number of lines in the document.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get an immutable reference to a line's format.
    pub fn line_format(&self, line: usize) -> &LineFormat {
        &self.lines[line].format
    }

    /// Get a mutable reference to a line's format.
    pub fn line_format_mut(&mut self, line: usize) -> &mut LineFormat {
        &mut self.lines[line].format
    }

    /// Get the spans for a given line.
    pub fn spans(&self, line: usize) -> &[(Range<usize>, SpanFormat)] {
        &self.lines[line].spans
    }

    /// Returns the format at a specific position. If no span covers that
    /// position, returns `SpanFormat::default()`.
    pub fn format_at(&self, line: usize, col: usize) -> SpanFormat {
        for (range, fmt) in &self.lines[line].spans {
            if range.start <= col && col < range.end {
                return fmt.clone();
            }
        }
        SpanFormat::default()
    }

    /// Toggle bold on the given character range. If all characters in the range
    /// are already bold, remove bold; otherwise make them all bold.
    pub fn toggle_bold(&mut self, line: usize, col_range: Range<usize>) {
        let all_bold = self.all_in_range_have(line, &col_range, |f| f.bold);
        self.set_format_property(line, col_range, |f| f.bold = !all_bold);
    }

    /// Toggle italic on the given character range.
    pub fn toggle_italic(&mut self, line: usize, col_range: Range<usize>) {
        let all_italic = self.all_in_range_have(line, &col_range, |f| f.italic);
        self.set_format_property(line, col_range, |f| f.italic = !all_italic);
    }

    /// Toggle underline on the given character range.
    pub fn toggle_underline(&mut self, line: usize, col_range: Range<usize>) {
        let all_underline = self.all_in_range_have(line, &col_range, |f| f.underline);
        self.set_format_property(line, col_range, |f| f.underline = !all_underline);
    }

    /// Generic span mutation: ensures coverage of `col_range`, splits/merges
    /// spans as needed, and applies `setter` to all spans overlapping the range.
    pub fn set_format_property(
        &mut self,
        line: usize,
        col_range: Range<usize>,
        setter: impl Fn(&mut SpanFormat),
    ) {
        if col_range.is_empty() {
            return;
        }
        self.ensure_span_coverage(line, col_range.clone());

        for (range, fmt) in &mut self.lines[line].spans {
            if range.start < col_range.end && range.end > col_range.start {
                setter(fmt);
            }
        }

        self.merge_adjacent_spans(line);
    }

    /// Shift all spans on this line that start at or after `col` to the right
    /// by `len`. Spans that contain `col` are extended. New text inherits the
    /// format at the insertion point.
    pub fn insert_at(&mut self, line: usize, col: usize, len: usize) {
        if len == 0 {
            return;
        }

        let spans = &mut self.lines[line].spans;
        let mut found_containing = false;

        for (range, _fmt) in spans.iter_mut() {
            if range.start <= col && col < range.end {
                // Span contains the insertion point -- extend it.
                range.end += len;
                found_containing = true;
            } else if range.start >= col {
                // Span starts at or after insertion -- shift right.
                range.start += len;
                range.end += len;
            }
            // Spans ending at or before col are unaffected.
        }

        // If no span contained the insertion point, inherit the format at col
        // and create a new span for the inserted text.
        if !found_containing {
            let inherited = self.format_at(line, col);
            if !inherited.is_default() {
                // Find the right position to insert the new span (sorted by start).
                let insert_pos = self.lines[line]
                    .spans
                    .iter()
                    .position(|(r, _)| r.start > col)
                    .unwrap_or(self.lines[line].spans.len());
                self.lines[line]
                    .spans
                    .insert(insert_pos, (col..col + len, inherited));
            }
        }

        self.merge_adjacent_spans(line);
    }

    /// Remove characters in the range `[col_start, col_end)`, shift subsequent
    /// spans left, remove spans that become empty, and merge adjacent spans with
    /// the same format.
    pub fn delete_range(&mut self, line: usize, col_start: usize, col_end: usize) {
        if col_start >= col_end {
            return;
        }
        let delete_len = col_end - col_start;

        // Rebuild spans from scratch with correct new boundaries.
        let old_spans: Vec<_> = self.lines[line].spans.drain(..).collect();
        for (range, fmt) in old_spans {
            let old_start = range.start;
            let old_end = range.end;

            let new_start;
            let new_end;

            if old_end <= col_start {
                // Entirely before deletion -- unchanged.
                new_start = old_start;
                new_end = old_end;
            } else if old_start >= col_end {
                // Entirely after deletion -- shift left.
                new_start = old_start - delete_len;
                new_end = old_end - delete_len;
            } else {
                // Overlaps the deletion.
                // Characters surviving before the deleted region:
                let chars_before = col_start.saturating_sub(old_start);
                // Characters surviving after the deleted region:
                let chars_after = old_end.saturating_sub(col_end);
                new_start = old_start.min(col_start);
                new_end = new_start + chars_before + chars_after;
            }

            if new_start < new_end {
                self.lines[line].spans.push((new_start..new_end, fmt));
            }
        }

        self.merge_adjacent_spans(line);
    }

    /// Split a line at `col` (for Enter key). Spans before `col` stay on
    /// the current line; spans at/after `col` move to a new line inserted
    /// after. The new line inherits the original's `LineFormat`.
    pub fn split_line(&mut self, line: usize, col: usize) {
        let original = &self.lines[line];
        let line_format = original.format.clone();

        let mut before_spans = Vec::new();
        let mut after_spans = Vec::new();

        for (range, fmt) in &original.spans {
            if range.end <= col {
                // Entirely before the split.
                before_spans.push((range.clone(), fmt.clone()));
            } else if range.start >= col {
                // Entirely after the split -- shift to start of new line.
                let new_range = (range.start - col)..(range.end - col);
                after_spans.push((new_range, fmt.clone()));
            } else {
                // Spans the split point -- split it.
                before_spans.push((range.start..col, fmt.clone()));
                after_spans.push((0..(range.end - col), fmt.clone()));
            }
        }

        self.lines[line].spans = before_spans;

        let new_line = FormattedLine {
            format: line_format,
            spans: after_spans,
        };
        self.lines.insert(line + 1, new_line);
    }

    /// Merge line `line` with the next line (for Backspace at line start).
    /// Spans from the next line are offset by the current line's logical
    /// length and appended. The next line is then removed.
    pub fn merge_lines(&mut self, line: usize) {
        if line + 1 >= self.lines.len() {
            return;
        }

        // Determine the offset: the end of the last span on the current line,
        // or 0 if there are no spans.
        let offset = self.lines[line]
            .spans
            .last()
            .map(|(r, _)| r.end)
            .unwrap_or(0);

        let next_spans: Vec<_> = self.lines[line + 1].spans.clone();
        for (range, fmt) in next_spans {
            let shifted = (range.start + offset)..(range.end + offset);
            self.lines[line].spans.push((shifted, fmt));
        }

        self.lines.remove(line + 1);
        self.merge_adjacent_spans(line);
    }

    /// Grow or shrink to exactly `n` lines. New lines get default formatting.
    pub fn ensure_lines(&mut self, n: usize) {
        match n.cmp(&self.lines.len()) {
            std::cmp::Ordering::Greater => {
                let additional = n - self.lines.len();
                for _ in 0..additional {
                    self.lines.push(FormattedLine::new());
                }
            }
            std::cmp::Ordering::Less => {
                self.lines.truncate(n);
            }
            std::cmp::Ordering::Equal => {}
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Check whether all characters in `range` have a property (via `pred`).
    /// If there are no spans covering part of the range, those positions have
    /// `SpanFormat::default()`, so the predicate is evaluated on that too.
    fn all_in_range_have(
        &self,
        line: usize,
        range: &Range<usize>,
        pred: fn(&SpanFormat) -> bool,
    ) -> bool {
        if range.is_empty() {
            return true;
        }

        // Build coverage of the range from existing spans.
        let mut cursor = range.start;
        let spans = &self.lines[line].spans;

        // Collect spans that overlap the range, sorted by start.
        let mut relevant: Vec<&(Range<usize>, SpanFormat)> = spans
            .iter()
            .filter(|(r, _)| r.start < range.end && r.end > range.start)
            .collect();
        relevant.sort_by_key(|(r, _)| r.start);

        for (r, fmt) in &relevant {
            let overlap_start = r.start.max(range.start);
            if cursor < overlap_start {
                // Gap before this span -- default format applies.
                if !pred(&SpanFormat::default()) {
                    return false;
                }
            }
            if !pred(fmt) {
                return false;
            }
            cursor = cursor.max(r.end);
        }

        // Check trailing gap.
        if cursor < range.end && !pred(&SpanFormat::default()) {
            return false;
        }

        true
    }

    /// Ensure that span boundaries align with the given range boundaries.
    /// After this call, there will be span boundaries at `range.start` and
    /// `range.end` (splitting any span that straddles those points), and any
    /// gap within the range is filled with a default-format span.
    fn ensure_span_coverage(&mut self, line: usize, range: Range<usize>) {
        if range.is_empty() {
            return;
        }

        // Step 1: Split any span that straddles range.start.
        self.split_span_at(line, range.start);

        // Step 2: Split any span that straddles range.end.
        self.split_span_at(line, range.end);

        // Step 3: Fill gaps within the range with default-format spans.
        self.fill_gaps(line, range);
    }

    /// Split any span on `line` that contains `col` (strictly inside, not at
    /// a boundary) into two spans at `col`.
    fn split_span_at(&mut self, line: usize, col: usize) {
        let spans = &mut self.lines[line].spans;
        for i in 0..spans.len() {
            let (ref range, _) = spans[i];
            if range.start < col && col < range.end {
                let fmt = spans[i].1.clone();
                let old_end = spans[i].0.end;
                spans[i].0.end = col;
                spans.insert(i + 1, (col..old_end, fmt));
                return;
            }
        }
    }

    /// Fill any gaps in span coverage within `range` with default-format spans.
    fn fill_gaps(&mut self, line: usize, range: Range<usize>) {
        // Collect existing spans overlapping the range.
        let spans = &self.lines[line].spans;
        let mut covered: Vec<Range<usize>> = spans
            .iter()
            .filter(|(r, _)| r.start < range.end && r.end > range.start)
            .map(|(r, _)| r.clone())
            .collect();
        covered.sort_by_key(|r| r.start);

        let mut gaps = Vec::new();
        let mut cursor = range.start;
        for r in &covered {
            let overlap_start = r.start.max(range.start);
            if cursor < overlap_start {
                gaps.push(cursor..overlap_start);
            }
            cursor = cursor.max(r.end);
        }
        if cursor < range.end {
            gaps.push(cursor..range.end);
        }

        // Insert gap spans. We insert them in reverse order so indices stay valid.
        for gap in gaps.into_iter().rev() {
            let insert_pos = self.lines[line]
                .spans
                .iter()
                .position(|(r, _)| r.start >= gap.end)
                .unwrap_or(self.lines[line].spans.len());
            self.lines[line]
                .spans
                .insert(insert_pos, (gap, SpanFormat::default()));
        }
    }

    /// Merge consecutive spans on `line` that have identical `SpanFormat`,
    /// and remove any spans that are now default-format (plain text).
    fn merge_adjacent_spans(&mut self, line: usize) {
        let spans = &mut self.lines[line].spans;

        // Remove default-format spans that don't add information.
        spans.retain(|(_, fmt)| !fmt.is_default());

        if spans.len() < 2 {
            return;
        }

        // Sort spans by start position first.
        spans.sort_by_key(|(r, _)| r.start);

        let mut merged = Vec::with_capacity(spans.len());
        let mut current = spans[0].clone();

        for (range, fmt) in spans.iter().skip(1) {
            if current.0.end == range.start && current.1 == *fmt {
                // Extend the current span.
                current.0.end = range.end;
            } else {
                merged.push(current);
                current = (range.clone(), fmt.clone());
            }
        }
        merged.push(current);

        *spans = merged;
    }
}

impl Default for RichDocument {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_has_one_line() {
        let doc = RichDocument::new();
        assert_eq!(doc.line_count(), 1);
        assert_eq!(*doc.line_format(0), LineFormat::default());
        assert!(doc.spans(0).is_empty());
    }

    #[test]
    fn format_at_empty_line() {
        let doc = RichDocument::new();
        let fmt = doc.format_at(0, 0);
        assert!(fmt.is_default());
    }

    #[test]
    fn toggle_bold_on_range() {
        let mut doc = RichDocument::with_lines(1);
        doc.toggle_bold(0, 0..5);
        let spans = doc.spans(0);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, 0..5);
        assert!(spans[0].1.bold);
    }

    #[test]
    fn toggle_bold_off_range() {
        let mut doc = RichDocument::with_lines(1);
        doc.toggle_bold(0, 0..5);
        assert!(doc.spans(0)[0].1.bold);

        // Toggle again -- should remove bold.
        doc.toggle_bold(0, 0..5);
        // All spans should be removed (back to default).
        assert!(doc.spans(0).is_empty());
    }

    #[test]
    fn toggle_bold_partial() {
        let mut doc = RichDocument::with_lines(1);
        // Make 0..10 bold.
        doc.toggle_bold(0, 0..10);
        assert_eq!(doc.spans(0).len(), 1);
        assert_eq!(doc.spans(0)[0].0, 0..10);

        // Toggle bold on 3..7 -- since 3..7 is already bold, it should remove
        // bold from that sub-range, leaving 0..3 bold and 7..10 bold.
        doc.toggle_bold(0, 3..7);
        let spans = doc.spans(0);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].0, 0..3);
        assert!(spans[0].1.bold);
        assert_eq!(spans[1].0, 7..10);
        assert!(spans[1].1.bold);
    }

    #[test]
    fn insert_shifts_spans() {
        let mut doc = RichDocument::with_lines(1);
        // Create a bold span at 5..10.
        doc.toggle_bold(0, 5..10);

        // Insert 3 chars at position 2 (before the span).
        doc.insert_at(0, 2, 3);

        let spans = doc.spans(0);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, 8..13); // shifted right by 3
        assert!(spans[0].1.bold);
    }

    #[test]
    fn insert_extends_containing_span() {
        let mut doc = RichDocument::with_lines(1);
        // Create a bold span at 0..10.
        doc.toggle_bold(0, 0..10);

        // Insert 5 chars at position 5 (inside the span).
        doc.insert_at(0, 5, 5);

        let spans = doc.spans(0);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, 0..15); // extended by 5
        assert!(spans[0].1.bold);
    }

    #[test]
    fn delete_shrinks_spans() {
        let mut doc = RichDocument::with_lines(1);
        // Create a bold span at 5..15.
        doc.toggle_bold(0, 5..15);

        // Delete chars 3..8 (overlaps start of span).
        doc.delete_range(0, 3, 8);

        let spans = doc.spans(0);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, 3..10); // 15 - 5 deleted = 10, starts at 3
        assert!(spans[0].1.bold);
    }

    #[test]
    fn split_line_divides_spans() {
        let mut doc = RichDocument::with_lines(1);
        // Create a bold span at 0..10.
        doc.toggle_bold(0, 0..10);

        // Split at column 4.
        doc.split_line(0, 4);

        assert_eq!(doc.line_count(), 2);

        let spans0 = doc.spans(0);
        assert_eq!(spans0.len(), 1);
        assert_eq!(spans0[0].0, 0..4);
        assert!(spans0[0].1.bold);

        let spans1 = doc.spans(1);
        assert_eq!(spans1.len(), 1);
        assert_eq!(spans1[0].0, 0..6);
        assert!(spans1[0].1.bold);
    }

    #[test]
    fn merge_lines_combines_spans() {
        let mut doc = RichDocument::with_lines(2);
        // Line 0: bold 0..5
        doc.toggle_bold(0, 0..5);
        // Line 1: italic 0..3
        doc.toggle_italic(1, 0..3);

        doc.merge_lines(0);
        assert_eq!(doc.line_count(), 1);

        let spans = doc.spans(0);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].0, 0..5);
        assert!(spans[0].1.bold);
        assert_eq!(spans[1].0, 5..8); // offset by 5
        assert!(spans[1].1.italic);
    }

    #[test]
    fn adjacent_spans_merge() {
        let mut doc = RichDocument::with_lines(1);
        // Create bold on two adjacent ranges.
        doc.toggle_bold(0, 0..5);
        doc.toggle_bold(0, 5..10);

        let spans = doc.spans(0);
        // Should be merged into a single bold span.
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, 0..10);
        assert!(spans[0].1.bold);
    }

    #[test]
    fn ensure_lines_grows() {
        let mut doc = RichDocument::new();
        assert_eq!(doc.line_count(), 1);

        doc.ensure_lines(5);
        assert_eq!(doc.line_count(), 5);

        // All new lines have default formatting.
        for i in 0..5 {
            assert_eq!(*doc.line_format(i), LineFormat::default());
        }
    }

    #[test]
    fn ensure_lines_shrinks() {
        let mut doc = RichDocument::with_lines(10);
        assert_eq!(doc.line_count(), 10);

        doc.ensure_lines(3);
        assert_eq!(doc.line_count(), 3);
    }

    #[test]
    fn set_format_property_custom() {
        let mut doc = RichDocument::with_lines(1);
        doc.set_format_property(0, 2..8, |f| {
            f.size = Some(24.0);
        });

        let spans = doc.spans(0);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, 2..8);
        assert_eq!(spans[0].1.size, Some(24.0));
    }

    #[test]
    fn toggle_italic_and_bold_overlap() {
        let mut doc = RichDocument::with_lines(1);
        // Make 0..10 bold.
        doc.toggle_bold(0, 0..10);
        // Make 5..15 italic.
        doc.toggle_italic(0, 5..15);

        let spans = doc.spans(0);
        // Expected: 0..5 bold-only, 5..10 bold+italic, 10..15 italic-only
        assert_eq!(spans.len(), 3);

        assert_eq!(spans[0].0, 0..5);
        assert!(spans[0].1.bold);
        assert!(!spans[0].1.italic);

        assert_eq!(spans[1].0, 5..10);
        assert!(spans[1].1.bold);
        assert!(spans[1].1.italic);

        assert_eq!(spans[2].0, 10..15);
        assert!(!spans[2].1.bold);
        assert!(spans[2].1.italic);
    }
}
