//! Headless tests for the palette's "Blame file…" command: when it is offered, its file step,
//! and blaming the picked file at HEAD.

use super::palette_tests::repo_with_topic;
use super::tests::drive;
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PaletteMsg};
use crate::state::AppState;
use crate::state::blame::BlameView;
use crate::state::palette::{Items, Mode, PaletteCommand, Pick, PickKind, shown};
use std::time::Instant;

const BLAME: &str = "Blame file…";

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn labels(st: &AppState) -> Vec<String> {
    shown(st, "").into_iter().map(|s| s.entry.label).collect()
}

fn pick(st: &AppState) -> &Pick {
    match st.palette.as_ref().map(|p| &p.mode) {
        Some(Mode::Pick(p)) => p,
        _ => panic!("the palette is not in a sub-list"),
    }
}

#[test]
fn blame_entry_follows_file_history_and_hides_while_blame_is_open() {
    let (mut st, dir) = repo_with_topic("pick-blame-entry");
    let l = labels(&st);
    let at = l.iter().position(|l| l == BLAME).expect("listed");
    assert_eq!(l[at - 1], "File history…", "right after File history");
    st.session.blame.view = Some(BlameView::loading("a.txt".into(), None));
    let l = labels(&st);
    assert!(!l.iter().any(|l| l == BLAME || l == "File history…" || l == "Search commits"), "{l:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_file_lists_tracked_files_and_blames_the_pick() {
    let (mut st, dir) = repo_with_topic("pick-blame");
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Blame)));
    assert_eq!(pick(&st).kind, PickKind::Blame);
    assert_eq!(pick(&st).items, Items::Loading);
    let g = st.palette_gen;
    assert!(matches!(&fx[..], [Effect::LoadTrackedPaths { generation, .. }] if *generation == g), "{fx:?}");
    assert_eq!(st.palette_recent.keys(), ["blame"]);

    run(&mut st, Msg::Palette(PaletteMsg::TrackedPathsLoaded { generation: g, result: Ok(vec!["a.txt".into()]) }));
    assert_eq!(pick(&st).items, Items::Ready(vec!["a.txt".into()]));
    assert_eq!(pick(&st).command(0), Some(PaletteCommand::OpenBlame("a.txt".into())));

    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::OpenBlame("a.txt".into()))));
    assert!(st.palette.is_none());
    assert_eq!(st.palette_recent.keys(), ["blame"], "the pick itself isn't recorded");
    let v = st.session.blame.view.as_ref().expect("blame open");
    assert_eq!((v.path.as_str(), v.at), ("a.txt", None));
    assert!(v.data.as_ref().is_some_and(|d| d.blame.lines == ["one"]), "{:?}", v.error);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_step_texts() {
    let k = PickKind::Blame;
    assert_eq!((k.title(), k.prompt()), ("Blame", "Find a file to blame"));
    assert_eq!(k.footer(), "Up/Down to choose, Enter to blame, Backspace or Esc to go back");
    assert!(k.lists_files() && k.dims_folders());
    assert!(k.more_text(2, 9).contains("files"));
    assert_eq!(PaletteCommand::Blame.recall_key(), Some("blame"));
    assert_eq!(PaletteCommand::OpenBlame("a.txt".into()).recall_key(), None);
}
