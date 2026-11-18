//! Argument lists for git: [`args`], [`subcommand`] and [`utf8_args`].
//!
//! git's log family (log, show, blame, format-patch, range-diff, ...) re-encodes names and
//! messages into `i18n.logOutputEncoding`, or `i18n.commitEncoding` when that is unset, while
//! Glowfly reads output as UTF-8. `utf8_args` puts a `-c` override first; git hands it on to
//! the git children a command starts. `tests/source_checks/log_encoding.rs` keeps the command
//! lines that are parsed for commit text on it.

/// Converts a list of string slices into an owned argument vector.
pub fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

/// The git subcommand `args` runs: the first argument that isn't a global option (`-c k=v` and
/// `-C dir` take the next argument; `--git-dir=…`, `--no-pager` and any other leading `-…`
/// are skipped).
pub fn subcommand(args: &[String]) -> Option<&str> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-c" | "-C" => {
                it.next();
            }
            s if s.starts_with('-') => {}
            s => return Some(s),
        }
    }
    None
}

/// `a` as an owned argument list, like [`args`], with `-c i18n.logOutputEncoding=UTF-8` first:
/// for every command whose output is parsed for commit text.
pub fn utf8_args(a: &[&str]) -> Vec<String> {
    let mut v = vec!["-c".to_string(), "i18n.logOutputEncoding=UTF-8".to_string()];
    v.extend(a.iter().map(|s| s.to_string()));
    v
}
