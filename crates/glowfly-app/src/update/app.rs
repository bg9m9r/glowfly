//! Window-level actions from the menu bar and a drawn title bar: quitting, the About dialog and its notices,
//! and moving, resizing, minimising or maximising the window, showing its system menu, or
//! entering or leaving full screen (which narrows what the others do, `WindowCmd::in_window`).
//! Every way of quitting (File > Quit, the Quit key, the title bar's Close and the OS asking to
//! close the window) goes through `quit`, which asks first while work exists only in memory
//! (drafts, conflict choices, an edited dialog, unsaved Preferences edits), work runs or a
//! terminal runs a program (`state/quit_confirm.rs`; the terminals are asked about off the UI
//! thread first, update/terminal_busy.rs). An edited dialog stays under the question. Quit
//! anyway stops a stoppable job (a fetch) and closes once it has ended; the open question closes
//! the window by itself once what it named has ended (`settle`), never while it names unsaved
//! work.

use crate::effect::{Effect, UiEffect};
use crate::msg::AppMsg;
use crate::state::dialog_guard::open_over;
use crate::state::quit_confirm::{QuitAsk, asking, quit_confirm, running, unsaved};
use crate::state::terminal::busy::{self as terminal_busy, BusyTerminal};
use crate::state::{AppState, Modal};
use crate::terminal::BusyCheck;

pub(super) fn handle(st: &mut AppState, msg: AppMsg, fx: &mut Vec<Effect>) {
    match msg {
        AppMsg::Quit => quit(st, false, fx),
        AppMsg::CloseRequested => quit(st, true, fx),
        AppMsg::QuitConfirmed => {
            st.view.quit_ask = None;
            // A fetch is stopped first, so it removes its lock files; the window closes once
            // it has ended (`settle`).
            if st.job_cancellable() {
                super::job_stop::cancel(st, fx);
            }
            if st.job_cancelling() {
                st.view.quit_after_stop = true;
                return;
            }
            st.view.quitting = true;
            fx.push(Effect::Ui(UiEffect::CloseWindow));
        }
        // A full-screen window can't be moved or resized, and Maximize or Restore leaves full
        // screen.
        AppMsg::Window(cmd) => {
            if let Some(cmd) = cmd.in_window(st.view.window) {
                fx.push(Effect::Ui(UiEffect::Window(cmd)));
            }
        }
        // Like the shortcut sheet: not over another dialog, Preferences or a credential prompt,
        // and it replaces the palette. The notices' Back returns to it.
        AppMsg::ShowAbout => {
            if matches!(st.modal, None | Some(Modal::Notices)) && st.prefs.is_none() && !st.askpass.showing() {
                st.palette = None;
                st.modal = Some(Modal::About);
            }
        }
        // Only from About, which it replaces.
        AppMsg::ShowNotices => {
            if matches!(st.modal, Some(Modal::About)) {
                st.modal = Some(Modal::Notices);
            }
        }
    }
}

/// A quit from Glowfly (`os_request` false: it closes the window itself) or the OS asking to
/// close it (`os_request`: it closes after this pass unless cancelled). Tabs and settings are
/// saved as they change, so closing needs nothing else, but while work runs or a terminal runs
/// a program the quit question opens instead (over any other dialog, closing the palette) and
/// the window stays. Running shells are asked about first: the OS's close is cancelled
/// meanwhile, and the answer (`checked`) closes the window itself.
fn quit(st: &mut AppState, os_request: bool, fx: &mut Vec<Effect>) {
    // Quit went ahead (or is waiting for a stopped job, and is asked again): the close passes.
    if st.view.quitting || st.view.quit_after_stop {
        st.view.quitting = true;
        if !os_request {
            fx.push(Effect::Ui(UiEffect::CloseWindow));
        }
        return;
    }
    // Asked again while the question shows: it stays as it is.
    if asking(st) {
        if os_request {
            fx.push(Effect::Ui(UiEffect::CancelClose));
        }
        return;
    }
    let ids = terminal_busy::running_ids(st);
    if !ids.is_empty() {
        if os_request {
            fx.push(Effect::Ui(UiEffect::CancelClose));
        }
        super::terminal_busy::check(ids, BusyCheck::Quit, fx);
        return;
    }
    decide(st, Vec::new(), os_request, fx);
}

/// The shells' answer for a quit: asks if anything would be cut off, else closes. A quit that
/// went ahead or a question already open meanwhile stays as it is.
pub(super) fn checked(st: &mut AppState, busy: Vec<BusyTerminal>, fx: &mut Vec<Effect>) {
    if st.view.quitting || st.view.quit_after_stop || asking(st) {
        return;
    }
    decide(st, busy, false, fx);
}

fn decide(st: &mut AppState, busy: Vec<BusyTerminal>, os_request: bool, fx: &mut Vec<Effect>) {
    if let Some(modal) = quit_confirm(st, &busy) {
        if os_request {
            fx.push(Effect::Ui(UiEffect::CancelClose));
        }
        st.palette = None;
        let ask = QuitAsk { work: running(st).is_some(), terminals: busy, unsaved: unsaved(st) };
        // An edited form stays underneath, and comes back on Cancel.
        open_over(&mut st.modal, &mut st.dialog, modal);
        st.view.quit_ask = Some(ask);
        return;
    }
    st.view.quitting = true;
    if !os_request {
        fx.push(Effect::Ui(UiEffect::CloseWindow));
    }
}

/// After every message: a job stopped by Quit anyway has ended, so the window closes; and while
/// the quit question shows, once the work and the terminals it named have all ended it closes
/// the window by itself, or names only what is left when part of it has.
pub(super) fn settle(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.view.quit_after_stop && st.job.is_none() {
        st.view.quit_after_stop = false;
        st.view.quitting = true;
        fx.push(Effect::Ui(UiEffect::CloseWindow));
        return;
    }
    if !asking(st) {
        st.view.quit_ask = None;
        return;
    }
    let Some(ask) = st.view.quit_ask.take() else { return };
    let work = running(st).is_some();
    let named = ask.terminals.len();
    let terminals: Vec<BusyTerminal> = ask.terminals.into_iter().filter(|b| terminal_busy::running(st, b.id)).collect();
    // Unsaved work never ends by itself: only the user's answer closes the window then.
    if !ask.unsaved && !work && terminals.is_empty() {
        st.modal = None;
        st.view.quitting = true;
        fx.push(Effect::Ui(UiEffect::CloseWindow));
        return;
    }
    if (ask.work && !work) || terminals.len() != named {
        // The form kept underneath stays there.
        if let Some(modal) = quit_confirm(st, &terminals) {
            st.modal = Some(modal);
        }
    }
    st.view.quit_ask = Some(QuitAsk { work, terminals, unsaved: ask.unsaved });
}
