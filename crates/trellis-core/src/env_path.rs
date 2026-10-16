//! The PATH Trellis gives the programs it starts (git, ssh-keygen, gpg, editors, terminals).
//!
//! An app started from the macOS Dock or Finder gets launchd's minimal PATH
//! (`/usr/bin:/bin:/usr/sbin:/sbin`), not the one the user's shell sets up, so Apple's
//! `/usr/bin/git` would run instead of Homebrew's (or the Command Line Tools install prompt
//! would appear), and `git-lfs`, `gpg` or `code` wouldn't be found. On macOS the login shell's
//! PATH is read once, on first use (never on the UI thread: only workers start programs), and
//! merged in front of the inherited one; if the shell can't say, Homebrew's folders are put in
//! front instead. Elsewhere the inherited PATH is used as it is.
//!
//! Like the askpass environment in `gitcli`, this is set once and never changed afterwards. It
//! is only ever applied through `Command::env`; the process's own environment is not touched.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

/// How long the login shell may take to print its PATH.
pub const LOGIN_SHELL_TIMEOUT: Duration = Duration::from_secs(3);

/// Where Homebrew puts its programs (Apple silicon, then Intel), for when the login shell
/// doesn't answer.
pub const MACOS_FALLBACK: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin"];

/// The PATH to give children when it differs from the inherited one (macOS only).
static CHILD_PATH: LazyLock<Option<OsString>> = LazyLock::new(|| {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let shell = std::env::var_os("SHELL").filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| "/bin/zsh".into());
    let login = read_login_path(&shell, LOGIN_SHELL_TIMEOUT);
    Some(merge(login.as_deref(), std::env::var_os("PATH").as_deref(), MACOS_FALLBACK))
});

/// The PATH children get: the merged one on macOS, else the inherited one.
pub fn path() -> Option<OsString> {
    CHILD_PATH.clone().or_else(|| std::env::var_os("PATH"))
}

/// The PATH children get when it differs from the inherited one (macOS), else None.
pub fn override_path() -> Option<OsString> {
    CHILD_PATH.clone()
}

/// Gives `cmd` the PATH children get, when it differs from the inherited one. A program named
/// without a folder is then also looked up on that PATH.
pub fn apply(cmd: &mut Command) {
    if let Some(p) = CHILD_PATH.as_ref() {
        cmd.env("PATH", p);
    }
}

/// The PATH for children: `login`'s folders first (in order), then those of `current` it
/// doesn't list. Without a usable `login`, `fallback`'s folders that `current` lacks go in
/// front of `current` instead. Empty entries (which would mean the current folder) are dropped
/// and each folder is listed once.
pub fn merge(login: Option<&OsStr>, current: Option<&OsStr>, fallback: &[&str]) -> OsString {
    let split = |v: Option<&OsStr>| -> Vec<PathBuf> { v.map(|v| std::env::split_paths(v).collect()).unwrap_or_default() };
    let login = split(login);
    let current = split(current);
    let front: Vec<PathBuf> =
        if login.iter().any(|p| !p.as_os_str().is_empty()) { login } else { fallback.iter().map(PathBuf::from).collect() };
    let mut out: Vec<PathBuf> = Vec::new();
    for p in front.into_iter().chain(current) {
        if !p.as_os_str().is_empty() && !out.contains(&p) {
            out.push(p);
        }
    }
    std::env::join_paths(out).unwrap_or_default()
}

/// What the login shell prints between this marker and the end is its PATH; anything a
/// profile script echoes comes before it.
const MARKER: &str = "__TRELLIS_PATH__";

/// The PATH `shell` sets up as a login shell (`$SHELL -lc`), or None when it fails, prints
/// nothing usable or takes longer than `timeout` (it is then killed).
pub fn read_login_path(shell: &Path, timeout: Duration) -> Option<OsString> {
    let mut child = Command::new(shell)
        .arg("-lc")
        .arg(format!("printf '%s%s' '{MARKER}' \"$PATH\""))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        let _ = tx.send(buf);
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let out = rx.recv_timeout(Duration::from_millis(500)).ok()?;
    if !status.success() {
        return None;
    }
    parse_login_output(&String::from_utf8_lossy(&out)).map(OsString::from)
}

/// The PATH in what the login shell printed: the text after the last marker, if it looks like
/// a list of absolute folders.
fn parse_login_output(out: &str) -> Option<&str> {
    let p = out.rsplit_once(MARKER)?.1.trim_end_matches(['\n', '\r']);
    (p.starts_with('/') && !p.contains('\n')).then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(v: &[&str]) -> OsString {
        std::env::join_paths(v).unwrap()
    }

    #[test]
    fn login_folders_go_first_and_each_folder_once() {
        let login = joined(&["/opt/homebrew/bin", "/usr/bin", "/bin"]);
        let gui = joined(&["/usr/bin", "/bin", "/usr/sbin", "/sbin"]);
        assert_eq!(
            merge(Some(&login), Some(&gui), MACOS_FALLBACK),
            joined(&["/opt/homebrew/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"])
        );
    }

    #[test]
    fn without_a_login_path_homebrew_goes_in_front() {
        let gui = joined(&["/usr/bin", "/bin", "/usr/local/bin"]);
        assert_eq!(merge(None, Some(&gui), MACOS_FALLBACK), joined(&["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"]));
        assert_eq!(merge(Some(OsStr::new("")), None, MACOS_FALLBACK), joined(MACOS_FALLBACK));
    }

    #[test]
    fn empty_entries_never_mean_the_current_folder() {
        let login = OsString::from(if cfg!(windows) { ";/a;;/b;" } else { ":/a::/b:" });
        assert_eq!(merge(Some(&login), None, &[]), joined(&["/a", "/b"]));
    }

    #[test]
    fn profile_chatter_before_the_marker_is_skipped() {
        assert_eq!(parse_login_output("Welcome!\n__TRELLIS_PATH__/opt/homebrew/bin:/usr/bin"), Some("/opt/homebrew/bin:/usr/bin"));
        assert_eq!(parse_login_output("__TRELLIS_PATH__/usr/bin\n"), Some("/usr/bin"));
        for bad in ["", "/usr/bin", "__TRELLIS_PATH__", "__TRELLIS_PATH__relative:/x", "__TRELLIS_PATH__/a\n/b"] {
            assert_eq!(parse_login_output(bad), None, "{bad:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_login_shell_reports_its_path() {
        let p = read_login_path(Path::new("/bin/sh"), LOGIN_SHELL_TIMEOUT).expect("sh prints its PATH");
        assert!(std::env::split_paths(&p).count() > 0, "{p:?}");
        assert_eq!(read_login_path(Path::new("/no/such/shell"), LOGIN_SHELL_TIMEOUT), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_shell_that_hangs_is_given_up_on() {
        let dir = crate::paths::temp_dir().join(format!("trellis-env-path-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let shell = dir.join("slow-shell");
        std::fs::write(&shell, "#!/bin/sh\nsleep 30\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap();
        let start = Instant::now();
        assert_eq!(read_login_path(&shell, Duration::from_millis(300)), None);
        assert!(start.elapsed() < Duration::from_secs(5), "{:?}", start.elapsed());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
