//! Why git couldn't reach or read a remote, from its untranslated output (gitcli sets
//! `LANGUAGE=C`): the host can't be reached, the repository isn't there, the account may not see
//! it, or only git's "Could not read from remote repository". git prints that last line
//! whenever its ssh transport fails, so the real cause is looked for on the other lines first.
//! A server certificate git couldn't verify (common behind company proxies that inspect HTTPS)
//! is told apart from an unreachable host: it needs a certificate authority, not a network.
//! Sign-in and host-key failures are `auth_failure`'s. Pure: no I/O.

use crate::auth_failure::{bare, quoted_after, url_host};

/// Why git couldn't reach or read the remote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteFailure {
    /// The host didn't answer: its name didn't resolve, or the connection failed or timed out.
    /// `host` is the server's name when the output gives it.
    Unreachable { host: Option<String> },
    /// The remote repository isn't there (or isn't visible to the account).
    NotFound,
    /// The server refused the account access to the repository.
    NoAccess,
    /// Only git's "Could not read from remote repository." with no clearer cause.
    Unreadable,
    /// The TLS connection failed because git couldn't verify the server's certificate (an
    /// unknown or self-signed authority, an expired certificate, a name that doesn't match).
    /// `host` is the server's name when the output gives it.
    Certificate { host: Option<String> },
}

/// How curl's TLS backends (OpenSSL, GnuTLS, Schannel, Secure Transport) and Go-based helpers
/// such as git-lfs say a server certificate wasn't trusted, in lowercase. Matched anywhere in a
/// line, so the wording around them can differ between git and library versions.
const CERTIFICATE: [&str; 9] = [
    "ssl certificate problem",
    "server certificate verification failed",
    "certificate verify failed",
    "sec_e_untrusted_root",
    "cert_trust_is_untrusted_root",
    "issued by an authority that is not trusted",
    "certificate signed by unknown authority",
    "ssl peer certificate or ssh remote key was not ok",
    "certificate subject name",
];

/// The phrases of curl's and ssh's failed name lookups and connections that name the host
/// right after them, each with the text that must follow.
const NAMED: [(&str, Option<&str>); 4] = [
    ("Could not resolve hostname ", None),
    ("Could not resolve host: ", None),
    ("Failed to connect to ", Some(" port ")),
    ("connect to host ", Some(" port ")),
];
/// A connection that failed without naming the host.
const UNNAMED: [&str; 6] = [
    "Connection timed out",
    "Operation timed out",
    "Connection refused",
    "Network is unreachable",
    "No route to host",
    "Couldn't connect to server",
];
const NOT_FOUND: [&str; 4] = [
    "Repository not found",
    "does not appear to be a git repository",
    "The project you were looking for could not be found",
    "The requested URL returned error: 404",
];

/// Why git couldn't reach or read a remote, from its untranslated output; None when it says
/// nothing about that.
pub fn read(text: &str) -> Option<RemoteFailure> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.iter().any(|l| certificate(l)) {
        return Some(RemoteFailure::Certificate { host: lines.iter().find_map(|l| access_host(l)) });
    }
    let named = lines.iter().find_map(|l| named_host(l));
    if named.is_some() || lines.iter().any(|l| UNNAMED.iter().any(|m| l.contains(m))) {
        let host = named.or_else(|| lines.iter().find_map(|l| access_host(l)));
        return Some(RemoteFailure::Unreachable { host });
    }
    if lines.iter().any(|l| NOT_FOUND.iter().any(|m| l.contains(m)) || repository_not_found(l)) {
        return Some(RemoteFailure::NotFound);
    }
    let denied = |l: &str| l.contains("Permission to ") && l.contains(" denied to ");
    if lines.iter().any(|l| l.contains("The requested URL returned error: 403") || denied(l)) {
        return Some(RemoteFailure::NoAccess);
    }
    lines.iter().any(|l| l.contains("Could not read from remote repository")).then_some(RemoteFailure::Unreadable)
}

/// A line that says the server's certificate wasn't trusted.
fn certificate(line: &str) -> bool {
    let lower = line.to_lowercase();
    CERTIFICATE.iter().any(|m| lower.contains(m))
}

/// The host a failed lookup or connection on `line` names.
fn named_host(line: &str) -> Option<String> {
    NAMED.iter().find_map(|(phrase, then)| {
        let rest = &line[line.find(phrase)? + phrase.len()..];
        if then.is_some_and(|t| !rest.contains(t)) {
            return None;
        }
        let host = rest.trim_start().split(|c: char| c.is_whitespace() || c == ':' || c == '\'').next()?;
        (!host.is_empty()).then(|| host.to_ascii_lowercase())
    })
}

/// The host of the URL in curl's "unable to access '<url>'", without its port.
fn access_host(line: &str) -> Option<String> {
    let host = url_host(quoted_after(line, "unable to access '")?)?;
    Some(bare(&host).to_owned())
}

/// git's "repository '<url>' not found".
fn repository_not_found(line: &str) -> bool {
    const OPEN: &str = "repository '";
    line.find(OPEN).is_some_and(|i| line[i + OPEN.len()..].contains("' not found"))
}

#[cfg(test)]
#[path = "remote_failure_tests.rs"]
mod tests;
