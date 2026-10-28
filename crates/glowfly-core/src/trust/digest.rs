//! A short fingerprint of a file's contents, for the trust print: a hook script or the
//! repository's commands file. Only reads the file, and opens only a plain file (never a FIFO or
//! device, whose open could block). Fails closed: a file that can't be read gets a digest no
//! contents can have, so a decision made while it could be read doesn't cover it.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// The most bytes a refresh reads from one file (`Reading::Capped`): past this, the file's
/// length and modification time stand in for the rest of its bytes, so a refresh stays cheap
/// even with a large compiled hook. A full load reads every byte (`Reading::Whole`).
pub const MAX_BYTES: u64 = 1024 * 1024;

/// The digest of a file that can't be read (or isn't a plain file). Never the digest of
/// contents, which are hex.
pub(crate) const UNREADABLE: &str = "unreadable";

/// How much of a file is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reading {
    /// Every byte: what a trust decision is saved on (full loads).
    Whole,
    /// Up to [`MAX_BYTES`], then the length and modification time: only for noticing a change
    /// between refreshes (`repo::refs_key`), never compared with a saved print.
    Capped,
}

/// The first 8 bytes of the SHA-256 of the file at `path` (following links), as hex; see
/// [`Reading`] for large files and the module docs for unreadable ones. Files no larger than
/// [`MAX_BYTES`] get the same digest either way.
pub fn of_file(path: &Path, reading: Reading) -> String {
    read(path, reading).unwrap_or_else(|| UNREADABLE.to_owned())
}

fn read(path: &Path, reading: Reading) -> Option<String> {
    // Checked before opening: opening a FIFO blocks until something writes to it.
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut h = Sha256::new();
    if reading == Reading::Whole {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match file.read(&mut buf) {
                Ok(0) => return Some(hex(&h.finalize())),
                Ok(n) => h.update(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => return None,
            }
        }
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_BYTES {
        bytes.truncate(MAX_BYTES as usize);
        let modified = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
        h.update(format!("{}\0{modified}\0", meta.len()).as_bytes());
    }
    h.update(&bytes);
    Some(hex(&h.finalize()))
}

/// The digest of a file holding `bytes`: what [`of_file`] gives for it read whole.
pub fn of_bytes(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(sum: &[u8]) -> String {
    sum[..8].iter().map(|b| format!("{b:02x}")).collect()
}
