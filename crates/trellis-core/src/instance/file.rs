//! The instance file: the running window's address, token and process id.

use crate::persist::{self, WriteOptions};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct InstanceFile {
    /// `127.0.0.1:<port>`.
    pub addr: String,
    pub token: String,
    pub pid: u32,
}

/// The file at `path`; NotFound when there is none, InvalidData when it can't be read as one.
pub(super) fn read(path: &Path) -> io::Result<InstanceFile> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Writes `info` to `path`, readable by this user only.
pub(super) fn write(path: &Path, info: &InstanceFile) -> io::Result<()> {
    let text = serde_json::to_string(info).map_err(io::Error::other)?;
    persist::write_file(path, text.as_bytes(), WriteOptions { private: true, ..Default::default() })
}

/// Removes the file at `path` if it still holds `token`: a newer window may have replaced it.
pub(super) fn remove_if_ours(path: &Path, token: &str) {
    if read(path).is_ok_and(|f| crate::askpass::protocol::token_matches(&f.token, token)) {
        let _ = persist::remove_file(path);
    }
}
