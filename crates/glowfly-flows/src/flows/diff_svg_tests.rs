//! Headless tests for an SVG's Text / Preview switch: a text SVG shows the switch on Text and asks
//! for nothing; Preview (saved in the settings) asks for its sides, drawn by the runner, hides the
//! View menu and the change rows, and back on Text drops them; the setting holds for the next
//! SVG, a Preferences edit takes effect at once, and other text files never show the switch.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use glowfly_core::repo::DiffTarget;
use glowfly_model::effect::Effect;
use glowfly_model::effect::SaveEffect;
use glowfly_model::msg::{DiffMsg, Msg, RepoMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::diff_header::{self, ChangeText, FileKind};
use glowfly_model::state::diff_preview;
use std::path::{Path, PathBuf};
use std::time::Instant;

const OLD: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"16\">\n<rect width=\"24\" height=\"16\"/>\n</svg>\n";

fn svg() -> DiffTarget {
    DiffTarget::Unstaged("icon.svg".into())
}

/// A repository with icon.svg, other.svg and a.txt changed, opened (no diff shown).
fn repo(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    for f in ["icon.svg", "other.svg"] {
        std::fs::write(dir.join(f), OLD).unwrap();
    }
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "icons"]);
    for f in ["icon.svg", "other.svg"] {
        std::fs::write(dir.join(f), OLD.replace("width=\"24\" height", "width=\"32\" height")).unwrap();
    }
    std::fs::write(dir.join("a.txt"), "two\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// Opens `target`, applies its diff and returns the effects that raised, unrun.
fn open(st: &mut AppState, target: DiffTarget) -> Vec<Effect> {
    let fx = run(st, Msg::Repo(RepoMsg::OpenDiff(target)));
    let load = fx.into_iter().find(|e| matches!(e, Effect::Diff(glowfly_model::effect::DiffEffect::Load { .. }))).expect("a diff load");
    let msg = glowfly_runner::testing::execute(load).expect("a result");
    run(st, msg)
}

fn asks(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Diff(glowfly_model::effect::DiffEffect::BinarySides { .. }))).count()
}

/// Runs the sides loads among `fx` and applies their results.
fn finish(st: &mut AppState, fx: Vec<Effect>) {
    for e in fx.into_iter().filter(|e| matches!(e, Effect::Diff(glowfly_model::effect::DiffEffect::BinarySides { .. }))) {
        let msg = glowfly_runner::testing::execute(e).expect("a result");
        drive(st, msg);
    }
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_text_svg_starts_on_text_with_its_rows_and_view_menu() {
    let (mut st, dir) = repo("svg-switch-text");
    let fx = open(&mut st, svg());
    assert_eq!(asks(&fx), 0, "Text asks for no sides");
    assert_eq!(diff_preview::svg_switch(&st), Some(false));
    assert!(!diff_preview::drawn(&st));
    let m = diff_header::model(&st).unwrap();
    assert_eq!((m.svg, m.kind), (Some(false), FileKind::Image));
    assert!(m.view.is_some(), "the View menu");
    assert!(m.counter.is_some_and(|c| !c.files), "the counter is over the changes");

    open(&mut st, DiffTarget::Unstaged("a.txt".into()));
    assert_eq!(diff_preview::svg_switch(&st), None, "no switch for other text");
    assert_eq!(diff_header::model(&st).unwrap().svg, None);
    cleanup(&dir);
}

#[test]
fn preview_draws_both_sides_and_text_drops_them() {
    let (mut st, dir) = repo("svg-switch-preview");
    open(&mut st, svg());
    let fx = run(&mut st, Msg::Diff(DiffMsg::SetSvgPreview(true)));
    assert!(st.settings.diff_svg_preview, "saved in the settings");
    assert!(fx.iter().any(|e| matches!(e, Effect::Save(SaveEffect::Settings(_)))), "{fx:?}");
    assert_eq!(asks(&fx), 1);
    finish(&mut st, fx);
    let s = diff_preview::shown(&st).expect("drawn");
    let dims = |s: &Option<glowfly_core::repo::BinarySide>| s.as_ref().and_then(|s| s.image.as_ref()).map(|i| (i.width, i.height));
    assert_eq!((dims(&s.old), dims(&s.new)), (Some((24, 16)), Some((32, 16))));
    let m = diff_header::model(&st).unwrap();
    assert_eq!(m.svg, Some(true));
    assert!(m.view.is_none(), "no View menu over a drawing");
    assert!(matches!(m.change, ChangeText::Lines { added: 2, removed: 2 }), "the chip still counts the text change: {:?}", m.change);
    assert!(m.counter.is_some_and(|c| c.files), "no change rows: the counter is the file's place");

    let fx = run(&mut st, Msg::Diff(DiffMsg::SetSvgPreview(false)));
    assert_eq!(asks(&fx), 0);
    assert!(st.session.diff_preview.is_none(), "Text drops the sides");
    assert!(diff_header::model(&st).unwrap().view.is_some());
    cleanup(&dir);
}

#[test]
fn the_choice_holds_for_the_next_svg_and_follows_preferences() {
    let (mut st, dir) = repo("svg-switch-next");
    open(&mut st, svg());
    run(&mut st, Msg::Diff(DiffMsg::SetSvgPreview(true)));
    assert_eq!(asks(&open(&mut st, DiffTarget::Unstaged("other.svg".into()))), 1, "the next SVG opens drawn");
    assert_eq!(asks(&run(&mut st, Msg::Diff(DiffMsg::SetSvgPreview(true)))), 0, "the same choice changes nothing");

    // Preferences edits the setting in place: the diff on screen follows at once.
    st.settings.diff_svg_preview = false;
    let edit = glowfly_model::testing::settings_edit(&st, |_| {});
    run(&mut st, edit);
    assert!(st.session.diff_preview.is_none());
    st.settings.diff_svg_preview = true;
    let edit = glowfly_model::testing::settings_edit(&st, |_| {});
    let fx = run(&mut st, edit);
    assert_eq!(asks(&fx), 1);
    let edit = glowfly_model::testing::settings_edit(&st, |_| {});
    assert_eq!(asks(&run(&mut st, edit)), 0, "asked once");
    cleanup(&dir);
}
