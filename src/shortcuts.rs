use std::ops::Range;

/// A detected markdown pattern that should be converted to formatting.
#[derive(Debug, Clone, PartialEq)]
pub enum MarkdownAction {
    /// Apply bold to the content range, remove marker ranges.
    Bold {
        /// Range of the text content (between markers) in the line.
        content: Range<usize>,
        /// Byte ranges of the markers to remove (opening and closing).
        markers: Vec<Range<usize>>,
    },
    /// Apply italic.
    Italic {
        content: Range<usize>,
        markers: Vec<Range<usize>>,
    },
    /// Apply bold+italic.
    BoldItalic {
        content: Range<usize>,
        markers: Vec<Range<usize>>,
    },
    /// Set heading level and remove the prefix.
    Heading {
        level: u8,
        /// Range of the heading prefix to remove (e.g., "# " or "### ").
        marker: Range<usize>,
    },
    /// Apply code/mono font.
    Code {
        content: Range<usize>,
        markers: Vec<Range<usize>>,
    },
}

/// Scan a line for completed markdown patterns.
/// Returns all detected actions, ordered by position in the line.
///
/// This checks for heading prefixes first (which consume the whole line
/// context), then scans for inline patterns like bold, italic, and code.
pub fn detect_patterns(line: &str) -> Vec<MarkdownAction> {
    let mut actions = Vec::new();

    // Check for heading prefix: # , ## , ### , etc.
    if let Some(action) = detect_heading(line) {
        actions.push(action);
        return actions; // Heading consumes the whole line context
    }

    // Check for inline patterns
    detect_inline_patterns(line, &mut actions);

    actions
}

/// Detect a heading prefix at the start of a line.
///
/// Matches `# `, `## `, ... up to `###### ` (levels 1-6).
/// The `#` characters must be followed by a space.
fn detect_heading(line: &str) -> Option<MarkdownAction> {
    let bytes = line.as_bytes();
    let mut level = 0;
    while level < bytes.len() && level < 6 && bytes[level] == b'#' {
        level += 1;
    }
    if level == 0 {
        return None;
    }
    // Must be followed by a space.
    if level >= bytes.len() || bytes[level] != b' ' {
        return None;
    }
    Some(MarkdownAction::Heading {
        level: level as u8,
        marker: 0..level + 1, // includes the trailing space
    })
}

/// Scan for inline markdown patterns: code spans, bold+italic, bold, italic.
///
/// Priority order: code spans are detected first (backticks cannot contain
/// other formatting), then `***` (bold+italic), then `**` (bold), then `*`
/// (italic).
fn detect_inline_patterns(line: &str, actions: &mut Vec<MarkdownAction>) {
    let bytes = line.as_bytes();
    let len = bytes.len();

    // Track which byte positions are already claimed by a pattern,
    // so overlapping matches are prevented.
    let mut claimed = vec![false; len];

    // Pass 1: Code spans (backtick pairs).
    detect_code_spans(bytes, &mut claimed, actions);

    // Pass 2: Asterisk-based formatting (bold+italic, bold, italic).
    detect_asterisk_patterns(bytes, &mut claimed, actions);

    // Sort actions by content start position.
    actions.sort_by_key(|a| match a {
        MarkdownAction::Bold { content, .. }
        | MarkdownAction::Italic { content, .. }
        | MarkdownAction::BoldItalic { content, .. }
        | MarkdownAction::Code { content, .. } => content.start,
        MarkdownAction::Heading { .. } => 0,
    });
}

/// Detect code spans delimited by single backticks.
fn detect_code_spans(bytes: &[u8], claimed: &mut [bool], actions: &mut Vec<MarkdownAction>) {
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'`' && !claimed[i] {
            // Found opening backtick -- search for closing.
            let open = i;
            i += 1;
            while i < len {
                if bytes[i] == b'`' && !claimed[i] {
                    // Found closing backtick.
                    let close = i;
                    let content_start = open + 1;
                    let content_end = close;
                    if content_start < content_end {
                        // Mark all positions as claimed.
                        for flag in claimed.iter_mut().take(close + 1).skip(open) {
                            *flag = true;
                        }
                        actions.push(MarkdownAction::Code {
                            content: content_start..content_end,
                            markers: vec![open..open + 1, close..close + 1],
                        });
                    }
                    break;
                }
                i += 1;
            }
        }
        i += 1;
    }
}

/// Count consecutive asterisks starting at position `start`.
fn count_asterisks(bytes: &[u8], start: usize) -> usize {
    let mut count = 0;
    while start + count < bytes.len() && bytes[start + count] == b'*' {
        count += 1;
    }
    count
}

/// Detect asterisk-based patterns: `***bold+italic***`, `**bold**`, `*italic*`.
fn detect_asterisk_patterns(bytes: &[u8], claimed: &mut [bool], actions: &mut Vec<MarkdownAction>) {
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] != b'*' || claimed[i] {
            i += 1;
            continue;
        }

        let open_start = i;
        let open_count = count_asterisks(bytes, open_start);

        // Try to match the largest marker first (3, then 2, then 1).
        let mut matched = false;
        for marker_len in (1..=open_count.min(3)).rev() {
            let content_start = open_start + marker_len;
            if content_start >= len {
                continue;
            }

            // Search for a matching closing marker of the same length.
            if let Some(close_start) =
                find_closing_asterisks(bytes, content_start, marker_len, claimed)
            {
                let content_end = close_start;
                // Content must be non-empty.
                if content_start >= content_end {
                    continue;
                }

                let open_range = open_start..open_start + marker_len;
                let close_range = close_start..close_start + marker_len;

                // Mark positions as claimed.
                for flag in claimed
                    .iter_mut()
                    .take(open_range.end)
                    .skip(open_range.start)
                {
                    *flag = true;
                }
                for flag in claimed.iter_mut().take(content_end).skip(content_start) {
                    *flag = true;
                }
                for flag in claimed
                    .iter_mut()
                    .take(close_range.end)
                    .skip(close_range.start)
                {
                    *flag = true;
                }

                let action = match marker_len {
                    3 => MarkdownAction::BoldItalic {
                        content: content_start..content_end,
                        markers: vec![open_range, close_range],
                    },
                    2 => MarkdownAction::Bold {
                        content: content_start..content_end,
                        markers: vec![open_range, close_range],
                    },
                    1 => MarkdownAction::Italic {
                        content: content_start..content_end,
                        markers: vec![open_range, close_range],
                    },
                    _ => unreachable!(),
                };

                actions.push(action);
                i = close_start + marker_len;
                matched = true;
                break;
            }
        }

        if !matched {
            i += open_count;
        }
    }
}

/// Search for a closing sequence of exactly `marker_len` asterisks,
/// starting the search from `from`.
///
/// The closing marker must be exactly `marker_len` asterisks -- not
/// embedded within a longer run.
fn find_closing_asterisks(
    bytes: &[u8],
    from: usize,
    marker_len: usize,
    claimed: &[bool],
) -> Option<usize> {
    let len = bytes.len();
    let mut j = from;

    while j < len {
        if bytes[j] == b'*' && !claimed[j] {
            let run_start = j;
            let run_len = count_asterisks(bytes, run_start);

            // Check that none of the positions in this run are claimed.
            let all_unclaimed = (run_start..run_start + run_len).all(|p| !claimed[p]);
            if !all_unclaimed {
                j += run_len;
                continue;
            }

            if run_len == marker_len {
                return Some(run_start);
            }

            // If the run is longer than the marker, skip it entirely --
            // we need an exact match to avoid ambiguity.
            j += run_len;
        } else {
            j += 1;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_bold_pattern() {
        let actions = detect_patterns("**bold**");
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            MarkdownAction::Bold {
                content: 2..6,
                markers: vec![0..2, 6..8],
            }
        );
    }

    #[test]
    fn detect_italic_pattern() {
        let actions = detect_patterns("*italic*");
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            MarkdownAction::Italic {
                content: 1..7,
                markers: vec![0..1, 7..8],
            }
        );
    }

    #[test]
    fn detect_bold_italic_pattern() {
        let actions = detect_patterns("***bold italic***");
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            MarkdownAction::BoldItalic {
                content: 3..14,
                markers: vec![0..3, 14..17],
            }
        );
    }

    #[test]
    fn detect_code_pattern() {
        let actions = detect_patterns("`code`");
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            MarkdownAction::Code {
                content: 1..5,
                markers: vec![0..1, 5..6],
            }
        );
    }

    #[test]
    fn detect_heading_1() {
        let actions = detect_patterns("# Hello");
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            MarkdownAction::Heading {
                level: 1,
                marker: 0..2,
            }
        );
    }

    #[test]
    fn detect_heading_3() {
        let actions = detect_patterns("### Hello");
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            MarkdownAction::Heading {
                level: 3,
                marker: 0..4,
            }
        );
    }

    #[test]
    fn no_match_on_unclosed() {
        let actions = detect_patterns("**unclosed");
        assert!(actions.is_empty());
    }

    #[test]
    fn no_heading_without_space() {
        let actions = detect_patterns("#nospace");
        assert!(actions.is_empty());
    }

    #[test]
    fn mixed_patterns() {
        let actions = detect_patterns("**bold** and *italic*");
        assert_eq!(actions.len(), 2);
        assert_eq!(
            actions[0],
            MarkdownAction::Bold {
                content: 2..6,
                markers: vec![0..2, 6..8],
            }
        );
        assert_eq!(
            actions[1],
            MarkdownAction::Italic {
                content: 14..20,
                markers: vec![13..14, 20..21],
            }
        );
    }

    #[test]
    fn code_prevents_inner_bold() {
        let actions = detect_patterns("`**not bold**`");
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            MarkdownAction::Code {
                content: 1..13,
                markers: vec![0..1, 13..14],
            }
        );
    }

    #[test]
    fn empty_content_not_matched() {
        let actions = detect_patterns("****");
        assert!(actions.is_empty());
    }

    #[test]
    fn heading_level_6() {
        let actions = detect_patterns("###### Tiny heading");
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            MarkdownAction::Heading {
                level: 6,
                marker: 0..7,
            }
        );
    }

    #[test]
    fn seven_hashes_not_a_heading() {
        let actions = detect_patterns("####### Not a heading");
        assert!(actions.is_empty());
    }

    #[test]
    fn bold_with_surrounding_text() {
        let actions = detect_patterns("hello **world** there");
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            MarkdownAction::Bold {
                content: 8..13,
                markers: vec![6..8, 13..15],
            }
        );
    }

    #[test]
    fn multiple_code_spans() {
        let actions = detect_patterns("`a` and `b`");
        assert_eq!(actions.len(), 2);
        assert_eq!(
            actions[0],
            MarkdownAction::Code {
                content: 1..2,
                markers: vec![0..1, 2..3],
            }
        );
        assert_eq!(
            actions[1],
            MarkdownAction::Code {
                content: 9..10,
                markers: vec![8..9, 10..11],
            }
        );
    }

    #[test]
    fn single_asterisk_not_italic() {
        // A lone asterisk with no closing should produce nothing.
        let actions = detect_patterns("*");
        assert!(actions.is_empty());
    }

    #[test]
    fn empty_code_span_not_matched() {
        let actions = detect_patterns("``");
        assert!(actions.is_empty());
    }
}
