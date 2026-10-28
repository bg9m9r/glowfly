//! The rebase panel's update-ref rows (`rebase.updateRefs`): fixed rows that belong to the commit
//! or merge row right above them, and the row moves that carry them along. A row is moved with
//! its update-ref rows as one block, and never lands between another row and its own. Also the
//! branches an edit leaves on another commit (asked about before Start), and the branch aliases
//! that hold a rebase back, in the user's words.

use super::RebasePanel;
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::rebase::{RebaseAction, TodoStep};
use std::ops::Range;

fn is_ref(step: Option<&TodoStep>) -> bool {
    matches!(step, Some(TodoStep::UpdateRef(_)))
}

/// The rows that move with row `i`: itself and, for a commit or merge row, the update-ref rows
/// right after it.
fn block(items: &[TodoStep], i: usize) -> Range<usize> {
    let mut end = i + 1;
    if items[i].item().is_some() {
        while is_ref(items.get(end)) {
            end += 1;
        }
    }
    i..end
}

/// The block row `i` is part of: an update-ref row's is its commit's (one after an exec or break
/// row, or at the top, stands alone).
fn block_of(items: &[TodoStep], i: usize) -> Range<usize> {
    let mut owner = i;
    while owner > 0 && is_ref(items.get(owner)) {
        owner -= 1;
    }
    if owner == i || items[owner].item().is_some() { block(items, owner) } else { i..i + 1 }
}

/// The rows in their order (as old indices) after row `from` moves to `to`: its block goes
/// after the block holding row `to` when moving down, before it when moving up. `None` when
/// nothing changes.
pub(in crate::state) fn moved_order(items: &[TodoStep], from: usize, to: usize) -> Option<Vec<usize>> {
    let n = items.len();
    if from >= n || to >= n || from == to {
        return None;
    }
    let moving = block(items, from);
    let rest: Vec<usize> = (0..n).filter(|k| !moving.contains(k)).collect();
    let at = if to > from {
        let target = block_of(items, to.max(moving.end).min(n - 1));
        if target.start < moving.end {
            return None;
        }
        rest.iter().filter(|&&k| k < target.end).count()
    } else {
        let target = block_of(items, to);
        rest.iter().filter(|&&k| k < target.start).count()
    };
    let mut order = rest;
    order.splice(at..at, moving);
    (!order.iter().copied().eq(0..n)).then_some(order)
}

/// The commit the update-ref row at `i` leaves its branch on, and whether that commit melds
/// into an earlier one: inside a squash group the last squash or fixup after it (the list writes
/// the row after the group), else the nearest kept commit or merge row above it (exec and break
/// steps don't move HEAD). `None` when none is above it: a commit the rebase already made, or
/// the base.
fn lands_on(items: &[TodoStep], i: usize) -> Option<(Oid, bool)> {
    let kept = |s: &&TodoStep| match s {
        TodoStep::UpdateRef(_) => false,
        TodoStep::Commit(c) => c.action != RebaseAction::Drop,
        _ => true,
    };
    if let Some(c) = items[i + 1..].iter().find(kept).and_then(TodoStep::commit).filter(|c| c.action.combines()) {
        return Some((c.oid, true));
    }
    items[..i].iter().rev().find_map(|s| s.item().filter(|c| c.action != RebaseAction::Drop).map(|c| (c.oid, c.action.combines())))
}

/// Each update-ref row's branch (full ref name) and where it lands.
fn landings(items: &[TodoStep]) -> Vec<(&str, Option<(Oid, bool)>)> {
    items
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match s {
            TodoStep::UpdateRef(r) => Some((r.as_str(), lands_on(items, i))),
            _ => None,
        })
        .collect()
}

impl RebasePanel {
    /// The branches (short names) the edited list leaves on another commit than the list as
    /// loaded: the commit they point at is dropped or melds into another (or another into it),
    /// or another commit moved above a branch whose commit is already done.
    pub fn moved_refs(&self) -> Vec<String> {
        let before = landings(&self.original);
        landings(&self.items)
            .into_iter()
            .filter(|(r, now)| before.iter().find(|(b, _)| b == r).is_some_and(|(_, was)| was != now))
            .map(|(r, _)| r.strip_prefix("refs/heads/").unwrap_or(r).to_owned())
            .collect()
    }

    /// The paragraph Start's question adds for [`Self::moved_refs`], if any branch moves.
    pub fn moved_refs_note(&self, t: Lang) -> Option<String> {
        let refs = self.moved_refs();
        (!refs.is_empty()).then(|| t.trf("rebase.update_ref.moved.body", &[("branches", &refs.join(", "))]))
    }

    /// Why a branch alias holds the rebase back, in the user's words, when `blocker` is that
    /// reason (`start_blocker` gives it in English).
    pub fn held_text(&self, t: Lang, blocker: &str) -> Option<String> {
        let h = self.plan.as_ref()?.held.first().filter(|h| h.text() == blocker)?;
        let key = if h.elsewhere { "rebase.update_ref.alias_busy" } else { "rebase.update_ref.alias_not_branch" };
        Some(t.trf(key, &[("alias", &h.alias), ("target", &h.target)]))
    }

    /// Where a step added at `at` goes instead: below the update-ref rows there, so it doesn't
    /// come between a commit and its branches.
    pub(in crate::state) fn past_refs(&self, mut at: usize) -> usize {
        while is_ref(self.items.get(at)) {
            at += 1;
        }
        at
    }
}

#[cfg(test)]
mod tests {
    use super::moved_order;
    use git2::Oid;
    use glowfly_core::rebase::{RebaseAction, TodoItem, TodoStep};

    fn commit(n: u8) -> TodoStep {
        let oid = Oid::from_bytes(&[n; 20]).unwrap();
        let summary = format!("c{n}");
        TodoStep::Commit(TodoItem {
            oid,
            action: RebaseAction::Pick,
            summary: summary.clone(),
            message: summary,
            new_message: None,
            combined_message: None,
            author: String::new(),
            time: 0,
        })
    }

    fn ur(b: &str) -> TodoStep {
        TodoStep::UpdateRef(format!("refs/heads/{b}"))
    }

    /// c1, lower, c2, c3, mid, other, c4
    fn list() -> Vec<TodoStep> {
        vec![commit(1), ur("lower"), commit(2), commit(3), ur("mid"), ur("other"), commit(4)]
    }

    #[test]
    fn a_commit_moves_with_its_update_refs() {
        let items = list();
        // c1 down one: past c2.
        assert_eq!(moved_order(&items, 0, 1), Some(vec![2, 0, 1, 3, 4, 5, 6]));
        assert_eq!(moved_order(&items, 0, 2), Some(vec![2, 0, 1, 3, 4, 5, 6]));
        // c3 up one: before c2; dropped onto lower's row: before c1's block.
        assert_eq!(moved_order(&items, 3, 2), Some(vec![0, 1, 3, 4, 5, 2, 6]));
        assert_eq!(moved_order(&items, 3, 1), Some(vec![3, 4, 5, 0, 1, 2, 6]));
        // c2 down onto mid's row: past c3's whole block.
        assert_eq!(moved_order(&items, 2, 4), Some(vec![0, 1, 3, 4, 5, 2, 6]));
        // c4 up onto other's row: before c3.
        assert_eq!(moved_order(&items, 6, 5), Some(vec![0, 1, 2, 6, 3, 4, 5]));
    }

    #[test]
    fn moves_that_change_nothing_are_none() {
        let items = list();
        assert_eq!(moved_order(&items, 3, 3), None);
        assert_eq!(moved_order(&items, 6, 7), None);
        // Down within its own block, with nothing after it.
        let tail = vec![commit(1), commit(2), ur("top")];
        assert_eq!(moved_order(&tail, 1, 2), None);
    }

    #[test]
    fn an_update_ref_after_an_exec_step_stands_alone() {
        let items = vec![commit(1), TodoStep::Exec("make".into()), ur("x"), commit(2)];
        assert_eq!(moved_order(&items, 0, 1), Some(vec![1, 0, 2, 3]));
        assert_eq!(moved_order(&items, 3, 2), Some(vec![0, 1, 3, 2]));
    }
}
