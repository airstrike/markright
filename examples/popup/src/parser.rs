pub enum Segment<'a> {
    Text(&'a str),
    Formula { expr: &'a str, raw: &'a str },
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

        if let Some(pos) = self.remaining.find("{=") {
            if pos > 0 {
                let text = &self.remaining[..pos];
                self.remaining = &self.remaining[pos..];
                return Some(Segment::Text(text));
            }

            if let Some(end) = self.remaining[2..].find('}') {
                let raw = &self.remaining[..end + 3];
                let expr = &self.remaining[2..end + 2];
                self.remaining = &self.remaining[end + 3..];
                return Some(Segment::Formula { expr, raw });
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
