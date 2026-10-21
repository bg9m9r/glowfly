//! Expanding one `IdentityFile` value as ssh does (ssh_config(5) TOKENS): a leading `~`, then
//! `%` tokens and `${VAR}` environment variables in one pass. Values that depend on the host ssh
//! connects to, or that Glowfly can't fill in, are reported instead of guessed. Whether the
//! result is a full path follows a per-OS rule on the text, never the running system's
//! `Path::is_absolute`. Pure, apart from [`from_env`].

use std::path::{Path, PathBuf};

/// Why an IdentityFile value can't be turned into a file path here.
#[derive(Clone, Debug, PartialEq)]
pub enum Unresolved {
    /// A token that depends on the host ssh connects to (`%h`, `%r`, `%p`, `%n`, `%C`, `%k`,
    /// `%j`).
    HostToken(char),
    /// A token Glowfly doesn't fill in (`%L`, `%l`, `%i` without a user id, an unknown one, a
    /// trailing `%`), or `$` for an unterminated `${`.
    Unsupported(char),
    /// `~name/…` for a user other than the current one.
    OtherUser(String),
    /// `${NAME}` with NAME unset.
    Variable(String),
    /// Relative to the folder ssh runs in.
    Relative,
    /// A `/`-rooted path on Windows that isn't an MSYS drive path (`/c/…`).
    NotWindowsPath,
}

/// What expanding a value needs to know about the local user.
pub struct ExpandCx<'a> {
    pub home: &'a Path,
    pub user: &'a str,
    /// The local user id (`%i`); None where there is none (Windows).
    pub uid: Option<u32>,
    pub env: &'a dyn Fn(&str) -> Option<String>,
    /// Decide paths by Windows' rules.
    pub windows: bool,
}

/// The file an IdentityFile value names, or why it can't be known here.
pub fn expand_identity(raw: &str, cx: &ExpandCx) -> Result<PathBuf, Unresolved> {
    let (head, rest) = expand_tilde(raw, cx)?;
    let mut s = head;
    let mut chars = rest.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '%' => match chars.next() {
                None => return Err(Unresolved::Unsupported('%')),
                Some('%') => s.push('%'),
                Some('d') => s.push_str(&cx.home.to_string_lossy()),
                Some('u') => s.push_str(cx.user),
                Some('i') => s.push_str(&cx.uid.ok_or(Unresolved::Unsupported('i'))?.to_string()),
                Some(t @ ('C' | 'h' | 'j' | 'k' | 'n' | 'p' | 'r')) => return Err(Unresolved::HostToken(t)),
                Some(t) => return Err(Unresolved::Unsupported(t)),
            },
            '$' if chars.peek() == Some(&'{') => {
                chars.next();
                let mut name = String::new();
                loop {
                    match chars.next() {
                        None => return Err(Unresolved::Unsupported('$')),
                        Some('}') => break,
                        Some(n) => name.push(n),
                    }
                }
                match (cx.env)(&name) {
                    Some(v) => s.push_str(&v),
                    None => return Err(Unresolved::Variable(name)),
                }
            }
            c => s.push(c),
        }
    }
    absolute_for(&s, cx.windows).map(PathBuf::from)
}

/// A leading `~` (`~`, `~/…`, or `~user/…` for the current user) as the home folder: the text
/// so far, and the rest still to expand.
fn expand_tilde<'r>(raw: &'r str, cx: &ExpandCx) -> Result<(String, &'r str), Unresolved> {
    let Some(after) = raw.strip_prefix('~') else { return Ok((String::new(), raw)) };
    let is_sep = |c: char| c == '/' || (cx.windows && c == '\\');
    let (name, rest) = match after.find(is_sep) {
        Some(i) => (&after[..i], &after[i..]),
        None => (after, ""),
    };
    if !name.is_empty() && name != cx.user {
        return Err(Unresolved::OtherUser(name.to_owned()));
    }
    let mut home = cx.home.to_string_lossy().into_owned();
    if !rest.is_empty() && home.ends_with(is_sep) {
        home.pop();
    }
    Ok((home, rest))
}

/// `s` if it is a full path by `windows`' rules or the Unix ones. On Windows that is `X:/`,
/// `X:\` or a UNC prefix (`//`, `\\`), and an MSYS drive path (`/c/Users/…`, which Git for
/// Windows' ssh accepts) becomes `C:/Users/…`.
pub fn absolute_for(s: &str, windows: bool) -> Result<String, Unresolved> {
    if !windows {
        return if s.starts_with('/') { Ok(s.to_owned()) } else { Err(Unresolved::Relative) };
    }
    let b = s.as_bytes();
    let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\');
    if drive || s.starts_with("//") || s.starts_with(r"\\") {
        return Ok(s.to_owned());
    }
    if let Some(rest) = s.strip_prefix('/') {
        let mut parts = rest.splitn(2, '/');
        let first = parts.next().unwrap_or("");
        if first.len() == 1 && first.as_bytes()[0].is_ascii_alphabetic() {
            return Ok(format!("{}:/{}", first.to_ascii_uppercase(), parts.next().unwrap_or("")));
        }
        return Err(Unresolved::NotWindowsPath);
    }
    Err(Unresolved::Relative)
}

/// The local user's name as ssh's `%u` and `~user` see it: USERNAME on Windows; USER, else
/// LOGNAME, elsewhere. Empty when none is set.
pub fn user_name_for(windows: bool, env: impl Fn(&str) -> Option<String>) -> String {
    let names: &[&str] = if windows { &["USERNAME"] } else { &["USER", "LOGNAME"] };
    names.iter().find_map(|n| env(n).filter(|v| !v.is_empty())).unwrap_or_default()
}

/// The user name and id of the running process.
pub(super) fn from_env() -> (String, Option<u32>) {
    (user_name_for(cfg!(windows), |n| std::env::var(n).ok()), current_uid())
}

#[cfg(unix)]
fn current_uid() -> Option<u32> {
    // SAFETY: getuid has no preconditions and can't fail.
    Some(unsafe { libc::getuid() })
}

#[cfg(not(unix))]
fn current_uid() -> Option<u32> {
    None
}
