//! Font loading for the editor example.
//!
//! Today this is a handful of hardcoded URLs fetched at startup. The plan is
//! to replace it with lazy-loading from Google Fonts:
//!
//! 1. **Font catalog** — fetch the Google Fonts metadata JSON once, cache it
//!    on disk (`~/.cache/markright/fonts/`). Expose a searchable list of
//!    families the user can pick from in a font picker.
//!
//! 2. **On-demand download** — when the user selects a family, download only
//!    the weights/styles they need (regular, bold, italic, bold-italic).
//!    Cache the TTF/OTF files locally so subsequent launches are instant.
//!
//! 3. **Async integration** — downloads happen as `Task`s so the UI stays
//!    responsive. A placeholder font (system default) is used until the
//!    requested family arrives.
//!
//! 4. **Font registry** — a small struct that tracks which families are
//!    loaded, maps family names to `iced::Font` values, and emits a message
//!    when a new font is ready so the editor can re-layout.

use iced::Task;

/// Messages emitted by the font subsystem.
#[derive(Debug, Clone)]
pub enum Message {
    /// A font finished loading (or failed).
    Loaded(Result<(), iced::font::Error>),
}

/// URLs for the default font family (IBM Plex Sans) plus a monospace variant.
const PLEX_SANS_REGULAR: &str = "https://raw.githubusercontent.com/IBM/plex/master/packages/plex-sans/fonts/complete/ttf/IBMPlexSans-Regular.ttf";
const PLEX_SANS_BOLD: &str = "https://raw.githubusercontent.com/IBM/plex/master/packages/plex-sans/fonts/complete/ttf/IBMPlexSans-Bold.ttf";
const PLEX_SANS_ITALIC: &str = "https://raw.githubusercontent.com/IBM/plex/master/packages/plex-sans/fonts/complete/ttf/IBMPlexSans-Italic.ttf";
const PLEX_SANS_BOLD_ITALIC: &str = "https://raw.githubusercontent.com/IBM/plex/master/packages/plex-sans/fonts/complete/ttf/IBMPlexSans-BoldItalic.ttf";
const PLEX_MONO_REGULAR: &str = "https://raw.githubusercontent.com/IBM/plex/master/packages/plex-mono/fonts/complete/ttf/IBMPlexMono-Regular.ttf";

const ALL: &[&str] = &[
    PLEX_SANS_REGULAR,
    PLEX_SANS_BOLD,
    PLEX_SANS_ITALIC,
    PLEX_SANS_BOLD_ITALIC,
    PLEX_MONO_REGULAR,
];

/// Returns a batch task that fetches and loads all default fonts.
pub fn load_defaults() -> Task<Message> {
    Task::batch(ALL.iter().map(|url| {
        Task::future(fetch(url.to_string()))
            .then(iced::font::load)
            .map(Message::Loaded)
    }))
}

/// Fetch font bytes from a URL.
async fn fetch(url: String) -> Vec<u8> {
    reqwest::get(&url)
        .await
        .expect("font fetch failed")
        .bytes()
        .await
        .expect("font bytes failed")
        .to_vec()
}
