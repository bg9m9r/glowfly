//! Headless tests for the command palette: opening, the commands it lists, moving the
//! selection, and running commands through the existing messages.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{FolderPurpose, Msg};
use crate::state::palette::{PaletteCommand, shown};
use crate::state::{AppState, Modal, PrefPage, PrefsState, Selection, Tab};
use std::time::Instant;
use trellis_core::repo::DiffTarget;
use trellis_core::settings::ThemeChoice;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn labels(st: &AppState, query: &str) -> Vec<String> {
    shown(st, query).into_iter().map(|s| s.entry.label).collect()
}

fn has(st: &AppState, label: &str) -> bool {
    labels(st, "").iter().any(|l| l == label)
}

fn open_palette(st: &mut AppState) {
    run(st, Msg::TogglePalette);
    assert!(st.palette.is_some());
}

/// A scratch repository (a.txt modified) with a second branch `topic`, opened in `st`.
pub(super) fn repo_with_topic(name: &str) -> (AppState, std::path::PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["branch", "topic"]);
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    assert!(st.snap.is_some());
    (st, dir)
}

#[test]
fn toggle_opens_and_closes() {
    let mut st = state();
    run(&mut st, Msg::TogglePalette);
    let p = st.palette.as_ref().expect("open");
    assert!(p.query.is_empty());
    assert_eq!(p.selected, 0);
    run(&mut st, Msg::TogglePalette);
    assert!(st.palette.is_none());
    open_palette(&mut st);
    run(&mut st, Msg::ClosePalette);
    assert!(st.palette.is_none());
}

#[test]
fn toggle_ignored_under_dialogs() {
    let mut st = state();
    st.modal = Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::Refresh) });
    run(&mut st, Msg::TogglePalette);
    assert!(st.palette.is_none());

    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::General));
    run(&mut st, Msg::TogglePalette);
    assert!(st.palette.is_none());
}

#[test]
fn entries_without_repo() {
    let mut st = state();
    st.settings.theme = ThemeChoice::Dark;
    assert_eq!(labels(&st, ""), ["Open repository…", "Open Preferences", "Theme: Dark", "Theme: Light", "Theme: Follow system"]);
    let list = shown(&st, "");
    assert_eq!(list[2].entry.detail.as_deref(), Some("current"));
    assert!(list[3].entry.detail.is_none() && list[4].entry.detail.is_none());

    st.settings.theme = ThemeChoice::System;
    assert_eq!(shown(&st, "")[4].entry.detail.as_deref(), Some("current"));

    st.tabs.push(Tab::empty());
    assert!(has(&st, "Close tab"));
}

#[test]
fn entries_with_repo() {
    let (mut st, dir) = repo_with_topic("palette-entries");
    assert!(!has(&st, "Fetch all remotes"), "no remote");
    assert!(!has(&st, "Pull"));
    assert!(has(&st, "Push"));
    assert!(has(&st, "Stage all files"));
    assert!(!has(&st, "Unstage all files"));
    assert!(has(&st, "Create branch at HEAD…"));
    assert!(has(&st, "Search commits"));
    assert!(has(&st, "Close tab"));
    assert!(has(&st, "Checkout topic"));
    assert!(!has(&st, "Checkout main"), "the checked-out branch is skipped");

    drive(&mut st, Msg::StageAll);
    assert!(has(&st, "Unstage all files"));
    assert!(!has(&st, "Stage all files"));

    // The active repository isn't offered among the recent ones; others are.
    st.recent = vec![dir.clone(), "C:/elsewhere/other-repo".into()];
    let all = labels(&st, "");
    assert!(all.contains(&"Open other-repo".to_owned()));
    assert_eq!(
        all.iter().filter(|l| l.starts_with("Open ") && l.as_str() != "Open repository…" && l.as_str() != "Open Preferences").count(),
        1
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn move_wraps_and_resets() {
    let mut st = state();
    open_palette(&mut st);
    assert_eq!(shown(&st, "").len(), 5);
    run(&mut st, Msg::PaletteMove(-1));
    let p = st.palette.as_ref().unwrap();
    assert_eq!(p.selected, 4);
    assert!(p.scroll_to_selected);
    run(&mut st, Msg::PaletteMove(1));
    assert_eq!(st.palette.as_ref().unwrap().selected, 0);

    // Three theme entries match "theme".
    st.palette.as_mut().unwrap().query = "theme".into();
    for _ in 0..3 {
        run(&mut st, Msg::PaletteMove(1));
    }
    assert_eq!(st.palette.as_ref().unwrap().selected, 0);
    // A selection past the end is clamped to the last row, then moved.
    st.palette.as_mut().unwrap().selected = 7;
    run(&mut st, Msg::PaletteMove(1));
    assert_eq!(st.palette.as_ref().unwrap().selected, 0);
    // No matches: stays on 0.
    st.palette.as_mut().unwrap().query = "zzzz".into();
    run(&mut st, Msg::PaletteMove(1));
    assert_eq!(st.palette.as_ref().unwrap().selected, 0);

    // Closed: ignored.
    st.palette = None;
    run(&mut st, Msg::PaletteMove(1));
    assert!(st.palette.is_none());
}

#[test]
fn run_closes_palette_and_stages() {
    let (mut st, dir) = repo_with_topic("palette-stage");
    open_palette(&mut st);
    drive(&mut st, Msg::RunPaletteCommand(PaletteCommand::StageAll));
    assert!(st.palette.is_none());
    assert!(st.snap.as_ref().unwrap().staged.iter().any(|f| f.path == "a.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_checkout_switches_branch() {
    let (mut st, dir) = repo_with_topic("palette-checkout");
    // Checkout keeps the modified a.txt (topic is at the same commit).
    drive(&mut st, Msg::RunPaletteCommand(PaletteCommand::Checkout("topic".into())));
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("topic"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_theme_saves_once() {
    let mut st = state();
    st.settings.theme = ThemeChoice::Dark;
    open_palette(&mut st);
    let fx = run(&mut st, Msg::RunPaletteCommand(PaletteCommand::Theme(ThemeChoice::Light)));
    assert_eq!(st.settings.theme, ThemeChoice::Light);
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.theme == ThemeChoice::Light));
    assert!(st.palette.is_none());
    assert!(run(&mut st, Msg::SetTheme(ThemeChoice::Light)).is_empty());
    // Nothing left for the per-frame settings check to save.
    assert!(run(&mut st, Msg::SettingsEdited).is_empty());
}

#[test]
fn run_search_commits_closes_diff_and_focuses() {
    let (mut st, dir) = repo_with_topic("palette-search");
    drive(&mut st, Msg::OpenDiff(DiffTarget::Unstaged("a.txt".into())));
    assert!(st.diff.is_some());
    open_palette(&mut st);
    run(&mut st, Msg::RunPaletteCommand(PaletteCommand::SearchCommits));
    assert!(st.diff.is_none());
    assert!(st.view.show_search);
    assert!(st.view.focus_search);
    assert!(st.palette.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_create_branch_uses_selected_commit() {
    let (mut st, dir) = repo_with_topic("palette-branch");
    let oid = st.snap.as_ref().unwrap().head_oid.unwrap();
    st.selection = Selection::Commit(oid);
    let short = &oid.to_string()[..7];
    assert!(has(&st, &format!("Create branch at {short}…")));
    run(&mut st, Msg::RunPaletteCommand(PaletteCommand::CreateBranch));
    assert!(matches!(&st.modal, Some(Modal::NewBranch { start: Some(s), name, .. }) if *s == oid.to_string() && name.is_empty()));

    st.modal = None;
    st.selection = Selection::Wip;
    run(&mut st, Msg::RunPaletteCommand(PaletteCommand::CreateBranch));
    assert!(matches!(&st.modal, Some(Modal::NewBranch { start: None, .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_prefs_close_tab_open_repo() {
    let mut st = state();
    run(&mut st, Msg::RunPaletteCommand(PaletteCommand::OpenPrefs));
    assert!(st.prefs.is_some());

    let mut st = state();
    st.tabs.push(Tab::empty());
    run(&mut st, Msg::RunPaletteCommand(PaletteCommand::CloseTab));
    assert_eq!(st.tabs.len(), 1);

    let fx = run(&mut st, Msg::RunPaletteCommand(PaletteCommand::OpenRepo));
    assert!(matches!(&fx[..], [Effect::PickFolder { purpose: FolderPurpose::OpenRepo, .. }]));

    // Fetch on a repository with a remote (a bare clone of itself).
    let dir = scratch_repo("palette-fetch");
    let bare = dir.with_extension("git");
    let _ = std::fs::remove_dir_all(&bare);
    git(&dir, &["clone", "-q", "--bare", ".", bare.to_str().unwrap()]);
    git(&dir, &["remote", "add", "origin", bare.to_str().unwrap()]);
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    assert!(has(&st, "Fetch all remotes"));
    assert!(has(&st, "Pull"));
    let fx = run(&mut st, Msg::RunPaletteCommand(PaletteCommand::Fetch));
    assert!(matches!(&fx[..], [Effect::RunGit { .. }]));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Fetch all"));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&bare);
}
