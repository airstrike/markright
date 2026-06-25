/// Sentinel characters used in place of backticks for matched code
/// delimiters. Using distinct open/close characters prevents an
/// orphaned opener from pairing with another span's opener or closer.
pub const CODE_OPEN: char = '\u{E000}';
pub const CODE_CLOSE: char = '\u{E001}';

pub enum Segment<'a> {
    Text(&'a str),
    Formula { expr: &'a str, raw: &'a str },
    Code { content: String, raw: &'a str },
}

pub struct Segments<'a> {
    remaining: &'a str,
}

impl<'a> Segments<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { remaining: source }
    }
}

impl<'a> Iterator for Segments<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        let formula_pos = self.remaining.find("{=");
        let backtick_pos = self.remaining.find('`');
        let fence_pos = self.remaining.find(CODE_OPEN);

        // Pick whichever delimiter comes first
        let next_special = [formula_pos, backtick_pos, fence_pos]
            .into_iter()
            .flatten()
            .min();

        if let Some(pos) = next_special {
            if pos > 0 {
                let text = &self.remaining[..pos];
                self.remaining = &self.remaining[pos..];
                return Some(Segment::Text(text));
            }

            // Try formula: {=...}
            if self.remaining.starts_with("{=") {
                if let Some(end) = self.remaining[2..].find('}') {
                    let raw = &self.remaining[..end + 3];
                    let expr = &self.remaining[2..end + 2];
                    self.remaining = &self.remaining[end + 3..];
                    return Some(Segment::Formula { expr, raw });
                }
            }

            // Try code fence: backtick or sentinel delimiter.
            // Both ` and CODE_FENCE open a code span; the closing
            // delimiter must match (` pairs with `, fence with fence).
            // Output source_value always uses sentinels so matched
            // pairs can't accidentally re-pair after dissolution.
            let opener = if self.remaining.starts_with('`') {
                Some(('`', '`'))
            } else if self.remaining.starts_with(CODE_OPEN) {
                Some((CODE_OPEN, CODE_CLOSE))
            } else {
                None
            };
            if let Some((open_char, close_char)) = opener {
                let open_len = open_char.len_utf8();
                let inner = &self.remaining[open_len..];
                let mut pos = 0;
                let mut found = false;
                while pos < inner.len() {
                    let ch = &inner[pos..];
                    if ch.starts_with(close_char) {
                        if close_char == '`' && ch.len() > 1 && ch.as_bytes().get(1) == Some(&b'`')
                        {
                            pos += 2;
                        } else {
                            found = true;
                            break;
                        }
                    } else if open_char != close_char && ch.starts_with(open_char) {
                        break;
                    } else {
                        pos += ch.chars().next().map_or(1, |c| c.len_utf8());
                    }
                }
                if found {
                    let close_len = close_char.len_utf8();
                    let raw = &self.remaining[..open_len + pos + close_len];
                    let content = if close_char == '`' {
                        inner[..pos].replace("``", "`")
                    } else {
                        inner[..pos].to_string()
                    };
                    self.remaining = &self.remaining[open_len + pos + close_len..];
                    return Some(Segment::Code { content, raw });
                }

                // Unmatched opener — emit just the delimiter as text
                // and continue scanning on the next call.
                let delim = &self.remaining[..open_len];
                self.remaining = &self.remaining[open_len..];
                return Some(Segment::Text(delim));
            }
        }

        let text = self.remaining;
        self.remaining = "";
        Some(Segment::Text(text))
    }
}

pub fn eval(expr: &str) -> String {
    let expr = expr.trim();
    if let Some((a, b)) = expr.split_once('+') {
        if let (Ok(a), Ok(b)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
            return format!("{}", a + b);
        }
    }
    if let Some((a, b)) = expr.split_once('*') {
        if let (Ok(a), Ok(b)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
            return format!("{}", a * b);
        }
    }
    if let Ok(n) = expr.parse::<f64>() {
        return format!("{n}");
    }
    format!("?{expr}")
}
