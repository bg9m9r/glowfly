//! `core.hooksPath` with a leading `~` expanded as git expands it: `~` and `~/x` from the home
//! folder, `~user/x` from that user's home as the system has it (the password file on Unix;
//! only the current user on Windows, where git can't expand another's). A user whose home
//! can't be found is None: where hooks come from is then unclear, and the caller follows the
//! whole working tree. The path is read by its parts, so one that isn't UTF-8 (Unix) keeps its
//! bytes; a user name that isn't is one whose home can't be found.

use std::path::{Component, Path, PathBuf};

/// `v` expanded; without a home folder it stays as written (a folder named `~` in the working
/// tree).
pub(super) fn home_expanded(v: &Path, home: Option<&Path>) -> Option<PathBuf> {
    let mut parts = v.components();
    let first = match parts.next() {
        Some(Component::Normal(first)) if first.as_encoded_bytes().starts_with(b"~") => first,
        _ => return Some(v.to_path_buf()),
    };
    let Some(home) = home else { return Some(v.to_path_buf()) };
    let user = first.to_str()?.strip_prefix('~')?;
    let base = if user.is_empty() { home.to_path_buf() } else { users_home(user, home)? };
    let rest = parts.as_path();
    Some(if rest.as_os_str().is_empty() { base } else { base.join(rest) })
}

/// The home folder of `user`: `home` for the user running Glowfly, else the system's record.
fn users_home(user: &str, home: &Path) -> Option<PathBuf> {
    let me = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok();
    if me.is_some_and(|m| if cfg!(windows) { m.eq_ignore_ascii_case(user) } else { m == user }) {
        return Some(home.to_path_buf());
    }
    if cfg!(windows) {
        return None;
    }
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().map(|l| l.split(':').collect::<Vec<_>>()).find(|f| f.len() >= 6 && f[0] == user).map(|f| PathBuf::from(f[5]))
}
