//! Keeping submodules up to date and editing them: which git commands, undos and redos move the
//! parent's work tree (after which its submodules may need updating), the batch
//! `git submodule update` commands, and the commands that change a submodule's URL and branch.

use crate::clone_ops::check_clone_branch;
use crate::undo::{Action, ResetMode};
use git2::RepositoryState;

/// The most bytes of pathspecs one batch command carries.
pub const BATCH_PATH_BYTES: usize = 16 * 1024;

/// Whether running `cmds` (git argument lists) can change the commits the parent records for its
/// submodules: a branch checkout or switch, a pull, merge or rebase, a cherry-pick or revert
/// (also `--continue`, `--skip` and `--abort`), a stash pop or apply, or a reset other than
/// `--soft`. Checking out or resetting paths doesn't count. One that stops for conflicts fails,
/// so nothing follows it until the operation is finished.
pub fn moves_work_tree(cmds: &[Vec<String>]) -> bool {
    cmds.iter().any(|c| moves(c))
}

/// Whether `cmds`, run while the repository is in `state`, finish a merge, cherry-pick or revert
/// that stopped for conflicts: a commit, which records what the stopped operation left in the
/// index (the gitlinks it moved included).
pub fn finishes_operation(state: RepositoryState, cmds: &[Vec<String>]) -> bool {
    use RepositoryState::*;
    matches!(state, Merge | CherryPick | CherryPickSequence | Revert | RevertSequence)
        && cmds.iter().any(|c| crate::gitcli::subcommand(c) == Some("commit"))
}

/// Whether undoing (or, with `redo`, redoing) `action` can change the commits the parent's index
/// records for its submodules, as `moves_work_tree` says for a command: it moves HEAD with the
/// index (a checkout, a mixed or hard reset, a merge and the like, a restack) or puts index
/// changes back or away (stashes, Discard all). A soft reset (a commit's undo, a soft reset's)
/// leaves the index as it is, ref and single-file changes touch no gitlink, and the conflict
/// kinds (a saved resolution, Mark resolved, undoing an abort) leave an operation in progress,
/// which is finished first.
pub fn undo_moves_work_tree(action: &Action, redo: bool) -> bool {
    match action {
        Action::Checkout { .. } | Action::Moved { .. } | Action::Restacked { .. } => true,
        Action::Reset { mode, .. } => *mode != ResetMode::Soft,
        Action::StashPush { .. } | Action::StashApply { .. } | Action::DiscardAll { .. } => true,
        // Redoing aborts again, back to where the operation started.
        Action::AbortKept { .. } => redo,
        Action::Commit { .. }
        | Action::DeleteBranch { .. }
        | Action::CreateBranch { .. }
        | Action::CreateTag { .. }
        | Action::SetBranch { .. }
        | Action::StashDrop { .. }
        | Action::Discard { .. }
        | Action::Restore { .. }
        | Action::DeleteFile { .. }
        | Action::UnstageGone { .. }
        | Action::Resolve { .. }
        | Action::Conflicts { .. } => false,
    }
}

fn moves(cmd: &[String]) -> bool {
    let mut i = 0;
    // Global options: `-c <k=v>`, `-C <dir>`, `--literal-pathspecs` and the like.
    while let Some(a) = cmd.get(i) {
        match a.as_str() {
            "-c" | "-C" => i += 2,
            a if a.starts_with("--") => i += 1,
            _ => break,
        }
    }
    let Some(sub) = cmd.get(i) else { return false };
    let rest = &cmd[i + 1..];
    let has = |f: &str| rest.iter().any(|a| a == f);
    let paths_after_dashes = rest.iter().position(|a| a == "--").is_some_and(|d| d + 1 < rest.len());
    match sub.as_str() {
        "checkout" | "switch" => !paths_after_dashes && !["--ours", "--theirs", "-p", "--patch"].iter().any(|f| has(f)),
        "pull" | "merge" | "rebase" | "cherry-pick" | "revert" => true,
        "stash" => rest.first().is_some_and(|a| a == "pop" || a == "apply"),
        "reset" => !has("--soft") && !paths_after_dashes,
        _ => false,
    }
}

/// `git submodule update --init --recursive [--checkout]` for every submodule (`paths` None),
/// or for `paths` as `:(literal)` pathspecs, split into several commands so that none carries
/// more than `BATCH_PATH_BYTES` of them. Without `--checkout`, each submodule's
/// `submodule.<name>.update` setting decides how it is updated.
pub fn batch_update_cmds(paths: Option<&[String]>, checkout: bool) -> Vec<Vec<String>> {
    let mut base: Vec<String> = ["submodule", "update", "--init", "--recursive"].map(str::to_owned).to_vec();
    if checkout {
        base.push("--checkout".to_owned());
    }
    let Some(paths) = paths else { return vec![base] };
    let mut out = Vec::new();
    let mut cmd: Option<(Vec<String>, usize)> = None;
    for p in paths {
        let spec = format!(":(literal){p}");
        if let Some((c, size)) = cmd.as_mut()
            && *size + spec.len() < BATCH_PATH_BYTES
        {
            *size += spec.len() + 1;
            c.push(spec);
            continue;
        }
        if let Some((c, _)) = cmd.take() {
            out.push(c);
        }
        let mut c = base.clone();
        c.push("--".to_owned());
        let size = spec.len();
        c.push(spec);
        cmd = Some((c, size));
    }
    out.extend(cmd.map(|(c, _)| c));
    out
}

/// `git submodule set-url -- <path> <url>` (it takes a path, not a pathspec; the URL was checked
/// by `check_add_url`, which refuses a leading dash), then `git submodule sync` so this
/// repository's config and the submodule's origin follow.
pub fn set_url_cmds(path: &str, url: &str) -> Vec<Vec<String>> {
    vec![
        ["submodule", "set-url", "--", path, url.trim()].map(str::to_owned).to_vec(),
        ["submodule", "sync", "--", &format!(":(literal){path}")].map(str::to_owned).to_vec(),
    ]
}

/// `git submodule set-branch --branch <b> -- <path>`, or `--default` for None.
pub fn set_branch_cmd(path: &str, branch: Option<&str>) -> Vec<String> {
    let mut cmd: Vec<String> = ["submodule", "set-branch"].map(str::to_owned).to_vec();
    match branch {
        Some(b) => cmd.extend(["--branch".to_owned(), b.to_owned()]),
        None => cmd.push("--default".to_owned()),
    }
    cmd.extend(["--".to_owned(), path.to_owned()]);
    cmd
}

/// The Edit submodule dialog's branch: empty for the remote's default (None), `.` for the
/// parent's current branch name, else a valid branch name.
#[allow(clippy::result_unit_err)]
pub fn check_edit_branch(text: &str) -> Result<Option<String>, ()> {
    if text.trim() == "." { Ok(Some(".".to_owned())) } else { check_clone_branch(text) }
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
