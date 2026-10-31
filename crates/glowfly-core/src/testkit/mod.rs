//! The test kit every crate's tests share (their dev-dependencies turn on the `testkit` feature): one
//! `git` for building fixture repositories, plus config and file helpers.
//!
//! Starting a git process costs tens to hundreds of milliseconds on Windows, and the suites make
//! thousands of fixture commits. So `git` runs the everyday setup commands (`init`, `add`,
//! `commit`, `checkout`, `branch`, `tag`, `config`, `remote`, `update-ref`, `update-index
//! --refresh` and `push` to a local bare repository, in their plain forms; see `fast.rs`)
//! in-process through libgit2, with the files, reflog messages, identity and dates the git CLI
//! would leave (`tests.rs` builds one history both ways and compares them). Anything else, and
//! any repository where git's own behaviour could differ (hooks, signing, filters, submodules,
//! a merge in progress, ...; `guard.rs`), goes to the git CLI. A test about how the CLI itself
//! behaves calls [`git_cli`]; `GLOWFLY_TEST_GIT_CLI=1` sends every fixture command to the CLI,
//! to check that a failure isn't the kit's.

pub mod bin;
mod commit;
mod fast;
mod guard;
mod push;
mod refs;
#[cfg(test)]
mod tests;

use crate::gitcli::{args, run_with_input};
use std::path::Path;

/// Runs `git <a>` in `dir`, failing the test if it fails.
#[track_caller]
pub fn git(dir: &Path, a: &[&str]) {
    git_out_env(dir, a, &[]);
}

/// Like [`git`], returning the command's output (empty when it ran in-process: the commands
/// handled there print nothing a test reads).
#[track_caller]
pub fn git_out(dir: &Path, a: &[&str]) -> String {
    git_out_env(dir, a, &[])
}

/// Like [`git`], with extra environment variables (e.g. `GIT_AUTHOR_DATE` for a fixed date).
#[track_caller]
pub fn git_env(dir: &Path, a: &[&str], env: &[(&str, &str)]) {
    git_out_env(dir, a, env);
}

/// [`git_env`] returning the output, as [`git_out`].
#[track_caller]
pub fn git_out_env(dir: &Path, a: &[&str], env: &[(&str, &str)]) -> String {
    let (ok, out) = try_git_env(dir, a, env);
    assert!(ok, "git {a:?} in {} failed: {out}", dir.display());
    out
}

/// Runs `git <a>` with `env` in `dir` and reports success and the output, without failing the
/// test: for commands a test expects to fail.
pub fn try_git_env(dir: &Path, a: &[&str], env: &[(&str, &str)]) -> (bool, String) {
    crate::partial::allow_extension();
    if !cli_only()
        && let Some(ok) = fast::run(dir, a, env)
    {
        return (ok, String::new());
    }
    run_with_input(dir, &args(a), "", env)
}

/// Runs `git <a>` in `dir` through the git CLI always, failing the test if it fails: for tests
/// of what git itself does (hooks, filters, its messages).
#[track_caller]
pub fn git_cli(dir: &Path, a: &[&str]) -> String {
    let (ok, out) = run_with_input(dir, &args(a), "", &[]);
    assert!(ok, "git {a:?} in {} failed: {out}", dir.display());
    out
}

/// Whether `GLOWFLY_TEST_GIT_CLI` asks for every fixture command to go to the git CLI.
fn cli_only() -> bool {
    static CLI: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *CLI.get_or_init(|| std::env::var_os("GLOWFLY_TEST_GIT_CLI").is_some_and(|v| !v.is_empty() && v != "0"))
}

/// Writes `entries` to the repository's own config file, as `git config <key> <value>` would,
/// without starting a git process for each.
pub fn set_config(dir: &Path, entries: &[(&str, &str)]) {
    crate::partial::allow_extension();
    let repo = git2::Repository::open(dir).unwrap();
    let mut config = repo.config().unwrap().open_level(git2::ConfigLevel::Local).unwrap();
    for (key, value) in entries {
        config.set_str(key, value).unwrap();
    }
}

/// The test identity, no signing and no automatic maintenance (which every CLI commit would
/// otherwise start as another git process), set in `dir`'s own config.
pub fn set_identity(dir: &Path) {
    set_config(
        dir,
        &[("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false"), ("maintenance.auto", "false")],
    );
}

/// Writes `text` to `file` (relative to `dir`).
pub fn write(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
}
