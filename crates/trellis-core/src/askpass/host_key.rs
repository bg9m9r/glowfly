//! ssh's question about a server it has no host key for, read into fields. Pure: no I/O.
//!
//! OpenSSH asks, before it first trusts a server:
//!
//! ```text
//! The authenticity of host 'example.com (192.0.2.10)' can't be established.
//! ED25519 key fingerprint is SHA256:Xq7f….
//! Are you sure you want to continue connecting (yes/no/[fingerprint])?
//! ```
//!
//! [`parse`] picks the host, the address, the key type and the fingerprint out of that text so
//! the dialog can show them as fields. Other wordings return `None` and are shown as they are.

/// The server and key named by ssh's unknown-host question.
#[derive(Clone, Debug, PartialEq)]
pub struct HostKey {
    /// The host name (or address) that was connected to, without brackets or port.
    pub host: String,
    /// The port, when ssh shows one (it does only for a non-default port).
    pub port: Option<u16>,
    /// The IP address ssh resolved, when it differs from `host`.
    pub address: Option<String>,
    /// The key's algorithm as ssh names it, such as `ED25519`.
    pub key_type: String,
    /// The key's fingerprint, such as `SHA256:…`.
    pub fingerprint: String,
}

const HOST_LINE: &str = "The authenticity of host '";
const FINGERPRINT: &str = " key fingerprint is ";

/// Reads OpenSSH's unknown-host question, or `None` when `text` isn't in that wording (or
/// lacks the host or the fingerprint).
pub fn parse(text: &str) -> Option<HostKey> {
    // A key that differs from the one recorded for the address is a different question.
    if text.contains("differs from the key for the IP address") {
        return None;
    }
    let mut names: Option<(String, Option<u16>, Option<String>)> = None;
    let mut key: Option<(String, String)> = None;
    for line in text.lines().map(str::trim) {
        if names.is_none()
            && let Some(rest) = line.strip_prefix(HOST_LINE)
        {
            names = rest.split_once('\'').and_then(|(quoted, _)| host_and_address(quoted));
        } else if key.is_none()
            && let Some((kind, fp)) = line.split_once(FINGERPRINT)
        {
            let fp = fp.trim();
            let fp = fp.strip_suffix('.').unwrap_or(fp).trim();
            if !fp.is_empty() {
                key = Some((kind.trim().to_owned(), fp.to_owned()));
            }
        }
    }
    let (host, port, address) = names?;
    let (key_type, fingerprint) = key?;
    Some(HostKey { host, port, address, key_type, fingerprint })
}

/// `NAME` or `NAME (ADDR)`, each optionally `[host]:port`.
fn host_and_address(quoted: &str) -> Option<(String, Option<u16>, Option<String>)> {
    let quoted = quoted.trim();
    let (name, addr) = match quoted.strip_suffix(')').and_then(|q| q.rsplit_once(" (")) {
        Some((name, addr)) => (name, Some(addr)),
        None => (quoted, None),
    };
    let (host, port) = bracketed(name.trim());
    if host.is_empty() {
        return None;
    }
    let address = addr.map(|a| bracketed(a.trim()).0).filter(|a| !a.is_empty() && *a != host).map(str::to_owned);
    Some((host.to_owned(), port, address))
}

/// `[h]:p` as `(h, Some(p))`; anything else as `(s, None)`.
fn bracketed(s: &str) -> (&str, Option<u16>) {
    if let Some(rest) = s.strip_prefix('[')
        && let Some((h, p)) = rest.split_once("]:")
        && let Ok(p) = p.parse()
    {
        return (h, Some(p));
    }
    (s, None)
}
