//! The LOCAL section's rows as a tree of stacks (pure): each branch that isn't stacked where
//! it is in the branch list, with the branches stacked on it right under it, indented one step
//! per layer. Flat while the filter field has text.

use std::collections::{HashMap, HashSet};
use trellis_core::i18n::Lang;
use trellis_core::repo::{BranchInfo, Snapshot};
use trellis_core::stack::StackLink;

/// Indentation stops growing after this many layers.
pub const MAX_DEPTH: usize = 4;

/// One LOCAL row: the branch, how far it is indented, and its link to the branch below.
pub struct LocalRow<'a> {
    pub branch: &'a BranchInfo,
    pub depth: usize,
    pub link: Option<&'a StackLink>,
}

/// The rows of the branches `shown` keeps, in `snap.locals` order: a tree unless `filtering`.
/// Linear in the number of branches: it runs every frame the section is drawn.
pub fn local_rows<'a>(snap: &'a Snapshot, shown: &dyn Fn(&str) -> bool, filtering: bool) -> Vec<LocalRow<'a>> {
    let visible: Vec<&BranchInfo> = snap.locals.iter().filter(|b| shown(&b.name)).collect();
    let links: HashMap<&str, &StackLink> = snap.stacks.links.iter().map(|l| (l.branch.as_str(), l)).collect();
    let row = |branch: &'a BranchInfo, depth: usize| LocalRow { branch, depth, link: links.get(branch.name.as_str()).copied() };
    if filtering || links.is_empty() {
        return visible.into_iter().map(|b| row(b, 0)).collect();
    }
    let listed: HashSet<&str> = visible.iter().map(|b| b.name.as_str()).collect();
    // Each listed branch's children, in list order; a lower branch that isn't listed makes a root.
    let mut children: HashMap<&str, Vec<&BranchInfo>> = HashMap::new();
    let mut roots: Vec<&BranchInfo> = Vec::new();
    for b in &visible {
        match links.get(b.name.as_str()).map(|l| l.on.as_str()).filter(|on| listed.contains(on)) {
            Some(on) => children.entry(on).or_default().push(b),
            None => roots.push(b),
        }
    }
    let mut out: Vec<LocalRow<'a>> = Vec::with_capacity(visible.len());
    let mut seen: HashSet<&str> = HashSet::with_capacity(visible.len());
    // Roots first; branches only a cycle reaches come last, unindented.
    for root in roots.into_iter().chain(visible.iter().copied()) {
        let mut todo = vec![(root, 0)];
        while let Some((b, depth)) = todo.pop() {
            if !seen.insert(b.name.as_str()) {
                continue;
            }
            out.push(row(b, depth.min(MAX_DEPTH)));
            // Reversed, so the first child (in list order) is drawn first.
            let kids = children.get(b.name.as_str()).map(Vec::as_slice).unwrap_or_default();
            todo.extend(kids.iter().rev().map(|c| (*c, depth + 1)));
        }
    }
    out
}

/// The row tooltip's line for a stacked branch: "Stacked on search", or that it has moved on.
pub fn tip_line(t: Lang, link: Option<&StackLink>) -> Option<String> {
    let l = link?;
    let args: &[(&str, &dyn std::fmt::Display)] = &[("branch", &l.on)];
    Some(if l.behind { t.trf("sidebar.local.stacked_behind.tip", args) } else { t.trf("sidebar.local.stacked_on.tip", args) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Oid;
    use trellis_core::stack::Stacks;

    fn branch(name: &str) -> BranchInfo {
        BranchInfo {
            name: name.into(),
            oid: None,
            is_head: false,
            upstream: None,
            ahead: 0,
            behind: 0,
            tracking: None,
            upstream_ref: None,
            merged: false,
        }
    }

    fn snap(name: &str, locals: &[&str], links: &[(&str, &str)]) -> Snapshot {
        let mut s = crate::state::op_fixture::snapshot(name);
        s.locals = locals.iter().map(|n| branch(n)).collect();
        let link = |(b, on): &(&str, &str)| StackLink { branch: (*b).into(), on: (*on).into(), base: Oid::zero(), behind: false };
        s.stacks = Stacks { links: links.iter().map(link).collect() };
        s
    }

    fn shape(rows: &[LocalRow]) -> Vec<(String, usize)> {
        rows.iter().map(|r| (r.branch.name.clone(), r.depth)).collect()
    }

    fn all(_: &str) -> bool {
        true
    }

    fn owned(list: &[(&str, usize)]) -> Vec<(String, usize)> {
        list.iter().map(|(n, d)| ((*n).to_owned(), *d)).collect()
    }

    #[test]
    fn a_chain_is_drawn_as_a_tree() {
        let s = snap("rows-chain", &["a", "b", "c", "z"], &[("b", "a"), ("c", "b")]);
        assert_eq!(shape(&local_rows(&s, &all, false)), owned(&[("a", 0), ("b", 1), ("c", 2), ("z", 0)]));
    }

    #[test]
    fn children_follow_their_lower_branch() {
        let s = snap("rows-order", &["alpha", "mid", "zed"], &[("alpha", "zed")]);
        assert_eq!(shape(&local_rows(&s, &all, false)), owned(&[("mid", 0), ("zed", 0), ("alpha", 1)]));
    }

    #[test]
    fn depth_is_capped() {
        let names = ["b0", "b1", "b2", "b3", "b4", "b5"];
        let links: Vec<(&str, &str)> = names.windows(2).map(|w| (w[1], w[0])).collect();
        let s = snap("rows-deep", &names, &links);
        let depths: Vec<usize> = local_rows(&s, &all, false).iter().map(|r| r.depth).collect();
        assert_eq!(depths, [0, 1, 2, 3, MAX_DEPTH, MAX_DEPTH]);
    }

    #[test]
    fn filtering_flattens_but_keeps_the_link() {
        let s = snap("rows-filter", &["a", "b"], &[("b", "a")]);
        let rows = local_rows(&s, &all, true);
        assert_eq!(shape(&rows), owned(&[("a", 0), ("b", 0)]));
        assert_eq!(rows[1].link.map(|l| l.on.as_str()), Some("a"));
        assert_eq!(tip_line(Lang::En, rows[1].link).as_deref(), Some("Stacked on a"));
        assert_eq!(tip_line(Lang::En, rows[0].link), None);
    }

    #[test]
    fn a_lower_branch_that_is_not_listed_leaves_the_child_unindented() {
        let s = snap("rows-gone", &["a", "b"], &[("b", "gone")]);
        let rows = local_rows(&s, &all, false);
        assert_eq!(shape(&rows), owned(&[("a", 0), ("b", 0)]));
        assert!(rows[1].link.is_some());
        let hidden = |n: &str| n != "a";
        let s = snap("rows-hidden", &["a", "b"], &[("b", "a")]);
        assert_eq!(shape(&local_rows(&s, &hidden, false)), owned(&[("b", 0)]), "a filtered-out lower keeps b at depth 0");
    }

    #[test]
    fn a_behind_link_says_so() {
        let mut s = snap("rows-behind", &["a", "b"], &[("b", "a")]);
        s.stacks.links[0].behind = true;
        let line = tip_line(Lang::En, s.stacks.below("b")).unwrap();
        assert_eq!(line, "Stacked on a, which has moved on. Restack to bring it up to date.");
        assert_ne!(tip_line(Lang::De, s.stacks.below("b")).unwrap(), line);
    }
}
