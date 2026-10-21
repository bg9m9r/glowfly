//! Headless tests for the palette's stash, undo, branch, refresh, terminal, tab, diff and
//! toolbar commands: when each is offered, and that running it does what its button does.

use super::palette_tests::repo_with_topic;
use super::tests::{drive, git, keys, scratch_repo, state};
use super::update;
use crate::effect::OsEffect;
use crate::effect::{Effect, LoadKind};
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::state::palette::{Entry, PaletteCommand as C, entries, shown};
use crate::state::palette_commands::SHOW_REPO_FOLDER;
use crate::state::{AppState, Modal};
use std::time::Instant;
use trellis_core::settings::DiffLayout;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn find(st: &AppState, label: &str) -> Option<Entry> {
    entries(st).into_iter().find(|e| e.label == label)
}

fn has(st: &AppState, label: &str) -> bool {
    find(st, label).is_some()
}

fn matches(st: &AppState, query: &str, label: &str) -> bool {
    shown(st, query).iter().any(|s| s.entry.label == label)
}

fn open_palette(st: &mut AppState) {
    run(st, Msg::Palette(PaletteMsg::TogglePalette));
    assert!(st.palette.is_some());
}

#[test]
fn stash_and_pop_entries() {
    let (mut st, dir) = repo_with_topic("palette-more-stash");
    assert!(has(&st, "Stash changes"));
    assert!(!has(&st, "Pop stash"));

    open_palette(&mut st);
    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Stash)));
    assert!(st.palette.is_none());
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 1);
    assert!(!snap.is_dirty());
    assert!(!has(&st, "Stash changes"));
    let pop = find(&st, "Pop stash").expect("Pop stash");
    assert!(pop.detail.as_deref().is_some_and(|d| d.contains("main")), "{:?}", pop.detail);

    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::PopStash)));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.stashes.is_empty());
    assert!(snap.unstaged.iter().any(|f| f.path == "a.txt"));
    assert!(has(&st, "Stash changes"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_redo_entries_follow_history() {
    let (mut st, dir) = repo_with_topic("palette-more-undo");
    super::tests::clean_tree(&dir);
    drive(&mut st, Msg::Repo(crate::msg::RepoMsg::Refresh));
    assert!(!has(&st, "Undo"));
    assert!(!has(&st, "Redo"));

    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Checkout("topic".into()))));
    let undo = find(&st, "Undo").expect("Undo");
    assert_eq!(undo.detail.as_deref(), Some("checkout of topic"));
    assert_eq!(undo.shortcut(&st.keymap), Some(keys("Ctrl+Z").as_str()));

    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Undo)));
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("main"));
    let redo = find(&st, "Redo").expect("Redo");
    assert_eq!(redo.shortcut(&st.keymap), Some(keys("Ctrl+Y").as_str()));

    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Redo)));
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("topic"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_branch_entry() {
    let (mut st, dir) = repo_with_topic("palette-more-rename");
    assert!(has(&st, "Rename branch main…"));
    open_palette(&mut st);
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::RenameBranch("main".into()))));
    assert!(st.palette.is_none());
    assert!(matches!(&st.modal, Some(Modal::RenameBranch { old, name, .. }) if old == "main" && name == "main"));

    st.modal = None;
    git(&dir, &["checkout", "-q", "--detach"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.snap.as_ref().unwrap().head_branch.is_none());
    assert!(!entries(&st).iter().any(|e| e.label.starts_with("Rename branch")));

    assert!(!entries(&state()).iter().any(|e| e.label.starts_with("Rename branch")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refresh_entry() {
    let (mut st, dir) = repo_with_topic("palette-more-refresh");
    assert_eq!(find(&st, "Refresh").expect("Refresh").shortcut(&st.keymap), Some("F5"));
    open_palette(&mut st);
    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Refresh)));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { kind: LoadKind::Refresh, .. })));
    assert!(st.palette.is_none());

    assert!(!has(&state(), "Refresh"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn terminal_and_file_manager_entries() {
    let (mut st, dir) = repo_with_topic("palette-more-terminal");
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    assert!(has(&st, "Open in terminal"));
    assert!(has(&st, SHOW_REPO_FOLDER));

    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::OpenTerminal)));
    assert!(matches!(&fx[..], [Effect::Os(OsEffect::OpenTerminal(d))] if *d == workdir), "{fx:?}");
    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ShowInFileManager)));
    assert!(matches!(&fx[..], [Effect::Os(OsEffect::OpenFolder(d))] if *d == workdir), "{fx:?}");

    let empty = state();
    assert!(!has(&empty, "Open in terminal"));
    assert!(!has(&empty, SHOW_REPO_FOLDER));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn terminal_panel_entry_toggles() {
    let (mut st, dir) = repo_with_topic("palette-more-panel");
    let show = find(&st, "Show terminal panel").expect("Show terminal panel");
    assert_eq!(show.shortcut(&st.keymap), Some("Ctrl+`"));
    assert!(matches(&st, "integrated", "Show terminal panel"));

    open_palette(&mut st);
    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ToggleTerminal)));
    assert!(st.palette.is_none() && st.terminal.open);
    assert!(fx.iter().any(|e| matches!(e, Effect::Terminal(crate::terminal::TerminalEffect::Spawn { .. }))), "{fx:?}");
    assert!(has(&st, "Hide terminal panel"));
    assert!(!has(&st, "Show terminal panel"));
    // The external terminal entry is unchanged.
    assert!(has(&st, "Open in terminal"));

    assert!(!has(&state(), "Show terminal panel"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tab_entries() {
    let mut st = state();
    assert_eq!(find(&st, "New tab").expect("New tab").shortcut(&st.keymap), Some(keys("Ctrl+T").as_str()));
    assert!(!has(&st, "Reopen closed tab"));
    open_palette(&mut st);
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::NewTab)));
    assert!(st.palette.is_none());
    assert_eq!(st.tabs.len(), 2);

    let mut st = state();
    st.closed_tabs = vec![std::path::PathBuf::from("C:/work/old-repo").into()];
    let reopen = find(&st, "Reopen closed tab").expect("Reopen closed tab");
    assert_eq!(reopen.detail.as_deref(), Some("old-repo"));
    assert_eq!(reopen.shortcut(&st.keymap), Some(keys("Ctrl+Shift+T").as_str()));

    let dir = scratch_repo("palette-more-reopen");
    st.closed_tabs = vec![dir.clone().into()];
    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ReopenClosedTab)));
    assert!(st.closed_tabs.is_empty());
    assert!(st.snap.as_ref().is_some_and(|s| s.workdir == st.workdir().unwrap()));
    assert!(st.tabs.iter().any(|t| t.is(&dir)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn diff_toggles_name_the_next_state() {
    let (mut st, dir) = repo_with_topic("palette-more-diff");
    assert_eq!(st.settings.diff_layout, DiffLayout::Inline);
    assert!(!st.settings.diff_ignore_whitespace);
    assert!(has(&st, "Use split diff layout"));
    assert!(has(&st, "Ignore whitespace changes"));

    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::DiffLayout(DiffLayout::Split))));
    assert_eq!(st.settings.diff_layout, DiffLayout::Split);
    assert!(matches!(&fx[..], [Effect::SaveSettings(_)]), "{fx:?}");
    assert!(has(&st, "Use inline diff layout"));
    assert!(!has(&st, "Use split diff layout"));

    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::IgnoreWhitespace(true))));
    assert!(st.settings.diff_ignore_whitespace);
    assert!(has(&st, "Show whitespace changes"));
    assert!(!has(&st, "Ignore whitespace changes"));

    let empty = state();
    for label in ["Use split diff layout", "Use inline diff layout", "Ignore whitespace changes", "Show whitespace changes"] {
        assert!(!has(&empty, label), "{label}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn toolbar_labels_toggle_saves_once() {
    let mut st = state();
    assert!(st.settings.toolbar_labels);
    assert!(has(&st, "Hide toolbar labels"));
    open_palette(&mut st);
    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ToolbarLabels(false))));
    assert!(!st.settings.toolbar_labels);
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if !s.toolbar_labels), "{fx:?}");
    assert!(st.palette.is_none());
    // Nothing left for the per-frame settings check to save.
    assert!(run(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited)).is_empty());
    assert!(has(&st, "Show toolbar labels"));
    assert!(!has(&st, "Hide toolbar labels"));
    assert!(run(&mut st, Msg::Prefs(PrefsMsg::SetToolbarLabels(false))).is_empty());
}

#[test]
fn keywords_match_synonyms() {
    let (mut st, dir) = repo_with_topic("palette-more-keywords");
    // A stash, then a dirty tree again.
    drive(&mut st, Msg::git("Stash", &["stash", "push", "-u"]));
    std::fs::write(dir.join("b.txt"), "new\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.stashes.is_empty() && snap.is_dirty());

    assert!(matches(&st, "shelve", "Stash changes"));
    assert!(matches(&st, "describe", "Stash with message…"));
    assert!(matches(&st, "unstash", "Pop stash"));
    assert!(matches(&st, "console", "Open in terminal"));
    assert!(matches(&st, "finder", SHOW_REPO_FOLDER));
    assert!(matches(&st, "reload", "Refresh"));
    assert!(matches(&st, "side by side", "Use split diff layout"));
    assert!(matches(&st, "indentation", "Ignore whitespace changes"));
    assert!(matches(&st, "captions", "Hide toolbar labels"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tab_rename_move_and_reopen_entries() {
    use super::tabs_close_tests::{fake, fake_tabs, names};
    use crate::state::tabs::ClosedTab;
    let mut st = fake_tabs(&["a", "b", "c"], 0);
    assert!(!has(&st, "Move tab left"), "the first tab can't move left");
    let right = find(&st, "Move tab right").expect("Move tab right");
    assert_eq!(right.shortcut(&st.keymap), Some(keys("Ctrl+Shift+PgDn").as_str()));
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::MoveTab(1))));
    assert_eq!((names(&st), st.active_tab), (vec!["b".to_owned(), "a".into(), "c".into()], 1));
    let left = find(&st, "Move tab left").expect("Move tab left");
    assert_eq!(left.command, C::MoveTab(-1));
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::MoveTab(-1))));
    assert_eq!((names(&st), st.active_tab), (vec!["a".to_owned(), "b".into(), "c".into()], 0));

    assert_eq!(find(&st, "Rename tab…").map(|e| e.command), Some(C::RenameTab));
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::RenameTab)));
    assert!(matches!(&st.modal, Some(Modal::RenameTab(f)) if f.tab == st.tabs[0].id));
    st.modal = None;

    st.closed_tabs = vec![ClosedTab::from(fake("old")), ClosedTab { path: fake("web"), label: Some("Frontend".into()) }];
    let reopen: Vec<Entry> = entries(&st).into_iter().filter(|e| e.label.starts_with("Reopen closed tab: ")).collect();
    let labels: Vec<&str> = reopen.iter().map(|e| e.label.as_str()).collect();
    assert_eq!(labels, ["Reopen closed tab: Frontend", "Reopen closed tab: old"], "newest first");
    assert_eq!(reopen[1].detail.as_deref(), Some(trellis_core::native_path::shown(&fake("old")).as_str()));
    assert_eq!(find(&st, "Reopen closed tab").and_then(|e| e.detail), Some("Frontend".into()));
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(reopen[1].command.clone())));
    assert_eq!(st.closed_tabs.len(), 1);
    assert!(st.tabs[st.active_tab].is(&fake("old")));

    assert_eq!(C::RenameTab.recall_key(), Some("rename-tab"));
    assert_eq!(C::MoveTab(-1).recall_key(), Some("move-tab-left"));
    assert_eq!(C::MoveTab(1).recall_key(), Some("move-tab-right"));
    assert_eq!(C::ReopenClosed(fake("old")).recall_key(), None);
}
