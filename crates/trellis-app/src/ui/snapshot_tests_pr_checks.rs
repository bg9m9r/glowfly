//! Build-status markers on the sidebar's pull request rows, and the checks line in a row's
//! tooltip. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::state::{AppState, ForgeState, Status};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::forge::{CheckState, Checks, Provider, PullRequest, Reviewer, Target};
use trellis_core::settings::ThemeChoice;

fn pr(id: u64, title: &str, source: &str, draft: bool, checks: Option<Checks>) -> PullRequest {
    PullRequest {
        id,
        title: title.into(),
        description: String::new(),
        source: source.into(),
        target: "main".into(),
        author: "ada".into(),
        draft,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks,
        url: format!("https://github.com/o/r/pull/{id}"),
    }
}

/// The fixture connected to a GitHub repository with four open pull requests: one passed, one
/// failed, one pending and one without checks.
fn with_prs(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let target =
        Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    let mut f = ForgeState::new(Some(target), "origin".into(), None);
    f.host_status = Status::Ok;
    let mut search = pr(12, "Search results page", "search", false, Some(Checks::from_counts(6, 0, 0, true)));
    search.reviewers.push(Reviewer { name: "grace".into(), state: "approved".into(), required: false });
    f.prs = vec![
        search,
        pr(11, "Fix flaky login test", "login-page", false, Some(Checks::from_counts(3, 2, 0, true))),
        pr(10, "Dark mode", "dark-mode", true, Some(Checks::from_counts(4, 0, 2, true))),
        pr(9, "Update README", "readme", false, None),
    ];
    assert_eq!(f.prs[2].checks.as_ref().map(|c| c.state), Some(CheckState::Pending));
    st.forge = Some(f);
    (st, dir)
}

#[test]
fn sidebar_pr_checks() {
    let (st, dir) = with_prs(ThemeChoice::Dark, "pr-checks");
    render("sidebar_pr_checks", st, true);
    remove_fixture(&dir);
}

#[test]
fn sidebar_pr_checks_tooltip() {
    let (st, dir) = with_prs(ThemeChoice::Light, "pr-checks-tip");
    let mut h = harness(st, false);
    h.run();
    h.get_by_label("Fix flaky login test").hover();
    for _ in 0..4 {
        h.run();
    }
    let tip = "Checks: 3 of 5 passed, 2 failed";
    assert!(h.query_by_label_contains(tip).is_some(), "the tooltip says {tip:?}");
    h.check_snapshot("sidebar_pr_checks_tooltip");
    remove_fixture(&dir);
}
