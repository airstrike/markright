use std::sync::Arc;

use serde::Deserialize;

use crate::error::Error;
use crate::family::{Axis, Category, Family, Variants};

/// The Google Fonts catalog — a list of families sorted by popularity.
#[derive(Debug, Clone)]
pub struct Catalog {
    families: Arc<Vec<Family>>,
}

impl Catalog {
    /// All families, sorted by popularity (most popular first).
    pub fn families(&self) -> &[Family] {
        &self.families
    }

    /// Look up a family by name.
    pub fn get(&self, name: &str) -> Option<&Family> {
        self.families.iter().find(|f| f.name == name)
    }

    /// Names of all families, ordered by popularity.
    pub fn family_names(&self) -> Vec<String> {
        self.families.iter().map(|f| f.name.clone()).collect()
    }

    /// Names of the `n` most popular families.
    pub fn top(&self, n: usize) -> Vec<String> {
        self.families
            .iter()
            .take(n)
            .map(|f| f.name.clone())
            .collect()
    }

    /// Whether a family is a variable font.
    pub fn is_variable(&self, name: &str) -> Option<bool> {
        self.get(name)
            .map(|f| matches!(f.variants, Variants::Variable { .. }))
    }

    /// Number of families in the catalog.
    pub fn len(&self) -> usize {
        self.families.len()
    }

    /// Whether the catalog is empty.
    pub fn is_empty(&self) -> bool {
        self.families.is_empty()
    }
}

// --- serde types for the metadata JSON ---

#[derive(Deserialize)]
struct MetadataResponse {
    #[serde(rename = "familyMetadataList")]
    family_metadata_list: Vec<FamilyMetadata>,
}

#[derive(Deserialize)]
struct FamilyMetadata {
    family: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    popularity: u32,
    #[serde(default)]
    fonts: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    axes: Vec<AxisMetadata>,
}

#[derive(Deserialize)]
struct AxisMetadata {
    tag: String,
    #[serde(default)]
    min: f32,
    #[serde(default)]
    max: f32,
    #[serde(alias = "defaultValue", default)]
    default: f32,
}

/// Parse the metadata JSON into a [`Catalog`].
pub(crate) fn parse(json: &str) -> Result<Catalog, Error> {
    let resp: MetadataResponse = serde_json::from_str(json)?;

    let mut families: Vec<Family> = resp
        .family_metadata_list
        .into_iter()
        .map(|m| {
            let variants = if m.axes.is_empty() {
                let mut keys: Vec<String> = m.fonts.keys().cloned().collect();
                keys.sort();
                Variants::Static { keys }
            } else {
                Variants::Variable {
                    axes: m
                        .axes
                        .into_iter()
                        .map(|a| Axis {
                            tag: a.tag,
                            min: a.min,
                            max: a.max,
                            default: a.default,
                        })
                        .collect(),
                }
            };

            Family {
                name: m.family,
                category: Category::from_metadata(&m.category),
                popularity: m.popularity,
                variants,
            }
        })
        .collect();

    families.sort_by_key(|f| f.popularity);

    Ok(Catalog {
        families: Arc::new(families),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"{
        "familyMetadataList": [
            {
                "family": "Open Sans",
                "category": "SANS_SERIF",
                "popularity": 2,
                "fonts": {"400": {}, "700": {}},
                "axes": []
            },
            {
                "family": "Roboto",
                "category": "SANS_SERIF",
                "popularity": 1,
                "fonts": {"400": {}, "700": {}, "400i": {}, "700i": {}},
                "axes": [{"tag": "wght", "min": 100.0, "max": 900.0, "defaultValue": 400.0}]
            },
            {
                "family": "Fira Code",
                "category": "MONOSPACE",
                "popularity": 50,
                "fonts": {"400": {}, "700": {}},
                "axes": [{"tag": "wght", "min": 300.0, "max": 700.0, "defaultValue": 400.0}]
            }
        ]
    }"#;

    #[test]
    fn parse_sorts_by_popularity() {
        let catalog = parse(SAMPLE_JSON).unwrap();
        let names = catalog.family_names();
        assert_eq!(names, vec!["Roboto", "Open Sans", "Fira Code"]);
    }

    #[test]
    fn top_returns_n_most_popular() {
        let catalog = parse(SAMPLE_JSON).unwrap();
        assert_eq!(catalog.top(2), vec!["Roboto", "Open Sans"]);
    }

    #[test]
    fn get_finds_family() {
        let catalog = parse(SAMPLE_JSON).unwrap();
        let fira = catalog.get("Fira Code").unwrap();
        assert_eq!(fira.category, Category::Monospace);
        assert!(matches!(fira.variants, Variants::Variable { .. }));
    }

    #[test]
    fn static_vs_variable() {
        let catalog = parse(SAMPLE_JSON).unwrap();
        assert_eq!(catalog.is_variable("Open Sans"), Some(false));
        assert_eq!(catalog.is_variable("Roboto"), Some(true));
    }

    #[test]
    fn static_keys_sorted() {
        let catalog = parse(SAMPLE_JSON).unwrap();
        let open = catalog.get("Open Sans").unwrap();
        match &open.variants {
            Variants::Static { keys } => assert_eq!(keys, &["400", "700"]),
            _ => panic!("expected static"),
        }
    }
}
