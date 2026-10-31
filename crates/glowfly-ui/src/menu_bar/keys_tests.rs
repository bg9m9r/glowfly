//! The menu bar from the keyboard: Alt+letter opens a menu with its first item focused, the
//! arrows move within (wrapping) and between menus, Enter clicks, Esc only closes the menu, and the
//! mnemonics stay out of the way of bindings, the terminal, a hidden bar and macOS. Runs the
//! shortcut input before drawing, in the app's order.

use super::super::theme::{self, color};
use super::super::{UiCache, avatars::Avatars, draw, shortcuts};
use super::snapshot_tests::with_bar;
use crate::msg::{Msg, PrefsMsg, RebaseMsg, RepoMsg};
use crate::state::menu_keys::{TopMenu, mnemonics};
use crate::state::window_chrome::Chrome;
use crate::state::{AppState, PrefPage, PrefsState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{NOW, remove_fixture};
use eframe::egui::{self, Event, InputState, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{Node, Queryable};
use glowfly_core::repo::{DiffTarget, FileDiff};
use glowfly_core::settings::ThemeChoice;
use glowfly_core::spell::Speller;

pub(crate) type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The app's frame in the Garden dark palette at `size`: the shortcut input, then the window.
pub(crate) fn keys_harness(mut st: AppState, size: egui::Vec2) -> H {
    st.view.now = NOW;
    st.view.utc_offset = 0;
    let mut cache: Option<UiCache> = None;
    let mut h = crate::ui::snapshot_check::builder().with_size(size).build_state(
        move |ctx, (st, msgs): &mut (AppState, Vec<Msg>)| {
            color::set_palette(&theme::GARDEN_DARK);
            theme::apply_style(ctx, &theme::GARDEN_DARK);
            ctx.set_theme(theme::egui_theme(&theme::GARDEN_DARK));
            let cache = cache.get_or_insert_with(|| UiCache {
                avatars: Avatars::default(),
                speller: Speller::new(),
                diff_rows: Default::default(),
                history_text: Default::default(),
                history_preview: Default::default(),
                blame_rows: Default::default(),
                diff_images: Default::default(),
                markdown: Default::default(),
            });
            shortcuts::input(ctx, st, msgs);
            draw(ctx, st, cache, msgs);
        },
        (st, Vec::new()),
    );
    h.run_ok();
    h
}

/// Presses `key` with `modifiers` held, then lets go of them.
pub(crate) fn press(h: &mut H, modifiers: Modifiers, key: Key) {
    h.input_mut().modifiers = modifiers;
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
    h.run_ok();
    h.input_mut().modifiers = Modifiers::NONE;
    h.run_ok();
}

/// Presses `key` alone.
pub(crate) fn key(h: &mut H, key: Key) {
    press(h, Modifiers::NONE, key);
}

/// The widest node labelled `label` (a menu item is wider than a toolbar or tab bar button of
/// the same name).
pub(crate) fn widest<'a>(h: &'a H, label: &'a str) -> Node<'a> {
    let width = |n: &Node<'_>| n.bounding_box().map_or(0.0, |b| b.x1 - b.x0);
    h.query_all_by_label(label).max_by(|a, b| width(a).total_cmp(&width(b))).unwrap_or_else(|| panic!("no {label}"))
}

/// Whether a node labelled `label` has the keyboard focus.
pub(crate) fn focused(h: &H, label: &str) -> bool {
    h.query_all_by_label(label).any(|n| n.is_focused())
}

/// Whether the File menu is open (its Quit item is drawn).
fn file_open(h: &H) -> bool {
    h.query_by_label("Quit").is_some()
}

fn fixture(name: &str) -> (AppState, std::path::PathBuf) {
    with_bar(ThemeChoice::Dark, name)
}

const SIZE: egui::Vec2 = egui::vec2(1400.0, 820.0);

#[test]
fn alt_f_opens_file_with_its_first_item_focused() {
    let (st, dir) = fixture("menu-keys-open");
    let mut h = keys_harness(st, SIZE);
    assert!(!file_open(&h));
    press(&mut h, Modifiers::ALT, Key::F);
    assert!(file_open(&h), "Alt+F opens File");
    assert!(widest(&h, "New tab").is_focused(), "its first item has the focus");
    assert!(h.state().0.view.menu_keys.keyboard, "the letters stay underlined");
    // Its letter again keeps it open.
    press(&mut h, Modifiers::ALT, Key::F);
    assert!(file_open(&h));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn up_and_down_stay_in_the_menu_and_wrap() {
    let (st, dir) = fixture("menu-keys-down");
    let mut h = keys_harness(st, SIZE);
    press(&mut h, Modifiers::ALT, Key::F);
    key(&mut h, Key::ArrowDown);
    assert!(focused(&h, "Open repository…"), "Down moves to the next item");
    key(&mut h, Key::End);
    assert!(focused(&h, "Quit"), "End goes to the last item");
    let in_menu = |h: &H| {
        let f = h.ctx.memory(|m| m.focused()).expect("something focused");
        h.ctx.read_response(f).is_some_and(|r| r.layer_id.order == egui::Order::Foreground)
    };
    key(&mut h, Key::ArrowDown);
    assert!(widest(&h, "New tab").is_focused(), "Down on the last item wraps to the first");
    assert!(in_menu(&h), "no graph row took the focus");
    key(&mut h, Key::ArrowUp);
    assert!(focused(&h, "Quit"), "and Up on the first to the last");
    key(&mut h, Key::Home);
    assert!(widest(&h, "New tab").is_focused(), "Home goes to the first");
    assert!(file_open(&h));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn left_and_right_switch_menus() {
    let (st, dir) = fixture("menu-keys-step");
    let mut h = keys_harness(st, SIZE);
    press(&mut h, Modifiers::ALT, Key::F);
    key(&mut h, Key::ArrowRight);
    assert!(!file_open(&h), "File closed");
    assert!(h.query_by_label("Find commits…").is_some(), "Edit opened");
    assert!(focused(&h, "Undo"), "with its first item focused");
    key(&mut h, Key::ArrowLeft);
    key(&mut h, Key::ArrowLeft);
    assert!(h.query_by_label("Keyboard shortcuts").is_some(), "Left from File wraps to Help");
    key(&mut h, Key::ArrowRight);
    assert!(file_open(&h), "and Right from Help to File");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_arrows_work_in_a_menu_opened_with_the_mouse() {
    let (st, dir) = fixture("menu-keys-mouse");
    let mut h = keys_harness(st, SIZE);
    h.get_by_label("File").click();
    h.run_ok();
    assert!(file_open(&h));
    assert!(!h.state().0.view.menu_keys.keyboard);
    key(&mut h, Key::ArrowDown);
    assert!(widest(&h, "New tab").is_focused(), "Down with nothing focused focuses the first item");
    key(&mut h, Key::ArrowRight);
    assert!(h.query_by_label("Find commits…").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn enter_clicks_the_focused_item() {
    let (st, dir) = fixture("menu-keys-enter");
    let mut h = keys_harness(st, SIZE);
    press(&mut h, Modifiers::ALT, Key::F);
    for _ in 0..12 {
        if focused(&h, "Preferences…") {
            break;
        }
        key(&mut h, Key::ArrowDown);
    }
    assert!(focused(&h, "Preferences…"));
    h.state_mut().1.clear();
    key(&mut h, Key::Enter);
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_closes_the_menu_and_nothing_else() {
    let (mut st, dir) = fixture("menu-keys-esc");
    let diff = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.session.diff.view = Some((DiffTarget::Unstaged("README.md".into()), diff.into()));
    let mut h = keys_harness(st, SIZE);
    press(&mut h, Modifiers::ALT, Key::F);
    h.state_mut().1.clear();
    key(&mut h, Key::Escape);
    assert!(!file_open(&h), "Esc closed the menu");
    let closes = |m: &Msg| matches!(m, Msg::Repo(RepoMsg::CloseDiff) | Msg::Conflict(_) | Msg::Diff(_));
    assert!(!h.state().1.iter().any(closes), "the diff stays open");
    // With no menu open, Esc closes the diff as before.
    key(&mut h, Key::Escape);
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::CloseDiff))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn preferences_close_an_open_menu_so_esc_still_leaves_them() {
    let (st, dir) = fixture("menu-keys-prefs");
    let mut h = keys_harness(st, SIZE);
    press(&mut h, Modifiers::ALT, Key::F);
    assert!(file_open(&h));
    // Ctrl+, opens Preferences with the menu still open; the bar isn't drawn behind them.
    h.state_mut().0.prefs = Some(PrefsState::new(PrefPage::General));
    h.run_ok();
    assert!(!super::any_open(&h.ctx), "the bar's menu closed with Preferences shown");
    h.state_mut().1.clear();
    key(&mut h, Key::Escape);
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::ClosePrefs))), "Esc closes Preferences");
    h.state_mut().0.prefs = None;
    h.run_ok();
    assert!(!file_open(&h), "the menu doesn't come back with the window");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn other_alt_presses_open_nothing() {
    let (st, dir) = fixture("menu-keys-altgr");
    let mut h = keys_harness(st, SIZE);
    // AltGr reads as Ctrl+Alt on Windows.
    press(&mut h, Modifiers { alt: true, ctrl: true, command: true, ..Modifiers::NONE }, Key::F);
    assert!(!file_open(&h), "AltGr+F types a character");
    press(&mut h, Modifiers::ALT | Modifiers::SHIFT, Key::F);
    assert!(!file_open(&h), "Alt+Shift+F");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn bindings_keep_their_alt_keys() {
    let (mut st, dir) = fixture("menu-keys-binding");
    let head = st.snap.as_ref().unwrap().commits[0].oid;
    st.session.selection = Selection::Commit(head);
    let mut h = keys_harness(st, SIZE);
    h.state_mut().1.clear();
    press(&mut h, Modifiers::ALT, Key::S);
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Rebase(RebaseMsg::Edit(_)))), "Alt+S squashes");
    assert!(!file_open(&h) && h.query_by_label("Find commits…").is_none(), "no menu opened");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn mnemonics_wait_for_the_terminal_a_hidden_bar_and_macos() {
    type Change = fn(&mut AppState);
    let cases: [(&str, Change); 3] = [
        ("menu-keys-terminal", |st| st.terminal.focused = true),
        ("menu-keys-hidden", |st| st.settings.show_menu_bar = false),
        ("menu-keys-mac", |st| st.view.chrome = Chrome::MacOs),
    ];
    for (name, change) in cases {
        let (st, dir) = fixture(name);
        let mut h = keys_harness(st, SIZE);
        // After the first frame: a terminal that isn't drawn lets go of the keyboard.
        change(&mut h.state_mut().0);
        press(&mut h, Modifiers::ALT, Key::F);
        assert!(!file_open(&h), "{name}");
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn the_letter_typed_with_alt_is_taken_too() {
    let marks = mnemonics(["File", "Edit", "View", "Window", "Help"], &[]);
    let mut i = InputState::default();
    let alt = Modifiers::ALT;
    i.events = vec![
        Event::Key { key: Key::F, physical_key: None, pressed: true, repeat: false, modifiers: alt },
        Event::Text("f".into()),
        Event::Text("x".into()),
    ];
    assert_eq!(super::take(&mut i, &marks), Some(TopMenu::File));
    assert!(matches!(&i.events[..], [Event::Text(s)] if s == "x"), "{:?}", i.events);
    // A letter no menu has stays for the views.
    i.events = vec![Event::Key { key: Key::J, physical_key: None, pressed: true, repeat: false, modifiers: alt }];
    assert_eq!(super::take(&mut i, &marks), None);
    assert_eq!(i.events.len(), 1);
}

#[test]
fn a_focused_field_doesnt_get_the_letter() {
    let (mut st, dir) = fixture("menu-keys-typing");
    st.view.focus_message = true;
    let mut h = keys_harness(st, SIZE);
    assert!(h.ctx.wants_keyboard_input(), "the commit summary has the keyboard");
    h.input_mut().modifiers = Modifiers::ALT;
    h.input_mut().events.push(Event::Key { key: Key::F, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::ALT });
    h.input_mut().events.push(Event::Text("f".into()));
    h.run_ok();
    h.input_mut().modifiers = Modifiers::NONE;
    h.run_ok();
    assert!(file_open(&h));
    assert!(h.state().0.session.draft.summary.is_empty(), "{:?}", h.state().0.session.draft.summary);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_mnemonics() {
    let (st, dir) = fixture("menu-keys-marks");
    let mut h = keys_harness(st, SIZE);
    h.input_mut().modifiers = Modifiers::ALT;
    h.run_ok();
    h.check_snapshot("menu_bar_mnemonics");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_mnemonics_spanish() {
    let (mut st, dir) = fixture("menu-keys-marks-es");
    st.settings.language = "es".into();
    let mut h = keys_harness(st, SIZE);
    h.input_mut().modifiers = Modifiers::ALT;
    h.run_ok();
    h.check_snapshot("menu_bar_mnemonics_spanish");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_keyboard_file() {
    let (st, dir) = fixture("menu-keys-file");
    let mut h = keys_harness(st, SIZE);
    press(&mut h, Modifiers::ALT, Key::F);
    h.check_snapshot("menu_bar_keyboard_file");
    drop(h);
    remove_fixture(&dir);
}
