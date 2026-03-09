use crate::error::Error;

const METADATA_URL: &str = "https://fonts.google.com/metadata/fonts";

/// Fetch the full catalog metadata JSON from Google Fonts.
pub(crate) async fn metadata() -> Result<String, Error> {
    let text = reqwest::get(METADATA_URL).await?.text().await?;
    // The endpoint may prefix the JSON with ")]}'\n" as XSS protection.
    let json = text
        .strip_prefix(")]}'")
        .map(|s| s.trim_start())
        .unwrap_or(&text);
    Ok(json.to_owned())
}

/// Fetch the CSS2 stylesheet for a family's variants.
pub(crate) async fn css(family: &str, variants: &[String]) -> Result<String, Error> {
    let url = crate::css::build_url(family, variants);
    tracing::debug!("fetching CSS: {url}");
    let text = reqwest::get(&url).await?.text().await?;
    Ok(text)
}

/// Download raw bytes from a URL (typically a font file on fonts.gstatic.com).
pub(crate) async fn bytes(url: &str) -> Result<Vec<u8>, Error> {
    let data = reqwest::get(url).await?.bytes().await?;
    Ok(data.to_vec())
}
