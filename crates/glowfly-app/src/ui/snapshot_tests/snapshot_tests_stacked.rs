//! Several commits that aren't one line of history: the gap named under the title and each
//! commit's own changes stacked (with one of their diffs open, and in German), and the mainline
//! dialog for several commits with merges among them. Shares the fixture repository and renderer
//! with `snapshot_tests`.

use super::common::short;
use super::snapshot_tests::{git, harness, reload, remove_fixture, render, with_repo};
use super::snapshot_tests_select::with_selection;
use crate::state::pick::{MainlineForm, PickOp};
use crate::state::{AppState, Modal, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::intraline;
use glowfly_core::repo::{self, DiffTarget, LoadOptions};
use glowfly_core::settings::ThemeChoice;

const STACKED: [&str; 3] = ["Search results page", "Fix build on Windows", "Initial commit"];

fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap().oid
}

#[test]
fn stacked_changes() {
    let (st, dir) = with_selection(ThemeChoice::Dark, "stacked-changes", &STACKED);
    let Selection::Commits(m) = &st.selection else { panic!("not a multi-selection") };
    assert_eq!(m.gap(), Some(commit(&st, "Search results page")));
    let stack = m.stacked.clone().unwrap().unwrap();
    assert_eq!(stack.len(), 3);
    assert!(stack[2].files.iter().any(|f| f.path == "README.md" && f.status == 'A'), "{:?}", stack[2].files);
    let gap = format!("Not one line of history: the parent of {} isn't selected.", short(commit(&st, "Search results page")));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(&gap).is_some(), "the gap is named");
    assert!(h.query_by_label("Each commit's own changes, newest first").is_some());
    h.check_snapshot("stacked_changes");
    drop(h);
    remove_fixture(&dir);

    // One section's file open: its row is highlighted and the selection stays.
    let (mut st, dir) = with_selection(ThemeChoice::Dark, "stacked-changes-diff", &STACKED);
    let target = DiffTarget::Commit(commit(&st, "Fix build on Windows"), "app.txt".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.intraline = intraline::intraline(&diff);
    st.session.diff.view = Some((target, diff.into()));
    render("stacked_changes_diff", st, true);
    remove_fixture(&dir);
}

#[test]
fn stacked_changes_german() {
    let (mut st, dir) = with_selection(ThemeChoice::Dark, "stacked-changes-german", &STACKED);
    st.settings.language = "de".into();
    let mut h = harness(st, true);
    h.run();
    let de = Lang::De;
    assert!(h.query_by_label(de.tr("details.multi.stacked.title")).is_some());
    assert!(h.query_all_by_label(&de.trn("details.multi.stacked.files", 1, &[])).count() >= 1);
    for key in ["details.multi.stacked.title", "details.multi.stacked.merge_note", "details.multi.stacked.no_files"] {
        assert!(h.query_by_label(Lang::En.tr(key)).is_none(), "{key} isn't left in English");
    }
    assert!(h.query_by_label_contains("Not one line of history").is_none());
    h.check_snapshot("stacked_changes_german");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn mainline_dialog_several() {
    // A merge and a commit that isn't one, picked onto a branch from the first commit.
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "mainline-several");
    let (merge, login) = (commit(&st, "Merge branch 'login-page'"), commit(&st, "Add login page"));
    git(&dir, &["checkout", "-q", "-b", "hotfix", &commit(&st, "Initial commit").to_string()], "2026-01-11T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    let snap = st.snap.as_ref().unwrap();
    let form = MainlineForm::new(PickOp::CherryPick, &[login, merge], snap).expect("a merge");
    assert_eq!((form.choices(), form.plain), (1, Some(login)));
    st.modal = Some(Modal::Mainline(form));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Parent 1").is_some() && h.query_by_label("Parent 2").is_none(), "only parent 1 fits");
    h.check_snapshot("mainline_dialog_several");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn mainline_dialog_merges() {
    // Two merges: login-page's, and search merged too.
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "mainline-merges");
    git(&dir, &["merge", "-q", "--no-ff", "search", "-m", "Merge branch 'search'"], "2026-01-11T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    let (first, second) = (commit(&st, "Merge branch 'login-page'"), commit(&st, "Merge branch 'search'"));
    let snap = st.snap.as_ref().unwrap();
    let form = MainlineForm::new(PickOp::Revert, &[second, first], snap).expect("merges");
    assert_eq!((form.choices(), form.merges.len()), (2, 2));
    st.modal = Some(Modal::Mainline(form));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Parent 1").is_some() && h.query_by_label("Parent 2").is_some());
    h.check_snapshot("mainline_dialog_merges");
    drop(h);
    remove_fixture(&dir);
}
