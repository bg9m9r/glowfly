//! Headless tests for the palette's recently run commands: which runs are remembered, how they
//! order the list for an empty query, and how they break ties for a typed one.

use super::palette_tests::repo_with_topic;
use super::tests::{drive, state};
use super::update;
use crate::effect::Effect;
use crate::logs::{LogKind, LogsMsg};
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::remote::RemoteMsg;
use crate::state::AppState;
use crate::state::palette::{PaletteCommand as C, StashAction, entries, shown};
use glowfly_core::gitflow::FlowKind;
use glowfly_core::settings::{DiffLayout, ThemeChoice};
use std::collections::HashSet;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn run_cmd(st: &mut AppState, cmd: C) -> Vec<Effect> {
    run(st, Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)))
}

fn labels(st: &AppState, query: &str) -> Vec<String> {
    shown(st, query).into_iter().map(|s| s.entry.label).collect()
}

/// Opens the palette, closing whatever is open first (the palette or the shortcut sheet).
fn reopen(st: &mut AppState) {
    st.palette = None;
    st.modal = None;
    run(st, Msg::Palette(PaletteMsg::TogglePalette));
    assert!(st.palette.is_some());
}

#[test]
fn recent_commands_list_first_for_empty_query() {
    let mut st = state();
    run_cmd(&mut st, C::Theme(ThemeChoice::Light));
    run_cmd(&mut st, C::KeyboardShortcuts);
    reopen(&mut st);
    let all = labels(&st, "");
    assert_eq!(all[0..2], ["Keyboard shortcuts", "Theme: Light"]);
    let open = all.iter().position(|l| l == "Open repository…").unwrap();
    assert!(open > 1, "{all:?}");
    assert_eq!(all.len(), entries(&st).len());
}

#[test]
fn fresh_session_keeps_default_order() {
    let st = state();
    assert!(st.palette_recent.is_empty());
    let default: Vec<String> = entries(&st).into_iter().map(|e| e.label).collect();
    assert_eq!(labels(&st, ""), default);
}

#[test]
fn toggle_entry_ranks_first_after_flip() {
    let mut st = state();
    run_cmd(&mut st, C::ToolbarLabels(false));
    assert_eq!(labels(&st, "")[0], "Show toolbar labels");

    let (mut st, dir) = repo_with_topic("palette-recent-toggle");
    run_cmd(&mut st, C::IgnoreWhitespace(true));
    assert_eq!(labels(&st, "")[0], "Show whitespace changes");
    run_cmd(&mut st, C::DiffLayout(DiffLayout::Split));
    assert_eq!(labels(&st, "")[0..2], ["Use inline diff layout", "Show whitespace changes"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn query_ranking_beats_recency() {
    let mut st = state();
    run_cmd(&mut st, C::OpenPrefs);
    st.prefs = None;
    assert_eq!(labels(&st, "new tab")[0], "New tab");
}

#[test]
fn recency_breaks_score_ties() {
    let mut st = state();
    let before = labels(&st, "theme");
    assert_eq!(before[..3], ["Theme: Dark", "Theme: Sand", "Theme: Light"]);
    run_cmd(&mut st, C::Theme(ThemeChoice::Light));
    let after = labels(&st, "theme");
    assert_eq!(after[..3], ["Theme: Light", "Theme: Dark", "Theme: Sand"]);
    assert_eq!(after.len(), before.len());
}

#[test]
fn picks_and_checkouts_are_not_recorded() {
    let (mut st, dir) = repo_with_topic("palette-recent-picks");
    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Checkout("topic".into()))));
    assert!(st.palette_recent.is_empty());
    let oid = st.snap.as_ref().and_then(|s| s.head_oid).unwrap();
    let unkeyed = [
        C::StashOp { action: StashAction::Apply, oid },
        C::StashWithMessage("x".into()),
        C::CheckoutRemote { remote: "origin".into(), branch: "main".into() },
        C::CheckoutTag("v1".into()),
        C::OpenRecent(dir.clone()),
        C::OpenFileHistory("a.txt".into()),
        C::OpenBlame("a.txt".into()),
        C::FinishGitFlow("feature/x".into()),
        C::Remote(RemoteMsg::Fetch("origin".into())),
        C::RepoList(crate::state::palette_commands::RepoListCommand::OpenFavorite { index: 0, path: dir.clone() }),
    ];
    for cmd in unkeyed {
        assert_eq!(cmd.recall_key(), None, "{cmd:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_step_opener_is_recorded() {
    let (mut st, dir) = repo_with_topic("palette-recent-two-step");
    run_cmd(&mut st, C::FileHistory);
    assert!(st.palette.is_some(), "the file step opens");
    assert_eq!(st.palette_recent.keys(), ["file-history"]);
    run_cmd(&mut st, C::OpenFileHistory("a.txt".into()));
    assert_eq!(st.palette_recent.keys(), ["file-history"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn recent_survives_reopen() {
    let (mut st, dir) = repo_with_topic("palette-recent-reopen");
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    run_cmd(&mut st, C::Refresh);
    assert!(st.palette.is_none());
    // Open, close and open again.
    for _ in 0..3 {
        run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    }
    assert_eq!(shown(&st, "")[0].entry.command, C::Refresh);
    assert_eq!(st.palette.as_ref().map(|p| p.selected), Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn recall_keys_are_unique() {
    let oid = git2::Oid::zero();
    let keyed = [
        C::Fetch,
        C::Pull,
        C::Push,
        C::Stash,
        C::PopStash,
        C::ChooseStash(StashAction::Apply),
        C::ChooseStash(StashAction::Pop),
        C::ChooseStash(StashAction::Drop),
        C::StashMessage,
        C::StashDialog(glowfly_core::stash_ops::StashScope::All),
        C::StashDialog(glowfly_core::stash_ops::StashScope::Staged),
        C::StashDialog(glowfly_core::stash_ops::StashScope::Files),
        C::StageAll,
        C::UnstageAll,
        C::Undo,
        C::Redo,
        C::CreateBranch,
        C::CreateTag { annotated: false },
        C::CreateTag { annotated: true },
        C::RenameBranch("main".into()),
        C::SearchCommits,
        C::InitializeLfs,
        C::ConfigureLfs,
        C::ConfigureGitFlow,
        C::ConfigureSparse,
        C::StartGitFlow(FlowKind::Feature),
        C::StartGitFlow(FlowKind::Release),
        C::StartGitFlow(FlowKind::Hotfix),
        C::Refresh,
        C::OpenTerminal,
        C::ToggleTerminal,
        C::ShowInFileManager,
        C::DiffLayout(DiffLayout::Inline),
        C::IgnoreWhitespace(true),
        C::OpenRepo,
        C::NewTab,
        C::ReopenClosedTab,
        C::CloseTab,
        C::OpenPrefs,
        C::KeyboardShortcuts,
        C::ToolbarLabels(true),
        C::Theme(ThemeChoice::Dark),
        C::Theme(ThemeChoice::Light),
        C::Theme(ThemeChoice::System),
        C::Theme(ThemeChoice::HighContrastDark),
        C::Theme(ThemeChoice::HighContrastLight),
        C::Theme(ThemeChoice::Sand),
        C::FileHistory,
        C::Blame,
        C::Remote(RemoteMsg::AddDialog),
        C::Logs(LogsMsg::Show(LogKind::Activity)),
        C::Logs(LogsMsg::Show(LogKind::Errors)),
        C::Logs(LogsMsg::Show(LogKind::Performance)),
        C::Logs(LogsMsg::OpenFolder),
        C::Logs(LogsMsg::SetExtended(true)),
    ];
    let keys: Vec<&str> = keyed.iter().map(|c| c.recall_key().unwrap_or_else(|| panic!("{c:?} has no key"))).collect();
    let unique: HashSet<&str> = keys.iter().copied().collect();
    assert_eq!(unique.len(), keys.len(), "{keys:?}");
    // A toggle keeps its key whichever way it flips.
    assert_eq!(C::ToolbarLabels(false).recall_key(), C::ToolbarLabels(true).recall_key());
    assert_eq!(C::IgnoreWhitespace(false).recall_key(), C::IgnoreWhitespace(true).recall_key());
    assert_eq!(C::DiffLayout(DiffLayout::Split).recall_key(), C::DiffLayout(DiffLayout::Inline).recall_key());
    assert_eq!(C::StashOp { action: StashAction::Drop, oid }.recall_key(), None);
}
