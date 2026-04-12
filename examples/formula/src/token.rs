use std::ops::Range;

use crate::eval;

pub type FormulaId = usize;

static NEXT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

fn next_id() -> FormulaId {
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug, Clone)]
pub enum Token {
    Text(String),
    Formula { id: FormulaId, expr: String },
}

impl Token {
    pub fn source_len(&self) -> usize {
        match self {
            Token::Text(s) => s.len(),
            Token::Formula { expr, .. } => expr.len() + 3, // {= + expr + }
        }
    }

    pub fn display_value(&self) -> String {
        match self {
            Token::Text(s) => s.clone(),
            Token::Formula { expr, .. } => eval::eval_display(expr),
        }
    }
}

pub fn parse(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let bytes = source.as_bytes();
    let mut last = 0;
    let mut i = 0;

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'=' {
            if i > last {
                tokens.push(Token::Text(source[last..i].to_string()));
                last = i;
            }
            if let Some(close) = source[i + 2..].find('}') {
                let expr = &source[i + 2..i + 2 + close];
                tokens.push(Token::Formula {
                    id: next_id(),
                    expr: expr.to_string(),
                });
                last = i + 2 + close + 1;
                i = last;
            } else {
                // Unclosed — treat as text
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    if last < source.len() {
        tokens.push(Token::Text(source[last..].to_string()));
    }
    if tokens.is_empty() {
        tokens.push(Token::Text(String::new()));
    }
    tokens
}

pub fn serialize(tokens: &[Token]) -> String {
    tokens
        .iter()
        .map(|t| match t {
            Token::Text(s) => s.clone(),
            Token::Formula { expr, .. } => format!("{{={expr}}}"),
        })
        .collect()
}

#[cfg(test)]
pub fn display_text(tokens: &[Token]) -> String {
    tokens.iter().map(Token::display_value).collect()
}

// ── token map (display ↔ source position mapping) ──────────────────────

#[derive(Debug, Clone)]
pub struct TokenRegion {
    pub display_range: Range<usize>,
    pub source_range: Range<usize>,
    pub token_index: usize,
    pub is_formula: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TokenMap {
    pub regions: Vec<TokenRegion>,
}

impl TokenMap {
    pub fn build(tokens: &[Token]) -> Self {
        let mut regions = Vec::new();
        let mut dcol = 0;
        let mut scol = 0;

        for (i, token) in tokens.iter().enumerate() {
            let dlen = token.display_value().len();
            let slen = token.source_len();

            regions.push(TokenRegion {
                display_range: dcol..dcol + dlen,
                source_range: scol..scol + slen,
                token_index: i,
                is_formula: matches!(token, Token::Formula { .. }),
            });

            dcol += dlen;
            scol += slen;
        }
        Self { regions }
    }

    /// Find the formula whose display range the cursor is adjacent to or inside.
    pub fn adjacent_formula(&self, col: usize, tokens: &[Token]) -> Option<FormulaId> {
        for r in &self.regions {
            if !r.is_formula {
                continue;
            }
            if col >= r.display_range.start
                && col <= r.display_range.end
                && let Token::Formula { id, .. } = &tokens[r.token_index]
            {
                return Some(*id);
            }
        }
        None
    }

    /// Map a display column to a source byte offset.
    pub fn display_to_source(&self, col: usize) -> usize {
        for r in &self.regions {
            if col <= r.display_range.end {
                if r.is_formula {
                    // Within or at edge of formula: map to corresponding edge
                    if col <= r.display_range.start {
                        return r.source_range.start;
                    }
                    return r.source_range.end;
                }
                // Text token: linear map
                let local = col.saturating_sub(r.display_range.start);
                return r.source_range.start + local;
            }
        }
        // Past all tokens
        self.regions.last().map(|r| r.source_range.end).unwrap_or(0)
    }

    /// Map a source byte offset to a display column.
    pub fn source_to_display(&self, offset: usize) -> usize {
        for r in &self.regions {
            if offset <= r.source_range.end {
                if r.is_formula {
                    if offset <= r.source_range.start {
                        return r.display_range.start;
                    }
                    return r.display_range.end;
                }
                let local = offset.saturating_sub(r.source_range.start);
                return r.display_range.start + local;
            }
        }
        self.regions
            .last()
            .map(|r| r.display_range.end)
            .unwrap_or(0)
    }

    /// Check whether the given source byte offset is inside a formula.
    /// Returns the source range of that formula if so.
    pub fn source_offset_in_formula(&self, offset: usize) -> Option<Range<usize>> {
        for r in &self.regions {
            if r.is_formula && offset >= r.source_range.start && offset < r.source_range.end {
                return Some(r.source_range.clone());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple() {
        let tokens = parse("Tim had {=1+3} apples");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(&tokens[0], Token::Text(s) if s == "Tim had "));
        assert!(matches!(&tokens[1], Token::Formula { expr, .. } if expr == "1+3"));
        assert!(matches!(&tokens[2], Token::Text(s) if s == " apples"));
    }

    #[test]
    fn parse_multiple() {
        let tokens = parse("a{=1}b{=2}c");
        assert_eq!(tokens.len(), 5);
    }

    #[test]
    fn parse_no_formulas() {
        let tokens = parse("just text");
        assert_eq!(tokens.len(), 1);
    }

    #[test]
    fn parse_empty() {
        let tokens = parse("");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Text(s) if s.is_empty()));
    }

    #[test]
    fn parse_unclosed() {
        let tokens = parse("hello {=1+2 world");
        // Unclosed: "{=" starts but no "}", so "{" is text and "=1+2 world" is text
        // Actually our parser treats the "{=" as part of continuing text scan
        assert_eq!(tokens.len(), 2);
        assert!(matches!(&tokens[0], Token::Text(s) if s == "hello "));
        assert!(matches!(&tokens[1], Token::Text(s) if s == "{=1+2 world"));
    }

    #[test]
    fn serialize_roundtrip() {
        let source = "Tim had {=1+3} apples";
        let tokens = parse(source);
        assert_eq!(serialize(&tokens), source);
    }

    #[test]
    fn display_evaluates() {
        let tokens = parse("Tim had {=1+3} apples");
        assert_eq!(display_text(&tokens), "Tim had 4 apples");
    }

    #[test]
    fn map_display_to_source() {
        // source:  "Tim had {=1+3} apples"  (len 21)
        //           0       8     14
        //           {=1+3} is 6 bytes: source range 8..14
        // display: "Tim had 4 apples"  (len 16)
        //           0       8 9
        let tokens = parse("Tim had {=1+3} apples");
        let map = TokenMap::build(&tokens);

        // Text region: identity map
        assert_eq!(map.display_to_source(0), 0);
        assert_eq!(map.display_to_source(5), 5);

        // Left edge of formula
        assert_eq!(map.display_to_source(8), 8);
        // Right edge (past formula display)
        assert_eq!(map.display_to_source(9), 14);

        // Text after formula: offset by source-display delta
        assert_eq!(map.display_to_source(10), 15);
    }

    #[test]
    fn map_source_to_display() {
        let tokens = parse("Tim had {=1+3} apples");
        let map = TokenMap::build(&tokens);

        assert_eq!(map.source_to_display(0), 0);
        assert_eq!(map.source_to_display(8), 8); // formula start → display start
        assert_eq!(map.source_to_display(14), 9); // formula end → display end
        assert_eq!(map.source_to_display(15), 10); // text after formula
    }

    #[test]
    fn adjacency_detection() {
        let tokens = parse("Tim had {=1+3} apples");
        let map = TokenMap::build(&tokens);

        assert!(map.adjacent_formula(7, &tokens).is_none());
        assert!(map.adjacent_formula(8, &tokens).is_some());
        assert!(map.adjacent_formula(9, &tokens).is_some());
        assert!(map.adjacent_formula(10, &tokens).is_none());
    }

    #[test]
    fn source_in_formula() {
        let tokens = parse("Tim had {=1+3} apples");
        let map = TokenMap::build(&tokens);

        assert!(map.source_offset_in_formula(7).is_none());
        assert_eq!(map.source_offset_in_formula(8), Some(8..14));
        assert_eq!(map.source_offset_in_formula(10), Some(8..14));
        assert!(map.source_offset_in_formula(14).is_none()); // end is exclusive
    }
}
