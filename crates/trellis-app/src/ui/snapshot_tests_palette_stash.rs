//! The command palette's stash steps over the main window: the apply sub-list, a filtered
//! delete sub-list, the stash message step, and the keys and clicks they turn into messages.
//! Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::palette::{InputKind, Mode, Palette, PaletteCommand, Pick, StashAction};
use eframe::egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use trellis_core::repo::StashInfo;
use trellis_core::settings::ThemeChoice;

fn pushed(h: &Harness<'_, (AppState, Vec<Msg>)>, want: impl Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

/// Three made-up stashes, on the first three commits of the fixture (distinct oids).
fn fixture_stashes(st: &AppState) -> Vec<StashInfo> {
    let snap = st.snap.as_ref().unwrap();
    let oid = |i: usize| snap.commits[i].oid;
    vec![
        StashInfo { index: 0, message: "On main: login form tweaks".into(), oid: oid(0) },
        StashInfo { index: 1, message: "WIP on main: 1a2b3c4 Add search".into(), oid: oid(1) },
        StashInfo { index: 2, message: "On login-page: half-done validation".into(), oid: oid(2) },
    ]
}

/// A palette in the `action` sub-list over the fixture stashes, with `query` typed.
fn stash_list(st: &AppState, action: StashAction, query: &str, selected: usize) -> Palette {
    let pk = Pick::stashes(action, &fixture_stashes(st));
    Palette { query: query.into(), selected, mode: Mode::Pick(pk), ..Default::default() }
}

fn message_step(query: &str) -> Palette {
    Palette { query: query.into(), mode: Mode::Input(InputKind::StashMessage), ..Default::default() }
}

fn queue_prompt(st: &mut AppState) {
    let prompt = trellis_core::askpass::Prompt::new("Password for 'https://example.com': ", None);
    st.askpass.queue.push_back(crate::state::askpass::PendingPrompt { id: 1, prompt, retry: false, rejected_before: false });
}

#[test]
fn palette_stashes() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-stashes");
    st.palette = Some(stash_list(&st, StashAction::Apply, "", 1));
    render("palette_stashes", st, true);
    remove_fixture(&dir);
}

#[test]
fn palette_stashes_filtered_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "palette-stashes-light");
    st.palette = Some(stash_list(&st, StashAction::Drop, "login", 0));
    render("palette_stashes_filtered_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn palette_stash_message() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-stash-message");
    st.palette = Some(message_step("Half-done login"));
    render("palette_stash_message", st, true);
    remove_fixture(&dir);
}

#[test]
fn palette_stash_keys() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-stash-keys");
    let stashes = fixture_stashes(&st);
    st.palette = Some(stash_list(&st, StashAction::Apply, "", 0));
    let mut h = harness(st, true);
    h.run();
    let applies = |want: Oid| move |m: &Msg| matches!(m, Msg::RunPaletteCommand(PaletteCommand::StashOp { action: StashAction::Apply, oid }) if *oid == want);
    h.press_key(Key::Enter);
    h.run();
    assert!(pushed(&h, applies(stashes[0].oid)));

    h.state_mut().1.clear();
    h.get_by_label("WIP on main: 1a2b3c4 Add search").click();
    h.run();
    assert!(pushed(&h, applies(stashes[1].oid)));
    assert!(!pushed(&h, applies(stashes[0].oid)));

    h.state_mut().1.clear();
    h.press_key(Key::Backspace);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::PaletteBack)));

    h.state_mut().1.clear();
    h.press_key(Key::Escape);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::PaletteBack)));
    assert!(!pushed(&h, |m| matches!(m, Msg::ClosePalette)), "Esc goes back rather than closing");

    // Backspace with text in the field edits the text.
    h.state_mut().0.palette.as_mut().unwrap().query = "x".into();
    h.run();
    h.state_mut().1.clear();
    h.press_key(Key::Backspace);
    h.run();
    assert!(!pushed(&h, |m| matches!(m, Msg::PaletteBack)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_stash_message_keys() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-stash-message-keys");
    st.palette = Some(message_step(""));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("untracked files included").is_some());
    h.press_key(Key::Enter);
    h.run();
    assert!(!pushed(&h, |m| matches!(m, Msg::RunPaletteCommand(_))), "a blank message runs nothing");
    h.press_key(Key::Backspace);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::PaletteBack)));

    h.state_mut().1.clear();
    h.state_mut().0.palette = Some(message_step("wip"));
    h.run();
    h.press_key(Key::Enter);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::RunPaletteCommand(PaletteCommand::StashWithMessage(t)) if t == "wip")));

    h.state_mut().1.clear();
    h.press_key(Key::Escape);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::PaletteBack)));
    assert!(!pushed(&h, |m| matches!(m, Msg::ClosePalette)));

    // Under a credential prompt the step takes no keys.
    h.state_mut().0.palette = Some(message_step(""));
    queue_prompt(&mut h.state_mut().0);
    h.run();
    h.state_mut().1.clear();
    h.press_key(Key::Enter);
    h.press_key(Key::Escape);
    h.press_key(Key::Backspace);
    h.run();
    assert!(!pushed(&h, |m| matches!(m, Msg::PaletteBack | Msg::ClosePalette | Msg::RunPaletteCommand(_))));
    drop(h);
    remove_fixture(&dir);
}
