//! The app's shortcuts, matched through the effective keymap: each frame's key presses are
//! turned into chords, looked up, and handled when what is open lets the binding through
//! (Preferences, the palette, a dialog, a credential prompt, a focused text field). Presses no
//! binding takes are left for the views; Esc goes on to the Esc chain in `ui/central.rs`.

use super::chords::take_press;
use super::record;
use crate::keymap::{BindingId, Chord, KeyAction, KeyName};
use crate::msg::{ConflictMsg, DiffMsg, JobMsg, LayoutMsg, Msg, PaletteMsg, PrefsMsg, RepoMsg, SearchMsg, TabsMsg};
use crate::state::layout::Panel;
use crate::state::{AppState, Modal, PrefPage};
use crate::terminal::TerminalMsg;
use crate::ui::central;
use eframe::egui;

/// Turns this frame's shortcut presses into messages (or view-state changes, for the search
/// bar). While Preferences > Keyboard records a key, it gets every key press instead.
pub(crate) fn input(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if record::keys(ctx, st, msgs) {
        return;
    }
    let live = Live::now(ctx, st);
    while let Some((id, chord)) = ctx.input_mut(|i| take_press(i, &st.keymap, |id| allowed(id, &live))) {
        fire(id, chord, st, msgs);
    }
    central::escape_key(ctx, st, msgs);
}

/// What is open this frame, for deciding which bindings apply.
struct Live {
    prefs: bool,
    palette: bool,
    askpass: bool,
    /// The shortcut sheet.
    sheet: bool,
    /// A dialog other than the sheet.
    dialog: bool,
    /// A text field has the keyboard.
    typing: bool,
    working_diff: bool,
    /// A diff or the conflict view is open (and no blame view over it).
    stepping: bool,
}

impl Live {
    fn now(ctx: &egui::Context, st: &AppState) -> Self {
        let sheet = matches!(st.modal, Some(Modal::Shortcuts { .. }));
        Self {
            prefs: st.prefs.is_some(),
            palette: st.palette.is_some(),
            askpass: st.askpass.showing(),
            sheet,
            dialog: st.modal.is_some() && !sheet,
            typing: ctx.wants_keyboard_input(),
            working_diff: st.diff.as_ref().is_some_and(|(t, _)| t.is_working_tree()),
            stepping: st.blame.is_none() && (st.conflict.is_some() || st.diff.is_some()),
        }
    }
}

/// Whether `id` applies with what is open. Nothing reaches the main window's actions under
/// Preferences, a credential prompt, the palette or a dialog, except where noted; text fields
/// keep plain keys and their own undo.
fn allowed(id: BindingId, l: &Live) -> bool {
    use BindingId as I;
    let covered = l.prefs || l.askpass || l.palette || l.dialog;
    match id {
        I::Palette => !l.prefs && !l.dialog && !l.sheet && !l.askpass,
        // Closes the sheet too.
        I::Sheet => !covered,
        I::FocusMessage => !covered && !l.sheet,
        I::StageAll | I::UnstageAll => !covered && !l.sheet && !l.typing,
        I::Stage | I::Unstage => !covered && !l.sheet && !l.typing && l.working_diff,
        I::ToggleTerminal | I::ToggleLeftPanel | I::ToggleDetailsPanel | I::ToggleToolbar => !covered && !l.sheet,
        // Zoom works over everything (recording a key takes every press before this).
        I::ZoomIn | I::ZoomOut | I::ZoomReset => true,
        I::NewTab | I::CloseTab | I::ReopenTab | I::NextTab | I::PrevTab | I::LastTab | I::GoToTab => !l.prefs && !l.palette,
        I::Undo | I::Redo => !covered && !l.sheet && !l.typing,
        I::NextChange | I::PrevChange => !covered && !l.sheet && !l.typing && l.stepping,
        I::Refresh => true,
        I::Search | I::SearchFilter => !l.palette && !l.prefs,
        I::Prefs => !l.prefs && !l.palette,
        I::Commit
        | I::StageAllAndCommit
        | I::SelectRow
        | I::CloseView
        | I::PaletteMove
        | I::PaletteRun
        | I::PaletteBack
        | I::PaletteClose
        | I::SearchNext
        | I::DialogSubmit => false,
    }
}

/// Does what `id` does; `chord` is the key pressed (its digit picks the tab for Go to tab).
fn fire(id: BindingId, chord: Chord, st: &mut AppState, msgs: &mut Vec<Msg>) {
    use BindingId as I;
    let step = |d: isize, st: &AppState| {
        if st.conflict.is_some() { Msg::Conflict(ConflictMsg::ConflictStep(d)) } else { Msg::Diff(DiffMsg::JumpToChange(d)) }
    };
    let msg = match id {
        I::Stage => Msg::Shortcut(KeyAction::Stage),
        I::Unstage => Msg::Shortcut(KeyAction::Unstage),
        I::StageAll => Msg::Shortcut(KeyAction::StageAll),
        I::UnstageAll => Msg::Shortcut(KeyAction::UnstageAll),
        I::FocusMessage => Msg::Shortcut(KeyAction::FocusMessage),
        I::Sheet => Msg::Shortcut(KeyAction::ToggleSheet),
        I::Palette => Msg::Palette(PaletteMsg::TogglePalette),
        I::Prefs => Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)),
        I::Refresh => Msg::Repo(RepoMsg::Refresh),
        I::Undo => Msg::Job(JobMsg::Undo),
        I::Redo => Msg::Job(JobMsg::Redo),
        I::ToggleTerminal => Msg::Terminal(TerminalMsg::Toggle),
        I::ZoomIn => Msg::Layout(LayoutMsg::ZoomIn),
        I::ZoomOut => Msg::Layout(LayoutMsg::ZoomOut),
        I::ZoomReset => Msg::Layout(LayoutMsg::ZoomReset),
        I::ToggleLeftPanel => Msg::Layout(LayoutMsg::TogglePanel(Panel::Left)),
        I::ToggleDetailsPanel => Msg::Layout(LayoutMsg::TogglePanel(Panel::Details)),
        I::ToggleToolbar => Msg::Layout(LayoutMsg::TogglePanel(Panel::Toolbar)),
        I::NewTab => Msg::Tabs(TabsMsg::NewTab),
        I::CloseTab => Msg::Tabs(TabsMsg::CloseTab(usize::MAX)),
        I::ReopenTab => Msg::Tabs(TabsMsg::ReopenClosedTab),
        I::NextTab => Msg::Tabs(TabsMsg::CycleTab(1)),
        I::PrevTab => Msg::Tabs(TabsMsg::CycleTab(-1)),
        I::LastTab => Msg::Tabs(TabsMsg::SelectTab(st.tabs.len().saturating_sub(1))),
        I::GoToTab => match chord.key {
            KeyName::Digit(d) => Msg::Tabs(TabsMsg::SelectTab(usize::from(d).saturating_sub(1))),
            _ => return,
        },
        I::NextChange => step(1, st),
        I::PrevChange => step(-1, st),
        // The search bar's visibility is view state.
        I::Search => {
            st.view.show_search = !st.view.show_search;
            return;
        }
        I::SearchFilter => {
            st.view.show_search = true;
            Msg::Search(SearchMsg::ToggleFilter)
        }
        I::Commit
        | I::StageAllAndCommit
        | I::SelectRow
        | I::CloseView
        | I::PaletteMove
        | I::PaletteRun
        | I::PaletteBack
        | I::PaletteClose
        | I::SearchNext
        | I::DialogSubmit => return,
    };
    msgs.push(msg);
}
