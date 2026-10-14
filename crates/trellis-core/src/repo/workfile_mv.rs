//! Renaming a tracked working-tree file so the index records the rename too.
//!
//! The move goes through `git mv` (the git CLI) rather than libgit2 index writes, so git's own
//! index rules, sparse-checkout refusals and case handling apply. The path checks are the ones
//! the plain rename in `workfile` uses; the index is only read here, never written.

use super::workfile::{create_parents, ensure_inside, plan_rename, prune_empty_parents, remove_dirs};
use crate::error::{Error, Result};
use crate::gitcli::{NO_PROMPTS, args, run_with_input};
use std::path::Path;

/// Renames the tracked working-tree file `from` to the typed path `to` with `git mv`, so the
/// index records the rename. Same refusals as `rename_file`, plus: `from` must have a
/// stage-0 index entry, and `to` must not have one.
pub fn rename_file_staged(workdir: &Path, from: &str, to: &str) -> Result<()> {
    let (repo, p) = plan_rename(workdir, from, to)?;
    let index = repo.index()?;
    if index.get_path(Path::new(&p.from), 0).is_none() {
        return Err(Error::Other(format!("{} isn't tracked by git, so the rename can't be staged", p.from)));
    }
    if !p.case_only && index.get_path(Path::new(&p.to), 0).is_some() {
        return Err(Error::Other(format!("{} is still in the index (deleted but not staged); stage or discard that deletion first", p.to)));
    }
    // Hold nothing open in the repository while git works on it (Windows locks open files).
    drop(index);
    drop(repo);
    // git mv refuses a destination whose folder doesn't exist yet.
    let created = create_parents(&p.root, &p.to)?;
    if let Err(e) = ensure_inside(&p.root, p.full_to.parent().unwrap_or(&p.root)) {
        remove_dirs(&created);
        return Err(e);
    }
    let a = args(&["--literal-pathspecs", "mv", "--", &p.from, &p.to]);
    let (ok, out) = run_with_input(&p.root, &a, "", NO_PROMPTS);
    if !ok {
        remove_dirs(&created);
        return Err(Error::Other(git_message(&out)));
    }
    // git mv leaves the source's emptied folders behind.
    prune_empty_parents(&p.root, &p.from);
    Ok(())
}

/// The line of git's output that says what went wrong: the last `fatal:` or `error:` line
/// (without that prefix), or else the last non-empty line.
fn git_message(out: &str) -> String {
    let lines: Vec<&str> = out.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let reason = lines.iter().rev().find_map(|l| l.strip_prefix("fatal: ").or_else(|| l.strip_prefix("error: ")));
    match reason.or(lines.last().copied()) {
        Some(line) => line.to_owned(),
        None => "git mv failed".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::git_message;

    #[test]
    fn picks_the_reason_line() {
        assert_eq!(
            git_message("fatal: Unable to create 'x/index.lock': File exists.\n\nAnother git process..."),
            "Unable to create 'x/index.lock': File exists."
        );
        assert_eq!(git_message("warning: a\nsomething broke\n"), "something broke");
        assert_eq!(git_message(""), "git mv failed");
    }
}
