//! Renaming a stash list entry by rebuilding the top of `refs/stash` with `git stash drop` and
//! `git stash store`. It runs through `gitcli`, so the reference-transaction hook and the user's
//! config apply. Only the reflog message changes; the stash commit and its position stay.
//!
//! Two limits follow from that: the stash commit's own message (what the commit details show)
//! keeps the old text, and the reflog dates of `stash@{0..N}` become the time of the rename.
//!
//! `push` builds the Stash dialog's `git stash push` command lines (every change, only the
//! staged changes, or chosen paths).

use crate::error::Result;
use crate::gitcli::{self, args};
use crate::repo::StashInfo;
use git2::Oid;
use std::path::Path;

mod push;
pub use push::{PATHS_ON_COMMAND_LINE, PushCommand, STAGED_MIN_GIT, StashScope, push_command, push_message, staged_unsupported};

/// Splits a stash list text into the branch it was made on and the part the user can edit:
/// "On <branch>: <rest>" and "WIP on <branch>: <rest>" give `(Some(branch), rest)`, anything
/// else `(None, message)`. The branch ends at the first ':' (branch names can't hold one), and
/// that ':' must be followed by a space.
pub fn split_message(message: &str) -> (Option<&str>, &str) {
    for prefix in ["WIP on ", "On "] {
        if let Some(rest) = message.strip_prefix(prefix)
            && let Some((branch, body)) = rest.split_once(':')
            && let Some(body) = body.strip_prefix(' ')
            && !branch.is_empty()
        {
            return (Some(branch), body);
        }
    }
    (None, message)
}

/// The stash list text for `body` typed over the entry `original`: whitespace runs (newlines and
/// tabs too, since a reflog line can't hold them) become single spaces and the ends are trimmed.
/// A branch prefix is kept as "On <branch>: " (what `git stash push -m` writes, so "WIP on"
/// becomes "On"). None when nothing is left.
pub fn edited_message(original: &str, body: &str) -> Option<String> {
    let body = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if body.is_empty() {
        return None;
    }
    Some(match split_message(original).0 {
        Some(branch) => format!("On {branch}: {body}"),
        None => body,
    })
}

/// The stash list of the repository at `workdir`, newest first (as a repository load reads it).
pub fn list(workdir: &Path) -> Result<Vec<StashInfo>> {
    let mut repo = git2::Repository::open(workdir)?;
    let mut out = Vec::new();
    repo.stash_foreach(|index, message, oid| {
        out.push(StashInfo { index, message: message.to_owned(), oid: *oid });
        true
    })?;
    Ok(out)
}

/// What a rename did: whether it worked, and the commands with their output for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameReport {
    pub ok: bool,
    pub output: String,
}

fn drop_top() -> Vec<String> {
    args(&["stash", "drop", "stash@{0}"])
}

fn store(message: &str, oid: Oid) -> Vec<String> {
    args(&["stash", "store", "-m", message, &oid.to_string()])
}

/// The commands that rename `stash@{index}` of `entries` to `message`: drop the top `index + 1`
/// entries, then store them back from the deepest up, the renamed one with `message` and the
/// others with their own.
pub fn rename_commands(entries: &[StashInfo], index: usize, message: &str) -> Vec<Vec<String>> {
    let top = &entries[..entries.len().min(index + 1)];
    let mut cmds: Vec<Vec<String>> = top.iter().map(|_| drop_top()).collect();
    for e in top.iter().rev() {
        cmds.push(store(if e.index == index { message } else { &e.message }, e.oid));
    }
    cmds
}

/// Gives `stash@{index}` the list text `message`, if it still is commit `oid`. Nothing runs when
/// it isn't, or when it already has that text.
pub fn rename(workdir: &Path, index: usize, oid: Oid, message: &str) -> RenameReport {
    rename_with(workdir, index, oid, message, &mut |a| gitcli::run(workdir, a))
}

/// `rename`, running each git command through `run` (tests make one of them fail).
fn rename_with(workdir: &Path, index: usize, oid: Oid, message: &str, run: &mut dyn FnMut(&[String]) -> (bool, String)) -> RenameReport {
    let failed = |output: String| RenameReport { ok: false, output };
    let entries = match list(workdir) {
        Ok(e) => e,
        Err(e) => return failed(format!("Could not read the stash list: {e}")),
    };
    if entries.get(index).map(|e| e.oid) != Some(oid) {
        return failed(format!("stash@{{{index}}} is no longer the stash that was picked; nothing was changed."));
    }
    if entries[index].message == message {
        return RenameReport { ok: true, output: format!("stash@{{{index}}} already has this message.") };
    }
    let mut output = String::new();
    let mut step = |cmd: &[String]| {
        output.push_str(&format!("$ git {}\n", crate::text::redact_credentials(&cmd.join(" "))));
        let (ok, text) = run(cmd);
        if !text.is_empty() {
            output.push_str(&text);
            output.push('\n');
        }
        ok
    };
    let cmds = rename_commands(&entries, index, message);
    let (drops, stores) = cmds.split_at(index + 1);
    // (message, oid) of each entry that is out of the list and could not be put back.
    let mut missing: Vec<(String, Oid)> = Vec::new();
    let dropped = drops.iter().take_while(|c| step(c)).count();
    if dropped < drops.len() {
        // Put back what was dropped, deepest first, with the original messages.
        for e in entries[..dropped].iter().rev() {
            if !step(&store(&e.message, e.oid)) {
                missing.push((e.message.clone(), e.oid));
            }
        }
    } else {
        // Keep storing after a failure, so the rest stay in the list (possibly out of order).
        for (cmd, e) in stores.iter().zip(entries[..=index].iter().rev()) {
            if !step(cmd) {
                let msg = if e.index == index { message.to_owned() } else { e.message.clone() };
                missing.push((msg, e.oid));
            }
        }
    }
    let ok = dropped == drops.len() && missing.is_empty();
    if !missing.is_empty() {
        // Deepest first, the order that rebuilds the list as it was.
        output.push_str("Not back in the stash list:\n");
        for (msg, oid) in &missing {
            output.push_str(&format!("git stash store -m \"{}\" {oid}\n", msg.replace('"', "\\\"")));
        }
    }
    RenameReport { ok, output: output.trim_end().to_owned() }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "stash_ops/push_tests.rs"]
mod push_tests;
