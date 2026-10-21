//! Worktree tab groups: the open tabs whose repositories share one common git dir (`Tab::group`)
//! and sit side by side. Pure, shared by update (where tabs open and how a drop moves them) and
//! the tab bar (the colored strip and the tooltip line). An order is a list of old indexes in
//! their new places: `order[new] == old`.

use super::Tab;
use std::ops::Range;

/// Whether tabs `a` and `b` show worktrees of one repository.
pub fn same_group(a: &Tab, b: &Tab) -> bool {
    matches!((&a.group, &b.group), (Some(x), Some(y)) if x.same(y))
}

/// The tab strip cut into blocks that move as one: maximal runs of adjacent tabs of one group.
/// A tab without a group is a block of its own, and so is every tab when grouping is off.
pub fn blocks(tabs: &[Tab], on: bool) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::new();
    for i in 0..tabs.len() {
        let joins = on && i > 0 && same_group(&tabs[i - 1], &tabs[i]);
        match out.last_mut() {
            Some(b) if joins => b.end = i + 1,
            _ => out.push(i..i + 1),
        }
    }
    out
}

/// The order after tab `from` is dropped on slot `to`. Inside its own block (of 2+ tabs) it
/// moves alone; elsewhere its whole block moves, after the block holding `to` when moving right
/// and before it when moving left, so no block is ever split. With every tab its own block this
/// is the single move of `tabs::index_after_move`. Out of range, or `to == from`: unchanged.
pub fn move_order(blocks: &[Range<usize>], from: usize, to: usize) -> Vec<usize> {
    let n = blocks.last().map_or(0, |b| b.end);
    let mut order: Vec<usize> = (0..n).collect();
    if from >= n || to >= n || from == to {
        return order;
    }
    let block_of = |i: usize| blocks.iter().position(|b| b.contains(&i)).unwrap_or(0);
    let (bf, bt) = (block_of(from), block_of(to));
    if bf == bt {
        let tab = order.remove(from);
        order.insert(to, tab);
        return order;
    }
    let mut ids: Vec<usize> = (0..blocks.len()).collect();
    ids.remove(bf);
    // Moving right, `bt` shifted left by one when `bf` came out: inserting at `bt` lands after it.
    // Moving left, `bt` didn't shift: inserting there lands before it.
    ids.insert(bt, bf);
    ids.into_iter().flat_map(|b| blocks[b].clone()).collect()
}

/// The order that brings every group's tabs together: left to right, each group where its
/// leftmost tab was, its other tabs right after it in their order. Stable: tabs already grouped
/// stay where they are.
pub fn grouped_order(tabs: &[Tab]) -> Vec<usize> {
    let mut placed = vec![false; tabs.len()];
    let mut order = Vec::with_capacity(tabs.len());
    for i in 0..tabs.len() {
        if placed[i] {
            continue;
        }
        placed[i] = true;
        order.push(i);
        if tabs[i].group.is_none() {
            continue;
        }
        for j in i + 1..tabs.len() {
            if !placed[j] && same_group(&tabs[i], &tabs[j]) {
                placed[j] = true;
                order.push(j);
            }
        }
    }
    order
}

/// Where the tab at old index `old` is in `order`.
pub fn new_index(order: &[usize], old: usize) -> usize {
    order.iter().position(|&o| o == old).unwrap_or(old)
}

/// Whether `order` leaves every tab where it is.
pub fn is_identity(order: &[usize]) -> bool {
    order.iter().enumerate().all(|(i, &o)| i == o)
}

/// The groups the tab bar marks: runs of 2+ tabs of one repository, each with its number from
/// the left (which picks its color). Nothing when grouping is off.
pub fn shown_groups(tabs: &[Tab], on: bool) -> Vec<(Range<usize>, usize)> {
    blocks(tabs, on).into_iter().filter(|b| b.len() >= 2).enumerate().map(|(n, b)| (b, n)).collect()
}

/// The shown group tab `i` belongs to.
pub fn shown_at(shown: &[(Range<usize>, usize)], i: usize) -> Option<&(Range<usize>, usize)> {
    shown.iter().find(|(b, _)| b.contains(&i))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tabs::index_after_move;
    use glowfly_core::repo::CommonDir;

    /// Tabs named by their group: "a1" and "a2" are of group "a"; "x" (one letter) has none.
    fn tabs(names: &[&str]) -> Vec<Tab> {
        let base = glowfly_core::paths::temp_dir();
        names
            .iter()
            .map(|n| {
                let mut t = Tab::unloaded(base.join(n));
                if n.len() > 1 {
                    let g = &n[..1];
                    t.group = Some(CommonDir { dir: base.join(g).join(".git"), folder: None, name: g.into() });
                }
                t
            })
            .collect()
    }

    fn names(all: &[Tab], order: &[usize]) -> Vec<String> {
        order.iter().map(|&i| all[i].name.clone()).collect()
    }

    #[test]
    fn blocks_are_runs_of_one_group() {
        let t = tabs(&["a1", "a2", "x", "b1", "b2", "a3", "y"]);
        assert_eq!(blocks(&t, false), (0..7).map(|i| i..i + 1).collect::<Vec<_>>(), "off: every tab alone");
        assert_eq!(blocks(&t, true), vec![0..2, 2..3, 3..5, 5..6, 6..7], "a3 is apart from a1 and a2");
        let none = tabs(&["x", "y"]);
        assert_eq!(blocks(&none, true), vec![0..1, 1..2], "tabs without a group never join");
        assert_eq!(shown_groups(&t, true), vec![(0..2, 0), (3..5, 1)], "only groups of 2+, numbered");
        assert!(shown_groups(&t, false).is_empty());
        assert_eq!(shown_at(&shown_groups(&t, true), 4), Some(&(3..5, 1)));
    }

    #[test]
    fn singleton_blocks_move_like_one_tab() {
        let singles: Vec<Range<usize>> = (0..5).map(|i| i..i + 1).collect();
        for from in 0..5 {
            for to in 0..5 {
                let order = move_order(&singles, from, to);
                for old in 0..5 {
                    assert_eq!(new_index(&order, old), index_after_move(old, from, to), "{from} -> {to}, tab {old}");
                }
            }
        }
        assert!(is_identity(&move_order(&singles, 1, 9)), "out of range");
    }

    #[test]
    fn drops_inside_and_outside_a_group() {
        let t = tabs(&["a1", "a2", "a3", "x", "b1", "b2"]);
        let b = blocks(&t, true);
        assert_eq!(names(&t, &move_order(&b, 0, 2)), ["a2", "a3", "a1", "x", "b1", "b2"], "reordered inside its group");
        assert_eq!(names(&t, &move_order(&b, 1, 4)), ["x", "b1", "b2", "a1", "a2", "a3"], "the whole group moves right, past b");
        assert_eq!(names(&t, &move_order(&b, 1, 3)), ["x", "a1", "a2", "a3", "b1", "b2"], "past x");
        assert_eq!(names(&t, &move_order(&b, 5, 1)), ["b1", "b2", "a1", "a2", "a3", "x"], "left: before the group under the drop");
        assert_eq!(names(&t, &move_order(&b, 3, 1)), ["x", "a1", "a2", "a3", "b1", "b2"], "a lone tab snaps before a group");
        assert_eq!(names(&t, &move_order(&b, 3, 4)), ["a1", "a2", "a3", "b1", "b2", "x"], "and after one");
    }

    #[test]
    fn grouping_pulls_members_to_the_leftmost() {
        let t = tabs(&["a1", "x", "a2", "b1", "b2", "a3"]);
        assert_eq!(names(&t, &grouped_order(&t)), ["a1", "a2", "a3", "x", "b1", "b2"]);
        let done = tabs(&["a1", "a2", "x", "b1", "b2"]);
        assert!(is_identity(&grouped_order(&done)), "already grouped");
    }
}
