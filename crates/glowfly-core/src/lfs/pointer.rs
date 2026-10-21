//! Git LFS pointer files: the small text a commit stores in place of a file kept in LFS, read
//! from bytes, a blob or a file; what a diff's two sides are (`LfsChange`); sizes for the screen.

use git2::{ObjectType, Oid, Repository};
use std::path::Path;

/// The longest pointer file read (git-lfs writes about 130 bytes).
pub const MAX_POINTER_BYTES: usize = 1024;

const VERSION_LINE: &str = "version https://git-lfs.github.com/spec/v1";

/// What a pointer file names: the object's SHA-256 and its size in bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pointer {
    pub oid: String,
    pub size: u64,
}

/// The pointer `bytes` hold, or None when they aren't one: at most 1 KB of UTF-8, the version
/// line first, then `key value` lines including `oid sha256:<64 hex digits>` and `size <n>`.
pub fn parse(bytes: &[u8]) -> Option<Pointer> {
    if bytes.len() > MAX_POINTER_BYTES {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut lines = text.lines();
    if lines.next()?.trim_end() != VERSION_LINE {
        return None;
    }
    let (mut oid, mut size) = (None, None);
    for line in lines.map(str::trim_end).filter(|l| !l.is_empty()) {
        let (key, value) = line.split_once(' ')?;
        match key {
            "oid" => {
                let hex = value.strip_prefix("sha256:")?;
                let ok = hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
                oid = Some(ok.then(|| hex.to_owned())?);
            }
            "size" => size = Some(value.parse::<u64>().ok()?),
            _ => {}
        }
    }
    Some(Pointer { oid: oid?, size: size? })
}

/// The pointer blob `id` holds, or None (the zero id, not a blob, over [`MAX_POINTER_BYTES`] or
/// not a pointer). The size comes from the object header, so a large blob is never inflated.
pub fn read_blob(repo: &Repository, id: Oid) -> Option<Pointer> {
    if id.is_zero() {
        return None;
    }
    let (size, kind) = repo.odb().ok()?.read_header(id).ok()?;
    if kind != ObjectType::Blob || size > MAX_POINTER_BYTES {
        return None;
    }
    parse(repo.find_blob(id).ok()?.content())
}

/// The pointer the regular file at `path` holds, or None (missing, a folder or link, over
/// [`MAX_POINTER_BYTES`] or not a pointer).
pub fn read_file(path: &Path) -> Option<Pointer> {
    let meta = path.symlink_metadata().ok()?;
    if !meta.is_file() || meta.len() > MAX_POINTER_BYTES as u64 {
        return None;
    }
    parse(&std::fs::read(path).ok()?)
}

/// `bytes` for the screen, in 1024-based units: "1 byte", "512 bytes", "1.5 KB", "12.4 MB".
pub fn size_text(bytes: u64) -> String {
    match bytes {
        1 => "1 byte".into(),
        0..1024 => format!("{bytes} bytes"),
        _ => {
            let mut value = bytes as f64 / 1024.0;
            for unit in ["KB", "MB", "GB"] {
                // Rounded as shown, so 1023.96 KB reads "1.0 MB" rather than "1024.0 KB".
                if (value * 10.0).round() < 10240.0 {
                    return format!("{value:.1} {unit}");
                }
                value /= 1024.0;
            }
            format!("{value:.1} TB")
        }
    }
}

/// One side of a diff as Git LFS sees it.
#[derive(Clone, Debug, PartialEq)]
pub enum LfsSide {
    /// The file isn't on this side (added or deleted).
    Absent,
    Pointer(Pointer),
    /// A regular file of `size` bytes.
    Regular {
        size: u64,
    },
}

/// The two sides of a file diff where at least one is an LFS pointer.
#[derive(Clone, Debug, PartialEq)]
pub struct LfsChange {
    pub old: LfsSide,
    pub new: LfsSide,
    /// The new side is the working copy (an unstaged change, a commit-to-working-tree diff).
    pub new_in_workdir: bool,
    /// The path has `filter=lfs` (the working tree's attributes, then the index's). Only worked
    /// out when `new_in_workdir`.
    pub filtered: bool,
}

impl LfsChange {
    pub fn has_pointer(&self) -> bool {
        matches!(self.old, LfsSide::Pointer(_)) || matches!(self.new, LfsSide::Pointer(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OID: &str = "4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393";

    fn pointer(oid: &str, size: &str, eol: &str) -> String {
        format!("{VERSION_LINE}{eol}oid sha256:{oid}{eol}size {size}{eol}")
    }

    #[test]
    fn parses_a_pointer_with_either_line_ending() {
        let want = Some(Pointer { oid: OID.into(), size: 12345 });
        assert_eq!(parse(pointer(OID, "12345", "\n").as_bytes()), want);
        assert_eq!(parse(pointer(OID, "12345", "\r\n").as_bytes()), want);
        // No final newline, and an extension key.
        let ext = format!("{VERSION_LINE}\next-0-foo sha256:{OID}\noid sha256:{OID}\nsize 12345");
        assert_eq!(parse(ext.as_bytes()), want);
    }

    #[test]
    fn rejects_what_is_not_a_pointer() {
        let other_version = pointer(OID, "1", "\n").replace("spec/v1", "spec/v2");
        assert_eq!(parse(other_version.as_bytes()), None);
        assert_eq!(parse(format!("{VERSION_LINE}\noid sha256:{OID}\n").as_bytes()), None, "no size");
        assert_eq!(parse(format!("{VERSION_LINE}\nsize 1\n").as_bytes()), None, "no oid");
        assert_eq!(parse(pointer(&OID[1..], "1", "\n").as_bytes()), None, "short oid");
        assert_eq!(parse(pointer(&OID.to_uppercase(), "1", "\n").as_bytes()), None, "uppercase oid");
        assert_eq!(parse(pointer(OID, "-1", "\n").as_bytes()), None, "bad size");
        let long = format!("{}{}", pointer(OID, "1", "\n"), "x y\n".repeat(300));
        assert_eq!(parse(long.as_bytes()), None, "over 1 KB");
        let mut bad = pointer(OID, "1", "\n").into_bytes();
        bad.extend_from_slice(b"note \xff\n");
        assert_eq!(parse(&bad), None, "not UTF-8");
        assert_eq!(parse(b"hello\n"), None);
        assert_eq!(parse(b""), None);
    }

    #[test]
    fn sizes_read_in_1024_based_units() {
        assert_eq!(size_text(0), "0 bytes");
        assert_eq!(size_text(1), "1 byte");
        assert_eq!(size_text(1023), "1023 bytes");
        assert_eq!(size_text(1024), "1.0 KB");
        assert_eq!(size_text(1536), "1.5 KB");
        assert_eq!(size_text(1024 * 1024 - 1), "1.0 MB");
        assert_eq!(size_text(13_002_342), "12.4 MB");
        assert_eq!(size_text(1_181_116_006), "1.1 GB");
        assert_eq!(size_text(3 << 40), "3.0 TB");
    }

    #[test]
    fn reads_pointers_from_blobs_without_inflating_large_ones() {
        let dir = crate::testutil::scratch_repo("lfs-pointer-blob");
        let repo = Repository::open(&dir).unwrap();
        let text = pointer(
            OID, "12345", "
",
        );
        let id = repo.blob(text.as_bytes()).unwrap();
        assert_eq!(read_blob(&repo, id), Some(Pointer { oid: OID.into(), size: 12345 }));
        assert_eq!(
            read_blob(
                &repo,
                repo.blob(
                    b"plain text
"
                )
                .unwrap()
            ),
            None
        );
        let long = format!("{text}{}", "x".repeat(MAX_POINTER_BYTES));
        assert_eq!(read_blob(&repo, repo.blob(long.as_bytes()).unwrap()), None, "over 1 KB");
        assert_eq!(read_blob(&repo, Oid::zero()), None);
        drop(repo);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_pointers_from_regular_files_only() {
        let dir = crate::testutil::scratch_repo("lfs-pointer-file");
        std::fs::create_dir_all(dir.join("folder")).unwrap();
        assert_eq!(read_file(&dir.join("folder")), None);
        assert_eq!(read_file(&dir.join("missing.bin")), None);
        std::fs::write(dir.join("big.bin"), vec![b'x'; 2048]).unwrap();
        assert_eq!(read_file(&dir.join("big.bin")), None);
        std::fs::write(
            dir.join("art.psd"),
            pointer(
                OID, "7", "
",
            ),
        )
        .unwrap();
        assert_eq!(read_file(&dir.join("art.psd")), Some(Pointer { oid: OID.into(), size: 7 }));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
