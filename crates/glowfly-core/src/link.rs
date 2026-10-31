//! `glowfly://` links: a repository, a branch, a commit or a tag that someone else can open in
//! their own copy of the repository.
//!
//! The format:
//!
//! - `glowfly://repo?url=<url>&root=<sha>`
//! - `glowfly://branch/<name>?url=<url>&root=<sha>`
//! - `glowfly://commit/<full sha>?url=<url>&root=<sha>`
//! - `glowfly://tag/<name>?url=<url>&root=<sha>`
//!
//! Names and values are percent-encoded, everything but letters, digits and `-._~` escaped, so
//! a `/` in a branch name is `%2F` and the name stays one path segment.
//!
//! A link names its repository two ways, and needs at least one: `url`, a remote it can be
//! fetched or cloned from (compared by [`same_url`], so the https, ssh and scp spellings of one
//! repository match), and `root`, the commit at the end of the first-parent chain from HEAD,
//! which every full clone of the repository shares whatever its remotes are called. A link never
//! carries a folder path.
//!
//! [`parse`] refuses: another scheme, a kind this version doesn't know (a newer Glowfly made
//! it), a missing name or sha, a sha that isn't 7 to 64 hex digits, a root that isn't 40 or 64,
//! a known parameter given twice, a link with neither url nor root, bad percent-encoding or
//! UTF-8, control characters, a ref name git would refuse, a url that is empty, starts with `-`
//! or uses git's `ext::` or `fd::` transports, a url that names a local or network file path
//! rather than a remote ([`is_remote_url`]), and text over [`MAX_LINK`]. Unknown parameters
//! and a fragment are ignored, so newer links still open.

pub mod find;

#[cfg(test)]
mod find_tests;
#[cfg(test)]
mod tests;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

/// The longest link text [`parse`] reads.
pub const MAX_LINK: usize = 8192;

/// Everything but the unreserved characters of RFC 3986.
const ESCAPED: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'.').remove(b'_').remove(b'~');

/// What a link points at inside its repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Repository,
    /// A branch name, without `refs/heads/` (or a remote's name for it).
    Branch(String),
    /// A commit's sha in lowercase hex: full in links Glowfly makes, abbreviated (7 or more
    /// digits) in typed ones.
    Commit(String),
    /// A tag name, without `refs/tags/`.
    Tag(String),
}

/// A parsed link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// A remote URL the repository can be fetched from.
    pub url: Option<String>,
    /// The root commit (lowercase hex).
    pub root: Option<String>,
    pub target: Target,
}

/// Why a text isn't a link Glowfly can open. Decide behaviour from the variant, never the
/// message.
#[derive(thiserror::Error, Debug, PartialEq)]
pub enum LinkError {
    #[error("it isn't a glowfly:// link")]
    NotALink,
    #[error("it points at a \"{0}\", which this version of Glowfly doesn't know (was it made by a newer Glowfly?)")]
    UnknownKind(String),
    #[error("it doesn't say which branch, commit or tag it points at")]
    MissingName,
    #[error("\"{0}\" isn't a commit id (7 to 64 hexadecimal digits)")]
    BadSha(String),
    #[error("its root commit isn't a full commit id")]
    BadRoot,
    #[error("\"{0}\" isn't a name git accepts for a branch or tag")]
    BadRefName(String),
    #[error("it doesn't say which repository it belongs to (no url or root)")]
    NoRepository,
    #[error("its remote URL can't be used")]
    BadUrl,
    /// The url is a file path (local, UNC or `file:`) rather than a remote. Hardening: a link
    /// arrives from outside, and has no reason to point Glowfly at a path.
    #[error("its URL is a file or network path, not a remote URL")]
    LocalUrl,
    #[error("it gives \"{0}\" more than once")]
    Duplicate(&'static str),
    #[error("it contains characters that aren't allowed or aren't encoded properly")]
    BadEncoding,
    #[error("it is longer than {MAX_LINK} characters")]
    TooLong,
}

fn enc(s: &str) -> String {
    utf8_percent_encode(s, ESCAPED).to_string()
}

/// Percent-decodes `s` into UTF-8 without control or bidi formatting characters.
fn dec(s: &str) -> Result<String, LinkError> {
    let bytes: Vec<u8> = percent_decode_str(s).collect();
    // A `%` that doesn't start an escape is passed through by the decoder; refuse it here.
    let escapes_ok = s
        .bytes()
        .enumerate()
        .filter(|(_, b)| *b == b'%')
        .all(|(i, _)| s.as_bytes().get(i + 1..i + 3).is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit)));
    let text = String::from_utf8(bytes).map_err(|_| LinkError::BadEncoding)?;
    // Bidi controls could make a name or URL read differently from what it is.
    if !escapes_ok || text.chars().any(|c| c.is_control() || crate::text::is_bidi_control(c)) {
        return Err(LinkError::BadEncoding);
    }
    Ok(text)
}

fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Link {
    /// The link's text. `url` comes before `root`, and a missing one is left out, so building,
    /// parsing and building again gives the same text.
    pub fn to_uri(&self) -> String {
        let mut s = String::from("glowfly://");
        match &self.target {
            Target::Repository => s.push_str("repo"),
            Target::Branch(n) => s.push_str(&format!("branch/{}", enc(n))),
            Target::Commit(sha) => s.push_str(&format!("commit/{}", enc(sha))),
            Target::Tag(n) => s.push_str(&format!("tag/{}", enc(n))),
        }
        let params = [("url", &self.url), ("root", &self.root)];
        for (i, (key, value)) in params.iter().filter_map(|(k, v)| v.as_ref().map(|v| (k, v))).enumerate() {
            s.push(if i == 0 { '?' } else { '&' });
            s.push_str(&format!("{key}={}", enc(value)));
        }
        s
    }

    /// What the link points at, for messages: "branch feature/x", "commit abc1234", "tag v1",
    /// "repository".
    pub fn describe(&self) -> String {
        match &self.target {
            Target::Repository => "repository".to_owned(),
            Target::Branch(n) => format!("branch {n}"),
            Target::Commit(sha) => format!("commit {}", &sha[..sha.len().min(7)]),
            Target::Tag(n) => format!("tag {n}"),
        }
    }

    /// Which repository the link names, for messages: its URL, else "the repository with root
    /// commit abc1234".
    pub fn repo_label(&self) -> String {
        match (&self.url, &self.root) {
            (Some(url), _) => url.clone(),
            (None, Some(root)) => format!("the repository with root commit {}", &root[..root.len().min(7)]),
            (None, None) => "an unknown repository".to_owned(),
        }
    }
}

/// Whether `scheme` is ours, `glowfly`, in any case.
fn is_our_scheme(scheme: &str) -> bool {
    scheme.eq_ignore_ascii_case(crate::url_scheme::SCHEME)
}

/// Whether a command-line argument is a link rather than a path: it starts with `glowfly:`, in
/// any case. An argument that isn't UTF-8 is a path.
pub fn looks_like_link(arg: &std::ffi::OsStr) -> bool {
    arg.to_str().and_then(|s| s.split_once(':')).is_some_and(|(scheme, _)| is_our_scheme(scheme))
}

/// Reads a link's text (see the module docs for what is refused).
pub fn parse(text: &str) -> Result<Link, LinkError> {
    if text.len() > MAX_LINK {
        return Err(LinkError::TooLong);
    }
    let text = text.trim();
    if text.chars().any(char::is_control) {
        return Err(LinkError::BadEncoding);
    }
    let (scheme, rest) = text.split_once("://").ok_or(LinkError::NotALink)?;
    if !is_our_scheme(scheme) {
        return Err(LinkError::NotALink);
    }
    let rest = rest.split_once('#').map_or(rest, |(before, _)| before);
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let path = path.strip_suffix('/').unwrap_or(path);
    let (kind, name) = match path.split_once('/') {
        Some((k, n)) => (k, Some(dec(n)?)),
        None => (path, None),
    };
    let name = name.filter(|n| !n.is_empty());
    let (mut url, mut root) = (None, None);
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let slot = match key {
            "url" => &mut url,
            "root" => &mut root,
            _ => continue, // from a newer Glowfly
        };
        if slot.is_some() {
            return Err(LinkError::Duplicate(if key == "url" { "url" } else { "root" }));
        }
        *slot = Some(dec(value)?);
    }
    let target = match kind.to_ascii_lowercase().as_str() {
        "repo" => Target::Repository,
        "branch" => Target::Branch(ref_name(name, "refs/heads/")?),
        "tag" => Target::Tag(ref_name(name, "refs/tags/")?),
        "commit" => {
            let sha = name.ok_or(LinkError::MissingName)?;
            if !is_hex(&sha) || !(7..=64).contains(&sha.len()) {
                return Err(LinkError::BadSha(sha));
            }
            Target::Commit(sha.to_ascii_lowercase())
        }
        _ => return Err(LinkError::UnknownKind(kind.to_owned())),
    };
    if let Some(r) = &root
        && (!is_hex(r) || !matches!(r.len(), 40 | 64))
    {
        return Err(LinkError::BadRoot);
    }
    if let Some(u) = &url {
        let lower = u.to_ascii_lowercase();
        if u.trim().is_empty() || u.starts_with('-') || lower.starts_with("ext::") || lower.starts_with("fd::") {
            return Err(LinkError::BadUrl);
        }
        if !is_remote_url(u) {
            return Err(LinkError::LocalUrl);
        }
    }
    if url.is_none() && root.is_none() {
        return Err(LinkError::NoRepository);
    }
    Ok(Link { url, root: root.map(|r| r.to_ascii_lowercase()), target })
}

/// Whether `url` names a remote git reaches over the network rather than a file path: a
/// `scheme://` URL other than `file:`, a remote helper's `<transport>::<address>` whose address
/// passes this check too, or git's scp form `[user@]host:path`. Paths are refused: UNC paths
/// (`\\host\share`, `//host/share` and their `\\?\` and `\\.\` device forms), `file:` in any
/// case, drive paths (`C:\x`, `C:x`), absolute, relative and `~` paths, and anything else git
/// would read as a path (no `:` before the first `/` or `\`). Hardening for links, which come
/// from outside; a URL typed into the clone dialog isn't checked by this.
pub fn is_remote_url(url: &str) -> bool {
    let u = url.trim();
    let starts_as_path = u.starts_with(['/', '\\', '.', '~']);
    if u.is_empty() || starts_as_path || u.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("file:")) {
        return false;
    }
    let Some(colon) = u.find(':') else { return false };
    let before = &u[..colon];
    // A separator before the colon makes it a path with a colon further on; one letter is a
    // drive, never a host.
    if before.contains(['/', '\\']) || (before.len() == 1 && before.as_bytes()[0].is_ascii_alphabetic()) {
        return false;
    }
    let is_scheme = before.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.'));
    let rest = &u[colon + 1..];
    if let Some(address) = rest.strip_prefix(':') {
        // A `transport::address` helper: the address mustn't be a path either.
        return is_remote_url(address);
    }
    match rest.strip_prefix("//") {
        // `scheme://host…`: the host must be there, and not start another path.
        Some(after) if is_scheme => !after.is_empty() && !after.starts_with(['/', '\\']),
        // The scp form `host:path`; a `\` straight after the colon only reads as a path.
        _ => !rest.starts_with('\\'),
    }
}

/// A branch or tag name git accepts under `prefix`.
fn ref_name(name: Option<String>, prefix: &str) -> Result<String, LinkError> {
    let name = name.ok_or(LinkError::MissingName)?;
    if git2::Reference::is_valid_name(&format!("{prefix}{name}")) { Ok(name) } else { Err(LinkError::BadRefName(name)) }
}

/// The form two URLs of one repository share. Network URLs (https, http, ssh and git's scp
/// form) become `host/path` lowercased, without a user, port (ssh), trailing `/` or `.git`, and
/// with the services' ssh host aliases mapped to their web host. Anything else (a local path,
/// `file://`) is the trimmed text without a trailing `/` or `.git`, with `/` separators,
/// lowercased.
pub fn url_key(url: &str) -> String {
    if let Some((host, segs, _ssh)) = crate::forge::split_url(url) {
        let mut key = host;
        for s in segs {
            key.push('/');
            key.push_str(&s);
        }
        return key.to_lowercase();
    }
    let t = url.trim();
    let t = t.strip_prefix("file://").unwrap_or(t).replace('\\', "/");
    // file:///C:/x names the local path C:/x.
    let t = match t.strip_prefix('/') {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') && rest.as_bytes()[0].is_ascii_alphabetic() => rest.to_owned(),
        _ => t,
    };
    let t = t.trim_end_matches('/');
    let t = t.strip_suffix(".git").unwrap_or(t).trim_end_matches('/');
    t.to_lowercase()
}

/// `url` as a link may carry it, without credentials: an http(s) or ftp(s) URL loses its whole
/// user part (a token is often given as the user name), any other `scheme://user:password@`
/// URL its password. An ssh user (`ssh://git@host`, `git@host:o/r`) is kept, since the clone
/// needs it, and [`same_url`] ignores the user anyway.
pub fn shareable_url(url: &str) -> String {
    let Some(i) = url.find("://") else { return url.to_owned() };
    let scheme = url[..i].to_ascii_lowercase();
    let rest = &url[i + 3..];
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let Some(at) = rest[..end].rfind('@') else { return url.to_owned() };
    let user = if scheme.starts_with("http") || scheme.starts_with("ftp") {
        ""
    } else {
        let name = &rest[..at];
        &name[..name.find(':').unwrap_or(name.len())]
    };
    let host = &rest[at + 1..];
    if user.is_empty() { format!("{}{host}", &url[..i + 3]) } else { format!("{}{user}@{host}", &url[..i + 3]) }
}

/// Whether two remote URLs name the same repository (see [`url_key`]).
pub fn same_url(a: &str, b: &str) -> bool {
    let (a, b) = (url_key(a), url_key(b));
    !a.is_empty() && a == b
}
