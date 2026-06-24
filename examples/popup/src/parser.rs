pub enum Segment<'a> {
    Text(&'a str),
    Formula { expr: &'a str, raw: &'a str },
    Code { content: &'a str, raw: &'a str },
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

        // Pick whichever delimiter comes first
        let next_special = match (formula_pos, backtick_pos) {
            (Some(f), Some(b)) => Some(f.min(b)),
            (Some(f), None) => Some(f),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };

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

            // Try backtick: `...`
            if self.remaining.starts_with('`') {
                if let Some(close) = self.remaining[1..].find('`') {
                    let raw = &self.remaining[..close + 2];
                    let content = &self.remaining[1..close + 1];
                    self.remaining = &self.remaining[close + 2..];
                    return Some(Segment::Code { content, raw });
                }
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
