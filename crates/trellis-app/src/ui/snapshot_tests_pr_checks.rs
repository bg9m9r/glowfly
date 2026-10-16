//! Build-status markers on the sidebar's pull request and branch rows and in the graph's ref
//! labels, the checks line in a row's tooltip, and the pull request menu's build page item.
//! Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use super::snapshot_tests_select::click;
use crate::msg::Msg;
use crate::state::{AppState, ForgeState, Status};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::PointerButton;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::time::Instant;
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

/// The fixture's LOCAL branches with known tip status: main passed, search failed, login-page
/// pending (no pull requests).
#[test]
fn sidebar_tip_checks() {
    let (mut st, dir) = with_prs(ThemeChoice::Dark, "tip-checks");
    let snap = st.snap.as_ref().unwrap();
    let tip = |name: &str| snap.locals.iter().find(|b| b.name == name).and_then(|b| b.oid).unwrap();
    let tips =
        [("main", CheckState::Passed), ("search", CheckState::Failed), ("login-page", CheckState::Pending)].map(|(n, s)| (tip(n), s));
    let f = st.forge.as_mut().unwrap();
    f.prs.clear();
    let now = Instant::now();
    for (oid, state) in tips {
        f.tips.seed(oid, Checks { state, ..Checks::from_counts(2, 0, 0, true) }, now);
    }
    render("sidebar_tip_checks", st, true);
    remove_fixture(&dir);
}

/// Right-clicks the sidebar row of the pull request titled `title`.
fn pr_menu(h: &mut Harness<'_, (AppState, Vec<Msg>)>, title: &str) {
    let rect = h.get_all_by_label(title).filter_map(|n| n.bounding_box()).min_by(|a, b| a.x0.total_cmp(&b.x0)).expect("the row");
    let center = eframe::egui::pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32);
    click(h, center, PointerButton::Secondary);
}

#[test]
fn pr_menu_build_page() {
    let (mut st, dir) = with_prs(ThemeChoice::Light, "pr-menu-build");
    let f = st.forge.as_mut().unwrap();
    f.prs[1].checks.as_mut().unwrap().url = "https://ci.example.com/build/7".into();
    let mut h = harness(st, false);
    h.run();
    pr_menu(&mut h, "Fix flaky login test");
    assert!(h.query_by_label("Open failed build").is_some_and(|n| !n.is_disabled()));
    h.check_snapshot("pr_menu_build_page");
    h.get_by_label("Open failed build").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::OpenUrl(u) if u == "https://ci.example.com/build/7")));
    drop(h);

    // The service gave no link for this one's checks; one without checks has no item.
    let (st, _) = with_prs(ThemeChoice::Light, "pr-menu-build");
    let mut h = harness(st, false);
    h.run();
    pr_menu(&mut h, "Search results page");
    assert!(h.get_by_label("Open build page").is_disabled());
    drop(h);
    let (st, _) = with_prs(ThemeChoice::Light, "pr-menu-build");
    let mut h = harness(st, false);
    h.run();
    pr_menu(&mut h, "Update README");
    assert!(h.query_by_label("Open build page").is_none() && h.query_by_label("Open in browser").is_some());
    drop(h);
    remove_fixture(&dir);
}
