//! The pane keys and the graph's keyboard focus, drawn: each pane key lands the focus in its
//! pane (the left panel, the graph or the central view, the details panel's first file list, the
//! diff, the commit message) and F6 goes round them; the graph keeps its keys after a click on a
//! button and while it has the focus; Esc on a focused list gives the graph the focus back; the
//! graph's Page, Home and End, H, [ and ], Enter and Ctrl+C; and the graph's selection drawn
//! without the focus (`graph_focus_inactive_dark`).

use super::pane_focus::{focused_pane, graph_id};
use super::snapshot_tests::{keys_harness, remove_fixture};
use super::snapshot_tests_line_counts::with_wip;
use super::snapshot_tests_wip_keys::H;
use crate::keymap::KeyAction;
use crate::msg::{Msg, RepoMsg, SelectionMsg};
use crate::state::graph_rows::{DisplayRow, GraphRows};
use crate::state::layout::Pane;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{self, DiffTarget, RefKind};
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

/// The pane keys' modifier: Alt, Control on macOS.
fn pane_mods() -> Modifiers {
    if cfg!(target_os = "macos") { Modifiers::CTRL } else { Modifiers::ALT }
}

/// Presses `key` with `mods`, then lets `update` handle the pane requests it sent (the harness
/// runs no update) and draws until the focus settles. Returns every message the press sent.
fn press(h: &mut H, mods: Modifiers, key: Key) -> Vec<Msg> {
    h.state_mut().1.clear();
    h.press_key_modifiers(mods, key);
    h.run_ok();
    let sent = std::mem::take(&mut h.state_mut().1);
    for m in &sent {
        if let Msg::Shortcut(a @ (KeyAction::FocusPane(_) | KeyAction::OpenDetails)) = m {
            crate::update::update(&mut h.state_mut().0, Msg::Shortcut(*a), Instant::now());
        }
    }
    h.run_ok();
    h.state_mut().1.clear();
    sent
}

fn pane_key(h: &mut H, n: u8) {
    let key = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5][usize::from(n) - 1];
    press(h, pane_mods(), key);
}

fn focused(h: &H) -> Option<eframe::egui::Id> {
    h.ctx.memory(|m| m.focused())
}

#[test]
fn each_pane_key_lands_the_focus_in_its_pane() {
    let (st, dir) = with_wip(ThemeChoice::Dark, "pane-keys-land");
    let mut h = keys_harness(st, true);
    h.run();
    pane_key(&mut h, 1);
    assert_eq!(focused_pane(&h.ctx), Some(Pane::Sidebar));
    pane_key(&mut h, 2);
    assert_eq!(focused(&h), Some(graph_id()), "the graph is the central view");
    assert_eq!(focused_pane(&h.ctx), Some(Pane::Central));
    pane_key(&mut h, 3);
    assert_eq!(focused(&h), Some(crate::ui::wip_keys::list_id(false)), "the details panel's first list: Unstaged Files");
    assert_eq!(focused_pane(&h.ctx), Some(Pane::Files));
    pane_key(&mut h, 5);
    assert!(crate::ui::shortcuts::text_field_focused(&h.ctx), "the commit summary");
    assert_eq!(focused_pane(&h.ctx), Some(Pane::Message));
    pane_key(&mut h, 4);
    assert_eq!(focused_pane(&h.ctx), Some(Pane::Message), "no diff is open: the focus stays");

    // F6 goes on round the panes, Shift+F6 back.
    let mut seen = Vec::new();
    for _ in 0..4 {
        press(&mut h, Modifiers::NONE, Key::F6);
        seen.push(focused_pane(&h.ctx));
    }
    assert_eq!(seen, [Some(Pane::Sidebar), Some(Pane::Central), Some(Pane::Files), Some(Pane::Message)]);
    press(&mut h, Modifiers::SHIFT, Key::F6);
    assert_eq!(focused_pane(&h.ctx), Some(Pane::Files));
    drop(h);

    // With a diff open, the diff is the central view: both keys land there.
    let (mut st, dir2) = with_wip(ThemeChoice::Dark, "pane-keys-diff");
    let target = DiffTarget::Unstaged("app.txt".into());
    st.session.diff.view = Some((target.clone(), repo::load_diff(&dir2, &target).unwrap().into()));
    let mut h = keys_harness(st, true);
    h.run();
    pane_key(&mut h, 4);
    assert_eq!(focused_pane(&h.ctx), Some(Pane::Central));
    assert_ne!(focused(&h), Some(graph_id()));
    pane_key(&mut h, 1);
    pane_key(&mut h, 2);
    assert_eq!(focused_pane(&h.ctx), Some(Pane::Central));
    drop(h);
    remove_fixture(&dir);
    remove_fixture(&dir2);
}

/// The selection message of the frames' graph keys.
fn selects(sent: &[Msg]) -> Vec<String> {
    sent.iter()
        .filter_map(|m| match m {
            Msg::Repo(RepoMsg::SelectCommit(o)) => Some(o.to_string()[..7].to_owned()),
            Msg::Repo(RepoMsg::SelectWip) => Some("wip".to_owned()),
            Msg::Selection(SelectionMsg::SelectCommitRange(o)) => Some(format!("range {}", &o.to_string()[..7])),
            _ => None,
        })
        .collect()
}

#[test]
fn the_graph_keeps_its_keys_after_a_click_on_a_button() {
    let (st, dir) = with_wip(ThemeChoice::Dark, "pane-keys-stage-all");
    let first = st.snap.as_ref().unwrap().commits[0].oid.to_string()[..7].to_owned();
    let mut h = keys_harness(st, true);
    h.run();
    let b = h.get_by_role_and_label(Role::Button, "Stage all").bounding_box().expect("Stage all has bounds");
    let at = pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
    super::snapshot_tests_select::click(&mut h, at, PointerButton::Primary);
    h.run_ok();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Stage(_))), "Stage all was clicked");
    assert_eq!(selects(&press(&mut h, Modifiers::NONE, Key::ArrowDown)), [first.as_str()], "Down left the WIP row");

    // With the graph focused (a pane key), its arrows stay its own, frame after frame.
    pane_key(&mut h, 2);
    assert_eq!(focused(&h), Some(graph_id()));
    for _ in 0..2 {
        assert_eq!(selects(&press(&mut h, Modifiers::NONE, Key::ArrowDown)), [first.as_str()]);
        assert_eq!(focused(&h), Some(graph_id()), "the arrow kept the focus on the graph");
    }
    // A focused list takes them instead.
    pane_key(&mut h, 3);
    assert!(selects(&press(&mut h, Modifiers::NONE, Key::ArrowDown)).is_empty(), "the list's Down");
    // Esc on it gives the graph the focus back, and the graph its keys.
    press(&mut h, Modifiers::NONE, Key::Escape);
    assert_eq!(focused(&h), Some(graph_id()), "Esc returned the focus to the graph");
    assert_eq!(selects(&press(&mut h, Modifiers::NONE, Key::ArrowDown)), [first]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_keys() {
    let (st, dir) = with_wip(ThemeChoice::Dark, "pane-keys-graph");
    let snap = st.snap.as_ref().unwrap();
    let short = |o: git2::Oid| o.to_string()[..7].to_owned();
    let last = short(snap.commits.last().unwrap().oid);
    let head = short(snap.head_oid.unwrap());
    let rows = GraphRows::new(snap, &st.session.search, false);
    let has_ref = |i: usize| {
        snap.refs
            .get(&snap.commits[i].oid)
            .is_some_and(|l| l.iter().any(|r| matches!(r.kind, RefKind::Local | RefKind::Remote | RefKind::Tag)))
    };
    let with_refs: Vec<String> = (0..rows.len())
        .filter_map(|r| match rows.at(r) {
            Some(DisplayRow::Commit(i)) if has_ref(i) => Some(short(snap.commits[i].oid)),
            _ => None,
        })
        .collect();
    assert!(with_refs.len() >= 2, "the fixture has branches and tags: {with_refs:?}");
    let mut h = keys_harness(st, true);
    h.run();
    // From the WIP row.
    assert_eq!(selects(&press(&mut h, Modifiers::NONE, Key::End)), [last.as_str()]);
    assert_eq!(selects(&press(&mut h, Modifiers::NONE, Key::Home)), ["wip"]);
    let page = selects(&press(&mut h, Modifiers::NONE, Key::PageDown));
    assert_eq!(page.len(), 1, "{page:?}");
    assert_eq!(selects(&press(&mut h, Modifiers::NONE, Key::H)), [head]);
    assert_eq!(selects(&press(&mut h, Modifiers::NONE, Key::CloseBracket)), [with_refs[0].clone()]);
    assert!(selects(&press(&mut h, Modifiers::NONE, Key::OpenBracket)).is_empty(), "no commit above the WIP row");
    let sent = press(&mut h, Modifiers::NONE, Key::Enter);
    assert!(sent.iter().any(|m| matches!(m, Msg::Shortcut(KeyAction::OpenDetails))), "Enter asked for the details panel");
    assert_eq!(focused_pane(&h.ctx), Some(Pane::Files), "Enter went to the details panel's file list");

    // Ctrl+C, as a key or as egui's Copy event, copies the selection's IDs while the graph shows.
    let copies = |sent: &[Msg]| sent.iter().filter(|m| matches!(m, Msg::Selection(SelectionMsg::CopyIds))).count();
    pane_key(&mut h, 2);
    assert_eq!(copies(&press(&mut h, Modifiers::COMMAND, Key::C)), 1);
    h.input_mut().events.push(Event::Copy);
    h.run_ok();
    assert_eq!(copies(&std::mem::take(&mut h.state_mut().1)), 1, "the Copy event");
    drop(h);
    remove_fixture(&dir);
}

/// A commit selected while its file list has the focus: the graph's row is drawn with the
/// neutral inactive selection.
#[test]
fn graph_focus_inactive() {
    let (st, dir, _) = super::snapshot_tests_details_commit::with_commit(ThemeChoice::Dark, "pane-keys-inactive", "Tidy the app");
    let mut h = keys_harness(st, true);
    h.run();
    pane_key(&mut h, 3);
    assert_eq!(focused_pane(&h.ctx), Some(Pane::Files));
    assert!(!super::pane_focus::graph_active(&h.ctx));
    h.check_snapshot("graph_focus_inactive_dark");
    drop(h);
    remove_fixture(&dir);
}
