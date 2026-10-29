//! Checksum files in the format `sha256sum` writes: `<64 hex digits>  <name>` (text mode) or
//! `<64 hex digits> *<name>` (binary mode), one file per line.

use sha2::{Digest, Sha256};

fn hex_digest(s: &str) -> Option<[u8; 32]> {
    let bytes = s.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in bytes.chunks(2).enumerate() {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        out[i] = (hi * 16 + lo) as u8;
    }
    Some(out)
}

/// Every well-formed line of `text` as (name, digest). Blank lines, `#` comments and malformed
/// lines are skipped; hex digits may be in either case and lines may end in CRLF. The name is
/// the rest of the line, spaces included.
pub fn parse(text: &str) -> Vec<(String, [u8; 32])> {
    text.lines()
        .filter_map(|line| {
            let line = line.strip_suffix('\r').unwrap_or(line);
            if line.trim().is_empty() || line.starts_with('#') {
                return None;
            }
            let (digest, rest) = (line.get(..64)?, line.get(64..)?);
            let name = rest.strip_prefix("  ").or_else(|| rest.strip_prefix(" *"))?;
            if name.is_empty() {
                return None;
            }
            Some((name.to_owned(), hex_digest(digest)?))
        })
        .collect()
}

/// The digest `text` gives for `name` (its first entry).
pub fn expected(text: &str, name: &str) -> Option<[u8; 32]> {
    parse(text).into_iter().find(|(n, _)| n == name).map(|(_, d)| d)
}

/// The SHA-256 digest of `bytes`.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// The SHA-256 digest of everything `reader` gives, read in chunks rather than held whole.
pub(super) fn sha256_reader(mut reader: impl std::io::Read) -> std::io::Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => return Ok(hasher.finalize().into()),
            Ok(n) => hasher.update(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}
