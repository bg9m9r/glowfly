//! Making text from outside (servers, remote URLs) safe to show or log.

/// Unicode bidirectional formatting characters, which can make text display in a different
/// order than it reads (e.g. hide the real end of a URL or file name).
pub fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Characters drawn as nothing at all (zero-width spaces and joiners, word joiners, invisible
/// operators, the byte order mark, soft hyphens, Hangul fillers and the other default-ignorable
/// format characters): they can hide inside a name or an address without showing.
pub fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{115F}'
            | '\u{1160}'
            | '\u{17B4}'
            | '\u{17B5}'
            | '\u{180B}'..='\u{180F}'
            | '\u{200B}'..='\u{200D}'
            | '\u{2060}'..='\u{2065}'
            | '\u{206A}'..='\u{206F}'
            | '\u{3164}'
            | '\u{FEFF}'
            | '\u{FFA0}'
            | '\u{FFF0}'..='\u{FFF8}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0000}'..='\u{E0FFF}'
    )
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
        if c.is_control() || is_bidi_control(c) || is_invisible(c) {
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

/// `s` with its lines kept (`\r\n` and a lone `\r` read as `\n`) and tabs kept, but every other
/// control character, every bidi character and every invisible one (`is_invisible`) removed: for
/// multi-line text such as a description.
pub fn printable_lines(s: &str) -> String {
    let s = s.replace("\r\n", "\n").replace('\r', "\n");
    s.chars().filter(|&c| matches!(c, '\n' | '\t') || !(c.is_control() || is_bidi_control(c) || is_invisible(c))).collect()
}

/// `text` with the user/password part of every URL in it replaced by `***`
/// (`https://user:token@host/x` → `https://***@host/x`), for echoing commands into logs. A
/// plain `ssh://git@host` user name is kept.
pub fn redact_credentials(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("://") {
        // Just after the last character that can't be in a scheme (which may take several bytes).
        let scheme_start = rest[..i]
            .char_indices()
            .rfind(|&(_, c)| !(c.is_ascii_alphanumeric() || "+-.".contains(c)))
            .map_or(0, |(p, c)| p + c.len_utf8());
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
    fn printable_lines_keeps_lines_and_tabs_only() {
        assert_eq!(printable_lines("a\r\nb\rc\n\tcode\x1b[0m\u{202e}gpj.exe\u{7}"), "a\nb\nc\n\tcode[0mgpj.exe");
        assert_eq!(printable_lines("  keeps  spaces "), "  keeps  spaces ");
    }

    #[test]
    fn invisible_characters_are_dropped() {
        for c in ['\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}', '\u{2061}', '\u{2064}', '\u{FEFF}', '\u{00AD}', '\u{E0041}'] {
            let s = format!("github{c}.com");
            assert_eq!(printable(&s), "github.com", "{:04X}", c as u32);
            assert_eq!(printable_lines(&format!("a\n{s}")), "a\ngithub.com", "{:04X}", c as u32);
        }
        assert!(!is_invisible('a') && !is_invisible(' ') && !is_invisible('\u{FF0E}'));
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

    #[test]
    fn a_character_of_several_bytes_before_a_url_is_kept_whole() {
        let r = redact_credentials;
        assert_eq!(r("«https://user:TOKEN@host/x»"), "«https://***@host/x»");
        assert_eq!(r("é://u:p@h/x"), "é://***@h/x");
        assert_eq!(r("\u{fffd}to://m"), "\u{fffd}to://m", "found by the redact_credentials fuzz check");
    }
}
