//! Downloading an offered file: its checksum first, then the file, checked against it before a
//! byte reaches the disk. The file is held in memory (at most [`MAX_DOWNLOAD`]) and written
//! atomically through `persist`.

use super::{Feed, Offer, SumsSource, sums};
use std::path::{Path, PathBuf};

/// The largest file Glowfly downloads.
pub const MAX_DOWNLOAD: u64 = 512 * 1024 * 1024;
/// The largest checksum file read.
const MAX_SUMS: u64 = 64 * 1024;

/// `name` split into its stem and its extension (with the dot); `.tar.gz` counts as one.
pub fn split_extension(name: &str) -> (&str, &str) {
    if let Some(stem) = name.strip_suffix(".tar.gz").filter(|s| !s.is_empty()) {
        return (stem, &name[stem.len()..]);
    }
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

/// `name` when `taken` says it is free, else `stem (1).ext`, `stem (2).ext` and so on.
pub fn free_name(name: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(name) {
        return name.to_owned();
    }
    let (stem, ext) = split_extension(name);
    (1..).map(|n| format!("{stem} ({n}){ext}")).find(|n| !taken(n)).unwrap_or_else(|| name.to_owned())
}

/// A verified download: where it was saved and its SHA-256.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub path: PathBuf,
    pub sha256: [u8; 32],
}

/// Downloads `offer`'s file from `feed` into `dir` once its SHA-256 matches the release's
/// published checksum (read from the same feed); returns where it was saved. Nothing is written
/// when the checksum is missing or doesn't match, or the file isn't the announced size.
pub fn download(feed: &Feed, offer: &Offer, dir: &Path) -> Result<PathBuf, String> {
    download_saved(feed, offer, dir).map(|s| s.path)
}

/// [`download`], also returning the checksum the file matched, which Install and restart
/// checks again ([`read_verified`]) before it runs or unpacks the file.
pub fn download_saved(feed: &Feed, offer: &Offer, dir: &Path) -> Result<Saved, String> {
    let Some(asset) = &offer.asset else { return Err("This release has no download for this system.".to_owned()) };
    let no_sum = || format!("This release doesn't publish a checksum for {}, so it wasn't downloaded.", asset.name);
    let sums_asset = match &offer.sums {
        SumsSource::Combined(a) | SumsSource::Single(a) => a,
        SumsSource::None => return Err(no_sum()),
    };
    let text = feed.fetch(sums_asset, MAX_SUMS).map_err(|e| format!("Couldn't read the checksums: {e}"))?;
    let want = sums::expected(&String::from_utf8_lossy(&text), &asset.name).ok_or_else(no_sum)?;
    if asset.size > MAX_DOWNLOAD {
        return Err(format!("{} is larger than {} MB, so it wasn't downloaded.", asset.name, MAX_DOWNLOAD / (1024 * 1024)));
    }
    let bytes = feed.fetch(asset, asset.size).map_err(|e| format!("Couldn't download {}: {e}", asset.name))?;
    if bytes.len() as u64 != asset.size {
        return Err(format!("The download of {} was {} bytes instead of {}, so it was discarded.", asset.name, bytes.len(), asset.size));
    }
    if sums::sha256(&bytes) != want {
        return Err("The download didn't match its published checksum and was discarded.".to_owned());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't create {}: {e}", crate::native_path::shown(dir)))?;
    let same = dir.join(&asset.name);
    if std::fs::read(&same).is_ok_and(|old| sums::sha256(&old) == want) {
        return Ok(Saved { path: same, sha256: want });
    }
    let path = dir.join(free_name(&asset.name, |n| dir.join(n).symlink_metadata().is_ok()));
    crate::persist::write_file(&path, &bytes, crate::persist::WriteOptions::default())
        .map_err(|e| format!("Couldn't save {}: {e}", crate::native_path::shown(&path)))?;
    Ok(Saved { path, sha256: want })
}

/// The bytes of the downloaded file at `path` (at most [`MAX_DOWNLOAD`]), once their SHA-256
/// is still `want`: the file may have changed since it was checked.
pub fn read_verified(path: &Path, want: &[u8; 32]) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let shown = crate::native_path::shown(path);
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(MAX_DOWNLOAD + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("Couldn't read {shown}: {e}"))?;
    if bytes.len() as u64 > MAX_DOWNLOAD || sums::sha256(&bytes) != *want {
        return Err("The downloaded file changed after it was checked, so it wasn't installed. Download it again.".to_owned());
    }
    Ok(bytes)
}
