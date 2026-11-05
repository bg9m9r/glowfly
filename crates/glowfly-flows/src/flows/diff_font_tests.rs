//! A font file's binary diff end to end: opening a changed fonts/brand.ttf asks for its sides
//! once, the runner draws them on its font worker with each face's names, and the header keeps
//! the binary icon and the sizes chip with no View menu; an added font, a truncated one, and
//! another file dropping the preview.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use glowfly_core::bundled_fonts::{NOTO_SANS, NOTO_SANS_SEMIBOLD};
use glowfly_core::repo::{BinarySide, DiffTarget};
use glowfly_model::effect::{DiffEffect, Effect};
use glowfly_model::msg::{Msg, RepoMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::diff_header::{self, ChangeText, FileKind};
use glowfly_model::state::diff_preview;
use std::path::{Path, PathBuf};
use std::time::Instant;

const PATH: &str = "fonts/brand.ttf";

/// A repository with fonts/brand.ttf (Noto Sans Regular) committed, changed to `new` in the
/// working tree, and a.txt added; opened, no diff shown.
fn repo(name: &str, new: &[u8]) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    std::fs::create_dir_all(dir.join("fonts")).unwrap();
    std::fs::write(dir.join(PATH), NOTO_SANS).unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "font"]);
    std::fs::write(dir.join(PATH), new).unwrap();
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

/// Opens `target`, runs its diff load and every side request it raises through the runner,
/// and returns how many side requests there were.
fn open(st: &mut AppState, target: DiffTarget) -> usize {
    let fx = update(st, Msg::Repo(RepoMsg::OpenDiff(target)), Instant::now());
    let load = fx.into_iter().find(|e| matches!(e, Effect::Diff(DiffEffect::Load { .. }))).expect("a diff load");
    let msg = glowfly_runner::testing::execute(load).expect("a result");
    let fx = update(st, msg, Instant::now());
    let asks: Vec<Effect> = fx.into_iter().filter(|e| matches!(e, Effect::Diff(DiffEffect::BinarySides { .. }))).collect();
    let n = asks.len();
    for e in asks {
        let msg = glowfly_runner::testing::execute(e).expect("a result");
        drive(st, msg);
    }
    n
}

fn style(s: &Option<BinarySide>) -> Option<&str> {
    s.as_ref().and_then(|s| s.font.as_ref()).map(|f| f.style.as_str())
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_changed_font_is_drawn_on_both_sides_with_a_binary_header() {
    let (mut st, dir) = repo("font-changed", NOTO_SANS_SEMIBOLD);
    assert_eq!(open(&mut st, DiffTarget::Unstaged(PATH.into())), 1, "asked once");
    let s = diff_preview::shown(&st).expect("loaded");
    assert_eq!((style(&s.old), style(&s.new)), (Some("Regular"), Some("SemiBold")));
    assert!(s.old.iter().chain(&s.new).all(|s| s.image.is_some() && s.bytes.is_none()));
    let m = diff_header::model(&st).expect("a header");
    assert_eq!((m.kind, m.view.is_none()), (FileKind::Binary, true));
    let sizes = (Some(NOTO_SANS.len() as u64), Some(NOTO_SANS_SEMIBOLD.len() as u64));
    assert_eq!(m.change, ChangeText::Sizes { old: sizes.0, new: sizes.1 });

    open(&mut st, DiffTarget::Unstaged("a.txt".into()));
    assert!(st.session.diff_preview.is_none(), "another file drops the preview");
    cleanup(&dir);
}

#[test]
fn an_added_font_and_a_truncated_one() {
    let (mut st, dir) = repo("font-added", &NOTO_SANS[..1024]);
    std::fs::write(dir.join("fonts/new.ttf"), NOTO_SANS).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    open(&mut st, DiffTarget::Unstaged("fonts/new.ttf".into()));
    let s = diff_preview::shown(&st).expect("loaded");
    assert!(s.old.is_none(), "untracked");
    assert_eq!(style(&s.new), Some("Regular"));

    open(&mut st, DiffTarget::Unstaged(PATH.into()));
    let s = diff_preview::shown(&st).expect("loaded, though After can't be drawn");
    assert_eq!(style(&s.old), Some("Regular"));
    let new = s.new.as_ref().expect("an After side");
    assert!(new.font.is_none() && new.image.is_none() && new.bytes.is_none());
    assert_eq!(new.size, 1024);
    cleanup(&dir);
}
