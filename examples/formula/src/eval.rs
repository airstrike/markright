/// Evaluate a simple arithmetic expression.
///
/// Supports `+`, `-`, `*`, `/`, parentheses, and decimal numbers.
pub fn eval(input: &str) -> Result<f64, String> {
    let tokens = lex(input)?;
    let mut p = Parser {
        tokens: &tokens,
        pos: 0,
    };
    let v = p.expr()?;
    if p.pos < p.tokens.len() {
        return Err(format!("unexpected '{}'", p.peek_str()));
    }
    Ok(v)
}

/// Evaluate and format for display — integers show without decimals.
pub fn eval_display(input: &str) -> String {
    match eval(input) {
        Ok(v) if v == v.floor() && v.abs() < 1e15 => format!("{}", v as i64),
        Ok(v) => format!("{v}"),
        Err(_) => "#ERR".into(),
    }
}

#[derive(Debug, Clone)]
enum Tok {
    Num(f64),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

fn lex(input: &str) -> Result<Vec<Tok>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' => {
                chars.next();
            }
            '+' => {
                tokens.push(Tok::Plus);
                chars.next();
            }
            '-' => {
                tokens.push(Tok::Minus);
                chars.next();
            }
            '*' => {
                tokens.push(Tok::Star);
                chars.next();
            }
            '/' => {
                tokens.push(Tok::Slash);
                chars.next();
            }
            '(' => {
                tokens.push(Tok::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Tok::RParen);
                chars.next();
            }
            '0'..='9' | '.' => {
                let mut num = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' {
                        num.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let v: f64 = num.parse().map_err(|_| format!("invalid number: {num}"))?;
                tokens.push(Tok::Num(v));
            }
            _ => return Err(format!("unexpected character: {c}")),
        }
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Tok],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn peek_str(&self) -> &'static str {
        match self.peek() {
            Some(Tok::Num(_)) => "number",
            Some(Tok::Plus) => "+",
            Some(Tok::Minus) => "-",
            Some(Tok::Star) => "*",
            Some(Tok::Slash) => "/",
            Some(Tok::LParen) => "(",
            Some(Tok::RParen) => ")",
            None => "end of input",
        }
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    /// expr → term (('+' | '-') term)*
    fn expr(&mut self) -> Result<f64, String> {
        let mut left = self.term()?;
        loop {
            match self.peek() {
                Some(Tok::Plus) => {
                    self.advance();
                    left += self.term()?;
                }
                Some(Tok::Minus) => {
                    self.advance();
                    left -= self.term()?;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// term → factor (('*' | '/') factor)*
    fn term(&mut self) -> Result<f64, String> {
        let mut left = self.factor()?;
        loop {
            match self.peek() {
                Some(Tok::Star) => {
                    self.advance();
                    left *= self.factor()?;
                }
                Some(Tok::Slash) => {
                    self.advance();
                    let right = self.factor()?;
                    if right == 0.0 {
                        return Err("division by zero".into());
                    }
                    left /= right;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// factor → '-' factor | '(' expr ')' | number
    fn factor(&mut self) -> Result<f64, String> {
        match self.peek() {
            Some(Tok::Minus) => {
                self.advance();
                Ok(-self.factor()?)
            }
            Some(Tok::LParen) => {
                self.advance();
                let v = self.expr()?;
                match self.peek() {
                    Some(Tok::RParen) => {
                        self.advance();
                        Ok(v)
                    }
                    _ => Err("expected ')'".into()),
                }
            }
            Some(Tok::Num(n)) => {
                let v = *n;
                self.advance();
                Ok(v)
            }
            _ => Err("expected number or '('".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_arithmetic() {
        assert_eq!(eval("1+3").unwrap(), 4.0);
        assert_eq!(eval("2*6").unwrap(), 12.0);
        assert_eq!(eval("10-3").unwrap(), 7.0);
        assert_eq!(eval("15/3").unwrap(), 5.0);
    }

    #[test]
    fn precedence() {
        assert_eq!(eval("2+3*4").unwrap(), 14.0);
        assert_eq!(eval("(2+3)*4").unwrap(), 20.0);
    }

    #[test]
    fn negation() {
        assert_eq!(eval("-5").unwrap(), -5.0);
        assert_eq!(eval("3+-2").unwrap(), 1.0);
    }

    #[test]
    fn decimals() {
        assert_eq!(eval("1.5+2.5").unwrap(), 4.0);
    }

    #[test]
    fn nested_parens() {
        assert_eq!(eval("((2+3))").unwrap(), 5.0);
        assert_eq!(eval("(1+(2*3))").unwrap(), 7.0);
    }

    #[test]
    fn display_integer() {
        assert_eq!(eval_display("1+3"), "4");
        assert_eq!(eval_display("2*6"), "12");
    }

    #[test]
    fn display_decimal() {
        assert_eq!(eval_display("1/3"), "0.3333333333333333");
    }

    #[test]
    fn display_error() {
        assert_eq!(eval_display("1+"), "#ERR");
        assert_eq!(eval_display("abc"), "#ERR");
    }
}
