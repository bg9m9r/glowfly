//! Which of a row's ref labels share a chip: a local branch whose upstream (the remote-tracking
//! branch it tracks) is at the same commit draws as one label with a remote mark, and the
//! upstream gets no label of its own. Every other ref keeps its own label.

use trellis_core::repo::{BranchInfo, RefKind, RefLabel};

/// One chip in a row's BRANCH / TAG column.
pub struct Chip<'a> {
    pub label: &'a RefLabel,
    /// The local branch's upstream, at the same commit, folded into this chip.
    pub upstream: Option<&'a RefLabel>,
}

impl Chip<'_> {
    /// The chip's name in a tooltip: the ref's, saying when its upstream is level with it.
    pub fn tip(&self) -> String {
        match self.upstream {
            Some(u) => format!("{}, in sync with {}", self.label.name, u.name),
            None => self.label.name.clone(),
        }
    }
}

/// The chips for one commit's `labels` (in their order), given the repository's local branches.
/// Only a branch's own upstream folds into it; a remote branch that merely shares the commit
/// keeps its label. A ref hidden from the graph isn't among `labels`, so it folds nothing.
pub fn chips<'a>(labels: &'a [RefLabel], locals: &[BranchInfo]) -> Vec<Chip<'a>> {
    let upstream = |l: &RefLabel| {
        if l.kind != RefKind::Local {
            return None;
        }
        let full = locals.iter().find(|b| b.name == l.name)?.upstream_ref.as_deref()?;
        labels.iter().position(|r| r.kind == RefKind::Remote && r.full == full)
    };
    let ups: Vec<Option<usize>> = labels.iter().map(upstream).collect();
    labels
        .iter()
        .enumerate()
        .filter(|(i, _)| !ups.contains(&Some(*i)))
        .map(|(i, label)| Chip { label, upstream: ups[i].map(|j| &labels[j]) })
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

    /// Each chip as "name" or "name+upstream".
    fn shown(labels: &[RefLabel], locals: &[BranchInfo]) -> Vec<String> {
        chips(labels, locals)
            .iter()
            .map(|c| match c.upstream {
                Some(u) => format!("{}+{}", c.label.name, u.name),
                None => c.label.name.clone(),
            })
            .collect()
    }

    #[test]
    fn a_branch_and_its_upstream_share_one_chip() {
        let labels = [label("refs/heads/main"), label("refs/remotes/origin/main")];
        let locals = [branch("main", Some("origin/main"))];
        assert_eq!(shown(&labels, &locals), ["main+origin/main"]);
        assert_eq!(chips(&labels, &locals)[0].tip(), "main, in sync with origin/main");
    }

    #[test]
    fn only_the_branchs_own_upstream_folds() {
        // main tracks upstream/main; origin/main is just another remote branch at the commit.
        let labels = [label("refs/heads/main"), label("refs/remotes/origin/main"), label("refs/remotes/upstream/main")];
        let locals = [branch("main", Some("upstream/main"))];
        assert_eq!(shown(&labels, &locals), ["main+upstream/main", "origin/main"]);
    }

    #[test]
    fn untracked_and_lone_refs_keep_their_labels() {
        let labels = [label("refs/heads/main"), label("refs/remotes/origin/main"), label("refs/tags/v1")];
        assert_eq!(shown(&labels, &[branch("main", None)]), ["main", "origin/main", "v1"], "no upstream");
        assert_eq!(shown(&labels, &[]), ["main", "origin/main", "v1"], "a branch the snapshot doesn't list");
        // A local branch tracking a remote branch elsewhere: both stay.
        let locals = [branch("main", Some("origin/other"))];
        assert_eq!(shown(&labels, &locals), ["main", "origin/main", "v1"], "upstream at another commit");
    }

    #[test]
    fn a_hidden_ref_folds_nothing() {
        let locals = [branch("main", Some("origin/main"))];
        // The upstream hidden: the plain local label.
        assert_eq!(shown(&[label("refs/heads/main")], &locals), ["main"]);
        // The branch hidden: the remote's own label.
        assert_eq!(shown(&[label("refs/remotes/origin/main")], &locals), ["origin/main"]);
    }

    #[test]
    fn several_branches_fold_their_own_upstreams_in_order() {
        let labels = [
            label("refs/heads/feature"),
            label("refs/heads/main"),
            label("refs/remotes/origin/feature"),
            label("refs/remotes/origin/main"),
            label("refs/tags/v1"),
        ];
        let locals = [branch("main", Some("origin/main")), branch("feature", Some("origin/feature"))];
        assert_eq!(shown(&labels, &locals), ["feature+origin/feature", "main+origin/main", "v1"]);
    }
}
