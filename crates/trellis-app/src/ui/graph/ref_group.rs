//! Which of a row's ref labels share a chip. A local branch's chip takes in the remote-tracking
//! branches at the same commit that are copies of it: its upstream (whatever its name) and every
//! ref with its name on any remote. Same-commit copies on several remotes with no local branch
//! claiming them share one remote chip. A folded chip draws a remote mark and its tooltip names
//! the copies; tags, HEAD and every other ref keep their own labels.

use crate::state::branches::default_remote;
use crate::state::remote_branch::split_among;
use crate::state::tags::remote_list_in;
use trellis_core::i18n::Lang;
use trellis_core::repo::{BranchInfo, RefKind, RefLabel, Snapshot};

/// What grouping needs from the snapshot; built once per frame.
pub struct RefContext<'a> {
    pub locals: &'a [BranchInfo],
    /// The remotes' names, in the snapshot's order.
    pub remotes: Vec<&'a str>,
    /// The remote whose copy leads a remote-only chip when no local branch tracks one.
    pub default_remote: Option<String>,
}

impl<'a> RefContext<'a> {
    pub fn of(snap: &'a Snapshot) -> Self {
        Self { locals: &snap.locals, remotes: snap.remotes.iter().map(|(r, _)| r.as_str()).collect(), default_remote: default_remote(snap) }
    }
}

/// One chip in a row's BRANCH / TAG column.
pub struct Chip<'a> {
    pub label: &'a RefLabel,
    /// The remote-tracking refs at the same commit folded into this chip, a local branch's
    /// upstream first. Empty for a chip that folds nothing.
    pub remotes: Vec<&'a RefLabel>,
}

impl Chip<'_> {
    /// Whether the chip stands for copies of the branch on remotes too.
    pub fn folds(&self) -> bool {
        !self.remotes.is_empty()
    }

    /// The chip's name in a tooltip: the ref's, naming the copies level with it.
    pub fn tip(&self, t: Lang) -> String {
        if !self.folds() {
            return self.label.name.clone();
        }
        let names: Vec<String> = self.remotes.iter().map(|r| r.name.clone()).collect();
        t.trf("graph.refs.in_sync.tip", &[("name", &self.label.name), ("others", &remote_list_in(t, &names))])
    }
}

/// The upstream ref of local branch `name`, when the snapshot lists one.
fn upstream_of<'a>(cx: &'a RefContext, name: &str) -> Option<&'a str> {
    cx.locals.iter().find(|b| b.name == name)?.upstream_ref.as_deref()
}

/// The copy that leads remote-only `group` (indexes into `labels`): the one the checked-out
/// branch tracks, else one any local branch tracks, else the default remote's, else the first.
fn lead(group: &[usize], labels: &[RefLabel], cx: &RefContext) -> usize {
    let tracked = |j: usize, head_only: bool| {
        let full = Some(labels[j].full.as_str());
        cx.locals.iter().any(|b| (b.is_head || !head_only) && b.upstream_ref.as_deref() == full)
    };
    let on_default = |j: usize| {
        let remote = split_among(&cx.remotes, &labels[j].name).map(|(r, _)| r);
        remote.is_some() && remote == cx.default_remote.as_deref()
    };
    let pick = |f: &dyn Fn(usize) -> bool| group.iter().copied().find(|&j| f(j));
    pick(&|j| tracked(j, true)).or_else(|| pick(&|j| tracked(j, false))).or_else(|| pick(&on_default)).unwrap_or(group[0])
}

/// The chips for one commit's `labels` (in their order). Each remote-tracking ref folds into at
/// most one chip: the local branch it is the upstream of, else the local branch with its branch
/// name, else a chip shared with its copies on other remotes. A ref hidden from the graph isn't
/// among `labels`, so it folds nothing.
pub fn chips<'a>(labels: &'a [RefLabel], cx: &RefContext) -> Vec<Chip<'a>> {
    let all = 0..labels.len();
    let is = |i: usize, kind: RefKind| labels[i].kind == kind;
    // The chip each remote label folds into, by the index of its lead label.
    let mut owner: Vec<Option<usize>> = vec![None; labels.len()];
    for i in all.clone().filter(|&i| is(i, RefKind::Local)) {
        let Some(full) = upstream_of(cx, &labels[i].name) else { continue };
        if let Some(j) = all.clone().find(|&j| is(j, RefKind::Remote) && labels[j].full == full && owner[j].is_none()) {
            owner[j] = Some(i);
        }
    }
    // Copies by branch name, then copies on several remotes, grouped in first-appearance order.
    let unowned: Vec<usize> = all.clone().filter(|&j| is(j, RefKind::Remote) && owner[j].is_none()).collect();
    let mut groups: Vec<(&str, Vec<usize>)> = Vec::new();
    for j in unowned {
        let Some((_, branch)) = split_among(&cx.remotes, &labels[j].name) else { continue };
        if let Some(i) = all.clone().find(|&i| is(i, RefKind::Local) && labels[i].name == branch) {
            owner[j] = Some(i);
        } else if let Some((_, g)) = groups.iter_mut().find(|(b, _)| *b == branch) {
            g.push(j);
        } else {
            groups.push((branch, vec![j]));
        }
    }
    for (_, g) in groups.iter().filter(|(_, g)| g.len() > 1) {
        let first = lead(g, labels, cx);
        for &j in g.iter().filter(|&&j| j != first) {
            owner[j] = Some(first);
        }
    }
    all.clone()
        .filter(|&i| owner[i].is_none())
        .map(|i| {
            let mut remotes: Vec<&RefLabel> = all.clone().filter(|&j| owner[j] == Some(i)).map(|j| &labels[j]).collect();
            if is(i, RefKind::Local) {
                let up = upstream_of(cx, &labels[i].name);
                remotes.sort_by_key(|r| Some(r.full.as_str()) != up);
            }
            Chip { label: &labels[i], remotes }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(full: &str) -> RefLabel {
        let (kind, name) = if let Some(n) = full.strip_prefix("refs/heads/") {
            (RefKind::Local, n)
        } else if let Some(n) = full.strip_prefix("refs/remotes/") {
            (RefKind::Remote, n)
        } else {
            (RefKind::Tag, full.strip_prefix("refs/tags/").unwrap())
        };
        RefLabel { name: name.into(), full: full.into(), kind, is_head: false }
    }

    fn branch(name: &str, upstream: Option<&str>) -> BranchInfo {
        BranchInfo {
            name: name.into(),
            oid: None,
            is_head: false,
            upstream: upstream.map(|u| u.into()),
            ahead: 0,
            behind: 0,
            tracking: None,
            upstream_ref: upstream.map(|u| format!("refs/remotes/{u}")),
            merged: false,
        }
    }

    fn head(name: &str, upstream: Option<&str>) -> BranchInfo {
        BranchInfo { is_head: true, ..branch(name, upstream) }
    }

    fn cx<'a>(locals: &'a [BranchInfo], remotes: &[&'a str], default: Option<&str>) -> RefContext<'a> {
        RefContext { locals, remotes: remotes.to_vec(), default_remote: default.map(str::to_owned) }
    }

    /// The usual remotes: origin (the default), fork, upstream and backup.
    fn std_cx(locals: &[BranchInfo]) -> RefContext<'_> {
        cx(locals, &["origin", "fork", "upstream", "backup"], Some("origin"))
    }

    /// Each chip as "name" or "name+copy1+copy2".
    fn shown(labels: &[RefLabel], cx: &RefContext) -> Vec<String> {
        chips(labels, cx)
            .iter()
            .map(|c| std::iter::once(&c.label.name).chain(c.remotes.iter().map(|r| &r.name)).cloned().collect::<Vec<_>>().join("+"))
            .collect()
    }

    fn labels(fulls: &[&str]) -> Vec<RefLabel> {
        fulls.iter().map(|f| label(f)).collect()
    }

    #[test]
    fn a_branch_and_its_upstream_share_one_chip() {
        let labels = labels(&["refs/heads/main", "refs/remotes/origin/main"]);
        let locals = [branch("main", Some("origin/main"))];
        let cx = std_cx(&locals);
        assert_eq!(shown(&labels, &cx), ["main+origin/main"]);
        assert_eq!(chips(&labels, &cx)[0].tip(Lang::En), "main, in sync with origin/main");
    }

    #[test]
    fn every_same_name_copy_folds_upstream_first() {
        let labels = labels(&["refs/heads/main", "refs/remotes/origin/main", "refs/remotes/upstream/main"]);
        let locals = [branch("main", Some("upstream/main"))];
        let cx = std_cx(&locals);
        assert_eq!(shown(&labels, &cx), ["main+upstream/main+origin/main"]);
        assert_eq!(chips(&labels, &cx)[0].tip(Lang::En), "main, in sync with upstream/main and origin/main");
    }

    #[test]
    fn untracked_and_unlisted_branches_fold_copies_by_name() {
        let labels = labels(&["refs/heads/main", "refs/remotes/origin/main", "refs/tags/v1"]);
        let none = [branch("main", None)];
        assert_eq!(shown(&labels, &std_cx(&none)), ["main+origin/main", "v1"], "no upstream");
        assert_eq!(shown(&labels, &std_cx(&[])), ["main+origin/main", "v1"], "a branch the snapshot doesn't list");
        let elsewhere = [branch("main", Some("origin/other"))];
        assert_eq!(shown(&labels, &std_cx(&elsewhere)), ["main+origin/main", "v1"], "upstream at another commit");
    }

    #[test]
    fn a_hidden_ref_folds_nothing() {
        let locals = [branch("main", Some("origin/main"))];
        let cx = std_cx(&locals);
        // The upstream hidden: the plain local label.
        assert_eq!(shown(&labels(&["refs/heads/main"]), &cx), ["main"]);
        // The branch hidden: the remote's own label, or one chip for its copies.
        assert_eq!(shown(&labels(&["refs/remotes/origin/main"]), &cx), ["origin/main"]);
        let copies = labels(&["refs/remotes/fork/main", "refs/remotes/origin/main"]);
        assert_eq!(shown(&copies, &cx), ["origin/main+fork/main"]);
    }

    #[test]
    fn several_branches_fold_their_own_upstreams_in_order() {
        let labels =
            labels(&["refs/heads/feature", "refs/heads/main", "refs/remotes/origin/feature", "refs/remotes/origin/main", "refs/tags/v1"]);
        let locals = [branch("main", Some("origin/main")), branch("feature", Some("origin/feature"))];
        assert_eq!(shown(&labels, &std_cx(&locals)), ["feature+origin/feature", "main+origin/main", "v1"]);
    }

    #[test]
    fn different_names_and_tags_keep_their_labels() {
        let labels = labels(&["refs/heads/main", "refs/remotes/origin/dev", "refs/tags/main"]);
        assert_eq!(shown(&labels, &std_cx(&[branch("main", None)])), ["main", "origin/dev", "main"]);
        // Names compare exactly.
        let labels = self::labels(&["refs/heads/Main", "refs/remotes/origin/main"]);
        assert_eq!(shown(&labels, &std_cx(&[])), ["Main", "origin/main"]);
    }

    #[test]
    fn remote_names_with_slashes_split_by_the_longest_remote() {
        let labels = labels(&["refs/heads/x", "refs/remotes/team/a/x"]);
        assert_eq!(shown(&labels, &cx(&[], &["origin", "team/a"], Some("origin"))), ["x+team/a/x"]);
        // With only remote team, the branch is a/x.
        let only_team = cx(&[], &["team"], Some("team"));
        assert_eq!(shown(&labels, &only_team), ["x", "team/a/x"]);
        let ax = self::labels(&["refs/heads/a/x", "refs/remotes/team/a/x"]);
        assert_eq!(shown(&ax, &only_team), ["a/x+team/a/x"]);
    }

    #[test]
    fn a_copy_on_an_unknown_remote_keeps_its_label() {
        let labels = labels(&["refs/heads/x", "refs/remotes/gone/x"]);
        assert_eq!(shown(&labels, &std_cx(&[branch("x", None)])), ["x", "gone/x"]);
        assert_eq!(shown(&labels, &std_cx(&[branch("x", Some("gone/x"))])), ["x+gone/x"], "its exact upstream");
        let copies = self::labels(&["refs/remotes/gone/x", "refs/remotes/origin/x"]);
        assert_eq!(shown(&copies, &std_cx(&[])), ["gone/x", "origin/x"]);
    }

    #[test]
    fn a_copy_another_branch_tracks_folds_into_that_branch() {
        let labels = labels(&["refs/heads/x", "refs/heads/y", "refs/remotes/origin/x"]);
        let locals = [branch("x", None), branch("y", Some("origin/x"))];
        assert_eq!(shown(&labels, &std_cx(&locals)), ["x", "y+origin/x"]);
    }

    #[test]
    fn remote_only_copies_share_one_chip_led_by_the_default_remote() {
        let labels = labels(&["refs/remotes/fork/x", "refs/remotes/origin/x"]);
        let cx = cx(&[], &["fork", "origin"], Some("origin"));
        assert_eq!(shown(&labels, &cx), ["origin/x+fork/x"]);
        assert_eq!(chips(&labels, &cx)[0].tip(Lang::En), "origin/x, in sync with fork/x");
    }

    #[test]
    fn remote_only_lead_prefers_a_tracked_copy() {
        // x isn't on this commit, so its upstream leads instead of the default remote's copy.
        let labels = labels(&["refs/remotes/fork/x", "refs/remotes/origin/x"]);
        let locals = [branch("x", Some("fork/x"))];
        assert_eq!(shown(&labels, &std_cx(&locals)), ["fork/x+origin/x"]);
        // The checked-out branch's upstream beats one another branch tracks.
        let labels = self::labels(&["refs/remotes/backup/x", "refs/remotes/fork/x", "refs/remotes/origin/x"]);
        let locals = [branch("y", Some("backup/x")), head("z", Some("fork/x"))];
        assert_eq!(shown(&labels, &std_cx(&locals)), ["fork/x+backup/x+origin/x"]);
    }

    #[test]
    fn remote_only_lead_falls_back_to_the_first_copy() {
        let labels = labels(&["refs/remotes/a/x", "refs/remotes/b/x"]);
        assert_eq!(shown(&labels, &cx(&[], &["origin", "a", "b"], Some("origin"))), ["a/x+b/x"]);
    }

    #[test]
    fn different_branch_parts_dont_group() {
        let labels = labels(&["refs/remotes/fork/x", "refs/remotes/origin/y"]);
        assert_eq!(shown(&labels, &std_cx(&[])), ["fork/x", "origin/y"]);
    }

    #[test]
    fn three_copies_join_with_commas_and_and() {
        let labels = labels(&["refs/heads/x", "refs/remotes/origin/x", "refs/remotes/fork/x", "refs/remotes/backup/x"]);
        let locals = [branch("x", Some("origin/x"))];
        let cx = std_cx(&locals);
        let chip = &chips(&labels, &cx)[0];
        assert_eq!(chip.tip(Lang::En), "x, in sync with origin/x, fork/x and backup/x");
        assert_eq!(chip.tip(Lang::De), "x, auf dem Stand von origin/x, fork/x und backup/x");
    }
}
