//! Which OpenPGP program the user configured, so the wrapper runs that one.

use std::path::Path;
use std::process::{Command, Stdio};

/// The program git would run without Trellis's override, from
/// `git config -z --get-regexp '^gpg\.(openpgp\.)?program$'` output (`key\nvalue\0` entries,
/// in the order git reads them): the last non-empty value that isn't `self_exe`. git lets the
/// last of `gpg.program` and `gpg.openpgp.program` win, and Trellis's own command-scope entry
/// comes last of all. Pure.
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
/// be run.
pub fn user_program(self_exe: &Path) -> String {
    let mut cmd = Command::new("git");
    cmd.args(["config", "-z", "--get-regexp", r"^gpg\.(openpgp\.)?program$"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    super::real::no_window(&mut cmd);
    cmd.output().ok().and_then(|out| from_config(&String::from_utf8_lossy(&out.stdout), self_exe)).unwrap_or_else(|| "gpg".to_owned())
}
