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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_drops_control_and_bidi_characters() {
        assert_eq!(printable("  Bad\tcredentials\r\n\x1b[31mred\u{202e}txt.exe \u{2066}x "), "Bad credentials [31mredtxt.exe x");
        assert_eq!(printable("plain"), "plain");
        assert!(is_bidi_control('\u{202E}') && !is_bidi_control('a'));
    }
}
