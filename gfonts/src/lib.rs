//! Google Fonts integration — catalog browsing and on-demand font loading.
//!
//! Fonts are downloaded from public Google Fonts endpoints (no API key needed)
//! and cached to disk at `{cache_dir}/gfonts/` (platform-dependent).

pub mod catalog;
pub mod error;
pub mod family;

mod cache;
mod css;
mod fetch;

pub use catalog::Catalog;
pub use error::Error;
pub use family::{Axis, Category, Family, Variants};

use std::collections::HashSet;
use std::time::Duration;

use iced::Task;

/// Default max age for cached catalog metadata (7 days).
pub const DEFAULT_CATALOG_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Font registry. Holds the Google Fonts catalog and resolves family names
/// to [`iced::Font`] values, interning the name strings so each unique
/// name is leaked only once.
#[derive(Debug, Default)]
pub struct Fonts {
    names: HashSet<&'static str>,
    catalog: Option<Catalog>,
}

impl Fonts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get an [`iced::Font`] for the given family name.
    pub fn get(&mut self, name: &str) -> iced::Font {
        iced::Font::with_name(self.intern(name))
    }

    /// Store the catalog once it has been fetched.
    pub fn set_catalog(&mut self, catalog: Catalog) {
        self.catalog = Some(catalog);
    }

    /// The catalog, if loaded.
    pub fn catalog(&self) -> Option<&Catalog> {
        self.catalog.as_ref()
    }

    fn intern(&mut self, s: &str) -> &'static str {
        if let Some(&existing) = self.names.get(s) {
            return existing;
        }
        let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
        self.names.insert(leaked);
        leaked
    }
}

/// Fetch the Google Fonts catalog, using a disk cache with the given max age.
pub fn catalog(max_age: Duration) -> Task<Result<Catalog, Error>> {
    Task::future(async move {
        let raw = cache::load_or_fetch_metadata(max_age).await?;
        catalog::parse(&raw)
    })
}

/// Load standard variants (400, 700, 400i, 700i) of a font family.
///
/// Downloads missing variants, caches them to disk, and registers them with
/// iced's font system. Emits one `Result` per variant file loaded.
pub fn load(family: String) -> Task<Result<(), Error>> {
    load_variants(
        family,
        vec!["400".into(), "700".into(), "400i".into(), "700i".into()],
    )
}

/// Load specific variants of a font family.
///
/// Variant keys follow Google Fonts conventions: `"400"`, `"700"`,
/// `"400i"` (italic), `"700i"`, etc.
pub fn load_variants(family: String, variants: Vec<String>) -> Task<Result<(), Error>> {
    Task::future(async move { cache::load_or_fetch_fonts(&family, &variants).await }).then(
        |result| match result {
            Ok(all_bytes) => Task::batch(
                all_bytes
                    .into_iter()
                    .map(|bytes| iced::font::load(bytes).map(|r| r.map_err(Error::font))),
            ),
            Err(e) => Task::done(Err(e)),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_same_font() {
        let mut fonts = Fonts::new();
        let a = fonts.get("Roboto");
        let b = fonts.get("Roboto");
        assert_eq!(a, b);
    }

    #[test]
    fn get_different_names() {
        let mut fonts = Fonts::new();
        let a = fonts.get("FontA");
        let b = fonts.get("FontB");
        assert_ne!(a, b);
    }
}
