//! The steps left in a rebase, cherry-pick or revert in progress that run a program when it
//! continues (`TrustScan::steps`): each `exec` step of its todo, and a merge strategy that
//! isn't one of git's own, which git runs as `git merge-<name>`. Read as bytes with nothing
//! run, and overmatched: a line git would refuse is listed too.
//!
//! The `exec` lines Glowfly's own rebases add around a reworded commit aren't listed while
//! they are exactly as Glowfly writes them for this repository (`rebase::own_files`) and the
//! files they name in its message folder are plain files, or not there yet: those lines only
//! record HEAD there and amend a message from there.

use super::keys::text_of;
use git2::Repository;
use std::path::Path;

/// The todo of a rebase, and of a cherry-pick or revert of several commits, in the git dir.
const TODOS: [&str; 2] = ["rebase-merge/git-rebase-todo", "sequencer/todo"];
/// Where a rebase keeps its merge strategy's name.
const STRATEGY: &str = "rebase-merge/strategy";
/// A cherry-pick's or revert's options, in git's config format.
const OPTS: &str = "sequencer/opts";

/// Every file `find` reads, relative to the git dir, for the scan's stamp.
pub(super) const FILES: [&str; 4] = [TODOS[0], TODOS[1], STRATEGY, OPTS];

/// The merge strategies that come with git.
const STRATEGIES: [&str; 6] = ["ort", "recursive", "resolve", "octopus", "ours", "subtree"];

/// The steps left in `repo` that run a program, in file order, each once: an `exec` step as
/// `exec <command>`, another strategy as `--strategy=<name>`.
pub(super) fn find(repo: &Repository) -> Vec<String> {
    let git = repo.path();
    let msg_dir = git.join(crate::rebase::MSG_DIR);
    let mut out: Vec<String> = Vec::new();
    let mut add = |step: String| {
        if !out.contains(&step) {
            out.push(step);
        }
    };
    for file in TODOS {
        let Ok(bytes) = std::fs::read(git.join(file)) else { continue };
        for line in bytes.split(|b| *b == b'\n') {
            let line = text_of(line);
            let line = line.trim();
            if let Some(command) = exec(line)
                && !own(&msg_dir, line)
            {
                add(format!("exec {command}").trim_end().to_owned());
            }
        }
    }
    let mut strategy = |name: &str| {
        if !STRATEGIES.contains(&name) {
            add(format!("--strategy={name}"));
        }
    };
    if let Ok(bytes) = std::fs::read(git.join(STRATEGY)) {
        // Git takes the line as written, less its line ending.
        strategy(text_of(&bytes).trim_end_matches(['\n', '\r']));
    }
    if let Ok(bytes) = std::fs::read(git.join(OPTS)) {
        for line in bytes.split(|b| *b == b'\n') {
            if let Some(name) = opts_strategy(&text_of(line)) {
                strategy(name);
            }
        }
    }
    out
}

/// The command of a todo line (trimmed) that is an `exec` step, long or short.
fn exec(line: &str) -> Option<&str> {
    let (verb, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    matches!(verb, "exec" | "x").then(|| rest.trim_start())
}

/// Whether the todo line `line` is one Glowfly wrote for the message folder `msg_dir`, whose
/// files it names are plain files (or missing) in a plain folder: nothing is read or written
/// through a link.
fn own(msg_dir: &Path, line: &str) -> bool {
    let Some(files) = crate::rebase::own_files(msg_dir, line) else { return false };
    let plain_file = |p: &Path| match std::fs::symlink_metadata(p) {
        Ok(m) => m.is_file(),
        Err(e) => e.kind() == std::io::ErrorKind::NotFound,
    };
    std::fs::symlink_metadata(msg_dir).is_ok_and(|m| m.is_dir()) && files.iter().all(|f| plain_file(f))
}

/// The value of a `strategy` key on one line of the options file, read loosely: after any
/// section header, quotes around the whole value taken off. Anything git would read otherwise
/// (a comment, a continued line) leaves a name that isn't one of git's.
fn opts_strategy(line: &str) -> Option<&str> {
    let line = line.trim();
    let line = if line.starts_with('[') { line.split_once(']')?.1 } else { line };
    let (key, value) = line.split_once('=')?;
    if !key.trim().eq_ignore_ascii_case("strategy") {
        return None;
    }
    let value = value.trim();
    Some(value.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(value))
}

#[cfg(test)]
#[path = "steps_tests.rs"]
mod tests;
