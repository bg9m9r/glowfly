//! The rebase panel's update-ref rows (`rebase.updateRefs`): fixed rows that belong to the commit
//! or merge row right above them, and the row moves that carry them along. A row is moved with
//! its update-ref rows as one block, and never lands between another row and its own.

use super::RebasePanel;
use glowfly_core::rebase::TodoStep;
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

impl RebasePanel {
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
