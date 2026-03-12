//! Font loading for the editor example.
//!
//! Delegates to `fount` for Google Fonts catalog browsing and on-demand
//! downloading with disk caching. Handles iced font registration.

use iced::Task;

/// Messages emitted by the font subsystem.
#[derive(Debug, Clone)]
pub enum Message {
    /// The catalog finished loading (or failed).
    CatalogLoaded(Result<fount::Catalog, fount::Error>),
    /// A font variant finished loading (or failed). Carries the family name.
    Loaded(String, Result<(), fount::Error>),
}

/// Fetch the catalog and load the default font in parallel.
pub fn init() -> Task<Message> {
    Task::batch([
        Task::future(fount::google::catalog(
            fount::google::DEFAULT_CATALOG_MAX_AGE,
        ))
        .map(Message::CatalogLoaded),
        load("IBM Plex Sans".into()),
    ])
}

/// Load a font family by name via Google Fonts.
pub fn load(name: String) -> Task<Message> {
    let n = name.clone();
    Task::future(async move { fount::google::load(&name).await }).then(
        move |result: Result<Vec<Vec<u8>>, fount::Error>| {
            let n = n.clone();
            match result {
                Ok(bytes_list) => Task::batch(bytes_list.into_iter().map({
                    let n = n.clone();
                    move |bytes| {
                        let n = n.clone();
                        iced::font::load(bytes)
                            .map(move |r| Message::Loaded(n.clone(), r.map_err(into_fount_error)))
                    }
                })),
                Err(e) => Task::done(Message::Loaded(n, Err(e))),
            }
        },
    )
}

fn into_fount_error(e: iced::font::Error) -> fount::Error {
    fount::Error::Io(format!("{e:?}"))
}
