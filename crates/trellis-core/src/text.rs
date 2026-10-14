//! Making text from outside (servers, remote URLs) safe to show or log.

/// Unicode bidirectional formatting characters, which can make text display in a different
/// order than it reads (e.g. hide the real end of a URL or file name).
pub fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// `s` with control and bidi characters removed and runs of whitespace collapsed to one space.
pub fn printable(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if c.is_control() || is_bidi_control(c) {
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    out
}

/// `text` with the user/password part of every URL in it replaced by `***`
/// (`https://user:token@host/x` → `https://***@host/x`), for echoing commands into logs. A
/// plain `ssh://git@host` user name is kept.
pub fn redact_credentials(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("://") {
        let scheme_start = rest[..i].rfind(|c: char| !(c.is_ascii_alphanumeric() || "+-.".contains(c))).map_or(0, |p| p + 1);
        let scheme = rest[scheme_start..i].to_ascii_lowercase();
        let after = &rest[i + 3..];
        let end = after.find(|c: char| "/?#".contains(c) || c.is_whitespace()).unwrap_or(after.len());
        out.push_str(&rest[..i + 3]);
        match after[..end].rfind('@') {
            Some(at) if scheme.starts_with("http") || scheme.starts_with("ftp") || after[..at].contains(':') => {
                out.push_str("***");
                out.push_str(&after[at..end]);
            }
            _ => out.push_str(&after[..end]),
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_drops_control_and_bidi_characters() {
        assert_eq!(printable("  Bad\tcredentials\r\n\x1b[31mred\u{202e}txt.exe \u{2066}x "), "Bad credentials [31mredtxt.exe x");
        assert_eq!(printable("plain"), "plain");
        assert!(is_bidi_control('\u{202E}') && !is_bidi_control('a'));
    }

    #[test]
    fn redacts_credentials_in_urls() {
        let r = redact_credentials;
        assert_eq!(r("push https://user:TOKEN@host/o/r.git main"), "push https://***@host/o/r.git main");
        assert_eq!(r("https://ghp_secret@github.com/o/r"), "https://***@github.com/o/r");
        assert_eq!(r("remote.o.url=HTTP://a:b@h:8080/x"), "remote.o.url=HTTP://***@h:8080/x");
        assert_eq!(r("ssh://u:pw@h/x and https://h/y"), "ssh://***@h/x and https://h/y");
        for same in ["ssh://git@h/x", "git@h:o/r", "https://h/o@r", "plain text", "C:/repos/x", "https://"] {
            assert_eq!(r(same), same);
        }
    }
}
