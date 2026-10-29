//! `core.hooksPath` with a leading `~` expanded as git expands it: `~` and `~/x` from the home
//! folder, `~user/x` from that user's home as the system has it (the password file on Unix;
//! only the current user on Windows, where git can't expand another's). A user whose home
//! can't be found is None: where hooks come from is then unclear, and the caller follows the
//! whole working tree.

use std::path::{Path, PathBuf};

/// `v` expanded; without a home folder it stays as written (a folder named `~` in the working
/// tree).
pub(super) fn home_expanded(v: &str, home: Option<&Path>) -> Option<PathBuf> {
    let Some(rest) = v.strip_prefix('~') else { return Some(PathBuf::from(v)) };
    let Some(home) = home else { return Some(PathBuf::from(v)) };
    let (user, path) = rest.split_once(std::path::is_separator).unwrap_or((rest, ""));
    let base = if user.is_empty() { home.to_path_buf() } else { users_home(user, home)? };
    Some(if path.is_empty() { base } else { base.join(path) })
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
