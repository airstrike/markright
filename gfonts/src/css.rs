/// A parsed `@font-face` entry from a Google Fonts CSS2 response.
#[derive(Debug, Clone)]
pub(crate) struct FontFace {
    pub weight: u16,
    pub italic: bool,
    pub url: String,
}

impl FontFace {
    /// Variant key matching Google Fonts conventions (e.g. `"400"`, `"700i"`).
    pub fn variant_key(&self) -> String {
        if self.italic {
            format!("{}i", self.weight)
        } else {
            self.weight.to_string()
        }
    }
}

/// Parse a Google Fonts CSS2 response into font-face entries.
pub(crate) fn parse(css: &str) -> Vec<FontFace> {
    let mut faces = Vec::new();

    for chunk in css.split("@font-face") {
        let Some(brace) = chunk.find('{') else {
            continue;
        };
        let body = &chunk[brace + 1..];

        let weight = extract_weight(body).unwrap_or(400);
        let italic = extract_style(body).is_some_and(|s| s == "italic");
        let Some(url) = extract_url(body) else {
            continue;
        };

        faces.push(FontFace {
            weight,
            italic,
            url,
        });
    }

    faces
}

/// Build the CSS2 API URL for the given family and variant keys.
pub(crate) fn build_url(family: &str, variants: &[String]) -> String {
    let encoded = family.replace(' ', "+");
    let has_italic = variants.iter().any(|v| v.ends_with('i'));

    let mut tuples: Vec<(u16, u16)> = variants
        .iter()
        .map(|v| {
            let (weight_str, ital) = if let Some(w) = v.strip_suffix('i') {
                (w, 1u16)
            } else {
                (v.as_str(), 0u16)
            };
            let weight = weight_str.parse().unwrap_or(400);
            (ital, weight)
        })
        .collect();
    tuples.sort();
    tuples.dedup();

    if has_italic {
        let tuples_str = tuples
            .iter()
            .map(|(ital, wght)| format!("{ital},{wght}"))
            .collect::<Vec<_>>()
            .join(";");
        format!("https://fonts.googleapis.com/css2?family={encoded}:ital,wght@{tuples_str}")
    } else {
        let weights_str = tuples
            .iter()
            .map(|(_, wght)| wght.to_string())
            .collect::<Vec<_>>()
            .join(";");
        format!("https://fonts.googleapis.com/css2?family={encoded}:wght@{weights_str}")
    }
}

fn extract_weight(block: &str) -> Option<u16> {
    let marker = "font-weight:";
    let start = block.find(marker)? + marker.len();
    let rest = block[start..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn extract_style(block: &str) -> Option<&str> {
    let marker = "font-style:";
    let start = block.find(marker)? + marker.len();
    let rest = block[start..].trim_start();
    let end = rest
        .find(|c: char| !c.is_alphanumeric())
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

fn extract_url(block: &str) -> Option<String> {
    let marker = "url(";
    let start = block.find(marker)? + marker.len();
    let rest = &block[start..];
    let end = rest.find(')')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CSS: &str = r#"
/* latin */
@font-face {
  font-family: 'Inter';
  font-style: normal;
  font-weight: 400;
  src: url(https://fonts.gstatic.com/s/inter/v18/regular.ttf) format('truetype');
}
/* latin */
@font-face {
  font-family: 'Inter';
  font-style: italic;
  font-weight: 400;
  src: url(https://fonts.gstatic.com/s/inter/v18/italic.ttf) format('truetype');
}
/* latin */
@font-face {
  font-family: 'Inter';
  font-style: normal;
  font-weight: 700;
  src: url(https://fonts.gstatic.com/s/inter/v18/bold.ttf) format('truetype');
}
"#;

    #[test]
    fn parse_extracts_all_faces() {
        let faces = parse(SAMPLE_CSS);
        assert_eq!(faces.len(), 3);
    }

    #[test]
    fn parse_extracts_weight_and_style() {
        let faces = parse(SAMPLE_CSS);
        assert_eq!(faces[0].weight, 400);
        assert!(!faces[0].italic);
        assert_eq!(faces[1].weight, 400);
        assert!(faces[1].italic);
        assert_eq!(faces[2].weight, 700);
        assert!(!faces[2].italic);
    }

    #[test]
    fn parse_extracts_urls() {
        let faces = parse(SAMPLE_CSS);
        assert!(faces[0].url.contains("regular.ttf"));
        assert!(faces[1].url.contains("italic.ttf"));
        assert!(faces[2].url.contains("bold.ttf"));
    }

    #[test]
    fn variant_key_format() {
        let normal = FontFace {
            weight: 400,
            italic: false,
            url: String::new(),
        };
        let italic = FontFace {
            weight: 700,
            italic: true,
            url: String::new(),
        };
        assert_eq!(normal.variant_key(), "400");
        assert_eq!(italic.variant_key(), "700i");
    }

    #[test]
    fn build_url_weights_only() {
        let url = build_url("Fira Code", &["400".into(), "700".into()]);
        assert_eq!(
            url,
            "https://fonts.googleapis.com/css2?family=Fira+Code:wght@400;700"
        );
    }

    #[test]
    fn build_url_with_italics() {
        let url = build_url(
            "Inter",
            &["400".into(), "700".into(), "400i".into(), "700i".into()],
        );
        assert_eq!(
            url,
            "https://fonts.googleapis.com/css2?family=Inter:ital,wght@0,400;0,700;1,400;1,700"
        );
    }

    #[test]
    fn build_url_encodes_spaces() {
        let url = build_url("IBM Plex Sans", &["400".into()]);
        assert!(url.contains("IBM+Plex+Sans"));
    }
}
