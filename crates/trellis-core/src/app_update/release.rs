//! A release as the hosting service's API describes it, or a local feed's `release.json` in the
//! same shape. The answer comes from outside, so links that aren't `https://` and file names that
//! could leave the downloads folder are dropped here, before anything else sees them.

use crate::error::Result;
use serde::Deserialize;

/// A published release.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// The tag, such as `v0.3.0`.
    pub tag: String,
    pub name: String,
    /// The release's web page; empty when the server sent something other than an https link.
    pub html_url: String,
    /// The release notes (Markdown).
    pub body: String,
    pub published_at: Option<String>,
    pub assets: Vec<Asset>,
}

/// A file attached to a release.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    /// Where it downloads from (always https); empty for a folder feed, whose files are read by
    /// name.
    pub url: String,
}

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

#[derive(Deserialize)]
struct RawAsset {
    name: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    browser_download_url: String,
}

/// Whether `name` is safe as a file name in the downloads folder: 1 to 128 characters of
/// `A-Z a-z 0-9 . _ -`, not starting with a dot. No separator, drive or `..` can get through.
pub fn asset_name_ok(name: &str) -> bool {
    (1..=128).contains(&name.len())
        && !name.starts_with('.')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Reads a release from the API's JSON.
pub fn parse_release(json: &[u8]) -> Result<Release> {
    let mut raw: RawRelease = serde_json::from_slice(json)?;
    let https = |u: &str| crate::forge::https_host(u).is_some();
    let assets = std::mem::take(&mut raw.assets)
        .into_iter()
        .filter(|a| https(&a.browser_download_url) && asset_name_ok(&a.name))
        .map(|a| Asset { name: a.name, size: a.size, url: a.browser_download_url })
        .collect();
    Ok(finish(raw, assets))
}

/// Reads a local feed's `release.json` (the API's shape). A listed file is kept when its name
/// is safe and `file_size` finds it (checked in that order, so no path is built from an
/// unchecked name); its size is the one on disk and its url empty.
pub(super) fn parse_feed_release(json: &[u8], file_size: impl Fn(&str) -> Option<u64>) -> Result<Release> {
    let mut raw: RawRelease = serde_json::from_slice(json)?;
    let assets = std::mem::take(&mut raw.assets)
        .into_iter()
        .filter(|a| asset_name_ok(&a.name))
        .filter_map(|a| file_size(&a.name).map(|size| Asset { name: a.name, size, url: String::new() }))
        .collect();
    Ok(finish(raw, assets))
}

/// The release `raw` describes, with `assets`: an empty name falls back to the tag, and a web
/// page that isn't https is dropped.
fn finish(raw: RawRelease, assets: Vec<Asset>) -> Release {
    let html_url = if crate::forge::https_host(&raw.html_url).is_some() { raw.html_url } else { String::new() };
    Release {
        name: raw.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| raw.tag_name.clone()),
        tag: raw.tag_name,
        html_url,
        body: raw.body.unwrap_or_default(),
        published_at: raw.published_at,
        assets,
    }
}
