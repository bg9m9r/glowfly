//! Which exits of a diff tool are failures. Like diff(1), many diff tools exit with 1 when the
//! files differ, and `git difftool --no-index` itself exits with 1 whenever they do. So for a
//! diff tool, 0, or 1 with nothing on stderr, is a normal end. Editors, terminals and merge
//! tools keep the plain rule (`spawn`, `run_to_exit`).

use super::{Launch, Watched, early_exit_message, watch};
use crate::error::{Error, Result};

/// Whether a diff tool that ended with `code`, having printed `stderr`, ended normally: 0, or 1
/// with nothing on stderr (blank lines and spaces are nothing). An end by signal (`None`) never.
pub fn diff_exit_ok(code: Option<i32>, stderr: &str) -> bool {
    code == Some(0) || (code == Some(1) && stderr.trim().is_empty())
}

/// What `git difftool` printed, without git's own line-ending notes on the copies
/// (`core.autocrlf`), which are not the tool's: git 2.37+'s one-line shape and the older
/// two-line one ([`is_old_warning`]).
pub fn without_line_ending_warnings(stderr: &str) -> String {
    stderr.lines().filter(|l| !crate::crlf_warnings::is_warning(l.trim()) && !is_old_warning(l.trim())).collect::<Vec<_>>().join("\n")
}

/// What git before 2.37 printed on the line after an older-shape warning.
const OLD_TAIL: &str = "The file will have its original line endings in your working directory";

/// Whether `line` (trimmed) is a line of git's pre-2.37 line-ending warning: "warning: LF will be
/// replaced by CRLF in <path>." (or CRLF by LF), or the line git printed after it.
fn is_old_warning(line: &str) -> bool {
    if line.strip_suffix('.').unwrap_or(line) == OLD_TAIL {
        return true;
    }
    ["warning: LF will be replaced by CRLF in ", "warning: CRLF will be replaced by LF in "]
        .iter()
        .any(|p| line.strip_prefix(p).and_then(|r| r.strip_suffix('.')).is_some_and(|path| !path.is_empty()))
}

/// A watched diff tool as a result: only an early exit [`diff_exit_ok`] doesn't allow is an error.
pub(crate) fn diff_watched(w: Watched) -> Result<()> {
    match w {
        Watched::Exited { code, stderr } if !diff_exit_ok(code, &stderr) => Err(Error::Other(early_exit_message(code, &stderr))),
        _ => Ok(()),
    }
}

/// [`super::spawn`] for a diff tool: an early exit [`diff_exit_ok`] allows is no failure.
pub fn spawn_diff(l: &Launch) -> Result<()> {
    diff_watched(watch(super::start(l, false)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_commands::find_sh;
    use crate::external_tools::spawn;

    #[test]
    fn diff_exit_ok_takes_0_and_a_quiet_1() {
        assert!(diff_exit_ok(Some(0), "anything"));
        for quiet in ["", "  \n\t\n", "\r\n"] {
            assert!(diff_exit_ok(Some(1), quiet), "{quiet:?}");
        }
        assert!(!diff_exit_ok(Some(1), "oops"));
        for code in [Some(2), Some(128), Some(-1), None] {
            assert!(!diff_exit_ok(code, ""), "{code:?}");
        }
    }

    #[test]
    fn line_ending_warnings_are_not_the_tools_words() {
        let warned = "warning: in the working copy of '/t/a.txt', LF will be replaced by CRLF the next time Git touches it\r\n\
                      warning: in the working copy of 'b', CRLF will be replaced by LF\n";
        let said = without_line_ending_warnings(warned);
        assert_eq!(said.trim(), "");
        assert!(diff_exit_ok(Some(1), &said));
        let more = format!("{warned}oops\n");
        assert_eq!(without_line_ending_warnings(&more), "oops");
        assert_eq!(without_line_ending_warnings("warning: something else\n"), "warning: something else");
    }

    #[test]
    fn older_gits_line_ending_warnings_are_not_the_tools_words() {
        let warned = "warning: LF will be replaced by CRLF in a.txt.\r\n\
                      The file will have its original line endings in your working directory\r\n\
                      warning: CRLF will be replaced by LF in d/b c.txt.\n\
                      The file will have its original line endings in your working directory.\n";
        let said = without_line_ending_warnings(warned);
        assert_eq!(said, "");
        assert!(diff_exit_ok(Some(1), &said));
        assert_eq!(without_line_ending_warnings(&format!("{warned}oops\n")), "oops");
        for kept in ["warning: LF will be replaced by CRLF in .", "warning: LF will be replaced by CRLF in a.txt", "The file will be gone"]
        {
            assert_eq!(without_line_ending_warnings(kept), kept);
        }
    }

    #[test]
    fn spawn_diff_follows_the_diff_rule() {
        let Ok(sh) = find_sh() else {
            eprintln!("skipping: no sh");
            return;
        };
        let run = |script: &str, f: fn(&Launch) -> Result<()>| {
            f(&Launch { program: sh.clone(), args: vec!["-c".into(), script.into()], cwd: crate::paths::temp_dir(), own_console: false })
        };
        assert!(run("exit 1", spawn_diff).is_ok());
        let e = run("echo oops >&2; exit 1", spawn_diff).unwrap_err().to_string();
        assert!(e.contains("code 1: oops"), "{e}");
        let e = run("exit 2", spawn_diff).unwrap_err().to_string();
        assert!(e.contains("code 2"), "{e}");
        assert!(run("exit 1", spawn).is_err(), "editors and terminals keep the plain rule");
    }
}
