//! The steps left in the rebase, cherry-pick, revert or `am` in progress: what `--continue` or
//! `--skip` brings in. A rebase's or pick's todo is read with git's own verbs; a word it names
//! resolves as git resolves it (a label to `refs/rewritten/<label>` first, else a revision).
//! (The patches left in an `am` are `Target::Applying`, patches.rs.) An apply-backend rebase in
//! progress, an `am -3`, or a todo that isn't UTF-8 is held.

use crate::trust::incoming::{Unchecked, commit, parents};
use git2::{Oid, Repository};
use std::collections::HashSet;

/// One step left that replays a commit.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pick {
    pub commit: Oid,
    pub revert: bool,
}

/// What the steps left take.
#[derive(Debug, Default)]
pub(crate) struct Todo {
    /// Each commit whose version they bring in: where it goes, every one picked (a revert's
    /// parents), a merge's or reset's commit.
    pub commits: Vec<Oid>,
    /// The picks in order, when every step left is a pick, a revert or one that brings in no
    /// tree (`exec`, `break`, `drop`); None when a step merges or resets (made from what the
    /// steps before it leave, which isn't simulated).
    pub picks: Option<Vec<Pick>>,
}

/// The steps left in the sequencer in progress in `repo`.
pub(crate) fn read(repo: &Repository) -> Result<Todo, Unchecked> {
    let git = repo.path();
    let mut todo = Todo { picks: Some(Vec::new()), ..Todo::default() };
    for onto in ["rebase-merge/onto", "rebase-apply/onto"] {
        if let Ok(onto) = std::fs::read_to_string(git.join(onto)) {
            todo.commits.push(commit(repo, onto.trim())?);
        }
    }
    let apply = git.join("rebase-apply");
    if apply.join("rebasing").exists() {
        return Err(Unchecked::Unmodeled("an apply-backend rebase in progress".to_owned()));
    }
    if apply.join("threeway").exists() {
        return Err(Unchecked::Unmodeled("git am --3way in progress".to_owned()));
    }
    let mut reverts = Vec::new();
    for file in ["rebase-merge/git-rebase-todo", "sequencer/todo"] {
        let Ok(bytes) = std::fs::read(git.join(file)) else { continue };
        let text = String::from_utf8(bytes).map_err(|_| Unchecked::Unreadable(format!("{file} isn't UTF-8")))?;
        add_steps(&text, &mut todo, &mut reverts, &mut |spec| commit(repo, spec))?;
    }
    todo.commits.extend(parents(repo, &reverts)?);
    Ok(todo)
}

/// Adds the steps in one todo file's `text` to `todo`. Each word that names a commit is
/// resolved by `commit` (as git resolves it now); the commits a revert names go to `reverts`
/// instead, since what it brings in is their parents.
pub(crate) fn add_steps(
    text: &str,
    todo: &mut Todo,
    reverts: &mut Vec<Oid>,
    commit: &mut dyn FnMut(&str) -> Result<Oid, Unchecked>,
) -> Result<(), Unchecked> {
    let mut labels: HashSet<String> = HashSet::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let mut words = line.split_whitespace();
        let verb = words.next().unwrap_or_default();
        let mut args = words.clone().filter(|w| !w.starts_with('-'));
        let (named, label) = match verb {
            "pick" | "p" | "reword" | "r" | "edit" | "e" | "squash" | "s" | "fixup" | "f" | "revert" => (args.next(), None),
            // `merge [-C <commit>] <label>...`: its commit, and the parents it merges.
            "merge" | "m" => {
                todo.picks = None;
                let c = words.clone().skip_while(|w| !matches!(*w, "-C" | "-c")).nth(1);
                let parents: Vec<&str> =
                    words.skip_while(|w| w.starts_with('-')).filter(|w| Some(*w) != c).take_while(|w| *w != "#").collect();
                for p in parents {
                    todo.commits.extend(labelled(commit, p, &labels)?);
                }
                (c, None)
            }
            "reset" | "t" => {
                todo.picks = None;
                (None, args.next())
            }
            "label" | "l" => {
                labels.extend(args.next().map(str::to_owned));
                (None, None)
            }
            "drop" | "d" | "exec" | "x" | "break" | "b" | "update-ref" | "u" | "noop" => (None, None),
            _ => return Err(Unchecked::Unresolved(line.to_owned())),
        };
        if let Some(l) = label {
            todo.commits.extend(labelled(commit, l, &labels)?);
        }
        let Some(word) = named else { continue };
        let oid = commit(word)?;
        let revert = verb == "revert";
        if let Some(p) = todo.picks.as_mut() {
            p.push(Pick { commit: oid, revert });
        }
        if revert { reverts.push(oid) } else { todo.commits.push(oid) }
    }
    Ok(())
}

/// The commit a `reset` or `merge` step names by `word`: a label set earlier in this todo is
/// what the steps before it leave (none known yet); else `refs/rewritten/<word>` as the rebase
/// left it, else the revision `word`.
fn labelled(
    commit: &mut dyn FnMut(&str) -> Result<Oid, Unchecked>,
    word: &str,
    labels: &HashSet<String>,
) -> Result<Option<Oid>, Unchecked> {
    if labels.contains(word) || word == "[new" {
        return Ok(None);
    }
    match commit(&format!("refs/rewritten/{word}")) {
        Ok(oid) => Ok(Some(oid)),
        Err(_) => commit(word).map(Some),
    }
}
