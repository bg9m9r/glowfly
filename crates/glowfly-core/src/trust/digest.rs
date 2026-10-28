//! A short fingerprint of a file's contents, for the trust print: a hook script or the
//! repository's commands file. Only reads the file. Fails closed: a file that can't be read gets
//! a digest no contents can have, so a decision made while it could be read doesn't cover it.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// The most bytes read from one file. Hooks and commands files are small; past this, the
/// file's length and modification time stand in for the rest of its bytes, so a refresh stays
/// cheap even with a large compiled hook.
pub const MAX_BYTES: u64 = 1024 * 1024;

/// The digest of a file that can't be read (or isn't a file). Never the digest of contents,
/// which are hex.
const UNREADABLE: &str = "unreadable";

/// The first 8 bytes of the SHA-256 of the file at `path` (following links), as hex; see the
/// module docs for files larger than [`MAX_BYTES`] and unreadable ones.
pub fn of_file(path: &Path) -> String {
    read(path).unwrap_or_else(|| UNREADABLE.to_owned())
}

fn read(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).ok()?;
    let mut h = Sha256::new();
    if bytes.len() as u64 > MAX_BYTES {
        bytes.truncate(MAX_BYTES as usize);
        let modified = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
        h.update(format!("{}\0{modified}\0", meta.len()).as_bytes());
    }
    h.update(&bytes);
    Some(h.finalize()[..8].iter().map(|b| format!("{b:02x}")).collect())
}
