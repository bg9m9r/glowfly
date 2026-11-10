//! Several files picked in the Unstaged or Staged Files list: Shift+click and Ctrl/Cmd+click
//! send their pick messages (a plain click opens the file), a picked row's Stage button and
//! menu act on every pick and the menu names the count (its discard, delete and stash off while
//! a pick has a hazard), and a row that isn't picked keeps its own menu. Shares the fixture repositories
//! with `snapshot_tests_wip_keys` and `snapshot_tests_line_counts`.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_line_counts::with_wip;
use super::snapshot_tests_wip_keys::{H, picks};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{self, DiffTarget, Hazard};
use glowfly_core::settings::ThemeChoice;
use glowfly_core::stash_ops::StashScope;
use glowfly_model::msg::{Msg, RepoMsg, StageMsg, StashMsg, WorkFileMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::wip_lists::{PickHow, WipPicks};
use std::path::PathBuf;

/// The working tree with `open`'s diff open and `picks` picked. Unstaged Files: README.md,
/// app.txt, login.txt, notes/todo.md; Staged Files: app.txt, search.txt.
fn with_picks(name: &str, open: DiffTarget, picks: WipPicks) -> (AppState, PathBuf) {
    with_picks_in(ThemeChoice::Dark, name, open, picks)
}

fn with_picks_in(theme: ThemeChoice, name: &str, open: DiffTarget, picks: WipPicks) -> (AppState, PathBuf) {
    let (mut st, dir) = with_wip(theme, name);
    let diff = repo::load_diff(&dir, &open).unwrap();
    st.session.diff.view = Some((open, diff.into()));
    st.cues.wip_picks = Some(picks);
    (st, dir)
}

/// README.md, app.txt and notes/todo.md picked in Unstaged Files (login.txt left out), app.txt open.
fn three_unstaged(name: &str) -> (AppState, PathBuf) {
    with_picks(name, DiffTarget::Unstaged("app.txt".into()), picks("app.txt", &["README.md", "app.txt", "notes/todo.md"]))
}

/// The first row named `name` in the list (Unstaged first): its middle, clear of the Stage or
/// Unstage button at its end.
pub(super) fn row(h: &H, name: &str) -> Pos2 {
    let b = h.get_all_by_role_and_label(Role::Button, name).find_map(|n| n.bounding_box()).expect("the row has bounds");
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

/// The Staged Files row named `name` (after its Unstaged twin, if any): its middle.
fn staged_row(h: &H, name: &str) -> Pos2 {
    let b = h.get_all_by_role_and_label(Role::Button, name).filter_map(|n| n.bounding_box()).next_back().expect("the row has bounds");
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

/// The messages a `button` click at `at` with `modifiers` held sends.
pub(super) fn click_with(h: &mut H, at: Pos2, button: PointerButton, modifiers: Modifiers) -> Vec<Msg> {
    h.state_mut().1.clear();
    h.input_mut().modifiers = modifiers;
    for pressed in [true, false] {
        let events = &mut h.input_mut().events;
        events.push(Event::PointerMoved(at));
        events.push(Event::PointerButton { pos: at, button, pressed, modifiers });
        h.step();
    }
    h.input_mut().modifiers = Modifiers::NONE;
    h.run_ok();
    std::mem::take(&mut h.state_mut().1)
}

pub(super) fn right_click(h: &mut H, at: Pos2) {
    click_with(h, at, PointerButton::Secondary, Modifiers::NONE);
}

pub(super) fn labels(h: &H) -> Vec<String> {
    h.get_all_by_role(Role::Button).filter_map(|n| n.label()).collect()
}

#[test]
fn wip_picks_menu_unstaged() {
    let (st, dir) = three_unstaged("wip-picks-menu-un");
    let mut h = harness(st, true);
    h.run();
    let at = row(&h, "README.md");
    right_click(&mut h, at);
    let all = labels(&h);
    let stage = all.iter().position(|l| l == "Stage 3 files").expect("the selection menu");
    let stash = all.iter().position(|l| l == "Stash 3 selected files…").expect("its stash");
    let discard = all.iter().position(|l| l == "Discard changes in 3 files…").expect("its discard");
    let delete = all.iter().position(|l| l == "Delete 3 files…").expect("its delete");
    assert!(stage < stash && stash < discard && discard < delete, "{all:?}");
    assert!(h.query_by_label("Copy path").is_none(), "not the single-file menu");
    h.check_snapshot("wip_picks_menu_unstaged_dark");
    h.get_by_label("Discard changes in 3 files…").click();
    h.run_ok();
    let want = ["README.md", "app.txt", "notes/todo.md"].map(String::from).to_vec();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::WorkFile(WorkFileMsg::DiscardPaths(p)) if *p == want)), "in on-screen order");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn wip_picks_menu_staged() {
    let (st, dir) = with_picks(
        "wip-picks-menu-st",
        DiffTarget::Staged("app.txt".into()),
        WipPicks { staged: true, ..picks("app.txt", &["app.txt", "search.txt"]) },
    );
    let mut h = harness(st, true);
    h.run();
    let at = staged_row(&h, "search.txt");
    right_click(&mut h, at);
    assert!(h.query_by_label("Unstage 2 files").is_some(), "{:?}", labels(&h));
    h.check_snapshot("wip_picks_menu_staged_dark");
    h.get_by_label("Unstage 2 files").click();
    h.run_ok();
    let want = ["app.txt", "search.txt"].map(String::from).to_vec();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Stage(StageMsg::UnstagePaths(p)) if *p == want)));
    right_click(&mut h, at);
    h.get_by_label("Delete 2 files…").click();
    h.run_ok();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::WorkFile(WorkFileMsg::DeletePaths(p)) if *p == want)));
    right_click(&mut h, at);
    h.get_by_label("Stash 2 selected files…").click();
    h.run_ok();
    let stash = |m: &Msg| matches!(m, Msg::Stash(StashMsg::PushDialog { scope: StashScope::Files, paths }) if *paths == want);
    assert!(h.state().1.iter().any(stash));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn wip_picks_menu_blocked() {
    let open = DiffTarget::Unstaged("app.txt".into());
    let (mut st, dir) =
        with_picks_in(ThemeChoice::Light, "wip-picks-menu-blocked", open, picks("app.txt", &["README.md", "app.txt", "notes/todo.md"]));
    let readme = st.snap.as_mut().unwrap().unstaged.iter_mut().find(|f| f.path == "README.md").unwrap();
    readme.hazard = Some(Hazard::Twins(vec!["readme.md".into()]));
    let mut h = harness(st, true);
    h.run();
    let at = row(&h, "app.txt");
    right_click(&mut h, at);
    for item in ["Discard changes in 3 files…", "Delete 3 files…", "Stash 3 selected files…"] {
        assert!(h.get_by_label(item).is_disabled(), "a pick with a hazard turns {item} off");
    }
    h.check_snapshot("wip_picks_menu_blocked_light");
    h.get_by_label("Discard changes in 3 files…").click();
    h.get_by_label("Delete 3 files…").click();
    h.get_by_label("Stash 3 selected files…").click();
    h.run_ok();
    let acted = |m: &Msg| {
        matches!(m, Msg::WorkFile(WorkFileMsg::DiscardPaths(_) | WorkFileMsg::DeletePaths(_)) | Msg::Stash(StashMsg::PushDialog { .. }))
    };
    assert!(!h.state().1.iter().any(acted));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn shift_and_command_clicks_send_pick_messages() {
    let (st, dir) = with_picks("wip-picks-clicks", DiffTarget::Unstaged("app.txt".into()), picks("app.txt", &["app.txt"]));
    let mut h = harness(st, true);
    h.run();
    let at = row(&h, "login.txt");
    let sent = |msgs: &[Msg], want: PickHow| {
        msgs.iter()
            .any(|m| matches!(m, Msg::Repo(RepoMsg::PickWipFile { staged: false, path, how }) if path == "login.txt" && *how == want))
    };
    let msgs = click_with(&mut h, at, PointerButton::Primary, Modifiers::SHIFT);
    assert!(sent(&msgs, PickHow::Range));
    let msgs = click_with(&mut h, at, PointerButton::Primary, Modifiers::COMMAND);
    assert!(sent(&msgs, PickHow::Toggle));
    let msgs = click_with(&mut h, at, PointerButton::Primary, Modifiers::SHIFT | Modifiers::COMMAND);
    assert!(sent(&msgs, PickHow::Range), "Ctrl/Cmd+Shift ranges too");
    let msgs = click_with(&mut h, at, PointerButton::Primary, Modifiers::NONE);
    assert!(msgs.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged(p))) if p == "login.txt")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn stage_button_on_a_picked_row_sends_every_pick() {
    let (st, dir) = three_unstaged("wip-picks-button");
    let mut h = harness(st, true);
    h.run();
    // The open row shows its Stage button without a hover.
    let y = row(&h, "app.txt").y;
    let stage = h
        .get_all_by_role_and_label(Role::Button, "Stage")
        .filter_map(|n| n.bounding_box())
        .find(|b| (b.y0 as f32) <= y && (b.y1 as f32) >= y)
        .expect("the open row's Stage button");
    let centre = pos2(((stage.x0 + stage.x1) / 2.0) as f32, ((stage.y0 + stage.y1) / 2.0) as f32);
    let msgs = click_with(&mut h, centre, PointerButton::Primary, Modifiers::NONE);
    let want = ["README.md", "app.txt", "notes/todo.md"].map(String::from).to_vec();
    assert!(msgs.iter().any(|m| matches!(m, Msg::Stage(StageMsg::StagePaths(p)) if *p == want)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn unpicked_row_menu_is_the_single_file_menu() {
    let (st, dir) = three_unstaged("wip-picks-unpicked");
    let mut h = harness(st, true);
    h.run();
    let at = row(&h, "login.txt");
    right_click(&mut h, at);
    assert!(h.query_by_label("Copy path").is_some(), "the row's own menu");
    assert!(h.query_by_label("Stage 3 files").is_none());
    assert_eq!(h.state().0.cues.wip_picks.as_ref().map(|p| p.paths.len()), Some(3), "the picks stay");
    drop(h);
    remove_fixture(&dir);
}
