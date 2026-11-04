//! Text cut with "…" and its whole text on hover: the WIP lists' headers in German (narrow, with
//! the more-actions menu, and wide) and in a narrow English panel (icon-only actions), graph columns at their narrowest, the filtered graph's
//! header, a cut commit message, a long issue title in the sidebar, the status bar's update
//! chip, the repository sync dialog's names, and the notes that wrap instead (a remote without
//! branches, a Git LFS pointer in an untrusted repository), and names that share a long start
//! cut in their middle in the left panel. Shares the fixtures of the other snapshot tests.

use crate::snapshot_check::CheckSnapshot;
use crate::snapshot_tests::snapshot_tests_columns::{H, grip, pointer};
use crate::snapshot_tests::snapshot_tests_history::with_file_view;
use crate::snapshot_tests::snapshot_tests_history_file::set_text;
use crate::snapshot_tests::snapshot_tests_issue::{crash, github, with_issue};
use crate::snapshot_tests::snapshot_tests_language_panels::click_header_action;
use crate::snapshot_tests::snapshot_tests_remotes::with_remotes;
use crate::snapshot_tests::snapshot_tests_repo_sync::{dialog, row};
use crate::snapshot_tests::snapshot_tests_search::{searching, send, with_two_authors};
use crate::snapshot_tests::snapshot_tests_updates::{sent, status_bar, the_offer};
use crate::snapshot_tests::snapshot_tests_worktrees::{center, p, with_worktrees};
use crate::snapshot_tests::{git, harness, harness_sized, reload, remove_fixture, with_repo};
use eframe::egui::{pos2, vec2};
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::Issue;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, LfsState, LoadOptions, WorktreeInfo};
use glowfly_core::repo_sync::Outcome;
use glowfly_core::settings::{ColumnWidths, DescriptionMode, ThemeChoice};
use glowfly_model::msg::{Msg, SearchMsg, UpdateMsg, WorkFileMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::repo_sync::{RowState, SyncKind, SyncRow};
use glowfly_model::state::updates::UpdateStatus;
use std::path::PathBuf;

/// Lets a hover's tooltip appear.
fn settle(h: &mut H) {
    for _ in 0..4 {
        h.step();
    }
    // A loading pane's spinner keeps repainting; stop at the step limit then.
    h.run_ok();
}

fn german_wip(name: &str, details: f32) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    st.settings.language = "de".into();
    st.settings.panel_sizes.details = Some(details);
    (st, dir)
}

#[test]
fn wip_header_german_narrow() {
    let (st, dir) = german_wip("truncate-wip-narrow", 300.0);
    let de = Lang::De;
    let (more, new_file, discard) =
        (de.tr("details.wip.more.tip"), de.tr("details.wip.new_file.button"), de.tr("details.wip.discard_all.button"));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(de.tr("details.wip.stage_all.button")).is_some(), "Alle bereitstellen stays a button");
    assert!(h.query_by_label(more).is_some(), "the more-actions button is shown");
    assert!(h.query_by_label(new_file).is_none(), "Neue Datei… moved into the menu");
    h.check_snapshot("wip_header_german_narrow");

    h.get_by_label(more).click();
    h.run();
    h.check_snapshot("wip_header_german_menu");
    h.get_by_label(new_file).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::WorkFile(WorkFileMsg::NewFileDialog(_)))));

    click_header_action(&mut h, discard, de.tr("details.wip.discard_all.item"), more);
    let title = de.tr("details.wip.discard_all.title");
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmDanger { title: t, .. } if t == title)), "Discard all asks in German");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn wip_header_german_wide() {
    // Wide enough for Stage all beside the whole title (Discard all and New file… keep to the
    // menu), in a wider window so the graph keeps
    // its columns.
    let (st, dir) = german_wip("truncate-wip-wide", 600.0);
    let de = Lang::De;
    let mut h = harness_sized(st, true, vec2(1600.0, 820.0));
    h.run();
    assert!(h.query_by_label(de.tr("details.wip.stage_all.button")).is_some());
    for key in ["details.wip.discard_all.button", "details.wip.new_file.button"] {
        assert!(h.query_by_label(de.tr(key)).is_none(), "{key} waits in the menu");
    }
    assert!(h.query_by_label(de.tr("details.wip.more.tip")).is_some());
    h.check_snapshot("wip_header_german_wide");
    drop(h);
    remove_fixture(&dir);
}

/// Whether `label` is a Small (24 px) square in `h`.
fn small_square(h: &H, label: &str) -> bool {
    let b = h.get_by_label(label).bounding_box().expect("bounds");
    (b.width(), b.height()) == (24.0, 24.0)
}

/// The WIP headers in English in a narrow panel: Stage all is a Small icon-only square (still
/// named "Stage all") beside the whole title, and the more-actions button stays.
#[test]
fn wip_header_icons() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "truncate-wip-icons");
    st.settings.panel_sizes.details = Some(250.0);
    let en = Lang::En;
    let mut h = harness(st, true);
    h.run();
    assert!(small_square(&h, en.tr("details.wip.stage_all.button")), "Stage all is a Small square");
    assert!(h.query_by_label(en.tr("details.wip.more.tip")).is_some());
    h.check_snapshot("wip_header_icons");
    drop(h);
    remove_fixture(&dir);
}

/// With every change staged, in German in the narrowest panel (English fits its labels there):
/// Staged Files' Unstage all is the minus square and Unstaged Files' New file… the
/// file-and-plus square, each beside its whole title.
#[test]
fn wip_header_icons_staged() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "truncate-wip-icons-staged");
    git(&dir, &["add", "-A"], "2026-01-11T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    st.settings.language = "de".into();
    st.settings.panel_sizes.details = Some(250.0);
    let de = Lang::De;
    let mut h = harness(st, true);
    h.run();
    assert!(small_square(&h, de.tr("details.wip.unstage_all.button")), "Unstage all is a Small square");
    assert!(small_square(&h, de.tr("details.wip.new_file.button")), "New file… is a Small square");
    h.check_snapshot("wip_header_icons_staged");
    drop(h);
    remove_fixture(&dir);
}

const LONG_AUTHOR: &str = "Augusta Ada King-Noel";

#[test]
fn graph_columns_narrow() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "truncate-columns");
    st.settings.column_widths =
        ColumnWidths { refs: 100.0, changes: 60.0, author: 80.0, commit: 80.0, date: 80.0, ..ColumnWidths::default() };
    // A name longer than the narrowest author column.
    let snap = st.snap.as_mut().unwrap();
    let c = std::sync::Arc::get_mut(&mut snap.commits).unwrap().iter_mut().find(|c| c.summary == "Search results page").unwrap();
    c.author = glowfly_core::repo::Author::new(LONG_AUTHOR, c.author.email());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("graph_columns_narrow");
    let x = grip(&h, "Resize author column").x + 20.0;
    let y = center(&h, "Search results page").y;
    pointer(&mut h, pos2(x, y), None);
    settle(&mut h);
    assert!(h.query_by_label(LONG_AUTHOR).is_some(), "the cut author's hover has the whole name");
    h.check_snapshot("graph_author_cut_tip");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn uncut_cells_have_no_hover() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "truncate-uncut");
    let mut h = harness(st, true);
    h.run();
    let x = grip(&h, "Resize commit column").x + 20.0;
    let y = center(&h, "Search results page").y - 7.0;
    pointer(&mut h, pos2(x, y), None);
    settle(&mut h);
    assert!(h.query_by_label("Ada Lovelace").is_none(), "a whole author has no hover");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_header_filtered() {
    let (mut st, dir) = with_two_authors(ThemeChoice::Dark, "truncate-header-filtered");
    searching(&mut st, "page");
    send(&mut st, SearchMsg::ToggleFilter);
    let mut h = harness_sized(st, true, vec2(1400.0, 360.0));
    h.run();
    h.check_snapshot("graph_header_filtered");
    drop(h);
    remove_fixture(&dir);
}

const LONG_SUMMARY: &str = "Read the settings file once at start and share it between the preferences window and the command line";
const LONG_BODY: &str = "The loader used to run for every window, so a hand-edited file could be read twice with different results.";

#[test]
fn graph_message_cut() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "truncate-message");
    std::fs::write(dir.join("settings.txt"), "settings\n").unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "settings.txt"], d);
    git(&dir, &["commit", "-q", "-m", LONG_SUMMARY, "-m", LONG_BODY], d);
    reload(&mut st, &dir, &LoadOptions::all(500));
    st.settings.columns.description = DescriptionMode::Always;
    let mut h = harness_sized(st, true, vec2(1400.0, 700.0));
    h.run();
    h.check_snapshot("graph_message_cut");
    // Near the message column's right end, where the cut text ends in "…".
    let x = grip(&h, "Resize commit column").x - 30.0;
    let y = center(&h, LONG_SUMMARY).y;
    pointer(&mut h, pos2(x, y), None);
    settle(&mut h);
    h.check_snapshot("graph_message_cut_tip");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn sidebar_issue_long_title() {
    let title = "Opening a repository from a network share hangs the window for a minute while the status of every \
                 submodule is read";
    let issue = Issue { title: title.into(), state: "In progress".into(), ..crash() };
    let (mut st, dir) = with_issue(ThemeChoice::Dark, "truncate-issue-row", github(), issue, None);
    st.settings.panel_sizes.sidebar = Some(240.0);
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("sidebar_issue_long_title");
    h.get_by_label(&format!("#12  {title}")).hover();
    settle(&mut h);
    h.check_snapshot("sidebar_issue_long_title_tip");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn status_update_pill_narrow() {
    let mut st = status_bar(UpdateStatus::Available(Box::new(the_offer())));
    st.settings.language = "de".into();
    let text = Lang::De.trf("updates.chip.available", &[("version", &"0.3.0")]);
    // Narrow enough that the German chip is cut: its full text then leads the tooltip.
    let mut h = harness_sized(st, true, vec2(150.0, 400.0));
    h.run();
    h.check_snapshot("status_update_pill_narrow");
    h.get_by_label(&text).hover();
    settle(&mut h);
    h.check_snapshot("status_update_pill_narrow_tip");
    h.get_by_label(&text).click();
    h.run();
    assert!(sent(&h, &UpdateMsg::ShowNotes));
}

#[test]
fn repo_sync_long_names() {
    let long = "a-repository-whose-folder-name-runs-far-past-the-name-column";
    assert_eq!(long.len(), 60);
    let rows: Vec<SyncRow> = vec![row("api-server", RowState::Done(Outcome::UpToDate)), row(long, RowState::Done(Outcome::UpToDate))];
    let mut h = harness(dialog(ThemeChoice::Dark, SyncKind::Fetch, rows), true);
    h.run();
    h.check_snapshot("repo_sync_long_names");
    h.get_by_label(long).hover();
    settle(&mut h);
    h.check_snapshot("repo_sync_long_names_tip");
}

#[test]
fn remote_hint_wraps() {
    let (mut st, dir) = with_remotes(ThemeChoice::Dark, "truncate-remote-hint");
    st.settings.panel_sizes.sidebar = Some(220.0);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("No branches yet. Fetch this remote to load them.").is_some());
    h.check_snapshot("remote_hint_wraps");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn history_file_lfs_trust_wraps() {
    let (mut st, dir) = with_file_view("truncate-history-lfs", ThemeChoice::Dark);
    let pointer = [
        "version https://git-lfs.github.com/spec/v1",
        "oid sha256:4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393",
        "size 18204",
    ];
    set_text(&mut st, pointer.map(String::from).to_vec(), LfsState::Pointer);
    let mut h = harness_sized(st, true, vec2(1400.0, 640.0));
    h.run();
    assert!(h.query_by_label(crate::history_file::LFS_UNTRUSTED).is_some());
    h.check_snapshot("history_file_lfs_trust_wraps");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn search_hint_cut() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "truncate-search-hint");
    searching(&mut st, "");
    let mut h = harness_sized(st, true, vec2(1400.0, 500.0));
    h.run();
    h.check_snapshot("search_hint_cut");
    drop(h);
    remove_fixture(&dir);
}

/// Five local branches and four worktrees whose names share a long start (`worktree-agent-a…`,
/// `agent-a…`), as agent tools make them.
const AGENT_IDS: [&str; 5] =
    ["a3f9c1d2-refresh-rows", "a5d02b8e-tab-titles", "a71e4f0c-picker-cut", "a9c6b3d1-palette", "ab48e2f7-push-menu"];

/// The fixture with `AGENT_IDS` as local branches and the first four as linked worktrees, each
/// on its branch, in a 260 px left panel.
fn shared_prefix(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_worktrees(theme, name);
    st.settings.panel_sizes.sidebar = Some(260.0);
    let snap = st.snap.as_mut().unwrap();
    let base = snap.locals.iter().find(|b| !b.is_head).or(snap.locals.first()).cloned().expect("a branch");
    let linked = snap.worktrees[1].clone();
    snap.worktrees.truncate(1);
    for (i, id) in AGENT_IDS.iter().enumerate() {
        let branch = format!("worktree-agent-{id}");
        snap.locals.push(BranchInfo { name: branch.clone(), is_head: false, upstream: None, ahead: 0, behind: 0, ..base.clone() });
        if i < 4 {
            let path = p(&format!("C:/work/app/.claude/worktrees/agent-{id}"));
            snap.worktrees.push(WorktreeInfo { name: Some(format!("agent-{id}")), path, branch: Some(branch), ..linked.clone() });
        }
    }
    (st, dir)
}

/// D16: in a 260 px left panel every one of those names is cut in its middle, so each row can
/// be told apart without hovering; each is still named in full (pixel goldens, dark and light).
#[test]
fn sidebar_shared_prefix() {
    for (theme, dark, golden) in
        [(ThemeChoice::Dark, true, "sidebar_shared_prefix_dark"), (ThemeChoice::Light, false, "sidebar_shared_prefix_light")]
    {
        let (st, dir) = shared_prefix(theme, golden);
        let mut h = harness(st, dark);
        h.run();
        for id in AGENT_IDS {
            assert!(h.query_by_label(&format!("worktree-agent-{id}")).is_some(), "{id}'s branch row is named in full");
        }
        for id in &AGENT_IDS[..4] {
            assert!(h.query_by_label(&format!("agent-{id}")).is_some(), "{id}'s worktree row is named in full");
        }
        h.check_snapshot(golden);
        drop(h);
        remove_fixture(&dir);
    }
}
