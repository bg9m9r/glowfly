//! Reading git's output for failures that need the user to sign in: a fetch that couldn't ask
//! for a password, a rejected password or token, an SSH key the server refused, or a host key
//! nobody confirmed. The only place that matches that text. Pure: no I/O.

/// What a remote needs before git can reach it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthNeed {
    /// A username and password or token over HTTPS.
    Https,
    /// An SSH key, passphrase or password.
    Ssh,
    /// Confirming the server's SSH host key, which ssh doesn't know yet.
    HostKey,
}

const HTTPS: &[&str] = &[
    "could not read Username for",
    "could not read Password for",
    "terminal prompts disabled",
    "Authentication failed for",
    "The requested URL returned error: 401",
    "user interactivity has been disabled",
];
const SSH: &[&str] = &["Permission denied, please try again", "Too many authentication failures"];
/// The authentication methods of ssh's "Permission denied (publickey,password)." that a user
/// can satisfy by signing in.
const SSH_METHODS: &[&str] = &["publickey", "password", "keyboard-interactive"];
const HOST_KEY: &str = "Host key verification failed";
/// A host key that no longer matches: possibly an attack, never offered as a prompt.
const HOST_KEY_CHANGED: &str = "REMOTE HOST IDENTIFICATION HAS CHANGED";

/// What `text` says the remote needs, if it failed for want of a sign-in. None whenever ssh
/// reports a changed host key.
pub fn need(text: &str) -> Option<AuthNeed> {
    if host_key_changed(text) {
        return None;
    }
    if text.contains(HOST_KEY) {
        return Some(AuthNeed::HostKey);
    }
    if SSH.iter().any(|m| text.contains(m)) || text.lines().any(ssh_denied) {
        return Some(AuthNeed::Ssh);
    }
    HTTPS.iter().any(|m| text.contains(m)).then_some(AuthNeed::Https)
}

/// Whether ssh reports that the server's host key no longer matches the one it knows.
pub fn host_key_changed(text: &str) -> bool {
    text.contains(HOST_KEY_CHANGED)
}

/// ssh's "Permission denied (publickey,password)." for a method the user can sign in with.
fn ssh_denied(line: &str) -> bool {
    let Some((_, rest)) = line.split_once("Permission denied (") else { return false };
    let methods = rest.split(')').next().unwrap_or("");
    methods.split(',').any(|m| SSH_METHODS.contains(&m.trim()))
}

/// The URLs of git's "Authentication failed for '<url>'" lines.
pub fn rejected_urls(output: &str) -> Vec<String> {
    output.lines().filter_map(|l| quoted_after(l, "Authentication failed for '")).map(str::to_owned).collect()
}

/// Whether the server turned down the credentials git sent (not just asked for some).
pub fn credentials_rejected(output: &str) -> bool {
    ["Authentication failed for '", "The requested URL returned error: 401", "HTTP Basic: Access denied"].iter().any(|m| output.contains(m))
}

/// The text between `prefix` and the next `'` in `line`.
pub(crate) fn quoted_after<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = &line[line.find(prefix)? + prefix.len()..];
    rest.split_once('\'').map(|(q, _)| q)
}

/// The lowercase host (with any port) of `scheme://[user@]host[:port]/…` or of scp-like
/// `[user@]host:path`; None for a local path.
pub fn url_host(url: &str) -> Option<String> {
    if url.contains("://") {
        return crate::askpass::user_and_host(url).1;
    }
    let (authority, _) = url.split_once(':')?;
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    // A drive letter ("C:/repo") or a path with a colon later on is not a host.
    let local = host.contains(['/', '\\']) || (host.len() == 1 && host.chars().all(|c| c.is_ascii_alphabetic()));
    (!host.is_empty() && !local).then(|| host.to_ascii_lowercase())
}

/// `host` without a `:port`.
pub(crate) fn bare(host: &str) -> &str {
    match host.rsplit_once(':') {
        Some((h, port)) if !h.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => h,
        _ => host,
    }
}

/// The host an error line names: the URL in `for '<url>'`, or the `<user>@<host>` before ssh's
/// ": Permission denied".
fn line_host(line: &str) -> Option<String> {
    if let Some(url) = quoted_after(line, "for '") {
        return url_host(url);
    }
    let (before, _) = line.split_once(": ")?;
    let word = before.split_whitespace().last()?;
    let (_, host) = word.rsplit_once('@')?;
    Some(host.to_ascii_lowercase()).filter(|h| !h.is_empty())
}

/// The remote named by git's "could not fetch '<name>' (exit code: …)" or older
/// "error: could not fetch <name>".
fn could_not_fetch(line: &str) -> Option<&str> {
    let rest = &line[line.find("could not fetch ")? + "could not fetch ".len()..];
    let name = match rest.strip_prefix('\'') {
        Some(r) => r.split('\'').next()?,
        None => rest.split_whitespace().next()?,
    };
    Some(name)
}

/// A remote whose fetch failed for want of a sign-in.
#[derive(Debug, PartialEq)]
pub struct FetchFailure {
    pub remote: String,
    pub need: AuthNeed,
}

/// The remotes in `remotes` (name, fetch URL) that a fetch's `output` says need a sign-in, in
/// `remotes` order. Output under a remote's "Fetching <name>" header belongs to that remote.
/// Anything else is attributed by the host the error names, then by git's "could not fetch"
/// line, then to the only remote fetched.
pub fn fetch_failures(output: &str, remotes: &[(String, String)]) -> Vec<FetchFailure> {
    let mut found: Vec<(&str, AuthNeed)> = Vec::new();
    let mut outside = Vec::new();
    let mut groups: Vec<(&str, String)> = Vec::new();
    for line in output.lines() {
        let header = line.strip_prefix("Fetching ").map(str::trim).and_then(|n| remotes.iter().find(|(r, _)| r == n));
        match (header, groups.last_mut()) {
            (Some((name, _)), _) => groups.push((name.as_str(), String::new())),
            (None, Some((_, text))) => {
                text.push_str(line);
                text.push('\n');
            }
            (None, None) => outside.push(line),
        }
    }
    for (name, text) in &groups {
        if let Some(n) = need(text) {
            found.push((*name, n));
        }
    }
    let text = outside.join("\n");
    if need(&text).is_some() {
        let named: Vec<&str> = outside.iter().filter_map(|l| could_not_fetch(l)).filter(|n| remotes.iter().any(|(r, _)| r == n)).collect();
        let fallback = match (named.as_slice(), remotes) {
            ([one], _) => Some(*one),
            (_, [(only, _)]) => Some(only.as_str()),
            _ => None,
        };
        for line in &outside {
            let Some(n) = need(line) else { continue };
            let by_host: Vec<&str> = line_host(line)
                .map(|h| {
                    let wanted = bare(&h).to_owned();
                    remotes.iter().filter(|(_, url)| url_host(url).is_some_and(|u| bare(&u) == wanted)).map(|(r, _)| r.as_str()).collect()
                })
                .unwrap_or_default();
            let remote = match by_host.as_slice() {
                [one] => Some(*one),
                _ => fallback,
            };
            found.extend(remote.map(|r| (r, n)));
        }
    }
    remotes
        .iter()
        .filter_map(|(name, _)| found.iter().find(|(r, _)| r == name).map(|(_, n)| FetchFailure { remote: name.clone(), need: *n }))
        .collect()
}

#[cfg(test)]
#[path = "auth_failure_tests.rs"]
mod tests;
