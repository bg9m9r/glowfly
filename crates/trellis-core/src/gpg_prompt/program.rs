//! Which OpenPGP program the user configured, so the wrapper runs that one.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The program git would run without Trellis's override, from
/// `git config -z --type=path --get-regexp '^gpg\.(openpgp\.)?program$'` output (`key\nvalue\0`
/// entries, in the order git reads them): the last non-empty value that isn't `self_exe`. git
/// lets the last of `gpg.program` and `gpg.openpgp.program` win, and Trellis's own
/// command-scope entry comes last of all. The value is as git has it; [`user_program`] maps an
/// MSYS-style path on Windows. Pure.
pub fn from_config(z_output: &str, self_exe: &Path) -> Option<String> {
    let me = crate::native_path::native(self_exe);
    z_output
        .split('\0')
        .filter_map(|entry| entry.split_once('\n').map(|(_, value)| value))
        .rfind(|value| {
            let p = Path::new(value);
            !value.is_empty() && !crate::paths::same_spelling(p, self_exe) && crate::native_path::native(p).as_os_str() != me.as_os_str()
        })
        .map(str::to_owned)
}

/// The user's OpenPGP program, read through git with the environment and PATH git passed to
/// the wrapper (deliberately not `env_path::apply`: the wrapper keeps git's PATH, and on macOS
/// that would start a login shell for every signature). `gpg` when nothing is set or git can't
/// be run. On Windows an MSYS-style value (`/usr/bin/gpg`, `/c/Tools/gpg.exe`), which git for
/// Windows can't start itself, is mapped to the file in the git install ([`msys_candidates`]).
pub fn user_program(self_exe: &Path) -> String {
    windows_spelling(query(self_exe, |_| {}))
}

/// The folders an MSYS2-based git install keeps its programs under, below the install root.
pub const MSYSTEM_DIRS: &[&str] = &["mingw64", "mingw32", "ucrt64", "clang64", "clang32", "clangarm64", "usr"];

/// The install root of the git whose exec path is `exec_path`: `<root>/<msystem>/libexec/git-core`
/// (Git for Windows) or `<root>/usr/lib/git-core` (MSYS2's own git). The folder names are fixed
/// install names, compared without regard to ASCII case. None for any other layout. Pure.
pub fn install_root(exec_path: &Path) -> Option<PathBuf> {
    if !exec_path.is_absolute() {
        return None;
    }
    let named =
        |p: &Path, names: &[&str]| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| names.iter().any(|w| n.eq_ignore_ascii_case(w)));
    let core = Some(exec_path).filter(|p| named(p, &["git-core"]))?;
    let lib = core.parent().filter(|p| named(p, &["libexec", "lib"]))?;
    let msystem = lib.parent().filter(|p| named(p, MSYSTEM_DIRS))?;
    msystem.parent().map(Path::to_path_buf)
}

/// Where an MSYS-style `value` may be, best first: `/c/…` and `/cygdrive/c/…` on drive `C:`;
/// `/bin/…` and `/usr/bin/…` as `<root>/usr/bin/…` then `<root>/bin/…` (Cygwin's layout); any
/// other `/…` below `root`. Empty for anything that doesn't start with a single `/` (a Windows
/// path, a bare name, a share), and for a root-relative value without a `root`. Pure.
pub fn msys_candidates(value: &str, root: Option<&Path>) -> Vec<PathBuf> {
    let Some(rest) = value.strip_prefix('/').filter(|r| !r.starts_with('/')) else { return Vec::new() };
    let parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
    let join = |base: PathBuf, parts: &[&str]| parts.iter().fold(base, |p, part| p.join(part));
    let drive = |p: &str| (p.len() == 1 && p.as_bytes()[0].is_ascii_alphabetic()).then(|| p.to_ascii_uppercase());
    let on_drive = match parts.as_slice() {
        ["cygdrive", d, rest @ ..] => drive(d).map(|d| (d, rest)),
        [d, rest @ ..] => drive(d).map(|d| (d, rest)),
        [] => None,
    };
    if let Some((letter, rest)) = on_drive {
        return vec![join(PathBuf::from(format!("{letter}:/")), rest)];
    }
    let Some(root) = root.filter(|_| !parts.is_empty()) else { return Vec::new() };
    match parts.as_slice() {
        ["bin", rest @ ..] | ["usr", "bin", rest @ ..] => {
            vec![join(root.join("usr").join("bin"), rest), join(root.join("bin"), rest)]
        }
        _ => vec![join(root.to_path_buf(), &parts[..])],
    }
}

/// The first of `candidates` that `exists`, with or without `.exe` (Windows runs `p.exe` for a
/// `p` without an extension), else the first. None when there are none. Pure.
pub fn pick(candidates: &[PathBuf], exists: impl Fn(&Path) -> bool) -> Option<&PathBuf> {
    let with_exe = |p: &Path| {
        let mut s = p.as_os_str().to_owned();
        s.push(".exe");
        PathBuf::from(s)
    };
    candidates.iter().find(|p| exists(p) || exists(&with_exe(p))).or(candidates.first())
}

/// `value` mapped into the git install when it is in MSYS form; the root comes from
/// `exec_path` (git's `GIT_EXEC_PATH`), else from `<git> --exec-path`. Unchanged when it isn't
/// MSYS-style or nothing matches.
#[cfg(windows)]
pub(super) fn resolve_msys(value: String, exec_path: Option<PathBuf>, git: &std::ffi::OsStr) -> String {
    if !value.starts_with('/') || value.starts_with("//") {
        return value;
    }
    let exec = exec_path.or_else(|| {
        let mut cmd = Command::new(git);
        cmd.arg("--exec-path").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        super::real::no_window(&mut cmd);
        let out = cmd.output().ok().filter(|o| o.status.success())?;
        Some(String::from_utf8_lossy(&out.stdout).trim().to_owned()).filter(|s| !s.is_empty()).map(PathBuf::from)
    });
    let root = exec.as_deref().and_then(install_root);
    let candidates = msys_candidates(&value, root.as_deref());
    match pick(&candidates, |p| p.is_file()) {
        Some(p) => crate::native_path::native(p).into_os_string().into_string().unwrap_or(value),
        None => value,
    }
}

/// The program as Windows can start it: git exports `GIT_EXEC_PATH` (in Windows form) to the
/// wrapper, which locates the install an MSYS-style value refers to.
#[cfg(windows)]
fn windows_spelling(p: String) -> String {
    resolve_msys(p, std::env::var_os("GIT_EXEC_PATH").filter(|v| !v.is_empty()).map(PathBuf::from), &wrapper_git())
}

/// Elsewhere the value is already a path the system can start.
#[cfg(not(windows))]
fn windows_spelling(p: String) -> String {
    p
}

/// The git the wrapper asks: the one that ran it, from [`super::ENV_GIT`] (set by Trellis next to
/// `gpg.program`), so a git chosen in Preferences > Git reads its own system config and gives
/// the same answer it would give itself. `git` from PATH when the variable is unset or empty.
pub fn wrapper_git() -> OsString {
    git_from(std::env::var_os(super::ENV_GIT))
}

/// [`wrapper_git`] for the variable's value. Pure.
pub(super) fn git_from(value: Option<OsString>) -> OsString {
    value.filter(|v| !v.is_empty()).unwrap_or_else(|| "git".into())
}

/// `user_program`, with `setup` adjusting the `git config` command first (tests point it at a
/// fixture's folder and config files). git treats `gpg.program` as a path, expanding `~/` and
/// `%(prefix)/` before running it, so the value is read as one (`--type=path`) and the wrapper
/// runs the same file git would.
pub(super) fn query(self_exe: &Path, setup: impl FnOnce(&mut Command)) -> String {
    let mut cmd = Command::new(wrapper_git());
    cmd.args(["config", "-z", "--type=path", "--get-regexp", r"^gpg\.(openpgp\.)?program$"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    super::real::no_window(&mut cmd);
    setup(&mut cmd);
    cmd.output().ok().and_then(|out| from_config(&String::from_utf8_lossy(&out.stdout), self_exe)).unwrap_or_else(|| "gpg".to_owned())
}
