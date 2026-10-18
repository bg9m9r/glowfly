//! The todo of a range that holds merge commits (`git rebase --rebase-merges`): each line of
//! history is replayed after a `reset` to where it starts, merges are redone with `merge -C`,
//! and `label` steps name the commits that later lines start from or merge in.
//!
//! The list is built in three passes over the range (oldest first):
//! 1. tips: every later parent of a merge that is in the range starts a line of its own and
//!    gets a label; HEAD is the last tip;
//! 2. branch points: a commit that two or more commits of the range have as a parent gets a
//!    label, so the lines that leave it can start from it;
//! 3. output: for each tip not shown yet, its first parents back to a commit outside the range
//!    or one already shown, oldest first, after a `reset` to where that walk stopped.
//!
//! The `label onto` line that names the new base is not a step: [`super::build`] writes it first
//! whenever the list holds structure steps.

use super::plan::item;
use super::todo::{MessageFile, guarded_amend, record_base, shell_quote};
use super::{MergeParent, MergeStep, RebaseAction, ResetTo, TodoStep, Upstream};
use crate::error::Result;
use git2::{Oid, Repository};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// The merge-preserving list for the range `oids` (oldest first, as `plan::range` walks it)
/// from `upstream` to `head`, all picks.
pub(super) fn script(repo: &Repository, head: Oid, upstream: Upstream, oids: &[Oid]) -> Result<Vec<TodoStep>> {
    let in_range: HashSet<Oid> = oids.iter().copied().collect();
    let mut parents: HashMap<Oid, Vec<Oid>> = HashMap::new();
    for o in oids {
        parents.insert(*o, repo.find_commit(*o)?.parent_ids().collect());
    }
    // Where the lines that start on the old base go: onto the new one.
    let bottom = match upstream {
        Upstream::Commit(u) => repo.merge_base(head, u).ok(),
        Upstream::Root => None,
    };
    let mut labels = Labels::default();

    let mut tips: Vec<Oid> = Vec::new();
    for o in oids {
        for p in parents[o].iter().skip(1) {
            if in_range.contains(p) && !tips.contains(p) {
                tips.push(*p);
                labels.add(*p);
            }
        }
    }
    if !tips.contains(&head) {
        tips.push(head);
    }

    let mut children: HashMap<Oid, usize> = HashMap::new();
    for o in oids {
        let mut seen = HashSet::new();
        for p in parents[o].iter().filter(|p| in_range.contains(p) && seen.insert(**p)) {
            *children.entry(*p).or_default() += 1;
        }
    }
    for o in oids {
        if children.get(o).is_some_and(|&n| n >= 2) {
            labels.add(*o);
        }
    }

    let mut shown: HashSet<Oid> = HashSet::new();
    let mut out: Vec<TodoStep> = Vec::new();
    for tip in tips {
        if shown.contains(&tip) {
            continue;
        }
        let mut line = Vec::new();
        let mut at = Some(tip);
        while let Some(o) = at.filter(|o| in_range.contains(o) && !shown.contains(o)) {
            line.push(o);
            at = parents[&o].first().copied();
        }
        line.reverse();
        let reset = match at {
            None => ResetTo::NewRoot,
            Some(o) if Some(o) == bottom => ResetTo::Onto,
            Some(o) if shown.contains(&o) => ResetTo::Label(labelled(&mut labels, &mut out, o)),
            Some(o) => ResetTo::Commit(o),
        };
        out.push(TodoStep::Reset(reset));
        for o in line {
            shown.insert(o);
            let it = item(repo, o)?;
            let ps = &parents[&o];
            if ps.len() > 1 {
                let later = ps[1..]
                    .iter()
                    .map(|p| if in_range.contains(p) { MergeParent::Label(labels.add(*p)) } else { MergeParent::Commit(*p) })
                    .collect();
                out.push(TodoStep::Merge(MergeStep { item: it, parents: later }));
            } else {
                out.push(TodoStep::Commit(it));
            }
            if let Some(name) = labels.get(o) {
                out.push(TodoStep::Label { name, at: o });
            }
        }
    }
    Ok(out)
}

/// The label of `o`, a commit already in `out`; one is added after it when it has none (which
/// the branch-point pass should rule out).
fn labelled(labels: &mut Labels, out: &mut Vec<TodoStep>, o: Oid) -> String {
    if let Some(name) = labels.get(o) {
        return name;
    }
    let name = labels.add(o);
    if let Some(at) = out.iter().position(|s| s.oid() == Some(o)) {
        out.insert(at + 1, TodoStep::Label { name: name.clone(), at: o });
    }
    name
}

/// Label names by commit: `at-` and the commit's short id, lengthened while two clash.
#[derive(Default)]
struct Labels {
    names: HashMap<Oid, String>,
    used: HashSet<String>,
}

impl Labels {
    fn get(&self, oid: Oid) -> Option<String> {
        self.names.get(&oid).cloned()
    }

    /// The commit's label, made when it has none.
    fn add(&mut self, oid: Oid) -> String {
        if let Some(name) = self.get(oid) {
            return name;
        }
        let hex = oid.to_string();
        let mut len = 7;
        let name = loop {
            let name = format!("at-{}", &hex[..len]);
            if !self.used.contains(&name) || len == hex.len() {
                break name;
            }
            len += 1;
        };
        self.used.insert(name.clone());
        self.names.insert(oid, name.clone());
        name
    }
}

/// The list holds label, reset or merge steps, so it runs with `--rebase-merges`.
pub(super) fn has_structure(steps: &[TodoStep]) -> bool {
    steps.iter().any(TodoStep::is_structure)
}

/// Writes a label, reset or merge step to `text` (with the message file of a reworded merge in
/// `files`); false for any other step, which is left to the caller. A dropped merge writes
/// nothing; a reworded one is amended after its merge the way a reworded pick is.
pub(super) fn write(step: &TodoStep, msg_dir: &Path, text: &mut String, files: &mut Vec<(PathBuf, String)>) -> bool {
    match step {
        TodoStep::Label { name, .. } => text.push_str(&format!("label {name}\n")),
        TodoStep::Reset(to) => {
            let to = match to {
                ResetTo::Onto => "onto".to_owned(),
                ResetTo::Label(name) => name.clone(),
                ResetTo::Commit(o) => o.to_string(),
                ResetTo::NewRoot => "[new root]".to_owned(),
            };
            text.push_str(&format!("reset {to}\n"));
        }
        TodoStep::Merge(m) => merge(m, msg_dir, text, files),
        _ => return false,
    }
    true
}

fn merge(m: &MergeStep, msg_dir: &Path, text: &mut String, files: &mut Vec<(PathBuf, String)>) {
    let item = &m.item;
    if item.action == RebaseAction::Drop {
        return;
    }
    let parents: Vec<String> = m
        .parents
        .iter()
        .map(|p| match p {
            MergeParent::Label(name) => name.clone(),
            MergeParent::Commit(o) => o.to_string(),
        })
        .collect();
    let summary = item.summary.replace(['\r', '\n'], " ");
    let summary = summary.trim();
    let comment = if summary.is_empty() { String::new() } else { format!(" # {summary}") };
    let line = format!("merge -C {} {}{comment}\n", item.oid, parents.join(" "));
    if !item.rewords() {
        text.push_str(&line);
        return;
    }
    // A merge's first parent is the HEAD recorded before it, so the amend's guard holds.
    let base = record_base(msg_dir, item.oid, text);
    text.push_str(&line);
    let path = msg_dir.join(MessageFile::Reword.name(item.oid));
    text.push_str(&guarded_amend(&base, &shell_quote(&path.to_string_lossy())));
    let typed = item.new_message.as_deref().unwrap_or_default();
    files.push((path, typed.trim_end().to_owned() + "\n"));
}

#[cfg(test)]
mod tests {
    use super::Labels;
    use git2::Oid;

    #[test]
    fn label_names_stay_unique() {
        let a = Oid::from_str("1234567aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let b = Oid::from_str("1234567bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();
        let mut labels = Labels::default();
        assert_eq!(labels.add(a), "at-1234567");
        assert_eq!(labels.add(b), "at-1234567b");
        assert_eq!(labels.add(a), "at-1234567", "a commit keeps its label");
    }
}
