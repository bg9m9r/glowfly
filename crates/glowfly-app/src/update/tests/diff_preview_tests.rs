//! Headless tests for the binary diff's sides: asked for once per shown binary diff, stale
//! results ignored, kept through a reload of the same file, cleared on close, none for text or
//! LFS-summary diffs, and asked for again when a tab comes back still loading.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{DiffMsg, Msg, RepoMsg};
use crate::runner;
use crate::state::AppState;
use crate::state::diff_preview::{self, DiffPreview};
use glowfly_core::repo::{BinarySide, BinarySides, DiffTarget};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn bin() -> DiffTarget {
    DiffTarget::Unstaged("b.bin".into())
}

/// A repository with b.bin, c.bin and a.txt changed, opened (no diff shown).
fn repo(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    std::fs::write(dir.join("b.bin"), b"\0\x01old\0").unwrap();
    std::fs::write(dir.join("c.bin"), b"\0\x01cee\0").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "binaries"]);
    std::fs::write(dir.join("b.bin"), b"\0\x02newer\0").unwrap();
    std::fs::write(dir.join("c.bin"), b"\0\x02cees\0").unwrap();
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

/// Opens `target` and returns the effects its DiffLoaded raised, unrun.
fn open(st: &mut AppState, target: DiffTarget) -> Vec<Effect> {
    let fx = run(st, Msg::Repo(RepoMsg::OpenDiff(target)));
    deliver_diff(st, fx)
}

/// Runs the LoadDiff among `fx` and applies its result, returning the effects that raised.
fn deliver_diff(st: &mut AppState, fx: Vec<Effect>) -> Vec<Effect> {
    let load = fx.into_iter().find(|e| matches!(e, Effect::LoadDiff { .. })).expect("a diff load");
    let msg = runner::execute(load).expect("a result");
    run(st, msg)
}

fn requests(fx: &[Effect]) -> Vec<(DiffTarget, u64)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::LoadBinarySides { target, generation, .. } => Some((target.clone(), *generation)),
            _ => None,
        })
        .collect()
}

fn sides(size: u64) -> Arc<BinarySides> {
    Arc::new(BinarySides { old: Some(BinarySide { size, image: None }), new: None })
}

fn loaded(target: DiffTarget, generation: u64, result: Result<Arc<BinarySides>, String>) -> Msg {
    Msg::Diff(DiffMsg::BinarySidesLoaded { target, generation, result })
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_binary_diff_asks_for_its_sides_once() {
    let (mut st, dir) = repo("preview-ask");
    let fx = open(&mut st, bin());
    assert_eq!(requests(&fx), [(bin(), st.session.diff.shown_gen)]);
    let p = st.session.diff_preview.as_ref().expect("a preview");
    assert!(p.target == bin() && p.sides.is_none(), "loading");
    // Running it fills the sides in.
    for e in fx.into_iter().filter(|e| matches!(e, Effect::LoadBinarySides { .. })) {
        let msg = runner::execute(e).expect("a result");
        assert!(matches!(&msg, Msg::Scoped { msg, .. } if matches!(**msg, Msg::Diff(DiffMsg::BinarySidesLoaded { .. }))));
        drive(&mut st, msg);
    }
    let s = diff_preview::shown(&st).expect("loaded");
    assert_eq!((s.old.as_ref().map(|s| s.size), s.new.as_ref().map(|s| s.size)), (Some(6), Some(8)));

    let fx = open(&mut st, DiffTarget::Unstaged("a.txt".into()));
    assert!(requests(&fx).is_empty(), "a text diff asks for nothing");
    assert!(st.session.diff_preview.is_none());
    cleanup(&dir);
}

#[test]
fn stale_results_are_ignored_and_errors_are_kept_quietly() {
    let (mut st, dir) = repo("preview-stale");
    open(&mut st, bin());
    let generation = st.session.diff.shown_gen;
    let log = st.log.len();
    drive(&mut st, loaded(bin(), generation - 1, Ok(sides(1))));
    drive(&mut st, loaded(DiffTarget::Unstaged("c.bin".into()), generation, Ok(sides(2))));
    assert!(st.session.diff_preview.as_ref().unwrap().sides.is_none(), "an older or other result");
    drive(&mut st, loaded(bin(), generation, Err("no sides".into())));
    assert!(matches!(&st.session.diff_preview.as_ref().unwrap().sides, Some(Err(e)) if e == "no sides"));
    assert!(diff_preview::shown(&st).is_none());
    assert_eq!(st.log.len(), log, "nothing logged");
    drive(&mut st, loaded(bin(), generation, Ok(sides(3))));
    assert_eq!(diff_preview::shown(&st).unwrap().old.as_ref().unwrap().size, 3);
    cleanup(&dir);
}

#[test]
fn a_reload_keeps_the_sides_and_another_file_or_close_drops_them() {
    let (mut st, dir) = repo("preview-reload");
    open(&mut st, bin());
    let generation = st.session.diff.shown_gen;
    drive(&mut st, loaded(bin(), generation, Ok(sides(5))));
    // A reload of the same file (as after a refresh) keeps the sides while asking again.
    let mut fx = Vec::new();
    super::repo::reload_diff(&mut st, &mut fx);
    let fx = deliver_diff(&mut st, fx);
    assert_eq!(requests(&fx), [(bin(), st.session.diff.shown_gen)]);
    assert_eq!(diff_preview::shown(&st).unwrap().old.as_ref().unwrap().size, 5, "kept until the new ones arrive");

    let fx = open(&mut st, DiffTarget::Unstaged("c.bin".into()));
    assert_eq!(requests(&fx).len(), 1);
    assert!(st.session.diff_preview.as_ref().unwrap().sides.is_none(), "another file's sides are dropped");

    let generation = st.session.diff.shown_gen;
    drive(&mut st, Msg::Repo(RepoMsg::CloseDiff));
    assert!(st.session.diff_preview.is_none());
    drive(&mut st, loaded(DiffTarget::Unstaged("c.bin".into()), generation, Ok(sides(1))));
    assert!(st.session.diff_preview.is_none(), "a late result after closing");
    cleanup(&dir);
}

#[test]
fn lfs_summaries_ask_for_nothing_and_tabs_ask_again_while_loading() {
    let (mut st, dir) = repo("preview-tabs");
    open(&mut st, bin());
    // An LFS summary shows instead: no preview.
    st.session.diff.lfs = Some(crate::state::lfs::LfsDiffNote { target: bin(), change: lfs_change(), show_pointer: false });
    let mut fx = Vec::new();
    super::diff_preview::diff_shown(&mut st, &mut fx);
    assert!(requests(&fx).is_empty() && st.session.diff_preview.is_none());
    st.session.diff.lfs = None;

    st.session.diff_preview = Some(DiffPreview { target: bin(), generation: 0, sides: None });
    let parked = super::tabs::park(&mut st);
    assert!(parked.session.diff_preview.is_some(), "parked with the tab");
    st.snap = parked.snap;
    st.session.diff.view = parked.session.diff.view;
    st.session.diff_preview = parked.session.diff_preview;
    let mut fx = Vec::new();
    super::diff_preview::reload_pending(&mut st, &mut fx);
    assert_eq!(requests(&fx), [(bin(), st.session.diff.shown_gen)], "still loading: asked again");
    st.session.diff_preview.as_mut().unwrap().sides = Some(Ok(sides(1)));
    let mut fx = Vec::new();
    super::diff_preview::reload_pending(&mut st, &mut fx);
    assert!(requests(&fx).is_empty(), "loaded: nothing to ask");
    cleanup(&dir);
}

fn lfs_change() -> glowfly_core::lfs::pointer::LfsChange {
    use glowfly_core::lfs::pointer::{LfsChange, LfsSide};
    LfsChange { old: LfsSide::Absent, new: LfsSide::Regular { size: 4 }, new_in_workdir: true, filtered: false }
}
