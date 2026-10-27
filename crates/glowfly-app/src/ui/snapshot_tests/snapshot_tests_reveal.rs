//! A file list scrolling to the file a next/previous change step landed on: the Unstaged Files
//! list and a commit's file list, each long enough that the file starts out of view. Without a
//! landing the list stays at the top. Shares the fixture repository and renderer with
//! `snapshot_tests`.

use super::super::snapshot_check::CheckSnapshot;
use super::super::snapshot_tests::{git, harness, remove_fixture, with_repo};
use crate::state::{AppState, Selection};
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{self, DiffTarget, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use std::path::Path;

/// Writes `count` small files named f00.txt and on into `dir`.
fn many_files(dir: &Path, count: usize, text: &str) {
    for i in 0..count {
        std::fs::write(dir.join(format!("f{i:02}.txt")), format!("{text} {i}\n")).unwrap();
    }
}

/// `target`'s diff on screen; with `landed`, as a step's landing.
fn open(st: &mut AppState, dir: &Path, target: DiffTarget, landed: bool) {
    let diff = repo::load_diff(dir, &target).unwrap();
    st.view.file_reveal = landed.then(|| (target.clone(), 7));
    st.session.diff.view = Some((target, diff.into()));
}

/// Renders `st` and returns whether the file list drew `label` (the diff's header always names
/// it too), comparing against `name` when given.
fn shows(st: AppState, label: &str, name: Option<&str>) -> bool {
    let mut h = harness(st, true);
    h.run();
    if let Some(name) = name {
        h.check_snapshot(name);
    }
    h.query_all_by_label(label).count() > 1
}

#[test]
fn files_reveal_unstaged() {
    for landed in [true, false] {
        let (mut st, dir) = with_repo(ThemeChoice::Dark, &format!("reveal-wip-{landed}"));
        many_files(&dir, 60, "new");
        st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
        open(&mut st, &dir, DiffTarget::Unstaged("f50.txt".into()), landed);
        let name = landed.then_some("files_reveal_unstaged");
        assert_eq!(shows(st, "f50.txt", name), landed, "landed: {landed}");
        remove_fixture(&dir);
    }
}

#[test]
fn files_reveal_commit() {
    for landed in [true, false] {
        let (mut st, dir) = with_repo(ThemeChoice::Dark, &format!("reveal-commit-{landed}"));
        many_files(&dir, 60, "added");
        git(&dir, &["add", "."], "2026-01-20T10:00:00+00:00");
        git(&dir, &["commit", "-q", "-m", "Add sixty files"], "2026-01-20T10:00:00+00:00");
        st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
        let oid = st.snap.as_ref().unwrap().commits[0].oid;
        st.session.selection = Selection::Commit(oid);
        st.session.detail = Some(repo::commit_detail(&dir, oid).unwrap());
        open(&mut st, &dir, DiffTarget::Commit(oid, "f55.txt".into()), landed);
        let name = landed.then_some("files_reveal_commit");
        assert_eq!(shows(st, "f55.txt", name), landed, "landed: {landed}");
        remove_fixture(&dir);
    }
}
