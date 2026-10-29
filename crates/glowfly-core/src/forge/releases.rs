//! Glowfly's own releases on GitHub: the newest release of its repository (one unauthenticated
//! request, nothing about the user sent) and the public files attached to it.

use super::http::{call_get, public_request, read_capped};
use crate::app_update::{Release, parse_release};
use crate::error::{Error, Result};

/// The most of the release JSON read.
const MAX_RELEASE_JSON: u64 = 1024 * 1024;

/// The API address of the newest release of `repository`
/// (`https://github.com/<owner>/<repo>`, with or without `.git` or a trailing slash). Other hosts
/// give None.
pub fn releases_api(repository: &str) -> Option<String> {
    let rest = repository.strip_prefix("https://github.com/")?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (owner, repo) = rest.split_once('/')?;
    let ok = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    (ok(owner) && ok(repo) && repo != "." && repo != "..").then(|| format!("https://api.github.com/repos/{owner}/{repo}/releases/latest"))
}

/// The newest published release of `repository`; None when it has none yet.
pub fn latest_release(repository: &str) -> Result<Option<Release>> {
    let url = releases_api(repository).ok_or_else(|| Error::Unsupported(format!("Glowfly can't check {repository} for releases")))?;
    let req = public_request(&url, false)?.set("Accept", "application/vnd.github+json").set("X-GitHub-Api-Version", "2022-11-28");
    match call_get(req) {
        Ok(resp) => Ok(Some(parse_release(&read_capped(resp.into_reader(), MAX_RELEASE_JSON)?)?)),
        Err(Error::NotFound(_)) => Ok(None),
        Err(Error::Forbidden(m)) | Err(Error::Http { status: 429, message: m }) => {
            let said = if m.is_empty() { String::new() } else { format!(" ({m})") };
            Err(Error::Other(format!("GitHub's rate limit for unauthenticated requests was reached{said}; try again later")))
        }
        Err(e) => Err(e),
    }
}

/// The bytes at the public `https://` address `url`, at most `limit` of them.
pub fn fetch_public(url: &str, limit: u64) -> Result<Vec<u8>> {
    let resp = call_get(public_request(url, true)?)?;
    read_capped(resp.into_reader(), limit)
}

/// The response at the public `https://` address `url`, read as it arrives (a release file
/// streamed to disk); the caller limits how much it reads.
pub fn open_public(url: &str) -> Result<Box<dyn std::io::Read + Send>> {
    let resp = call_get(public_request(url, true)?)?;
    Ok(Box::new(resp.into_reader()))
}
