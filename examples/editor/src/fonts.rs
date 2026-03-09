//! Font loading for the editor example.
//!
//! Delegates to `markright_gfonts` for Google Fonts catalog browsing and
//! on-demand downloading with disk caching.

pub use markright_gfonts as gfonts;

use iced::Task;

/// Messages emitted by the font subsystem.
#[derive(Debug, Clone)]
pub enum Message {
    /// The catalog finished loading (or failed).
    CatalogLoaded(Result<gfonts::Catalog, gfonts::Error>),
    /// A font variant finished loading (or failed).
    Loaded(Result<(), gfonts::Error>),
}

/// Fetch the catalog and load the default font in parallel.
pub fn init() -> Task<Message> {
    Task::batch([
        gfonts::catalog(gfonts::DEFAULT_CATALOG_MAX_AGE).map(Message::CatalogLoaded),
        gfonts::load("IBM Plex Sans".into()).map(Message::Loaded),
    ])
}

/// Load a font family by name.
pub fn load(name: String) -> Task<Message> {
    gfonts::load(name).map(Message::Loaded)
}
