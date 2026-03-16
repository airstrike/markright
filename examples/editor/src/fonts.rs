//! Font loading for the editor example.
//!
//! Delegates to `fount` for Google Fonts catalog browsing and on-demand
//! downloading with disk caching. Handles iced font registration.
//! Also enumerates system fonts at startup via `iced::font::list()`.

use iced::Task;
use iced::font::Family;

/// Messages emitted by the font subsystem.
#[derive(Debug, Clone)]
pub enum Message {
    /// The catalog finished loading (or failed).
    CatalogLoaded(Result<fount::Catalog, fount::Error>),
    /// System font families were enumerated.
    SystemFontsLoaded(Vec<Family>),
    /// A font variant finished loading (or failed). Carries the family name.
    Loaded(String, Result<(), fount::Error>),
}

/// Fetch the catalog, enumerate system fonts, and load the default font.
pub fn init() -> Task<Message> {
    Task::batch([
        Task::future(fount::google::catalog(
            fount::google::DEFAULT_CATALOG_MAX_AGE,
        ))
        .map(Message::CatalogLoaded),
        iced::font::list()
            .map(Result::ok)
            .and_then(Task::done)
            .map(Message::SystemFontsLoaded),
        load("IBM Plex Sans".into(), None),
    ])
}

/// Load a font family by name via Google Fonts.
///
/// When a catalog is available, requests only the variants the font actually
/// supports. Without a catalog (e.g. the initial default-font load at
/// startup), falls back to `fount::google::load()` which requests common
/// variants.
pub fn load(name: String, catalog: Option<&fount::Catalog>) -> Task<Message> {
    let variants = catalog.and_then(|c| c.get(&name)).map(variants_for_family);

    let n = name.clone();
    Task::future(async move {
        match variants {
            Some(v) => fount::google::load_variants(&name, &v).await,
            None => fount::google::load(&name).await,
        }
    })
    .then(move |result: Result<Vec<Vec<u8>>, fount::Error>| {
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
    })
}

/// Determine which variant keys to request for a catalog family.
fn variants_for_family(family: &fount::google::Family) -> Vec<String> {
    match &family.variants {
        fount::google::Variants::Static { keys } => keys.clone(),
        fount::google::Variants::Variable { axes } => {
            let has_ital = axes.iter().any(|a| a.tag == "ital");
            if has_ital {
                vec!["400".into(), "700".into(), "400i".into(), "700i".into()]
            } else {
                vec!["400".into(), "700".into()]
            }
        }
    }
}

fn into_fount_error(e: iced::font::Error) -> fount::Error {
    fount::Error::Io(format!("{e:?}"))
}
