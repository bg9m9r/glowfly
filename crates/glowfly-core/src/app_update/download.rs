//! Downloading an offered file: its checksum first, from the release's `SHA256SUMS` once its
//! minisign signature verifies, then the file, streamed to a temporary file
//! beside its final name and hashed on the way. It is renamed into place (through
//! `persist::write_streamed`) only once it is the announced size and matches the checksum, so a
//! half-written or mismatched file never appears under the final name. A cancelled or failed
//! download removes the temporary file. [`check_saved`] tells whether a saved file is still there
//! and unchanged.

use super::{Asset, Feed, Offer, SumsSource, UpdateError, signature, sums};
use crate::native_path::shown;
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// The largest file Glowfly downloads.
pub const MAX_DOWNLOAD: u64 = 512 * 1024 * 1024;
/// The largest checksum file read.
const MAX_SUMS: u64 = 64 * 1024;
/// How much is read, hashed and written at a time; a cancel is noticed between chunks.
const CHUNK: usize = 64 * 1024;

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

/// How far a download is: the bytes written so far, of `total` when the release gives the
/// file's size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: Option<u64>,
}

impl Progress {
    /// How much is done, from 0 to 1, when the total is known.
    pub fn fraction(&self) -> Option<f32> {
        self.total.filter(|&t| t > 0).map(|t| (self.done.min(t) as f64 / t as f64) as f32)
    }
}

/// What is wrong with a file saved earlier ([`check_saved`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavedProblem {
    /// Nothing (or a folder) is at its path any more.
    Missing,
    /// It no longer matches the checksum it was saved with.
    Changed,
}

/// Downloads `offer`'s file from `feed` into `dir` once its SHA-256 matches the release's
/// signed checksum (read from the same feed); returns where it was saved. Nothing is left
/// behind when the checksum or its signature is missing or doesn't match, or the file isn't
/// the announced size.
pub fn download(feed: &Feed, offer: &Offer, dir: &Path) -> Result<PathBuf, UpdateError> {
    download_saved(feed, offer, dir).map(|s| s.path)
}

/// [`download`], also returning the checksum the file matched, which Install and restart
/// checks again ([`read_verified`]) before it runs or unpacks the file.
pub fn download_saved(feed: &Feed, offer: &Offer, dir: &Path) -> Result<Saved, UpdateError> {
    download_streamed(feed, offer, dir, &mut |_| {}, &|| false)?.ok_or(UpdateError::Stopped)
}

/// [`download_saved`] with progress and a way to stop: `progress` hears after each chunk is
/// written, and `cancelled` is asked before each one. Once it says so, the temporary file is
/// removed and the answer is `Ok(None)`. A file of the same name that already matches is used
/// as it is, without downloading it again.
pub fn download_streamed(
    feed: &Feed,
    offer: &Offer,
    dir: &Path,
    progress: &mut dyn FnMut(Progress),
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Saved>, UpdateError> {
    let Some(asset) = &offer.asset else { return Err(UpdateError::NoDownload) };
    let want = signed_sum(feed, offer, asset)?;
    if asset.size > MAX_DOWNLOAD {
        return Err(UpdateError::TooLarge { name: asset.name.clone(), mb: MAX_DOWNLOAD / (1024 * 1024) });
    }
    std::fs::create_dir_all(dir).map_err(|e| UpdateError::CreateFailed { path: shown(dir), said: e.to_string() })?;
    let same = dir.join(&asset.name);
    if file_digest(&same, asset.size) == Some(want) {
        return Ok(Some(Saved { path: same, sha256: want }));
    }
    if cancelled() {
        return Ok(None);
    }
    let mut reader = feed.open(asset).map_err(|e| download_failed(asset, e))?;
    let path = dir.join(free_name(&asset.name, |n| dir.join(n).symlink_metadata().is_ok()));
    let mut stopped = None;
    let written = crate::persist::write_streamed(&path, crate::persist::WriteOptions::default(), |file| {
        let end = match copy_hashed(&mut *reader, file, asset, &path, progress, cancelled) {
            Ok(digest) if digest == want => return Ok(()),
            Ok(_) => Stop::Failed(UpdateError::ChecksumMismatch),
            Err(end) => end,
        };
        stopped = Some(end);
        Err(io::Error::other("the download stopped"))
    });
    match (written, stopped) {
        (Ok(()), _) => Ok(Some(Saved { path, sha256: want })),
        (Err(_), Some(Stop::Cancelled)) => Ok(None),
        (Err(_), Some(Stop::Failed(e))) => Err(e),
        (Err(e), None) => Err(UpdateError::SaveFailed { path: shown(&path), said: e.to_string() }),
    }
}

/// The SHA-256 the release's signed `SHA256SUMS` gives for `asset`. The signature is fetched
/// and checked against the keys `feed` trusts before any line of the list is read; a release
/// without one, a signature that doesn't verify, or a list without `asset` fails it, and
/// `asset` itself is never fetched then.
fn signed_sum(feed: &Feed, offer: &Offer, asset: &Asset) -> Result<[u8; 32], UpdateError> {
    let no_sum = || UpdateError::NoChecksum { name: asset.name.clone() };
    let SumsSource::Combined(sums_asset) = &offer.sums else { return Err(no_sum()) };
    let signature_asset = signature::signature_asset(&offer.release).ok_or(UpdateError::Unsigned)?;
    let text = feed.fetch(sums_asset, MAX_SUMS).map_err(|e| UpdateError::Checksums(Box::new(e)))?;
    let sig = feed.fetch(&signature_asset, signature::MAX_SIGNATURE).map_err(|e| UpdateError::SignatureUnreadable(Box::new(e)))?;
    signature::verify(&text, &sig, &feed.trusted_keys())?;
    sums::expected(&String::from_utf8_lossy(&text), &asset.name).ok_or_else(no_sum)
}

/// Why a transfer ended before the file was complete.
enum Stop {
    Cancelled,
    Failed(UpdateError),
}

/// "Couldn't download <asset>: <why>".
fn download_failed(asset: &Asset, why: UpdateError) -> UpdateError {
    UpdateError::DownloadFailed { name: asset.name.clone(), why: Box::new(why) }
}

/// Copies `reader` into `out` chunk by chunk, hashing it and reporting `progress`, until the
/// end; returns the digest. More bytes than `asset` announces, or fewer, fail it.
fn copy_hashed(
    reader: &mut dyn Read,
    out: &mut std::fs::File,
    asset: &Asset,
    path: &Path,
    progress: &mut dyn FnMut(Progress),
    cancelled: &dyn Fn() -> bool,
) -> Result<[u8; 32], Stop> {
    let total = (asset.size > 0).then_some(asset.size);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut done: u64 = 0;
    loop {
        if cancelled() {
            return Err(Stop::Cancelled);
        }
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(Stop::Failed(download_failed(asset, UpdateError::Said { said: e.to_string() }))),
        };
        done += n as u64;
        if done > asset.size {
            return Err(Stop::Failed(download_failed(asset, UpdateError::LargerThanExpected { name: asset.name.clone() })));
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| Stop::Failed(UpdateError::SaveFailed { path: shown(path), said: e.to_string() }))?;
        progress(Progress { done, total });
    }
    if done != asset.size {
        return Err(Stop::Failed(UpdateError::WrongSize { name: asset.name.clone(), got: done, want: asset.size }));
    }
    Ok(hasher.finalize().into())
}

/// The SHA-256 of the file at `path` when it is a regular file of `size` bytes.
fn file_digest(path: &Path, size: u64) -> Option<[u8; 32]> {
    path.symlink_metadata().ok().filter(|m| m.is_file() && m.len() == size)?;
    std::fs::File::open(path).and_then(|f| sums::sha256_reader(f.take(size.saturating_add(1)))).ok()
}

/// Whether the file saved at `path` is still there with the SHA-256 `want`: None when it is,
/// or when it can't be read just now (Install and restart says why if it still can't), else
/// what is wrong with it. Reads it in chunks.
pub fn check_saved(path: &Path, want: &[u8; 32]) -> Option<SavedProblem> {
    match std::fs::metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Some(SavedProblem::Missing),
        Err(_) => return None,
        Ok(m) if !m.is_file() => return Some(SavedProblem::Missing),
        Ok(m) if m.len() > MAX_DOWNLOAD => return Some(SavedProblem::Changed),
        Ok(_) => {}
    }
    match std::fs::File::open(path).and_then(|f| sums::sha256_reader(f.take(MAX_DOWNLOAD + 1))) {
        Ok(digest) if digest == *want => None,
        Ok(_) => Some(SavedProblem::Changed),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Some(SavedProblem::Missing),
        Err(_) => None,
    }
}

/// The bytes of the downloaded file at `path` (at most [`MAX_DOWNLOAD`]), once their SHA-256
/// is still `want`: the file may have changed since it was checked.
pub fn read_verified(path: &Path, want: &[u8; 32]) -> Result<Vec<u8>, UpdateError> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(MAX_DOWNLOAD + 1).read_to_end(&mut bytes))
        .map_err(|e| UpdateError::ReadFailed { path: shown(path), said: e.to_string() })?;
    if bytes.len() as u64 > MAX_DOWNLOAD || sums::sha256(&bytes) != *want {
        return Err(UpdateError::ChangedSinceChecked);
    }
    Ok(bytes)
}
