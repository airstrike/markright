//! Formula source format: `{=expr}` syntax with inline evaluation.

use iced::advanced::text::rich_editor::span;
use markright_core::{Paragraph, StyleRun, StyledLine};

use crate::FORMULA_COLOR;
use crate::token::{self, Token};

/// Format adapter for the formula source language.
///
/// Source text like `"Tim had {=1+3} apples"` is parsed into styled
/// lines where formula expressions are evaluated and displayed with
/// [`FORMULA_COLOR`].
pub struct FormulaFormat;

impl markright_core::Format for FormulaFormat {
    type Error = std::convert::Infallible;

    fn parse(input: &str) -> Result<Vec<StyledLine>, Self::Error> {
        let tokens = token::parse(input);
        Ok(vec![styled_line_from_tokens(&tokens)])
    }

    fn serialize(lines: &[StyledLine]) -> String {
        // Lossy: returns the display text only (formula expressions are
        // not stored in the styled lines).
        lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Build a [`StyledLine`] from parsed tokens.
///
/// This is also used by the app when it already has tokens and doesn't
/// need to re-parse the source string.
pub fn styled_line_from_tokens(tokens: &[Token]) -> StyledLine {
    let mut text = String::new();
    let mut runs = Vec::new();

    for token in tokens {
        let display = token.display_value();
        let start = text.len();
        text.push_str(&display);
        let end = text.len();

        if matches!(token, Token::Formula { .. }) && start < end {
            runs.push(StyleRun {
                range: start..end,
                style: span::Style {
                    color: Some(FORMULA_COLOR),
                    ..Default::default()
                },
            });
        }
    }

    StyledLine {
        text,
        runs,
        paragraph: Paragraph::default(),
    }
}
