/// Errors that can occur during Google Fonts operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("JSON parsing failed: {0}")]
    Json(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("font loading failed")]
    Font(iced::font::Error),

    #[error("no cache directory available")]
    NoCacheDir,

    #[error("no font URLs found in CSS response for {family}")]
    NoFontUrls { family: String },
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl Error {
    pub(crate) fn font(e: iced::font::Error) -> Self {
        Self::Font(e)
    }
}
