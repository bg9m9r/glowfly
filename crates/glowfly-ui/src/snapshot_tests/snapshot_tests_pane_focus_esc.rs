//! The graph's keys beside other panes' own: Ctrl+C with text selected in the details panel
//! copies that text, not the commit's ID; Esc on a focused control in Preferences or during the
//! tour keeps their own meaning (the save bar's Revert, closing Preferences, closing the tour)
//! rather than only giving the graph the focus back; and the file list's pane key goes to the
//! first list that has rows (Staged Files when nothing is unstaged).

use super::pane_focus::graph_id;
use super::snapshot_tests::{git, keys_harness, remove_fixture, with_repo};
use super::snapshot_tests_details_commit::with_commit;
use super::snapshot_tests_wip_keys::H;
use crate::keymap::KeyAction;
use crate::msg::{Msg, PrefsMsg, SelectionMsg};
use crate::onboarding::OnboardingMsg;
use crate::state::onboarding_tour::TourStep;
use crate::state::prefs::GitConfigValues;
use crate::state::{AppState, PrefPage, PrefsState, Selection};
use eframe::egui::{Event, Key, Modifiers, OutputCommand, PointerButton, Pos2, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

fn focused_name(h: &H) -> Option<String> {
    let n = h.query_by(|n| n.is_focused())?;
    Some(n.label().or_else(|| n.value()).unwrap_or_default())
}

/// Presses Tab until the widget named `name` has the keyboard focus.
#[track_caller]
fn tab_to(h: &mut H, name: &str) {
    for _ in 0..150 {
        if focused_name(h).as_deref() == Some(name) {
            return;
        }
        h.press_key(Key::Tab);
        h.run_ok();
    }
    panic!("Tab never reached {name:?} (last: {:?})", focused_name(h));
}

/// The messages one press of `key` sends.
fn press(h: &mut H, mods: Modifiers, key: Key) -> Vec<Msg> {
    h.state_mut().1.clear();
    h.press_key_modifiers(mods, key);
    h.run_ok();
    std::mem::take(&mut h.state_mut().1)
}

fn sent(msgs: &[Msg], f: impl Fn(&Msg) -> bool) -> bool {
    msgs.iter().any(f)
}

fn button(h: &mut H, pos: Pos2, pressed: bool) {
    h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
}

#[test]
fn ctrl_c_on_selected_details_text_copies_the_text() {
    let (st, dir, _) = with_commit(ThemeChoice::Dark, "pane-keys-copy-title", "Tidy the app");
    let mut h = keys_harness(st, true);
    h.run();
    // The details panel's title (the graph's row shows the summary too, further left).
    let b = h
        .query_all_by_label("Tidy the app")
        .filter_map(|n| n.bounding_box())
        .max_by(|a, b| a.x0.total_cmp(&b.x0))
        .expect("the details title");
    let y = ((b.y0 + b.y1) / 2.0) as f32;
    let (from, to) = (pos2(b.x0 as f32 + 1.0, y), pos2(b.x1 as f32 - 1.0, y));
    h.input_mut().events.push(Event::PointerMoved(from));
    button(&mut h, from, true);
    h.run();
    for t in [0.5, 1.0] {
        h.input_mut().events.push(Event::PointerMoved(from + (to - from) * t));
        h.run();
    }
    button(&mut h, to, false);
    h.run();
    // Ctrl+C: the window sends it as egui's Copy event.
    h.state_mut().1.clear();
    h.input_mut().events.push(Event::Copy);
    h.step();
    let copied: Vec<&str> = h
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|c| if let OutputCommand::CopyText(t) = c { Some(t.as_str()) } else { None })
        .collect();
    assert_eq!(copied, ["Tidy the app"], "the selected text was copied");
    let msgs = std::mem::take(&mut h.state_mut().1);
    assert!(!sent(&msgs, |m| matches!(m, Msg::Selection(SelectionMsg::CopyIds))), "not the commit's ID");
    drop(h);
    remove_fixture(&dir);
}

/// A repository with Preferences open on the Identity page, its name edited.
fn prefs_open(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir, _) = with_commit(ThemeChoice::Dark, name, "Tidy the app");
    let mut p = PrefsState::new(PrefPage::RepoIdentity);
    let git = GitConfigValues { repo_name: "Ada".into(), ..Default::default() };
    p.git_saved = Some(git.clone());
    p.git = Some(GitConfigValues { repo_name: "Grace".into(), ..git });
    st.prefs = Some(p);
    (st, dir)
}

#[test]
fn esc_on_the_save_bars_revert_reverts_the_page() {
    let (st, dir) = prefs_open("pane-keys-prefs-revert");
    assert!(st.snap.is_some(), "a repository is open behind Preferences");
    let mut h = keys_harness(st, true);
    h.run();
    tab_to(&mut h, "Revert");
    let msgs = press(&mut h, Modifiers::NONE, Key::Escape);
    assert!(sent(&msgs, |m| matches!(m, Msg::Prefs(PrefsMsg::RevertPage))), "{} messages, no RevertPage", msgs.len());
    assert_ne!(h.ctx.memory(|m| m.focused()), Some(graph_id()));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_on_a_focused_preferences_control_closes_preferences_at_once() {
    let (st, dir) = prefs_open("pane-keys-prefs-close");
    let mut h = keys_harness(st, true);
    h.run();
    h.press_key(Key::Tab);
    h.run_ok();
    let on = focused_name(&h);
    assert!(on.is_some() && on.as_deref() != Some("Revert") && on.as_deref() != Some("Save"), "a control: {on:?}");
    let msgs = press(&mut h, Modifiers::NONE, Key::Escape);
    assert!(sent(&msgs, |m| matches!(m, Msg::Prefs(PrefsMsg::ClosePrefs))), "the first Esc ({} messages)", msgs.len());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_on_a_focused_control_during_the_tour_closes_the_tour() {
    let (mut st, dir, _) = with_commit(ThemeChoice::Dark, "pane-keys-tour", "Tidy the app");
    st.onboarding.tour = Some(TourStep::Graph);
    let mut h = keys_harness(st, true);
    h.run();
    h.press_key(Key::Tab);
    h.run_ok();
    assert!(focused_name(&h).is_some(), "Tab focused a control");
    let msgs = press(&mut h, Modifiers::NONE, Key::Escape);
    assert!(sent(&msgs, |m| matches!(m, Msg::Onboarding(OnboardingMsg::CloseTour))), "the first Esc ({} messages)", msgs.len());
    drop(h);
    remove_fixture(&dir);
}

/// The WIP row of a repository with every change staged, or committed when not `staged`, after
/// the file list's pane key: which list has the focus, and its name.
fn file_list_key(name: &str, staged: bool) -> (Option<eframe::egui::Id>, Option<String>) {
    const D: &str = "2026-01-12T10:00:00+00:00";
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    std::fs::write(dir.join("new.txt"), "new\n").unwrap();
    git(&dir, &["add", "-A"], D);
    if !staged {
        git(&dir, &["commit", "-q", "-m", "Everything"], D);
    }
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    st.session.selection = Selection::Wip;
    let mut h = keys_harness(st, true);
    h.run();
    let pane = if cfg!(target_os = "macos") { Modifiers::CTRL } else { Modifiers::ALT };
    for m in press(&mut h, pane, Key::Num3) {
        if let Msg::Shortcut(a @ KeyAction::FocusPane(_)) = m {
            crate::update::update(&mut h.state_mut().0, Msg::Shortcut(a), Instant::now());
        }
    }
    h.run_ok();
    let got = (h.ctx.memory(|m| m.focused()), focused_name(&h));
    drop(h);
    remove_fixture(&dir);
    got
}

#[test]
fn the_file_list_pane_key_goes_to_the_first_list_with_rows() {
    let (id, name) = file_list_key("pane-keys-first-rows", true);
    assert_eq!(id, Some(crate::ui::wip_keys::list_id(true)), "Staged Files, past the empty Unstaged: {name:?}");
    assert_eq!(name.as_deref(), Some("Staged files (2)"));
    // With every list empty, the first: Unstaged Files.
    let (id, name) = file_list_key("pane-keys-no-rows", false);
    assert_eq!(id, Some(crate::ui::wip_keys::list_id(false)), "{name:?}");
}
