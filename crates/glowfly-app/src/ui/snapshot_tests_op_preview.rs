//! The "Preview: <op>" dialog: a rebase that stops at a conflict, a drop, a reset gone stale and
//! a merge on a git too old to predict it. The previews are worked out by the core against the
//! shared fixture repository, as the runner would.

use super::snapshot_tests::{git, harness, reload, remove_fixture, with_repo};
use crate::effect::OpPreviewJob;
use crate::msg::{Msg, OpPreviewMsg};
use crate::state::op_preview::PreviewDialog;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::git_version::{Feature, GitInfo};
use glowfly_core::op_preview::{PreviewOp, Request, preview};
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::ThemeChoice;
use std::path::Path;

fn commit(dir: &Path, file: &str, text: &str, message: &str, date: &str) {
    std::fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", file], date);
    git(dir, &["commit", "-q", "-m", message], date);
}

fn rev(dir: &Path, name: &str) -> git2::Oid {
    git2::Repository::open(dir).unwrap().revparse_single(name).unwrap().id()
}

/// Opens the preview of `op` against HEAD in `st`, worked out (`work`) or as given.
fn open(st: &mut AppState, op: PreviewOp, work: bool) {
    let snap = st.snap.as_ref().unwrap();
    let job = OpPreviewJob { workdir: snap.workdir.clone(), request: Request { op, head: snap.head_oid.unwrap() } };
    let result = work.then(|| preview(&job.workdir, &job.request).map_err(|e| e.to_string()));
    st.modal = Some(Modal::OpPreview(Box::new(PreviewDialog { job, result })));
}

#[test]
fn op_preview_rebase_stops() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "op-preview-rebase");
    // login-page grows a change to app.txt that main's "Fix build on Windows" also made, and a
    // commit after it.
    git(&dir, &["checkout", "-q", "login-page"], "2026-01-11T10:00:00+00:00");
    commit(&dir, "app.txt", "app\nlogin\n", "Link the login page from the app", "2026-01-11T10:00:00+00:00");
    commit(&dir, "form.txt", "form\n", "Validate the login form", "2026-01-12T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    open(&mut st, PreviewOp::Rebase { onto: "main".into(), oid: rev(&dir, "main") }, true);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("op_preview_rebase_stops");
    assert!(h.query_by_label("Preview: rebase login-page onto main").is_some());
    assert!(h.query_by_label_contains("with conflicts in 1 file").is_some());
    assert!(h.query_by_label("app.txt").is_some(), "the conflicted path");
    assert!(h.query_by_label("Uncommitted changes aren't part of this preview.").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn op_preview_drop() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "op-preview-drop");
    git(&dir, &["checkout", "-q", "search"], "2026-01-11T10:00:00+00:00");
    commit(&dir, "filters.txt", "filters\n", "Search filters", "2026-01-11T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    let (dropped, filters) = (rev(&dir, "search~1"), rev(&dir, "search"));
    open(&mut st, PreviewOp::Drop(vec![dropped]), true);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("op_preview_drop");
    assert!(h.query_by_label("Preview: drop 1 commit from search").is_some());
    let moved = format!("Search filters {} moved", &filters.to_string()[..7]);
    assert!(h.query_by_label(&moved).is_some(), "the rewritten commit");
    assert!(h.query_by_label("1 commit no branch or tag will reach").is_some());
    h.get_by_label("Drop…").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::OpPreview(OpPreviewMsg::Apply(None)))));
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::CloseModal)), "update closes it, after Apply");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn op_preview_reset_stale() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "op-preview-reset");
    open(&mut st, PreviewOp::Reset { to: rev(&dir, "main~1") }, true);
    commit(&dir, "late.txt", "late\n", "A late commit", "2026-01-11T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("op_preview_reset_stale");
    for b in ["Reset (soft)", "Reset (mixed)", "Reset (hard)…"] {
        assert!(h.get_by_label(b).is_disabled(), "{b} is off");
    }
    assert!(h.query_all_by_label("The branch changed since this preview. Preview again first.").count() > 0);
    h.get_by_label("Preview again").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::OpPreview(OpPreviewMsg::Again))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn op_preview_old_git() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "op-preview-old-git");
    st.git.info = Some(GitInfo { path: Some("git".into()), version: Some((2, 37, 1)), chosen: false });
    open(&mut st, PreviewOp::Merge { theirs: "search".into(), oid: rev(&dir, "search") }, false);
    let why = st.git.blocked(Feature::MergeTree).unwrap();
    if let Some(Modal::OpPreview(d)) = st.modal.as_mut() {
        d.result = Some(Err(why));
    }
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("op_preview_old_git");
    assert!(h.query_by_label_contains("needs git 2.38 or newer (you have 2.37.1)").is_some());
    assert!(!h.get_by_label("Merge").is_disabled(), "the merge itself still runs");
    assert!(h.query_by_label("Preview again").is_some());
    drop(h);
    remove_fixture(&dir);
}
