//! Renaming a tracked working-tree file so the index records the rename too.
//!
//! The move goes through `git mv` (the git CLI) rather than libgit2 index writes, so git's own
//! index rules, sparse-checkout refusals and case handling apply. The stored blob and the
//! working file's bytes both move unchanged, so no clean or smudge filter is involved. The path checks are the ones
//! the plain rename in `workfile` uses; the index is only read here, never written.

use super::workfile::{create_parents, ensure_inside, plan_rename, prune_empty_parents, remove_dirs};
use crate::error::{Error, Result};
use crate::gitcli::{NO_PROMPTS, args, run_with_input};
use std::path::Path;

/// Renames the tracked working-tree file `from` to the typed path `to` with `git mv`, so the
/// index records the rename. Same refusals as `rename_file`, plus: `from` must have a
/// stage-0 index entry, and no other index entry may sit at `to`, at a folder above it, or
/// inside it.
pub fn rename_file_staged(workdir: &Path, from: &str, to: &str) -> Result<()> {
    let (repo, p) = plan_rename(workdir, from, to)?;
    let index = repo.index()?;
    if index.get_path(Path::new(&p.from), 0).is_none() {
        return Err(Error::Other(format!("{} isn't tracked by git, so the rename can't be staged", p.from)));
    }
    // git mv replaces whatever index entries the new path collides with, without asking.
    let ignore_case = repo.config()?.get_bool("core.ignorecase").unwrap_or(false);
    let paths: Vec<Vec<u8>> = index.iter().map(|e| e.path).collect();
    if let Some(hit) = index_collision(paths.iter().map(Vec::as_slice), &p.from, &p.to, ignore_case) {
        return Err(Error::Other(format!("{hit} is still in the index (deleted but not staged); stage or discard that deletion first")));
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

/// The index entry (of any stage) that a staged rename of `from` onto `to` would replace: one
/// at `to` itself, one at a folder above `to`, or one inside a folder named `to`. `from`'s own
/// entry doesn't count. With `ignore_case` (core.ignorecase), paths match regardless of ASCII
/// case; otherwise a name differing from `to` only by case is a separate entry.
fn index_collision<'a>(entries: impl IntoIterator<Item = &'a [u8]>, from: &str, to: &str, ignore_case: bool) -> Option<String> {
    let same = |a: &[u8], b: &[u8]| if ignore_case { a.eq_ignore_ascii_case(b) } else { a == b };
    let to = to.as_bytes();
    let under = |inner: &[u8], outer: &[u8]| inner.len() > outer.len() && inner[outer.len()] == b'/' && same(&inner[..outer.len()], outer);
    entries
        .into_iter()
        .filter(|e| *e != from.as_bytes())
        .find(|e| same(e, to) || under(to, e) || under(e, to))
        .map(|e| String::from_utf8_lossy(e).into_owned())
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
    use super::{git_message, index_collision};

    #[test]
    fn finds_entries_the_target_collides_with() {
        let hit = |entries: &[&str], from: &str, to: &str, ic: bool| index_collision(entries.iter().map(|e| e.as_bytes()), from, to, ic);
        assert_eq!(hit(&["a.txt", "b.txt"], "b.txt", "a.txt/b.txt", false).as_deref(), Some("a.txt"));
        assert_eq!(hit(&["b.txt", "src/x.txt"], "b.txt", "src", false).as_deref(), Some("src/x.txt"));
        assert_eq!(hit(&["b.txt", "c.txt"], "b.txt", "c.txt", false).as_deref(), Some("c.txt"));
        // Near misses: a sibling sharing a prefix, and a folder only named like the target.
        assert_eq!(hit(&["a.txt", "a.txt2", "src-x/y", "srcx"], "a.txt", "src/a.txt", false), None);
        assert_eq!(hit(&["a.txt", "a.txtx/y"], "a.txt", "a.txt/b", false), None);
        // A change of case: the source's own entry is fine either way, but on a case-sensitive
        // file system a second entry under the new case is a different file.
        assert_eq!(hit(&["readme.md"], "readme.md", "README.md", true), None);
        assert_eq!(hit(&["readme.md"], "readme.md", "README.md", false), None);
        assert_eq!(hit(&["README.md", "readme.md"], "readme.md", "README.md", false).as_deref(), Some("README.md"));
        assert_eq!(hit(&["Docs/a.md", "b.md"], "b.md", "docs", true).as_deref(), Some("Docs/a.md"));
        assert_eq!(hit(&["Docs/a.md", "b.md"], "b.md", "docs", false), None);
    }

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
