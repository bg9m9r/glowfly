//! The command palette over the main window: the full list, a filtered list with matched
//! characters highlighted, no matches, and the keys and clicks it turns into messages. Shares
//! the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::palette::{Palette, PaletteCommand};
use eframe::egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::ThemeChoice;

fn pushed(h: &Harness<'_, (AppState, Vec<Msg>)>, want: impl Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

#[test]
fn palette_dark() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette");
    st.palette = Some(Palette::default());
    render("palette", st, true);
    remove_fixture(&dir);
}

#[test]
fn palette_filtered_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "palette-light");
    st.palette = Some(Palette { query: "ch".into(), selected: 1, scroll_to_selected: false });
    render("palette_filtered_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn palette_no_matches() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-none");
    st.palette = Some(Palette { query: "zzzz".into(), ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("No matching commands").is_some());
    // Enter with nothing listed runs nothing.
    h.press_key(Key::Enter);
    h.run();
    assert!(!pushed(&h, |m| matches!(m, Msg::RunPaletteCommand(_))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_keys_and_clicks() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-keys");
    st.palette = Some(Palette::default());
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Checkout search").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::RunPaletteCommand(PaletteCommand::Checkout(b)) if b == "search")));

    h.state_mut().1.clear();
    h.press_key(Key::ArrowDown);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::PaletteMove(1))));
    // The graph under the palette doesn't move its selection.
    assert!(!pushed(&h, |m| matches!(m, Msg::SelectCommit(_) | Msg::SelectWip)));

    h.state_mut().1.clear();
    h.press_key(Key::ArrowUp);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::PaletteMove(-1))));

    // The fixture has no remote, so the first row is Push.
    h.state_mut().1.clear();
    h.press_key(Key::Enter);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::RunPaletteCommand(PaletteCommand::Push))));

    h.state_mut().1.clear();
    h.press_key(Key::Escape);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::ClosePalette)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_ignores_keys_under_a_credential_prompt() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-askpass");
    st.palette = Some(Palette::default());
    let prompt = trellis_core::askpass::Prompt::new("Password for 'https://example.com': ", None);
    st.askpass.queue.push_back(crate::state::askpass::PendingPrompt { id: 1, prompt, retry: false, rejected_before: false });
    let mut h = harness(st, true);
    h.run();
    h.press_key(Key::Enter);
    h.press_key(Key::ArrowDown);
    h.press_key(Key::Escape);
    h.run();
    assert!(!pushed(&h, |m| matches!(m, Msg::RunPaletteCommand(_) | Msg::PaletteMove(_) | Msg::ClosePalette)));
    drop(h);
    remove_fixture(&dir);
}

/// Ctrl+P and Ctrl+Shift+P toggle the palette; nothing happens under a dialog.
#[test]
fn shortcut_toggles() {
    let st = AppState::new(super::snapshot_tests::settings(ThemeChoice::Dark), Vec::new(), std::time::Instant::now());
    let mut h = Harness::builder()
        .build_state(|ctx, (st, msgs): &mut (AppState, Vec<Msg>)| super::palette::shortcut(ctx, st, msgs), (st, Vec::new()));
    h.press_key_modifiers(Modifiers::COMMAND, Key::P);
    h.run();
    assert_eq!(h.state().1.iter().filter(|m| matches!(m, Msg::TogglePalette)).count(), 1);
    h.state_mut().1.clear();
    h.press_key_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::P);
    h.run();
    assert_eq!(h.state().1.iter().filter(|m| matches!(m, Msg::TogglePalette)).count(), 1);
    h.state_mut().1.clear();
    h.press_key(Key::P);
    h.run();
    assert!(h.state().1.is_empty(), "P alone types");

    h.state_mut().0.prefs = Some(crate::state::PrefsState::new(crate::state::PrefPage::General));
    h.press_key_modifiers(Modifiers::COMMAND, Key::P);
    h.run();
    assert!(h.state().1.is_empty(), "not under Preferences");
}

/// With the commit search bar open and empty, typed text still lands in the palette's field
/// rather than being lost to the two fields taking focus from each other.
#[test]
fn palette_takes_typing_over_an_empty_search_bar() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-search-bar");
    st.view.show_search = true;
    st.palette = Some(Palette::default());
    let mut h = harness(st, true);
    h.run();
    h.run();
    h.input_mut().events.push(eframe::egui::Event::Text("push".into()));
    h.run();
    assert_eq!(h.state().0.palette.as_ref().map(|p| p.query.as_str()), Some("push"));
    assert_eq!(h.state().0.view.search, "");
    drop(h);
    remove_fixture(&dir);
}
