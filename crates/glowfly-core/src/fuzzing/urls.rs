//! Checks for remote URL parsing (which service, host and repository a remote names) and for
//! the redaction of credentials in URLs before a command is logged.

use super::text;
use crate::forge::{Account, Connection, Provider, https_host, parse_host, parse_remote, split_url};
use crate::text::redact_credentials as redact;

/// The schemes a URL with a password in it is built with.
const SCHEMES: [&str; 8] = ["https", "http", "HTTPS", "ftp", "ssh", "git+ssh", "file", "x-custom"];

/// A remote URL, read with no saved connection and with one self-hosted server. Invariants: no
/// panic in any of the URL readers; a recognized remote's host is the whole authority of a
/// URL built from it, lowercased the same way, and its decoded names hold no line break or NUL
/// (either would move a request or add a line to a credential request).
pub fn remote_url(data: &[u8]) {
    let url = text(data);
    let _ = split_url(&url);
    let _ = crate::link::url_key(&url);
    for p in Provider::ALL {
        let _ = parse_host(p, &url);
    }
    let server = Connection {
        account: Account { provider: Provider::BitbucketDataCenter, host: "git.example.com".into(), org: String::new() },
        user: "u".into(),
        method: "token".into(),
    };
    for connections in [&[][..], &[server]] {
        let Some(t) = parse_remote(&url, connections) else { continue };
        assert_eq!(https_host(&format!("https://{}/x", t.host)), Some(t.host.to_ascii_lowercase()), "{url:?} gave {t:?}");
        for name in [&t.owner, &t.project, &t.repo] {
            assert!(!name.contains(['\n', '\r', '\0']), "{url:?} gave {t:?}");
        }
    }
}

/// Text holding a URL with a password: the bytes split at U+0001 into the text before it, the
/// scheme (picked from `SCHEMES` by length), the user, the password and what follows the `@`.
/// The text before ends in a space, as a command's arguments do. Invariants: no panic; the
/// password never shows in the redacted text; and, for the bytes as they are, text without
/// `://` is left as it is and redacting again changes nothing.
pub fn redact_credentials(data: &[u8]) {
    let whole = text(data);
    let once = redact(&whole);
    assert_eq!(redact(&once), once, "{whole:?}");
    if !whole.contains("://") {
        assert_eq!(once, whole);
    }
    let mut parts = whole.split('\u{1}');
    let mut next = || parts.next().unwrap_or_default();
    let (before, scheme, user, password, after) = (next(), next(), next(), next(), next());
    // What ends a URL's user part, and the `*` of the mask (which could spell the password).
    let ends = |c: &char| "/?#@*".contains(*c) || c.is_whitespace();
    let user: String = user.chars().filter(|c| !ends(c)).collect();
    let password: String = password.chars().filter(|c| !ends(c)).collect();
    let scheme = SCHEMES[scheme.len() % SCHEMES.len()];
    let before = if before.is_empty() { String::new() } else { format!("{before} ") };
    let line = format!("{before}{scheme}://{user}:{password}@{after}");
    // A password that shows elsewhere in the text (or is empty) can't be told apart.
    if password.is_empty() || line.matches(password.as_str()).count() != 1 {
        return;
    }
    let shown = redact(&line);
    assert!(!shown.contains(password.as_str()), "{line:?} was logged as {shown:?}");
}
