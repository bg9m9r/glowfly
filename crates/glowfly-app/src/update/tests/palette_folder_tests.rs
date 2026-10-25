//! Headless tests for folders in the palette's file history finder: the folder rows, Blame's
//! finder keeping to files, and opening the picked folder's history.

use super::palette_tests::repo_with_topic;
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PaletteMsg};
use crate::state::AppState;
use crate::state::history::HistoryKind;
use crate::state::palette::{Items, Mode, PaletteCommand, Pick};
use glowfly_core::repo::HistoryStart;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn pick(st: &AppState) -> &Pick {
    match st.palette.as_ref().map(|p| &p.mode) {
        Some(Mode::Pick(p)) => p,
        _ => panic!("the palette is not in a sub-list"),
    }
}

fn paths() -> Vec<String> {
    ["README.md", "src/main.rs", "src/ui/view.rs"].into_iter().map(String::from).collect()
}

/// Enters the finder of `command` and delivers `paths()` to it.
fn finder(st: &mut AppState, command: PaletteCommand) {
    run(st, Msg::Palette(PaletteMsg::TogglePalette));
    run(st, Msg::Palette(PaletteMsg::RunPaletteCommand(command)));
    let generation = st.palette_gen;
    run(st, Msg::Palette(PaletteMsg::TrackedPathsLoaded { generation, result: Ok(paths()) }));
}

#[test]
fn file_history_finder_lists_folders_before_their_files() {
    let (mut st, dir) = repo_with_topic("pick-folders");
    finder(&mut st, PaletteCommand::FileHistory);
    let want: Vec<String> = ["README.md", "src/", "src/main.rs", "src/ui/", "src/ui/view.rs"].into_iter().map(String::from).collect();
    assert_eq!(pick(&st).items, Items::Ready(want));
    assert_eq!(pick(&st).command(1), Some(PaletteCommand::OpenFolderHistory("src".into())));
    assert_eq!(pick(&st).command(2), Some(PaletteCommand::OpenFileHistory("src/main.rs".into())));
    assert_eq!(pick(&st).command(3), Some(PaletteCommand::OpenFolderHistory("src/ui".into())));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_finder_lists_files_only() {
    let (mut st, dir) = repo_with_topic("pick-folders-blame");
    finder(&mut st, PaletteCommand::Blame);
    assert_eq!(pick(&st).items, Items::Ready(paths()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn picking_a_folder_opens_its_history() {
    let (mut st, dir) = repo_with_topic("pick-folders-open");
    finder(&mut st, PaletteCommand::FileHistory);
    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::OpenFolderHistory("src/ui".into()))));
    assert!(st.palette.is_none());
    let v = st.history.as_ref().expect("history open");
    assert_eq!((v.path.as_str(), v.kind, v.at), ("src/ui", HistoryKind::Folder, None));
    assert!(
        fx.iter().any(|e| matches!(e, Effect::LoadFileHistory { path, folder: true, start: HistoryStart::Head, .. } if path == "src/ui")),
        "{fx:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
