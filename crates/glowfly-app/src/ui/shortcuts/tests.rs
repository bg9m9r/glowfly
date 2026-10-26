//! Matching the newer keys: which presses become `Msg::Shortcut`, and when nothing does
//! (dialogs, the palette, Preferences, a credential prompt, a focused text field).

use crate::keymap::KeyAction;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::askpass::PendingPrompt;
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::Harness;
use glowfly_core::askpass::Prompt;
use glowfly_core::repo::{DiffTarget, FileDiff};
use glowfly_core::settings::Settings;
use std::time::Instant;

struct Keys {
    st: AppState,
    msgs: Vec<Msg>,
    /// Draw a text field, and give it focus on the next frame.
    field: Option<(String, bool)>,
}

type H = Harness<'static, Keys>;

fn diff(target: DiffTarget) -> AppState {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    let empty = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.diff = Some((target, empty.into()));
    st
}

fn unstaged() -> AppState {
    diff(DiffTarget::Unstaged("f.txt".into()))
}

fn harness(st: AppState, field: bool) -> H {
    let keys = Keys { st, msgs: Vec::new(), field: field.then(|| (String::new(), true)) };
    let mut h = crate::ui::snapshot_check::builder().build_state(
        |ctx, k: &mut Keys| {
            // Like the shell: keys before the views draw.
            super::input(ctx, &mut k.st, &mut k.msgs);
            if let Some((text, focus)) = k.field.as_mut() {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let r = ui.text_edit_singleline(text);
                    if std::mem::take(focus) {
                        r.request_focus();
                    }
                });
            }
        },
        keys,
    );
    h.run();
    h
}

/// Presses `key` with `modifiers` for one frame and returns the actions it produced.
fn press(h: &mut H, modifiers: Modifiers, key: Key) -> Vec<KeyAction> {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = modifiers;
    input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    let msgs = std::mem::take(&mut h.state_mut().msgs);
    msgs.iter().map(|m| if let Msg::Shortcut(a) = m { *a } else { panic!("unexpected message") }).collect()
}

const CMD_SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::COMMAND };

#[test]
fn staging_keys() {
    let mut h = harness(unstaged(), false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::S), [KeyAction::Stage]);
    assert_eq!(press(&mut h, Modifiers::SHIFT, Key::S), [KeyAction::Stage], "Shift is tolerated");
    assert_eq!(press(&mut h, CMD_SHIFT, Key::S), [KeyAction::StageAll], "not Stage as well");
    assert_eq!(press(&mut h, CMD_SHIFT, Key::U), [KeyAction::UnstageAll]);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::S), []);
    assert_eq!(press(&mut h, Modifiers::ALT, Key::S), []);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::U), [KeyAction::Unstage], "update decides U does nothing here");
    assert_eq!(press(&mut h, CMD_SHIFT, Key::M), [KeyAction::FocusMessage]);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::Slash), [KeyAction::ToggleSheet]);

    let mut h = harness(diff(DiffTarget::Staged("f.txt".into())), false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::U), [KeyAction::Unstage]);
    let mut h = harness(diff(DiffTarget::Commit(git2::Oid::zero(), "f.txt".into())), false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::S), [], "not a working-tree diff");
    assert_eq!(press(&mut h, CMD_SHIFT, Key::S), [KeyAction::StageAll], "works without a working-tree diff");
}

#[test]
fn r_reverts_on_a_commit_or_comparison_diff() {
    let commit = || diff(DiffTarget::Commit(git2::Oid::zero(), "f.txt".into()));
    let mut h = harness(commit(), false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::R), [KeyAction::RevertHunk]);
    let mut h = harness(diff(DiffTarget::Between(git2::Oid::zero(), git2::Oid::zero(), "f.txt".into())), false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::R), [KeyAction::RevertHunk]);
    let mut h = harness(unstaged(), false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::R), [], "not on a working-tree diff");
    let mut h = harness(commit(), true);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::R), [], "not while typing");
    let mut st = commit();
    st.modal =
        Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::Repo(RepoMsg::Refresh)) });
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::R), [], "not under a dialog");
}

#[test]
fn nothing_under_dialogs_or_the_palette() {
    let mut st = unstaged();
    st.modal =
        Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::Repo(RepoMsg::Refresh)) });
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::S), []);
    assert_eq!(press(&mut h, CMD_SHIFT, Key::S), []);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::Slash), []);

    let mut st = unstaged();
    st.palette = Some(Default::default());
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::S), []);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::Slash), []);

    let mut st = unstaged();
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::S), []);
    assert_eq!(press(&mut h, CMD_SHIFT, Key::M), []);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::Slash), [KeyAction::ToggleSheet], "closes the sheet");
}

/// Every newer key, on a working-tree diff with files to stage, gives nothing.
fn all_quiet(st: AppState, what: &str) {
    let mut h = harness(st, false);
    for (mods, key) in [
        (Modifiers::NONE, Key::S),
        (Modifiers::NONE, Key::U),
        (CMD_SHIFT, Key::S),
        (CMD_SHIFT, Key::U),
        (CMD_SHIFT, Key::M),
        (Modifiers::COMMAND, Key::Slash),
    ] {
        assert_eq!(press(&mut h, mods, key), [], "{mods:?} {key:?} under {what}");
    }
}

#[test]
fn nothing_under_preferences_or_a_credential_prompt() {
    let mut st = unstaged();
    st.prefs = Some(PrefsState::new(PrefPage::General));
    all_quiet(st, "Preferences");

    let mut st = unstaged();
    let prompt = PendingPrompt { id: 1, prompt: Prompt::new("Password: ", None), retry: false, rejected_before: false };
    st.askpass.queue.push_back(prompt);
    all_quiet(st, "a credential prompt");

    let st = diff(DiffTarget::Staged("f.txt".into()));
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::U), [KeyAction::Unstage], "U works with nothing in the way");
}

#[test]
fn a_focused_text_field_keeps_plain_keys() {
    let mut h = harness(unstaged(), true);
    assert!(h.ctx.wants_keyboard_input(), "the field has focus");
    assert_eq!(press(&mut h, Modifiers::NONE, Key::S), []);
    assert_eq!(press(&mut h, CMD_SHIFT, Key::S), []);
    assert_eq!(press(&mut h, CMD_SHIFT, Key::M), [KeyAction::FocusMessage]);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::Slash), [KeyAction::ToggleSheet]);
}

#[test]
fn shortcuts_are_blocked_under_the_welcome() {
    let mut st = unstaged();
    st.onboarding.welcome = Some(Default::default());
    all_quiet(st, "the welcome");
    let mut st = unstaged();
    st.onboarding.welcome = Some(Default::default());
    let mut h = harness(st, false);
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = CMD_SHIFT;
    input.events.push(Event::Key { key: Key::L, physical_key: None, pressed: true, repeat: false, modifiers: CMD_SHIFT });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    assert!(h.state().msgs.is_empty(), "no panel toggle under the welcome");
    let mut st = unstaged();
    st.onboarding.welcome = Some(Default::default());
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::P), [], "nor the palette");
    // Under Preferences the welcome isn't drawn and Preferences alone decides.
    let mut st = unstaged();
    st.onboarding.welcome = Some(Default::default());
    st.prefs = Some(PrefsState::new(PrefPage::General));
    all_quiet(st, "Preferences over the welcome");
}

/// A state with a loaded blame of one line open.
fn blamed() -> AppState {
    use crate::state::blame::{BlameData, BlameView};
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    let blame = glowfly_core::repo::Blame {
        path: "a.txt".into(),
        at: git2::Oid::zero(),
        commits: Vec::new(),
        lines: vec!["x".into()],
        runs: Vec::new(),
        origins: Vec::new(),
        orig_lines: vec![0],
        language: None,
    };
    let mut view = BlameView::loading("a.txt".into(), None);
    view.loading = false;
    view.data = Some(BlameData::new(blame, None));
    st.session.blame.view = Some(view);
    st
}

/// Presses `key` for one frame and returns every message it produced.
fn press_msgs(h: &mut H, modifiers: Modifiers, key: Key) -> Vec<Msg> {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = modifiers;
    input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    std::mem::take(&mut h.state_mut().msgs)
}

#[test]
fn run_keys_and_find_in_the_blame_view() {
    use crate::msg::BlameMsg;
    let step = |m: &[Msg]| match m {
        [Msg::Blame(BlameMsg::StepRun(d))] => Some(*d),
        _ => None,
    };
    let mut h = harness(blamed(), false);
    assert_eq!(step(&press_msgs(&mut h, Modifiers::NONE, Key::N)), Some(1));
    assert_eq!(step(&press_msgs(&mut h, Modifiers::SHIFT, Key::N)), Some(-1));
    let shown = h.state().st.view.show_search;
    let m = press_msgs(&mut h, Modifiers::COMMAND, Key::F);
    assert!(matches!(&m[..], [Msg::Blame(BlameMsg::FocusFind)]), "Mod+F finds in the file");
    assert_eq!(h.state().st.view.show_search, shown, "commit search stays as it was");

    let mut h = harness(blamed(), true);
    assert!(press_msgs(&mut h, Modifiers::NONE, Key::N).is_empty(), "a focused field keeps N");
    let mut h = harness(unstaged(), false);
    assert!(press_msgs(&mut h, Modifiers::NONE, Key::N).is_empty(), "no blame open");
    let mut st = blamed();
    st.session.blame.view.as_mut().unwrap().data = None;
    let mut h = harness(st, false);
    assert!(press_msgs(&mut h, Modifiers::NONE, Key::N).is_empty(), "not loaded yet");
    let m = press_msgs(&mut h, Modifiers::COMMAND, Key::F);
    assert!(matches!(&m[..], [Msg::Blame(BlameMsg::FocusFind)]), "{}", m.len());
}

#[test]
fn move_tab_keys() {
    use crate::msg::TabsMsg;
    let moved = |m: &[Msg]| match m {
        [Msg::Tabs(TabsMsg::MoveActiveTab(d))] => Some(*d),
        _ => None,
    };
    let mut h = harness(unstaged(), false);
    assert_eq!(moved(&press_msgs(&mut h, CMD_SHIFT, Key::PageUp)), Some(-1));
    assert_eq!(moved(&press_msgs(&mut h, CMD_SHIFT, Key::PageDown)), Some(1));
    let mut h = harness(unstaged(), true);
    assert_eq!(moved(&press_msgs(&mut h, CMD_SHIFT, Key::PageDown)), Some(1), "while typing too");

    let mut st = unstaged();
    st.palette = Some(Default::default());
    let mut h = harness(st, false);
    assert!(press_msgs(&mut h, CMD_SHIFT, Key::PageUp).is_empty(), "not under the palette");
    let mut st = unstaged();
    st.prefs = Some(PrefsState::new(PrefPage::General));
    let mut h = harness(st, false);
    assert!(press_msgs(&mut h, CMD_SHIFT, Key::PageDown).is_empty(), "not under Preferences");
}

#[test]
fn find_and_stepping_in_a_history_s_file_pane() {
    use crate::msg::{BlameMsg, DiffMsg, HistoryMsg};
    use crate::state::history::{FileHistoryView, HistoryKind, HistoryPane};
    let with_history = |pane: HistoryPane| {
        let mut st = diff(DiffTarget::Commit(git2::Oid::zero(), "src/a.rs".into()));
        let mut view = FileHistoryView::loading_kind("src".into(), None, HistoryKind::Folder);
        view.pane = pane;
        st.session.history.view = Some(view);
        st
    };
    let mut h = harness(with_history(HistoryPane::File), false);
    let m = press_msgs(&mut h, Modifiers::COMMAND, Key::F);
    assert!(matches!(&m[..], [Msg::History(HistoryMsg::FocusFileFind)]), "Mod+F finds in the file shown");
    let m = press_msgs(&mut h, Modifiers::ALT, Key::ArrowDown);
    assert!(matches!(&m[..], [Msg::History(HistoryMsg::StepHistoryFile(1))]), "Next change steps to the next file");
    let m = press_msgs(&mut h, Modifiers::ALT, Key::ArrowUp);
    assert!(matches!(&m[..], [Msg::History(HistoryMsg::StepHistoryFile(-1))]));

    let mut h = harness(with_history(HistoryPane::Diff), false);
    let shown = h.state().st.view.show_search;
    assert!(press_msgs(&mut h, Modifiers::COMMAND, Key::F).is_empty(), "the Diff pane keeps the search bar toggle");
    assert_ne!(h.state().st.view.show_search, shown);
    let m = press_msgs(&mut h, Modifiers::ALT, Key::ArrowDown);
    assert!(matches!(&m[..], [Msg::Diff(DiffMsg::JumpToChange(1))]));

    let mut st = blamed();
    st.session.history.view = with_history(HistoryPane::File).session.history.view;
    let mut h = harness(st, false);
    let m = press_msgs(&mut h, Modifiers::COMMAND, Key::F);
    assert!(matches!(&m[..], [Msg::Blame(BlameMsg::FocusFind)]), "a blame view over it finds in the blame");
}
