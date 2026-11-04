//! Where releases are read from: the hosting service, or a local folder named by
//! GLOWFLY_UPDATE_FEED for testing installers and updates without the network.

use super::{Asset, Release, UpdateError, release};
use crate::forge::ServiceError;
use crate::native_path::shown;
use std::ffi::OsStr;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// The environment variable that replaces the release feed with a local folder.
pub const FEED_ENV: &str = "GLOWFLY_UPDATE_FEED";
/// The release, in the hosting service's JSON shape, inside a feed folder.
pub const FEED_RELEASE: &str = "release.json";
/// The most of `release.json` read.
const MAX_FEED_JSON: u64 = 1024 * 1024;

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
    pub fn from_value(value: Option<&OsStr>, repository: &str) -> Result<Feed, UpdateError> {
        let Some(value) = value.filter(|v| !v.is_empty()) else { return Ok(Feed::Hosted(repository.to_owned())) };
        let dir = PathBuf::from(value);
        if dir.is_absolute() {
            return Ok(Feed::Folder(dir));
        }
        Err(UpdateError::FeedNotAbsolute { value: crate::text::printable(&value.to_string_lossy()) })
    }

    /// The feed the environment names, read on every call.
    pub fn from_env() -> Result<Feed, UpdateError> {
        Feed::from_value(std::env::var_os(FEED_ENV).as_deref(), super::REPOSITORY)
    }

    /// `hosted <repository>` or `folder <path>`, for logs and `--check-update`.
    pub fn describe(&self) -> String {
        match self {
            Feed::Hosted(repo) => format!("hosted {repo}"),
            Feed::Folder(dir) => format!("folder {}", shown(dir)),
        }
    }

    /// The newest release; None when there is none yet (no `release.json` in a folder).
    pub fn latest(&self) -> Result<Option<Release>, UpdateError> {
        match self {
            Feed::Hosted(repo) => crate::forge::latest_release(repo).map_err(|e| UpdateError::Service(ServiceError::from(e))),
            Feed::Folder(dir) => {
                if !dir.is_dir() {
                    return Err(UpdateError::FeedFolderMissing { dir: shown(dir) });
                }
                let path = dir.join(FEED_RELEASE);
                let bytes = match read_capped_file(&path, MAX_FEED_JSON) {
                    Ok(bytes) => bytes,
                    Err(Capped::Io(e)) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
                    Err(Capped::Io(e)) => return Err(UpdateError::ReadFailed { path: shown(&path), said: e.to_string() }),
                    Err(Capped::TooLong(name)) => return Err(UpdateError::ReadTooLong { path: shown(&path), name }),
                };
                let size = |name: &str| std::fs::metadata(dir.join(name)).ok().filter(|m| m.is_file()).map(|m| m.len());
                release::parse_feed_release(&bytes, size)
                    .map(Some)
                    .map_err(|e| UpdateError::ReadFailed { path: shown(&path), said: e.to_string() })
            }
        }
    }

    /// The bytes of `asset`, at most `limit` of them.
    pub(super) fn fetch(&self, asset: &Asset, limit: u64) -> Result<Vec<u8>, UpdateError> {
        match self {
            Feed::Hosted(_) => crate::forge::fetch_public(&asset.url, limit).map_err(|e| UpdateError::Service(ServiceError::from(e))),
            Feed::Folder(dir) => {
                if !release::asset_name_ok(&asset.name) {
                    return Err(UpdateError::BadFileName { name: crate::text::printable(&asset.name) });
                }
                read_capped_file(&dir.join(&asset.name), limit).map_err(Capped::into_error)
            }
        }
    }

    /// `asset`'s bytes as they arrive, for a download streamed to disk. The caller limits how
    /// many it reads.
    pub(super) fn open(&self, asset: &Asset) -> Result<Box<dyn Read + Send>, UpdateError> {
        match self {
            Feed::Hosted(_) => crate::forge::open_public(&asset.url).map_err(|e| UpdateError::Service(ServiceError::from(e))),
            Feed::Folder(dir) => {
                if !release::asset_name_ok(&asset.name) {
                    return Err(UpdateError::BadFileName { name: crate::text::printable(&asset.name) });
                }
                let file = std::fs::File::open(dir.join(&asset.name)).map_err(|e| UpdateError::Said { said: e.to_string() })?;
                Ok(Box::new(file))
            }
        }
    }
}

/// Why a capped read failed: the system's error, or the file (by name) is longer than allowed.
enum Capped {
    Io(io::Error),
    TooLong(String),
}

impl Capped {
    fn into_error(self) -> UpdateError {
        match self {
            Capped::Io(e) => UpdateError::Said { said: e.to_string() },
            Capped::TooLong(name) => UpdateError::LargerThanExpected { name },
        }
    }
}

/// The contents of `path`, refused when longer than `limit`.
fn read_capped_file(path: &Path, limit: u64) -> Result<Vec<u8>, Capped> {
    let mut bytes = Vec::new();
    std::fs::File::open(path).and_then(|f| f.take(limit.saturating_add(1)).read_to_end(&mut bytes)).map_err(Capped::Io)?;
    if bytes.len() as u64 > limit {
        let name = path.file_name().map_or_else(|| shown(path), |n| n.to_string_lossy().into_owned());
        return Err(Capped::TooLong(name));
    }
    Ok(bytes)
}
