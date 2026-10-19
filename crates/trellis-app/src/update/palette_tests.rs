//! Headless tests for the command palette: opening, the commands it lists, moving the
//! selection, and running commands through the existing messages.

use super::tests::{accept_stash, drive, git, keys, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::PaletteMsg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::msg::{FolderPurpose, Msg};
use crate::state::palette::{PaletteCommand, shown};
use crate::state::palette_commands::SHOW_REPO_FOLDER;
use crate::state::{AppState, Modal, PrefPage, PrefsState, Selection, Tab};
use crate::tag::TagMsg;
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
    run(st, Msg::Palette(PaletteMsg::TogglePalette));
    assert!(st.palette.is_some());
}

/// A scratch repository (a.txt modified) with a second branch `topic`, opened in `st`.
pub(super) fn repo_with_topic(name: &str) -> (AppState, std::path::PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["branch", "topic"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    (st, dir)
}

#[test]
fn toggle_opens_and_closes() {
    let mut st = state();
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    let p = st.palette.as_ref().expect("open");
    assert!(p.query.is_empty());
    assert_eq!(p.selected, 0);
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    assert!(st.palette.is_none());
    open_palette(&mut st);
    run(&mut st, Msg::Palette(PaletteMsg::ClosePalette));
    assert!(st.palette.is_none());
}

#[test]
fn toggle_ignored_under_dialogs() {
    let mut st = state();
    st.modal =
        Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::Repo(RepoMsg::Refresh)) });
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    assert!(st.palette.is_none());

    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::General));
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    assert!(st.palette.is_none());
}

#[test]
fn entries_without_repo() {
    let mut st = state();
    st.settings.theme = ThemeChoice::Dark;
    assert_eq!(
        labels(&st, ""),
        [
            "Open repository…",
            "New tab",
            "Open dashboard",
            "Open preferences",
            "Keyboard shortcuts",
            "Show the welcome",
            "View release notes",
            "Check for updates",
            "Hide toolbar labels",
            "Zoom in",
            "Zoom out",
            "Hide left panel",
            "Hide commit details panel",
            "Hide toolbar",
            "Hide menu bar",
            "Theme: Dark",
            "Theme: Light",
            "Theme: Follow system",
            "Theme: High contrast dark",
            "Theme: High contrast light",
            "Theme: Sand",
            "Configure external tools",
            "Configure terminal",
            "View activity log",
            "View error log",
            "View performance log",
            "Open logs folder",
            "Turn on extended logging",
            "Find repositories in folders…"
        ]
    );
    let list = shown(&st, "");
    assert_eq!(list[15].entry.detail.as_deref(), Some("current"));
    assert!(list[16].entry.detail.is_none() && list[17].entry.detail.is_none());

    st.settings.theme = ThemeChoice::System;
    assert_eq!(shown(&st, "")[17].entry.detail.as_deref(), Some("current"));

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
    assert!(has(&st, "Create tag at HEAD…"));
    assert!(has(&st, "Create annotated tag at HEAD…"));
    let all = labels(&st, "");
    let at = all.iter().position(|l| l == "Create branch at HEAD…").unwrap();
    assert_eq!(all[at + 1..at + 3], ["Create tag at HEAD…", "Create annotated tag at HEAD…"]);
    assert!(has(&st, "Search commits"));
    assert!(has(&st, "Close tab"));
    assert!(has(&st, "Checkout topic"));
    assert!(!has(&st, "Checkout main"), "the checked-out branch is skipped");

    drive(&mut st, Msg::Stage(StageMsg::StageAll));
    assert!(has(&st, "Unstage all files"));
    assert!(!has(&st, "Stage all files"));

    // The active repository isn't offered among the recent ones; others are.
    st.recent = vec![dir.clone(), "C:/elsewhere/other-repo".into()];
    let all = labels(&st, "");
    assert!(all.contains(&"Open other-repo".to_owned()));
    let not_recent = [
        "Open repository…",
        "Open preferences",
        "Open dashboard",
        "Open in terminal",
        "Open repository in external editor",
        "Open logs folder",
        SHOW_REPO_FOLDER,
    ];
    assert_eq!(all.iter().filter(|l| l.starts_with("Open ") && !not_recent.contains(&l.as_str())).count(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn move_wraps_and_resets() {
    let mut st = state();
    open_palette(&mut st);
    assert_eq!(shown(&st, "").len(), 29);
    run(&mut st, Msg::Palette(PaletteMsg::PaletteMove(-1)));
    let p = st.palette.as_ref().unwrap();
    assert_eq!(p.selected, 28);
    assert!(p.scroll_to_selected);
    run(&mut st, Msg::Palette(PaletteMsg::PaletteMove(1)));
    assert_eq!(st.palette.as_ref().unwrap().selected, 0);

    // Six theme entries and "Show the welcome" match "theme".
    st.palette.as_mut().unwrap().query = "theme".into();
    for _ in 0..7 {
        run(&mut st, Msg::Palette(PaletteMsg::PaletteMove(1)));
    }
    assert_eq!(st.palette.as_ref().unwrap().selected, 0);
    // A selection past the end is clamped to the last row, then moved.
    st.palette.as_mut().unwrap().selected = 7;
    run(&mut st, Msg::Palette(PaletteMsg::PaletteMove(1)));
    assert_eq!(st.palette.as_ref().unwrap().selected, 0);
    // No matches: stays on 0.
    st.palette.as_mut().unwrap().query = "zzzz".into();
    run(&mut st, Msg::Palette(PaletteMsg::PaletteMove(1)));
    assert_eq!(st.palette.as_ref().unwrap().selected, 0);

    // Closed: ignored.
    st.palette = None;
    run(&mut st, Msg::Palette(PaletteMsg::PaletteMove(1)));
    assert!(st.palette.is_none());
}

#[test]
fn run_closes_palette_and_stages() {
    let (mut st, dir) = repo_with_topic("palette-stage");
    open_palette(&mut st);
    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::StageAll)));
    assert!(st.palette.is_none());
    assert!(st.snap.as_ref().unwrap().staged.iter().any(|f| f.path == "a.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_checkout_switches_branch() {
    let (mut st, dir) = repo_with_topic("palette-checkout");
    // The modified a.txt is stashed first and comes back after the checkout.
    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Checkout("topic".into()))));
    accept_stash(&mut st);
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("topic"));
    // Line endings as git's checkout writes them (core.autocrlf on Windows).
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap().replace("\r\n", "\n"), "one\ntwo\n");
    assert!(st.snap.as_ref().unwrap().stashes.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_theme_saves_once() {
    let mut st = state();
    st.settings.theme = ThemeChoice::Dark;
    open_palette(&mut st);
    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Theme(ThemeChoice::Light))));
    assert_eq!(st.settings.theme, ThemeChoice::Light);
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.theme == ThemeChoice::Light));
    assert!(st.palette.is_none());
    assert!(run(&mut st, Msg::Prefs(PrefsMsg::SetTheme(ThemeChoice::Light))).is_empty());
    // Nothing left for the per-frame settings check to save.
    assert!(run(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited)).is_empty());
}

#[test]
fn run_search_commits_closes_diff_and_focuses() {
    let (mut st, dir) = repo_with_topic("palette-search");
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into()))));
    assert!(st.diff.is_some());
    open_palette(&mut st);
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::SearchCommits)));
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
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::CreateBranch)));
    assert!(matches!(&st.modal, Some(Modal::NewBranch { start: Some(s), name, .. }) if *s == oid.to_string() && name.is_empty()));

    st.modal = None;
    st.selection = Selection::Wip;
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::CreateBranch)));
    assert!(matches!(&st.modal, Some(Modal::NewBranch { start: None, .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_prefs_close_tab_open_repo() {
    let mut st = state();
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::OpenPrefs)));
    assert!(st.prefs.is_some());

    let mut st = state();
    st.tabs.push(Tab::empty());
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::CloseTab)));
    assert_eq!(st.tabs.len(), 1);

    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::OpenRepo)));
    assert!(matches!(&fx[..], [Effect::PickFolder { purpose: FolderPurpose::OpenRepo, .. }]));

    // Fetch on a repository with a remote (a bare clone of itself).
    let dir = scratch_repo("palette-fetch");
    let bare = dir.with_extension("git");
    let _ = std::fs::remove_dir_all(&bare);
    git(&dir, &["clone", "-q", "--bare", ".", bare.to_str().unwrap()]);
    git(&dir, &["remote", "add", "origin", bare.to_str().unwrap()]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(has(&st, "Fetch all remotes"));
    assert!(has(&st, "Pull"));
    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Fetch)));
    assert!(matches!(&fx[..], [Effect::RunGit { .. }]));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Fetch all"));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn tag_entries_follow_selection() {
    let (mut st, dir) = repo_with_topic("palette-tag-entries");
    let oid = st.snap.as_ref().unwrap().head_oid.unwrap();
    let short = &oid.to_string()[..7];
    st.selection = Selection::Commit(oid);
    assert!(has(&st, &format!("Create tag at {short}…")));
    assert!(has(&st, &format!("Create annotated tag at {short}…")));
    st.selection = Selection::Wip;
    assert!(has(&st, "Create tag at HEAD…"));
    assert!(has(&st, "Create annotated tag at HEAD…"));

    let st = state();
    assert!(!labels(&st, "").iter().any(|l| l.contains("tag at")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_create_tag_opens_dialog() {
    let (mut st, dir) = repo_with_topic("palette-tag-run");
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    st.selection = Selection::Wip;
    open_palette(&mut st);
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::CreateTag { annotated: false })));
    assert!(st.palette.is_none());
    assert!(matches!(
        &st.modal,
        Some(Modal::NewTag(f)) if f.at == head.to_string() && f.name.is_empty() && f.message.is_empty() && !f.annotated
    ));

    st.modal = None;
    // Any selected commit is used as it is, not HEAD.
    let oid = git2::Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap();
    st.selection = Selection::Commit(oid);
    open_palette(&mut st);
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::CreateTag { annotated: true })));
    assert!(matches!(&st.modal, Some(Modal::NewTag(f)) if f.annotated && f.at == oid.to_string()));

    let mut st = state();
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::CreateTag { annotated: false })));
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_tag_msg_opens_modal() {
    let mut st = state();
    let sha = "0123456789abcdef0123456789abcdef01234567".to_owned();
    run(&mut st, Msg::Tag(TagMsg::NewTag { at: sha.clone(), annotated: true }));
    assert!(matches!(
        &st.modal,
        Some(Modal::NewTag(f)) if f.at == sha && f.name.is_empty() && f.message.is_empty() && f.annotated
    ));
}

#[test]
fn shortcut_hints() {
    let (st, dir) = repo_with_topic("palette-hints");
    let list = shown(&st, "");
    let hint = |label: &str| list.iter().find(|s| s.entry.label == label).map(|s| s.entry.shortcut(&st.keymap)).expect(label);
    assert_eq!(hint("Search commits"), Some(keys("Ctrl+F").as_str()));
    assert_eq!(hint("Close tab"), Some(keys("Ctrl+W").as_str()));
    assert_eq!(hint("Open preferences"), Some(keys("Ctrl+,").as_str()));
    assert_eq!(hint("Refresh"), Some("F5"));
    assert_eq!(hint("New tab"), Some(keys("Ctrl+T").as_str()));
    assert_eq!(hint("Stage all files"), Some(keys("Ctrl+Shift+S").as_str()));
    assert_eq!(hint("Keyboard shortcuts"), Some(keys("Ctrl+/").as_str()));
    for label in ["Push", "Stash changes", "Create tag at HEAD…", "Checkout topic"] {
        assert_eq!(hint(label), None, "{label}");
    }
    // F5 refreshes; it doesn't fetch.
    assert_eq!(PaletteCommand::Fetch.binding(), None);
    let _ = std::fs::remove_dir_all(&dir);
}
