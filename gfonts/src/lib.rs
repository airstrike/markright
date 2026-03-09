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
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use iced::Task;

/// Default max age for cached catalog metadata (7 days).
pub const DEFAULT_CATALOG_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

static INTERNED: LazyLock<Mutex<HashSet<&'static str>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// Intern a string, returning a `&'static str`.
///
/// Repeated calls with the same content return the same pointer without
/// allocating again. Use this for font family names passed to
/// [`iced::Font::with_name`].
pub fn intern(s: &str) -> &'static str {
    let mut set = INTERNED.lock().expect("intern lock poisoned");
    if let Some(&existing) = set.get(s) {
        return existing;
    }
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    set.insert(leaked);
    leaked
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
    fn intern_returns_same_pointer() {
        let a = intern("TestFont");
        let b = intern("TestFont");
        assert!(std::ptr::eq(a, b));
    }

    #[test]
    fn intern_different_strings() {
        let a = intern("FontA");
        let b = intern("FontB");
        assert_ne!(a, b);
    }
}
