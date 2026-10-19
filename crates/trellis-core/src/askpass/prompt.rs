//! What a git or ssh prompt asks for, decided from its text. Pure: no I/O.

/// The kinds of prompt the credential dialog distinguishes.
#[derive(Clone, Debug, PartialEq)]
pub enum PromptKind {
    /// git's HTTPS username prompt for `url` (with the path when `credential.useHttpPath` is
    /// set).
    Username { url: String },
    /// A password: git's HTTPS prompt (`target` is the URL) or ssh's (`target` is
    /// `user@host`). `user` is the URL's percent-decoded user or the part before `@`.
    Password { target: String, user: Option<String> },
    /// ssh asks for the passphrase of the private key at `key`.
    Passphrase { key: String },
    /// The signing wrapper asks for the passphrase of OpenPGP key `key` (long id or git's key
    /// spec), owned by `user`; `retry` when the last answer didn't unlock it. Only built from a
    /// structured request ([`super::Prompt::gpg`]), never by [`classify`].
    GpgPassphrase { key: String, user: Option<String>, retry: bool },
    /// A yes/no question that isn't in a wording Trellis reads, such as an agent's "Allow use
    /// of key?".
    Confirm,
    /// ssh's question about a server it has no host key for, read into fields
    /// ([`super::host_key::parse`]). Answered like [`PromptKind::Confirm`].
    HostKey(super::host_key::HostKey),
    /// Anything else; `secret` when the text mentions a password, PIN, token or the like.
    Other { secret: bool },
}

/// Classifies a prompt. `hint` is ssh's `SSH_ASKPASS_PROMPT` (`confirm` for yes/no questions).
pub fn classify(text: &str, hint: Option<&str>) -> PromptKind {
    if hint == Some("confirm") || text.contains("(yes/no") {
        return super::host_key::parse(text).map_or(PromptKind::Confirm, PromptKind::HostKey);
    }
    let t = text.trim_end();
    let t = t.strip_suffix(':').unwrap_or(t).trim_end();
    if let Some(rest) = t.strip_prefix("Username for '") {
        return PromptKind::Username { url: unquote_end(rest).to_owned() };
    }
    if let Some(rest) = t.strip_prefix("Password for '") {
        let target = unquote_end(rest).to_owned();
        let user = user_and_host(&target).0;
        return PromptKind::Password { target, user };
    }
    if let Some(rest) = t.strip_prefix("Enter passphrase for key '") {
        return PromptKind::Passphrase { key: unquote_end(rest).to_owned() };
    }
    if let Some(rest) = t.strip_prefix("Enter passphrase for ") {
        return PromptKind::Passphrase { key: rest.trim().trim_matches(['\'', '"']).to_owned() };
    }
    if let Some(target) = t.strip_suffix("'s password") {
        let target = target.trim().to_owned();
        let user = target.split_once('@').map(|(u, _)| u.to_owned()).filter(|u| !u.is_empty());
        return PromptKind::Password { target, user };
    }
    let lower = text.to_lowercase();
    let secret = ["password", "passphrase", "pin", "token", "secret"].iter().any(|w| lower.contains(w));
    PromptKind::Other { secret }
}

/// `rest` up to its closing quote (or all of it, when there is none).
fn unquote_end(rest: &str) -> &str {
    rest.rfind('\'').map_or(rest, |i| &rest[..i])
}

/// The host a Username or Password prompt is for (lowercase, with any port), if it names one.
pub fn host(kind: &PromptKind) -> Option<String> {
    match kind {
        PromptKind::Username { url } => user_and_host(url).1,
        PromptKind::Password { target, .. } => user_and_host(target).1,
        _ => None,
    }
}

/// The percent-decoded user and the lowercase host of `scheme://[user[:pass]@]host[:port]/path`
/// or of `user@host`.
pub fn user_and_host(url: &str) -> (Option<String>, Option<String>) {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split('/').next().unwrap_or("");
    let (user, host) = match authority.rsplit_once('@') {
        Some((u, h)) => (Some(u), h),
        None => (None, authority),
    };
    let user = user.map(|u| percent_decode(u.split(':').next().unwrap_or(u))).filter(|u| !u.is_empty());
    let host = Some(host.to_ascii_lowercase()).filter(|h| !h.is_empty());
    (user, host)
}

/// Decodes `%XX` escapes; anything malformed is kept as it is.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
