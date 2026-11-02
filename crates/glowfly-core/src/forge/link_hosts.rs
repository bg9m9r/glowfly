//! The hosts of links in Markdown from a hosting service, as the user should see them: an
//! address shown with its host in punycode (`shown_url`), so a look-alike host made of other
//! scripts can't pass for a familiar one, and whether a link's visible text names another host
//! than the one it really goes to (`mismatch`), which the app asks about before opening.

use crate::text::{is_bidi_control, is_invisible, printable};

/// The span of `url`'s host (after `scheme://` and any `user@`, before a port or path).
fn host_span(url: &str) -> Option<(usize, usize)> {
    let start = url.find("://")? + 3;
    let rest = &url[start..];
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let at = authority.rfind('@').map_or(0, |i| i + 1);
    let host = &authority[at..];
    let end = if host.starts_with('[') { host.find(']').map_or(host.len(), |i| i + 1) } else { host.find(':').unwrap_or(host.len()) };
    Some((start + at, start + at + end))
}

/// `host` lowercased, without a trailing dot, with any label that isn't ASCII in punycode
/// (`xn--…`); as it was (lowercased) when it isn't a valid name.
fn ascii_host(host: &str) -> String {
    let host = host.trim_end_matches('.');
    idna::domain_to_ascii(host).unwrap_or_else(|_| host.to_lowercase())
}

/// `url` made printable, with its host in punycode; the rest is unchanged.
pub fn shown_url(url: &str) -> String {
    let url = printable(url);
    match host_span(&url) {
        Some((a, b)) if a < b => format!("{}{}{}", &url[..a], ascii_host(&url[a..b]), &url[b..]),
        _ => url,
    }
}

/// `text` as it reads: invisible, control and bidi characters dropped, full-width dots, slashes
/// and colons as their ASCII forms, and surrounding punctuation (`(…)`, `→…`, `…,`) trimmed.
fn as_read(text: &str) -> String {
    let read: String = text
        .chars()
        .filter(|&c| !(c.is_control() || is_bidi_control(c) || is_invisible(c)))
        .map(|c| match c {
            '\u{FF0E}' | '\u{3002}' | '\u{FF61}' => '.',
            '\u{FF0F}' => '/',
            '\u{FF1A}' => ':',
            c => c,
        })
        .collect();
    read.trim_start_matches(|c: char| !c.is_alphanumeric()).trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/').to_owned()
}

/// The host `text` names when it reads as a web address (`https://host/…`, `www.host`,
/// `host.tld/…`), in punycode; None for ordinary words. A bare `name.tld` without a path
/// isn't one: it reads as a file name (`README.md`) as often as a site. The host is judged as
/// IDNA maps it (full-width letters as ASCII, …), so text can't look like a host yet fail the check.
fn text_host(text: &str) -> Option<String> {
    let text = as_read(text);
    if text.is_empty() || text.chars().any(char::is_whitespace) {
        return None;
    }
    let host = match host_span(&text) {
        Some((a, b)) => &text[a..b],
        None => {
            let end = text.find(['/', '?', '#', ':']).unwrap_or(text.len());
            let www = text.get(..4).is_some_and(|w| w.eq_ignore_ascii_case("www."));
            if !www && !text[end..].starts_with('/') {
                return None;
            }
            &text[..end]
        }
    };
    let host = idna::domain_to_ascii(host.trim_end_matches('.')).ok()?;
    let host = host.trim_end_matches('.');
    let labels: Vec<&str> = host.split('.').collect();
    let tld = labels.last()?;
    let named = labels.len() >= 2
        && labels.iter().all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && tld.len() >= 2
        && (tld.chars().all(|c| c.is_ascii_alphabetic()) || tld.starts_with("xn--"));
    named.then(|| host.to_owned())
}

/// When `text`, a link's visible text, reads as a web address on another host than `url`'s:
/// that host and `url`'s, both in punycode (`(shown, real)`). Case, a trailing dot and a leading
/// `www.` don't count as a difference.
pub fn mismatch(text: &str, url: &str) -> Option<(String, String)> {
    let shown = text_host(text)?;
    let (a, b) = host_span(url)?;
    let real = ascii_host(&url[a..b]);
    let bare = |h: &str| h.strip_prefix("www.").unwrap_or(h).to_owned();
    (bare(&shown) != bare(&real)).then_some((shown, real))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_are_shown_in_punycode() {
        assert_eq!(shown_url("https://g\u{456}thub.com/acme"), "https://xn--gthub-n2e.com/acme");
        assert_eq!(shown_url("http://G\u{456}tHub.com:8080/x?y#z"), "http://xn--gthub-n2e.com:8080/x?y#z");
        assert_eq!(shown_url("https://github.com/x"), "https://github.com/x");
        assert_eq!(shown_url("https://\u{202e}moc.evil/x"), "https://moc.evil/x", "printable first");
        assert_eq!(shown_url("a.md"), "a.md");
    }

    #[test]
    fn a_text_naming_another_host_is_a_mismatch() {
        let m = |t: &str, u: &str| mismatch(t, u);
        assert_eq!(m("https://github.com/acme/app", "https://evil.example/login"), Some(("github.com".into(), "evil.example".into())));
        assert_eq!(m("github.com/acme", "https://g\u{456}thub.com/acme"), Some(("github.com".into(), "xn--gthub-n2e.com".into())));
        assert_eq!(m("paypal.com/", "https://paypal.com.evil.example/"), Some(("paypal.com".into(), "paypal.com.evil.example".into())));
        assert_eq!(m("www.paypal.com", "https://evil.example/"), Some(("www.paypal.com".into(), "evil.example".into())));
    }

    #[test]
    fn hidden_characters_and_look_alike_dots_dont_hide_the_text_host() {
        let evil = "https://evil.example/login";
        for c in ['\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}', '\u{2063}', '\u{FEFF}', '\u{00AD}', '\u{202E}'] {
            let text = format!("https://github{c}.com/acme/app");
            assert_eq!(mismatch(&text, evil), Some(("github.com".into(), "evil.example".into())), "{:04X}", c as u32);
        }
        for text in ["github\u{FF0E}com/acme", "github\u{3002}com/acme", "https://github\u{FF61}com/x", "\u{FF47}ithub.com/acme"] {
            assert_eq!(mismatch(text, evil), Some(("github.com".into(), "evil.example".into())), "{text}");
        }
    }

    #[test]
    fn surrounding_punctuation_doesnt_hide_the_text_host() {
        let evil = "https://evil.example/";
        for (text, shown) in [
            ("(github.com/acme)", "github.com"),
            ("\u{2192}github.com/acme", "github.com"),
            ("\"www.github.com\",", "www.github.com"),
            ("<https://github.com/x>.", "github.com"),
            ("[github.com/acme]!", "github.com"),
        ] {
            assert_eq!(mismatch(text, evil), Some((shown.into(), "evil.example".into())), "{text}");
        }
        assert_eq!(mismatch("(github.com/acme)", "https://github.com/acme"), None);
    }

    #[test]
    fn matching_hosts_and_plain_words_are_not() {
        for (t, u) in [
            ("www.github.com", "https://github.com/"),
            ("GitHub.com/acme", "https://github.com/acme"),
            ("https://github.com.", "https://www.github.com/x"),
            ("docs", "https://evil.example/"),
            ("see the docs", "https://evil.example/"),
            ("v1.2", "https://evil.example/"),
            ("README.md", "https://evil.example/"),
            ("paypal.com", "https://evil.example/"),
            ("g\u{456}thub.com", "https://g\u{456}thub.com/"),
        ] {
            assert_eq!(mismatch(t, u), None, "{t} -> {u}");
        }
    }
}
