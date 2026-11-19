//! The local flow branches by kind, for the Git Flow panel and the left panel's GIT FLOW section.

use super::version_order::version_key;
use super::{Branches, FlowKind, GitFlowSettings, flow_kind, short_name};

/// One local flow branch.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowBranch<'b> {
    /// The full branch name ("feature/login").
    pub name: &'b str,
    /// The name after the prefix ("login").
    pub short: &'b str,
    /// It is the checked-out branch.
    pub head: bool,
}

/// The local branches of one kind.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowGroup<'b> {
    pub kind: FlowKind,
    pub branches: Vec<FlowBranch<'b>>,
}

impl FlowGroup<'_> {
    pub fn len(&self) -> usize {
        self.branches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.branches.is_empty()
    }
}

/// Feature, release and hotfix groups, always all three in `FlowKind::ALL` order (possibly
/// empty). The feature group keeps snapshot (`b.locals`) order; the release and hotfix groups
/// are in version order (digit runs as numbers, a leading `v` ignored, a suffixed version before
/// the bare one, names without digits first in snapshot order, equal names in snapshot order;
/// see `version_order.rs`). A branch's kind comes from `flow_kind` (longest prefix wins;
/// production, development and prefix-only names are left out), so the panel, the section and
/// Finish agree.
pub fn grouped<'b>(s: &GitFlowSettings, b: &Branches<'b>) -> Vec<FlowGroup<'b>> {
    let mut groups: Vec<FlowGroup<'b>> = FlowKind::ALL.into_iter().map(|kind| FlowGroup { kind, branches: Vec::new() }).collect();
    for name in b.locals.iter().copied() {
        let Some(kind) = flow_kind(s, name) else { continue };
        let Some(g) = groups.iter_mut().find(|g| g.kind == kind) else { continue };
        g.branches.push(FlowBranch { name, short: short_name(s, kind, name), head: Some(name) == b.head });
    }
    for g in groups.iter_mut().filter(|g| g.kind.versioned()) {
        g.branches.sort_by_cached_key(|b| version_key(b.short));
    }
    groups
}

/// The number of branches in all the groups.
pub fn total(groups: &[FlowGroup]) -> usize {
    groups.iter().map(FlowGroup::len).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitflow::{DEFAULT_FEATURE, DEFAULT_HOTFIX, DEFAULT_RELEASE};
    use crate::repo::{LoadOptions, load};
    use crate::testutil::{git, scratch_repo, write};

    fn defaults() -> GitFlowSettings {
        GitFlowSettings {
            production: "main".into(),
            development: "develop".into(),
            feature: DEFAULT_FEATURE.into(),
            release: DEFAULT_RELEASE.into(),
            hotfix: DEFAULT_HOTFIX.into(),
            version_tag: "v".into(),
        }
    }

    fn branches<'a>(locals: &[&'a str], head: Option<&'a str>) -> Branches<'a> {
        Branches { locals: locals.to_vec(), remote: vec![], tags: vec![], head, has_commits: true }
    }

    fn shorts<'a>(groups: &'a [FlowGroup]) -> Vec<(FlowKind, Vec<&'a str>)> {
        groups.iter().map(|g| (g.kind, g.branches.iter().map(|b| b.short).collect())).collect()
    }

    #[test]
    fn groups_by_kind_in_order() {
        let locals = ["main", "develop", "feature/login", "feature/", "release/1.2.0", "hotfix/1.2.1", "other"];
        let groups = grouped(&defaults(), &branches(&locals, None));
        assert_eq!(
            shorts(&groups),
            [(FlowKind::Feature, vec!["login"]), (FlowKind::Release, vec!["1.2.0"]), (FlowKind::Hotfix, vec!["1.2.1"])]
        );
        assert_eq!(groups[0].branches[0].name, "feature/login");
        assert_eq!(total(&groups), 3);
    }

    #[test]
    fn empty_groups_are_kept() {
        let groups = grouped(&defaults(), &branches(&["main", "develop"], Some("main")));
        assert_eq!(shorts(&groups), [(FlowKind::Feature, vec![]), (FlowKind::Release, vec![]), (FlowKind::Hotfix, vec![])]);
        assert!(groups.iter().all(FlowGroup::is_empty));
        assert_eq!(total(&groups), 0);
    }

    #[test]
    fn longest_prefix_wins() {
        let s = GitFlowSettings { feature: "f/".into(), release: "f/rel/".into(), ..defaults() };
        let groups = grouped(&s, &branches(&["f/rel/1.0", "f/x"], None));
        assert_eq!(shorts(&groups), [(FlowKind::Feature, vec!["x"]), (FlowKind::Release, vec!["1.0"]), (FlowKind::Hotfix, vec![])]);
    }

    #[test]
    fn empty_prefix_matches_nothing() {
        let s = GitFlowSettings { hotfix: String::new(), ..defaults() };
        let groups = grouped(&s, &branches(&["anything", "feature/a"], None));
        assert_eq!(shorts(&groups), [(FlowKind::Feature, vec!["a"]), (FlowKind::Release, vec![]), (FlowKind::Hotfix, vec![])]);
    }

    #[test]
    fn case_sensitive_and_in_snapshot_order() {
        let locals = ["feature/zeta", "Feature/x", "feature/alpha"];
        let groups = grouped(&defaults(), &branches(&locals, Some("feature/alpha")));
        assert_eq!(shorts(&groups)[0], (FlowKind::Feature, vec!["zeta", "alpha"]));
        let heads: Vec<bool> = groups[0].branches.iter().map(|b| b.head).collect();
        assert_eq!(heads, [false, true]);
    }

    fn release_shorts<'a>(groups: &'a [FlowGroup]) -> Vec<&'a str> {
        groups[1].branches.iter().map(|b| b.short).collect()
    }

    #[test]
    fn releases_and_hotfixes_in_version_order() {
        let locals = ["release/1.10.1", "release/1.10.0", "release/1.9.0", "hotfix/1.10.0", "hotfix/1.9.1", "feature/10", "feature/9"];
        let groups = grouped(&defaults(), &branches(&locals, None));
        assert_eq!(
            shorts(&groups),
            [
                (FlowKind::Feature, vec!["10", "9"]),
                (FlowKind::Release, vec!["1.9.0", "1.10.0", "1.10.1"]),
                (FlowKind::Hotfix, vec!["1.9.1", "1.10.0"])
            ]
        );
    }

    #[test]
    fn a_v_prefix_keeps_the_version_order() {
        let locals = ["release/v1.10.0", "release/1.9.0", "release/v1.2.0", "release/1.2.0"];
        let groups = grouped(&defaults(), &branches(&locals, None));
        assert_eq!(release_shorts(&groups), ["v1.2.0", "1.2.0", "1.9.0", "v1.10.0"]);
    }

    #[test]
    fn a_suffixed_version_comes_before_the_bare_one() {
        let locals = ["release/1.2.0", "release/1.2.0-rc.10", "release/1.2.1", "release/1.2.0-rc.2"];
        let groups = grouped(&defaults(), &branches(&locals, None));
        assert_eq!(release_shorts(&groups), ["1.2.0-rc.2", "1.2.0-rc.10", "1.2.0", "1.2.1"]);
    }

    #[test]
    fn names_without_digits_come_first_in_snapshot_order() {
        let locals = ["release/next", "release/1.0.0", "release/candidate", "release/0.9.0"];
        let groups = grouped(&defaults(), &branches(&locals, None));
        assert_eq!(release_shorts(&groups), ["next", "candidate", "0.9.0", "1.0.0"]);
    }

    #[test]
    fn very_long_digit_runs_sort_without_overflow() {
        let big = format!("release/1{}", "0".repeat(40));
        let nines = format!("release/{}", "9".repeat(40));
        let locals = [big.as_str(), nines.as_str(), "release/18446744073709551616"];
        let groups = grouped(&defaults(), &branches(&locals, None));
        assert_eq!(release_shorts(&groups), ["18446744073709551616", &nines["release/".len()..], &big["release/".len()..]]);
    }

    #[test]
    fn the_head_mark_follows_its_branch() {
        let locals = ["release/1.10.0", "release/1.9.0"];
        let groups = grouped(&defaults(), &branches(&locals, Some("release/1.10.0")));
        assert_eq!(release_shorts(&groups), ["1.9.0", "1.10.0"]);
        let heads: Vec<bool> = groups[1].branches.iter().map(|b| b.head).collect();
        assert_eq!(heads, [false, true]);
        assert_eq!(groups[1].branches[0].name, "release/1.9.0");
    }

    #[test]
    fn groups_a_loaded_repository() {
        let dir = scratch_repo("gitflow-groups");
        write(&dir, "a.txt", "one\n");
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "first"]);
        for b in ["develop", "feature/a", "feature/b", "release/1.0.0"] {
            git(&dir, &["branch", b]);
        }
        for (key, value) in [
            ("gitflow.branch.master", "main"),
            ("gitflow.branch.develop", "develop"),
            ("gitflow.prefix.feature", "feature/"),
            ("gitflow.prefix.release", "release/"),
            ("gitflow.prefix.hotfix", "hotfix/"),
            ("gitflow.prefix.versiontag", ""),
        ] {
            git(&dir, &["config", key, value]);
        }
        git(&dir, &["checkout", "-q", "feature/b"]);
        let snap = load(&dir, &LoadOptions::all(50)).unwrap();
        let s = snap.gitflow.settings().expect("initialized");
        let groups = grouped(&s, &Branches::of(&snap));
        assert_eq!(shorts(&groups), [(FlowKind::Feature, vec!["a", "b"]), (FlowKind::Release, vec!["1.0.0"]), (FlowKind::Hotfix, vec![])]);
        assert!(groups[0].branches[1].head && !groups[0].branches[0].head);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
