//! The terminal header's menus and states: renaming a tab in place, the shell menu (drawn from
//! the Windows list on every OS), More terminal actions, the panel maximised over the main
//! area, and Esc restoring it while the keyboard is outside the shell.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_esc::app_harness;
use super::snapshot_tests_terminal::{pushed, with_session};
use super::terminal::{shell_items, shell_menu};
use super::theme::{self, color};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::terminal::SessionStatus;
use crate::terminal::TerminalMsg;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Key, vec2};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{ShellChoice, TerminalPrefs, ThemeChoice};
use std::time::Instant;

/// `m` handled as the app would, before the window is drawn.
fn apply(st: &mut AppState, m: TerminalMsg) {
    crate::update::update(st, Msg::Terminal(m), Instant::now());
}

#[test]
fn terminal_tab_rename_dark() {
    let (mut st, dir) = with_session(ThemeChoice::Dark, "terminal-rename", SessionStatus::Running);
    apply(&mut st, TerminalMsg::StartRename(1));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("terminal_tab_rename");
    assert!(!h.state().0.terminal.focused, "the field has the keyboard, not the shell");
    assert!(h.query_by_label("pwsh \u{b7} demo").is_none(), "the field stands in for the pill");
    h.state_mut().1.clear();
    h.input_mut().events.push(egui::Event::Text("build".into()));
    h.run();
    h.press_key(Key::Enter);
    h.run();
    assert!(
        pushed(&h, |t| matches!(t, TerminalMsg::Rename { id: 1, name, submitted: true } if name == "build")),
        "{:?}",
        h.state().0.terminal.renaming
    );
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn focus_leaving_the_rename_field_renames_without_the_shell() {
    let (mut st, dir) = with_session(ThemeChoice::Dark, "terminal-rename-blur", SessionStatus::Running);
    apply(&mut st, TerminalMsg::StartRename(1));
    let mut h = app_harness(st);
    h.run();
    h.state_mut().1.clear();
    // Move to another text field (the first one that isn't the rename field holding the name).
    let other = h
        .query_all(By::new().role(Role::TextInput))
        .find(|n| n.value().as_deref() != Some("pwsh \u{b7} demo"))
        .expect("another text field in the window");
    other.focus();
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::Rename { id: 1, submitted: false, .. })));
    assert!(!pushed(&h, |t| matches!(t, TerminalMsg::Rename { submitted: true, .. })), "only Enter hands the shell the keyboard");
    assert!(!h.state().0.terminal.focused, "the shell did not take the keyboard");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_in_the_rename_field_cancels() {
    let (mut st, dir) = with_session(ThemeChoice::Dark, "terminal-rename-esc", SessionStatus::Running);
    apply(&mut st, TerminalMsg::StartRename(1));
    let mut h = app_harness(st);
    h.run();
    h.state_mut().1.clear();
    h.press_key(Key::Escape);
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::CancelRename)));
    assert!(!pushed(&h, |t| matches!(t, TerminalMsg::Rename { .. } | TerminalMsg::Input(_))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn terminal_shell_menu() {
    let mut prefs = TerminalPrefs::default();
    prefs.shell.choice = ShellChoice::Pwsh;
    prefs.shell.program = "nu".into();
    let items = shell_items(Lang::En, &prefs, true);
    let p = theme::DARK;
    let mut h = Harness::builder().with_size(vec2(280.0, 220.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            color::set_palette(&p);
            theme::apply_style(ui.ctx(), &p);
            ui.ctx().set_theme(theme::egui_theme(&p));
            ui.painter().rect_filled(ui.max_rect(), 0.0, p.panel);
            shell_menu(ui, &items, msgs);
        },
        Vec::new(),
    );
    h.run();
    h.check_snapshot("terminal_shell_menu");
    for label in ["Automatic", "PowerShell 7 (pwsh) (default)", "Windows PowerShell", "Command Prompt (cmd)", "Custom program (nu)"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    h.get_by_label("Bash (Git Bash)").click();
    h.run();
    assert!(h.state().iter().any(|m| matches!(m, Msg::Terminal(TerminalMsg::NewTab(Some(ShellChoice::Bash))))));
}

#[test]
fn the_caret_opens_the_shell_menu() {
    let (st, dir) = with_session(ThemeChoice::Dark, "terminal-caret", SessionStatus::Running);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("New terminal with another shell").click();
    h.run();
    h.get_by_label("Automatic (default)").click();
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::NewTab(Some(ShellChoice::Auto)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn terminal_more_menu_dark() {
    let (st, dir) = with_session(ThemeChoice::Dark, "terminal-more", SessionStatus::Running);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("More terminal actions").click();
    h.run();
    h.check_snapshot("terminal_more_menu");
    h.get_by_label("Kill all terminals").click();
    h.run();
    assert!(!pushed(&h, |t| matches!(t, TerminalMsg::AskKillAll)), "one shell runs");
    h.get_by_label("Rename terminal").click();
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::StartRename(1))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn terminal_maximized_dark() {
    let (mut st, dir) = with_session(ThemeChoice::Dark, "terminal-maximized", SessionStatus::Running);
    st.terminal.maximized = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("terminal_maximized");
    assert!(h.query_by_label("Uncommitted changes").is_none(), "the graph isn't drawn");
    assert!(h.query_by_label("Maximize panel").is_none());
    h.get_by_label("Restore panel").click();
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::ToggleMaximize)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_restores_a_maximised_panel_unless_the_shell_has_the_keyboard() {
    let (mut st, dir) = with_session(ThemeChoice::Dark, "terminal-max-esc", SessionStatus::Running);
    st.terminal.maximized = true;
    st.terminal.focus_requested = false;
    let mut h = app_harness(st);
    h.run();
    assert!(!h.state().0.terminal.focused);
    h.state_mut().1.clear();
    h.press_key(Key::Escape);
    h.run();
    let msgs: Vec<&Msg> = h.state().1.iter().filter(|m| !matches!(m, Msg::Terminal(TerminalMsg::Resize { .. }))).collect();
    assert!(matches!(msgs[..], [Msg::Terminal(TerminalMsg::ToggleMaximize)]), "{} messages", msgs.len());
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = with_session(ThemeChoice::Dark, "terminal-max-esc-shell", SessionStatus::Running);
    st.terminal.maximized = true;
    let mut h = app_harness(st);
    h.run();
    assert!(h.state().0.terminal.focused);
    h.state_mut().1.clear();
    h.press_key(Key::Escape);
    h.run();
    assert!(!pushed(&h, |t| matches!(t, TerminalMsg::ToggleMaximize)), "Esc goes to the shell");
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::Input(_))));
    drop(h);
    remove_fixture(&dir);
}
