use crate::StyledLine;

/// Converts between an external text format and the editor's internal
/// buffer model ([`StyledLine`]).
///
/// Implementations handle the (possibly lossy) conversion between a
/// text-based format and the styled-line model the widget operates on.
pub trait Format {
    /// The error type for parsing failures.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Parse text in this format into styled lines.
    fn parse(input: &str) -> Result<Vec<StyledLine>, Self::Error>;

    /// Serialize styled lines into this format.
    ///
    /// Serialization is infallible: if the format can't represent a
    /// particular style, the implementation silently drops or
    /// approximates it.
    fn serialize(lines: &[StyledLine]) -> String;
}

/// Trivial format: plain text with no styling.
pub struct PlainText;

impl Format for PlainText {
    type Error = std::convert::Infallible;

    fn parse(input: &str) -> Result<Vec<StyledLine>, Self::Error> {
        Ok(input
            .lines()
            .map(|line| StyledLine {
                text: line.to_string(),
                runs: vec![],
                paragraph_style: Default::default(),
            })
            .collect())
    }

    fn serialize(lines: &[StyledLine]) -> String {
        lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
