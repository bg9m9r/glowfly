//! The interactive rebase panel on a range with a merge: label and reset rows, the merge row
//! with its dot and its three actions, and a squash after a structure step. Shares the fixture
//! repository with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::state::AppState;
use crate::state::rebase::RebasePanel;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::rebase::{self, RebaseAction, RebaseBase, TodoStep};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

/// The panel for the fixture from "Add app skeleton" up: the login-page line, main's line and
/// the merge that joins them. A clean tree, so only the list decides whether Start is enabled.
fn with_merges(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged.clear();
    let skeleton = snap.commits.iter().find(|c| c.summary == "Add app skeleton").unwrap().oid;
    let base = RebaseBase::FromCommit(skeleton);
    let plan = rebase::plan(&dir, &base).unwrap();
    let mut panel = RebasePanel::loading(base);
    panel.fill(plan);
    st.session.rebase.view = Some(panel);
    (st, dir)
}

fn row(st: &AppState, summary: &str) -> usize {
    let p = st.session.rebase.view.as_ref().unwrap();
    p.items.iter().position(|s| s.item().is_some_and(|i| i.summary == summary)).unwrap()
}

#[test]
fn rebase_panel_merges() {
    let (st, dir) = with_merges(ThemeChoice::Dark, "rebase-merges");
    let p = st.session.rebase.view.as_ref().unwrap();
    assert!(p.items.iter().any(TodoStep::is_structure), "{:?}", p.items);
    assert!(p.items.iter().any(|s| s.merge().is_some()));
    let subtitle = p.subtitle();
    assert!(subtitle.starts_with("Rebasing 3 commits and 1 merge of main onto ") && subtitle.ends_with(" Initial commit"), "{subtitle}");
    assert!(p.start_blocker(st.snap.as_ref().unwrap(), false).is_some(), "nothing changed yet");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("rebase_panel_merges");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rebase_panel_merges_light() {
    let (mut st, dir) = with_merges(ThemeChoice::Light, "rebase-merges-light");
    let merge = row(&st, "Merge branch 'login-page'");
    let login = row(&st, "Add login page");
    let p = st.session.rebase.view.as_mut().unwrap();
    p.set_action(merge, RebaseAction::Drop);
    p.set_action(login, RebaseAction::Reword);
    assert_eq!(p.items[merge].item().unwrap().action, RebaseAction::Drop);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("rebase_panel_merges_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rebase_panel_merge_combo() {
    let (mut st, dir) = with_merges(ThemeChoice::Dark, "rebase-merge-combo");
    // "Fix build on Windows" starts main's line again after a reset: it can't be squashed.
    let fix = row(&st, "Fix build on Windows");
    let p = st.session.rebase.view.as_mut().unwrap();
    p.set_action(fix, RebaseAction::Squash);
    let blocker = p.start_blocker(st.snap.as_ref().unwrap(), false);
    assert_eq!(blocker.as_deref(), Some(rebase::COMBINE_AFTER_MERGE), "{:?}", st.session.rebase.view.as_ref().unwrap().items);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("rebase_panel_merge_combo");
    assert!(h.get_by_label("Start rebase").is_disabled());
    drop(h);
    remove_fixture(&dir);
}
