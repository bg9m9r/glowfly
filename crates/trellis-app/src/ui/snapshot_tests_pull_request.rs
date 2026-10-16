//! The pull request panel's Files changed tab: the files with one file's diff open, and a pull
//! request whose source branch isn't in the repository. Shares the fixture repository and
//! renderer with `snapshot_tests`.

use super::snapshot_tests::{git, harness, reload, remove_fixture, with_repo};
use super::snapshot_tests_select::click;
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::pull_request::{Files, PrTab, PrView};
use crate::state::{AppState, ForgeState, Status};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::PointerButton;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::sync::Arc;
use trellis_core::forge::{Provider, PullRequest, Reviewer, Target};
use trellis_core::repo::{self, DiffTarget, LoadOptions};
use trellis_core::settings::ThemeChoice;

const DESCRIPTION: &str = "Adds a page that lists search results.\n\nPaging and filters come in a later change.";

fn tip(st: &AppState, name: &str) -> git2::Oid {
    st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == name).and_then(|b| b.oid).unwrap()
}

/// The fixture with origin on GitHub (acme/demo) and pull request #12 from `search` into
/// `main`, open in the panel; `refs/remotes/origin/search` only `with_source`.
pub(super) fn with_pr(theme: ThemeChoice, name: &str, with_source: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let d = "2026-01-10T10:00:00+00:00";
    git(&dir, &["remote", "add", "origin", "https://github.com/acme/demo.git"], d);
    git(&dir, &["update-ref", "refs/remotes/origin/main", "main"], d);
    if with_source {
        git(&dir, &["update-ref", "refs/remotes/origin/search", "search"], d);
    }
    reload(&mut st, &dir, &LoadOptions::all(500));
    let target =
        Target { provider: Provider::GitHub, host: "github.com".into(), owner: "acme".into(), project: String::new(), repo: "demo".into() };
    let mut fs = ForgeState::new(Some(target), "origin".into(), None);
    fs.host_status = Status::Ok;
    let reviewers = if with_source {
        vec![
            Reviewer { name: "Grace".into(), state: "approved".into(), required: true },
            Reviewer { name: "Linus".into(), state: "changes requested".into(), required: false },
        ]
    } else {
        Vec::new()
    };
    fs.prs = vec![PullRequest {
        id: 12,
        title: "Search results page".into(),
        description: DESCRIPTION.into(),
        source: "search".into(),
        target: "main".into(),
        author: "ada".into(),
        draft: !with_source,
        reviewers,
        head: Some(tip(&st, "search").to_string()),
        merge_status: with_source.then(|| "mergeable".into()),
        url: "https://github.com/acme/demo/pull/12".into(),
    }];
    let view = PrView::new(fs.prs[0].clone(), &fs, st.snap.as_ref().unwrap());
    st.forge = Some(fs);
    st.pull_request = Some(view);
    (st, dir)
}

#[test]
fn pull_request_dark() {
    let (mut st, dir) = with_pr(ThemeChoice::Dark, "pr-view", true);
    let v = st.pull_request.as_mut().unwrap();
    v.tab = PrTab::Files;
    let (main, search) = v.tips.pair().expect("both branches are on origin");
    let changes = repo::branch_changes(&dir, main, search).unwrap().unwrap();
    let target = DiffTarget::Between(changes.base, search, "search.txt".into());
    v.files = Files::Loaded(changes);
    assert!(v.shows(&target));
    let d = repo::load_diff(&dir, &target).unwrap();
    st.diff = Some((target, Arc::new(d)));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("pull_request_dark");
    assert!(h.query_by_label("1 file changed").is_some());
    assert!(h.query_by_label("Files changed  1").is_some());
    assert!(h.query_by_label_contains("Fetch origin").is_none(), "nothing is out of sync");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_out_of_sync_light() {
    let (mut st, dir) = with_pr(ThemeChoice::Light, "pr-view-sync", false);
    st.pull_request.as_mut().unwrap().tab = PrTab::Files;
    assert_eq!(st.pull_request.as_ref().unwrap().files, Files::Missing);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("pull_request_out_of_sync_light");
    assert!(h.query_by_label_contains("origin/search isn't in this repository.").is_some());
    assert!(!h.get_by_label("Fetch origin").is_disabled());
    h.get_by_label("Close  (Esc)").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::PullRequest(PullRequestMsg::Close))));

    // The sidebar row's menu opens the panel too.
    h.state_mut().1.clear();
    let rect = h.get_by_label("[Draft] Search results page").bounding_box().expect("the sidebar row");
    let center = eframe::egui::pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32);
    click(&mut h, center, PointerButton::Secondary);
    assert_eq!(h.get_all_by_label("Open in browser").count(), 2, "the header's button and the menu's item");
    h.get_by_label("Show pull request").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::PullRequest(PullRequestMsg::Open(12)))));
    drop(h);
    remove_fixture(&dir);
}
