//! Paths saved in Trellis's own files (recent.txt, tabs.txt, settings keys), without losing any
//! (a Unix name needn't be UTF-8, a Windows one may hold a lone UTF-16 surrogate) and one per
//! line. A path that is plain text without control characters is saved as it is, as earlier
//! versions did, so their files still read; any other is marked and percent-encoded: the bytes
//! on Unix, WTF-8 on Windows.

use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, percent_encode};
use std::path::{Path, PathBuf};

/// Starts an encoded path. No saved path starts with it: they are absolute.
const MARK: &str = "~enc:";

/// What `encode` escapes besides control characters and non-ASCII bytes.
const ESCAPED: &AsciiSet = &CONTROLS.add(b'%');

/// `path` as one line of text that `decode` turns back into exactly `path`.
pub fn encode(path: &Path) -> String {
    match path.to_str() {
        Some(s) if !s.starts_with(MARK) && !s.chars().any(char::is_control) => s.to_owned(),
        _ => format!("{MARK}{}", percent_encode(&imp::bytes(path), ESCAPED)),
    }
}

/// The path `encode` made `text` from; any other text is taken as a path as it is.
pub fn decode(text: &str) -> PathBuf {
    match text.strip_prefix(MARK) {
        Some(rest) => {
            let bytes: Vec<u8> = percent_decode_str(rest).collect();
            imp::path(&bytes).unwrap_or_else(|| PathBuf::from(String::from_utf8_lossy(&bytes).into_owned()))
        }
        None => PathBuf::from(text),
    }
}

#[cfg(unix)]
mod imp {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    pub(super) fn bytes(path: &Path) -> Vec<u8> {
        path.as_os_str().as_bytes().to_vec()
    }

    pub(super) fn path(bytes: &[u8]) -> Option<PathBuf> {
        Some(PathBuf::from(OsStr::from_bytes(bytes)))
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};

    /// WTF-8: UTF-8, with a lone surrogate written as the three bytes UTF-8 would use for it.
    pub(super) fn bytes(path: &Path) -> Vec<u8> {
        let mut out = Vec::new();
        for unit in char::decode_utf16(path.as_os_str().encode_wide()) {
            match unit {
                Ok(c) => out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
                Err(e) => {
                    let u = e.unpaired_surrogate();
                    out.extend_from_slice(&[0xE0 | (u >> 12) as u8, 0x80 | ((u >> 6) & 0x3F) as u8, 0x80 | (u & 0x3F) as u8]);
                }
            }
        }
        out
    }

    /// The path WTF-8 `bytes` spell; None when they aren't WTF-8.
    pub(super) fn path(bytes: &[u8]) -> Option<PathBuf> {
        let mut units = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            let (len, init) = match b {
                0x00..=0x7F => (1, u32::from(b)),
                0xC2..=0xDF => (2, u32::from(b & 0x1F)),
                0xE0..=0xEF => (3, u32::from(b & 0x0F)),
                0xF0..=0xF4 => (4, u32::from(b & 0x07)),
                _ => return None,
            };
            let tail = bytes.get(i + 1..i + len)?;
            if tail.iter().any(|t| t & 0xC0 != 0x80) {
                return None;
            }
            let cp = tail.iter().fold(init, |acc, t| (acc << 6) | u32::from(t & 0x3F));
            let shortest = match len {
                1 => true,
                2 => cp >= 0x80,
                3 => cp >= 0x800,
                _ => (0x1_0000..=0x10_FFFF).contains(&cp),
            };
            if !shortest {
                return None;
            }
            if cp >= 0x1_0000 {
                let v = cp - 0x1_0000;
                units.push(0xD800 | (v >> 10) as u16);
                units.push(0xDC00 | (v & 0x3FF) as u16);
            } else {
                units.push(cp as u16);
            }
            i += len;
        }
        Some(PathBuf::from(OsString::from_wide(&units)))
    }
}
