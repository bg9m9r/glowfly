//! Git LFS pointer files: the small text a commit stores in place of a file kept in LFS.

/// The longest pointer file read (git-lfs writes about 130 bytes).
const MAX_POINTER_BYTES: usize = 1024;

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
}
