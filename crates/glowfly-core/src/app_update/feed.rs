//! Where releases are read from: the hosting service, or a local folder named by
//! GLOWFLY_UPDATE_FEED for testing installers and updates without the network.

use super::{Asset, Release, release};
use crate::native_path::shown;
use std::ffi::OsStr;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// The environment variable that replaces the release feed with a local folder.
pub const FEED_ENV: &str = "GLOWFLY_UPDATE_FEED";
/// Its name from before the app was renamed (it was called Trellis), read when [`FEED_ENV`] is
/// unset or empty.
pub const OLD_FEED_ENV: &str = "TRELLIS_UPDATE_FEED";
/// The release, in the hosting service's JSON shape, inside a feed folder.
pub const FEED_RELEASE: &str = "release.json";
/// The most of `release.json` read.
const MAX_FEED_JSON: u64 = 1024 * 1024;

/// The feed variable's value: [`FEED_ENV`], else the old [`OLD_FEED_ENV`]; an empty value counts
/// as unset.
pub(crate) fn feed_var(env: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<std::ffi::OsString> {
    [FEED_ENV, OLD_FEED_ENV].into_iter().find_map(|k| env(k).filter(|v| !v.is_empty()))
}

/// Where releases come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Feed {
    /// The hosting service's newest release of this repository (its URL), over https.
    Hosted(String),
    /// A folder holding `release.json` next to the files it lists.
    Folder(PathBuf),
}

impl Feed {
    /// The feed `value` (GLOWFLY_UPDATE_FEED's value) names: the hosting service of
    /// `repository` when it is unset or empty, else an absolute folder. A relative value is an
    /// error, never resolved against the working folder.
    pub fn from_value(value: Option<&OsStr>, repository: &str) -> Result<Feed, String> {
        let Some(value) = value.filter(|v| !v.is_empty()) else { return Ok(Feed::Hosted(repository.to_owned())) };
        let dir = PathBuf::from(value);
        if dir.is_absolute() {
            return Ok(Feed::Folder(dir));
        }
        Err(format!("{FEED_ENV} must name an absolute folder (got \"{}\")", crate::text::printable(&value.to_string_lossy())))
    }

    /// The feed the environment names, read on every call.
    pub fn from_env() -> Result<Feed, String> {
        Feed::from_value(feed_var(|k| std::env::var_os(k)).as_deref(), super::REPOSITORY)
    }

    /// `hosted <repository>` or `folder <path>`, for logs and `--check-update`.
    pub fn describe(&self) -> String {
        match self {
            Feed::Hosted(repo) => format!("hosted {repo}"),
            Feed::Folder(dir) => format!("folder {}", shown(dir)),
        }
    }

    /// The newest release; None when there is none yet (no `release.json` in a folder).
    pub fn latest(&self) -> Result<Option<Release>, String> {
        match self {
            Feed::Hosted(repo) => crate::forge::latest_release(repo).map_err(|e| e.to_string()),
            Feed::Folder(dir) => {
                if !dir.is_dir() {
                    return Err(format!("The update feed folder {} doesn't exist", shown(dir)));
                }
                let path = dir.join(FEED_RELEASE);
                let bytes = match read_capped_file(&path, MAX_FEED_JSON) {
                    Ok(bytes) => bytes,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
                    Err(e) => return Err(format!("Couldn't read {}: {e}", shown(&path))),
                };
                let size = |name: &str| std::fs::metadata(dir.join(name)).ok().filter(|m| m.is_file()).map(|m| m.len());
                release::parse_feed_release(&bytes, size).map(Some).map_err(|e| format!("Couldn't read {}: {e}", shown(&path)))
            }
        }
    }

    /// The bytes of `asset`, at most `limit` of them.
    pub(super) fn fetch(&self, asset: &Asset, limit: u64) -> Result<Vec<u8>, String> {
        match self {
            Feed::Hosted(_) => crate::forge::fetch_public(&asset.url, limit).map_err(|e| e.to_string()),
            Feed::Folder(dir) => {
                if !release::asset_name_ok(&asset.name) {
                    return Err(format!("{} isn't a file name Glowfly reads", crate::text::printable(&asset.name)));
                }
                read_capped_file(&dir.join(&asset.name), limit).map_err(|e| e.to_string())
            }
        }
    }
}

/// The contents of `path`, refused (as `InvalidData`) when longer than `limit`.
fn read_capped_file(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        let name = path.file_name().map_or_else(|| shown(path), |n| n.to_string_lossy().into_owned());
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{name} is larger than expected")));
    }
    Ok(bytes)
}
