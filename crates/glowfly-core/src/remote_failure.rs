//! Why git couldn't reach or read a remote, from its untranslated output (gitcli sets
//! `LANGUAGE=C`): the host can't be reached, the repository isn't there, the account may not see
//! it, or only git's "Could not read from remote repository". git prints that last line
//! whenever its ssh transport fails, so the real cause is looked for on the other lines first.
//! A server certificate git couldn't verify (common behind company proxies that inspect HTTPS)
//! is told apart from an unreachable host: it needs a certificate authority, not a network. So
//! is a revocation check Windows Schannel couldn't finish (the revocation server didn't answer):
//! the certificate may be fine. Certificate and revocation phrases count only on git's own
//! transport lines (curl's "unable to access", "RPC failed; curl" and git-lfs's), never on a
//! hook's `remote:` line or a signing helper's error. Sign-in and host-key failures are
//! `auth_failure`'s. Pure: no I/O.

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
    /// Schannel couldn't finish the revocation check: the revocation server was offline or
    /// unreachable, or there was no revocation information. The certificate may be fine.
    /// `host` as for `Certificate`.
    Revocation { host: Option<String> },
}

/// How curl's TLS backends (OpenSSL, GnuTLS, Schannel, Secure Transport) and Go-based helpers
/// such as git-lfs say a server certificate wasn't trusted, in lowercase. Matched anywhere in a
/// transport line, so the wording around them can differ between git and library versions.
const CERTIFICATE: [&str; 11] = [
    "ssl certificate problem",
    "server certificate verification failed",
    "certificate verify failed",
    "sec_e_untrusted_root",
    "cert_trust_is_untrusted_root",
    "issued by an authority that is not trusted",
    "certificate signed by unknown authority",
    "ssl peer certificate or ssh remote key was not ok",
    "certificate subject name",
    "x509: certificate",
    "tls: failed to verify certificate",
];

/// How curl's Schannel backend says Windows couldn't finish a revocation check, in lowercase:
/// the error names and codes (the description after them is in the Windows UI language, so
/// the codes are what is matched), curl's own chain-check flags, and the English description.
/// CRYPT_E_REVOKED (0x80092010), a certificate that really is revoked, isn't one of them.
const REVOCATION: [&str; 7] = [
    "crypt_e_no_revocation_check",
    "crypt_e_revocation_offline",
    "0x80092012",
    "0x80092013",
    "cert_trust_revocation_status_unknown",
    "cert_trust_is_offline_revocation",
    "the revocation function was unable to check revocation",
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
    let transport: Vec<&str> = lines.iter().copied().filter(|l| transport_line(l)).collect();
    let host = || transport.iter().find_map(|l| access_host(l));
    if transport.iter().any(|l| revocation(l)) {
        return Some(RemoteFailure::Revocation { host: host() });
    }
    if transport.iter().any(|l| certificate(l)) {
        return Some(RemoteFailure::Certificate { host: host() });
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

/// A line that says a revocation check couldn't finish. One that also names an untrusted
/// certificate stays a certificate failure.
fn revocation(line: &str) -> bool {
    let lower = line.to_lowercase();
    REVOCATION.iter().any(|m| lower.contains(m)) && !certificate(line)
}

/// A line of git's own HTTPS transport: curl's "unable to access" and "RPC failed" or a git-lfs
/// line. A hook's `remote:` line never is.
fn transport_line(l: &str) -> bool {
    if l.trim_start().starts_with("remote:") {
        return false;
    }
    l.starts_with("fatal: unable to access '") || l.starts_with("error: RPC failed; curl ") || lfs_line(l)
}

/// A git-lfs transport line: its batch API calls, an `/info/lfs` URL, or its `LFS: ` prefix.
fn lfs_line(l: &str) -> bool {
    ["batch request:", "batch response:", "/info/lfs"].iter().any(|m| l.contains(m)) || l.starts_with("LFS: ")
}

/// The http(s) URL git's transport names in `text` (curl's "unable to access '<url>'", else the
/// first quoted URL of a git-lfs line), for reading the settings that apply to it. Never returns
/// credentials: the user name and password in it are dropped.
pub fn transport_url(text: &str) -> Option<String> {
    let lines = || text.lines().filter(|l| transport_line(l));
    let curl = lines().find_map(|l| quoted_after(l, "unable to access '"));
    let url = curl.or_else(|| lines().filter(|l| lfs_line(l)).find_map(lfs_url))?;
    let (scheme, rest) = url.split_once("://")?;
    if !["http", "https"].iter().any(|s| scheme.eq_ignore_ascii_case(s)) {
        return None;
    }
    let end = rest.find('/').unwrap_or(rest.len());
    let host = match rest[..end].rfind('@') {
        Some(at) => &rest[at + 1..],
        None => rest,
    };
    Some(format!("{scheme}://{host}"))
}

/// The first double-quoted http(s) URL on a git-lfs line.
fn lfs_url(line: &str) -> Option<&str> {
    line.split('"').skip(1).step_by(2).find(|q| {
        let lower = q.to_ascii_lowercase();
        lower.starts_with("http://") || lower.starts_with("https://")
    })
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
