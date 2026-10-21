//! Git's C-quoted paths: a path with unusual bytes comes as `"..."` with backslash escapes and
//! octal bytes (`core.quotePath`).

/// Decodes a C-quoted path (`"q\"z"`, octal bytes such as `\303\251`); text without the
/// surrounding quotes comes back unchanged.
pub(crate) fn unquote_c(line: &str) -> String {
    let Some(inner) = line.strip_prefix('"').and_then(|l| l.strip_suffix('"')) else { return line.to_owned() };
    let b = inner.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' || i + 1 == b.len() {
            out.push(b[i]);
            i += 1;
            continue;
        }
        let c = b[i + 1];
        i += 2;
        let decoded = match c {
            b'a' => 0x07,
            b'b' => 0x08,
            b't' => b'\t',
            b'n' => b'\n',
            b'v' => 0x0b,
            b'f' => 0x0c,
            b'r' => b'\r',
            b'0'..=b'7' if b.get(i..i + 2).is_some_and(|d| d.iter().all(|x| (b'0'..=b'7').contains(x))) => {
                let v = (c - b'0') as u32 * 64 + (b[i] - b'0') as u32 * 8 + (b[i + 1] - b'0') as u32;
                i += 2;
                v as u8
            }
            other => other,
        };
        out.push(decoded);
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::unquote_c;

    #[test]
    fn octal_bytes_decode_as_utf8() {
        assert_eq!(unquote_c(r#""caf\303\251.txt""#), "café.txt");
    }

    #[test]
    fn escaped_quote_and_backslash() {
        assert_eq!(unquote_c(r#""q\"z""#), "q\"z");
        assert_eq!(unquote_c(r#""back\\slash""#), r"back\slash");
        assert_eq!(unquote_c(r#""a\tb""#), "a\tb");
    }

    #[test]
    fn unquoted_text_passes_through() {
        assert_eq!(unquote_c("plain/path.txt"), "plain/path.txt");
        assert_eq!(unquote_c(r#""half"#), r#""half"#);
    }
}
