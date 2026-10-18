//! Git LFS notices: the main window's banner when git-lfs is missing in a repository that uses
//! LFS, the LFS badge on file rows (working tree and commit details), and the diff's LFS summary
//! and pointer text.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::lfs::LfsMsg;
use crate::msg::{Msg, PrefsMsg};
use crate::state::lfs::{LfsDiffNote, LfsInstall};
use crate::state::lfs_notes::{BANNER_TEXT, POINTER_BAR};
use crate::state::{AppState, PrefPage, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::lfs::LfsPattern;
use trellis_core::lfs::pointer::{LfsChange, LfsSide, Pointer};
use trellis_core::repo::{self, DiffLine, DiffTarget, FileDiff, FileEntry, Hunk};
use trellis_core::settings::ThemeChoice;

fn pushed(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, want: impl Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

fn missing_lfs(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    st.lfs.install = LfsInstall::Missing;
    st.snap.as_mut().unwrap().lfs_patterns = vec![LfsPattern { raw: "*.psd".into(), display: "*.psd".into(), lockable: false }];
    (st, dir)
}

#[test]
fn lfs_banner() {
    let (st, dir) = missing_lfs(ThemeChoice::Dark, "lfs-banner");
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("lfs_banner");
    assert!(h.query_by_label_contains("git-lfs isn't installed").is_some());
    h.get_by_label("Git LFS settings…").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoLfs)))));
    h.get_by_label("Dismiss").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Lfs(LfsMsg::DismissBanner(w)) if *w == workdir)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn lfs_banner_hidden() {
    let (mut st, dir) = missing_lfs(ThemeChoice::Dark, "lfs-banner-hidden");
    st.lfs.install = LfsInstall::Installed("3.7.1".into());
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains(BANNER_TEXT).is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn lfs_badges_wip() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "lfs-badges-wip");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged.push(FileEntry { lfs: true, ..FileEntry::new("art/hero.psd", 'M') });
    snap.unstaged.push(FileEntry::new("art/notes.txt", 'M'));
    snap.staged.push(FileEntry { lfs: true, ..FileEntry::new("art/logo.psd", 'A') });
    snap.staged.push(FileEntry::new("art/credits.txt", 'A'));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("lfs_badges_wip");
    assert_eq!(h.query_all_by_label("LFS").count(), 2, "the two pointer rows");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn lfs_badge_commit() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "lfs-badge-commit");
    let head = st.snap.as_ref().and_then(|s| s.head_oid).expect("HEAD");
    st.selection = Selection::Commit(head);
    let mut detail = repo::commit_detail(&dir, head).unwrap();
    detail.files[0].lfs = true;
    st.detail = Some(detail);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("lfs_badge_commit");
    assert!(h.query_by_label("LFS").is_some());
    drop(h);
    remove_fixture(&dir);
}

const OLD_OID: &str = "4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393";
const NEW_OID: &str = "9b2c0c5f1a8e4d3b7c6a5f4e3d2c1b0a99887766554433221100ffeeddccbbaa";

/// The pointer text's change from 12.4 MB to 13.0 MB, as the diff's one hunk.
fn pointer_hunks() -> FileDiff {
    let line = |origin: char, old: Option<u32>, new: Option<u32>, text: String| DiffLine { origin, old, new, text };
    let lines = vec![
        line(' ', Some(1), Some(1), "version https://git-lfs.github.com/spec/v1".into()),
        line('-', Some(2), None, format!("oid sha256:{OLD_OID}")),
        line('-', Some(3), None, "size 13002342".into()),
        line('+', None, Some(2), format!("oid sha256:{NEW_OID}")),
        line('+', None, Some(3), "size 13631488".into()),
    ];
    FileDiff {
        binary: false,
        hunks: vec![Hunk { header: "@@ -1,3 +1,3 @@".into(), lines }],
        highlight: None,
        intraline: Default::default(),
        whole: None,
    }
}

fn pointer(oid: &str, size: u64) -> LfsSide {
    LfsSide::Pointer(Pointer { oid: oid.into(), size })
}

/// The fixture with a pointer diff open (`target`, else HEAD's art/hero.psd) and its LFS note.
fn lfs_diff(theme: ThemeChoice, name: &str, target: Option<DiffTarget>, change: LfsChange, show_pointer: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let target = target.unwrap_or_else(|| commit_target(&st));
    st.diff = Some((target.clone(), pointer_hunks().into()));
    st.lfs.diff = Some(LfsDiffNote { target, change, show_pointer });
    (st, dir)
}

fn commit_target(st: &AppState) -> DiffTarget {
    DiffTarget::Commit(st.snap.as_ref().and_then(|s| s.head_oid).unwrap(), "art/hero.psd".into())
}

fn changed() -> LfsChange {
    LfsChange { old: pointer(OLD_OID, 13_002_342), new: pointer(NEW_OID, 13_631_488), new_in_workdir: false, filtered: false }
}

#[test]
fn diff_lfs_summary() {
    let (st, dir) = lfs_diff(ThemeChoice::Dark, "lfs-diff-summary", None, changed(), false);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("diff_lfs_summary");
    assert!(h.query_by_label("Stored in Git LFS").is_some());
    assert!(h.query_by_label("Size: 13.0 MB, was 12.4 MB").is_some());
    assert!(h.query_by_label("Split").is_none() && h.query_by_label("Inline").is_none(), "no layout toggles");
    assert!(h.query_by_label("Ignore whitespace").is_none());
    h.get_by_label("Show pointer text").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Lfs(LfsMsg::ShowPointer(true)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn diff_lfs_regular() {
    let regular =
        LfsChange { old: pointer(OLD_OID, 13_002_342), new: LfsSide::Regular { size: 13_736_345 }, new_in_workdir: true, filtered: true };
    let target = DiffTarget::Unstaged("art/hero.psd".into());
    let (mut st, dir) = lfs_diff(ThemeChoice::Light, "lfs-diff-regular", Some(target), regular, false);
    st.lfs.install = LfsInstall::Missing;
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("diff_lfs_regular");
    assert!(h.query_by_label("Was stored in Git LFS (12.4 MB)").is_some());
    assert!(h.query_by_label_contains("committing it stores the whole file in Git").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn diff_lfs_pointer_text() {
    let (st, dir) = lfs_diff(ThemeChoice::Dark, "lfs-diff-pointer", None, changed(), true);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("diff_lfs_pointer_text");
    assert!(h.query_by_label(POINTER_BAR).is_some());
    assert!(h.query_by_label("Stored in Git LFS").is_none(), "the rows instead of the summary");
    h.get_by_label("Show summary").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Lfs(LfsMsg::ShowPointer(false)))));
    drop(h);
    remove_fixture(&dir);
}
