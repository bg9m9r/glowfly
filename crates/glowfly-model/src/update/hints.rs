//! Tips in context (state/hints.rs): the tip row's button and × menu, turning tips off or on and
//! showing them again, the Connect and Tidy triggers on loads, and hiding a tip whose place is
//! gone (`settle`, after every message). The Undo trigger and the per-tab commit run are kept in
//! step with the undo history in update/undo.rs. Every change to the records is saved.

use crate::effect::{Effect, LoadKind};
use crate::journal::JournalMsg;
use crate::msg::{Msg, PrefsMsg, RebaseMsg};
use crate::state::hints::{self, HintId};
use crate::state::{AppState, PrefPage};
use glowfly_core::rebase::RebaseBase;
use std::time::Instant;

/// Saves the settings when `changed`.
fn save_if(st: &mut AppState, changed: bool, fx: &mut Vec<Effect>) {
    if changed {
        fx.extend(super::prefs::save_settings(st));
    }
}

/// The tip's button: retires it, then sends what the button names.
pub(super) fn act(st: &mut AppState, id: HintId, now: Instant, fx: &mut Vec<Effect>) {
    let then = match id {
        HintId::Undo => Some(Msg::Journal(JournalMsg::Open)),
        HintId::TidyCommits => {
            st.snap.as_ref().and_then(hints::tidy_base).map(|oid| Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(oid))))
        }
        HintId::Connect => Some(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations))),
    };
    retire(st, id, fx);
    if let Some(msg) = then {
        fx.extend(super::update_sealed(st, msg, now));
    }
}

/// Retires `id` for good (dismissed, or its feature used by any route), saving when that's new.
pub(super) fn retire(st: &mut AppState, id: HintId, fx: &mut Vec<Effect>) {
    let changed = hints::retire(st, id);
    save_if(st, changed, fx);
}

/// Turns tips on or off. Off hides the tip showing and retires nothing.
pub(super) fn set(st: &mut AppState, on: bool, fx: &mut Vec<Effect>) {
    if !on {
        st.onboarding.hint = None;
    }
    let changed = st.settings.hints != on;
    st.settings.hints = on;
    save_if(st, changed, fx);
}

/// Show tips again: forgets the tips retired and ignored and the one-time notes, and lets this
/// session show a tip again (unless the welcome ran in it).
pub(super) fn reset(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let s = &mut st.settings;
    s.hints_seen.clear();
    s.hints_ignored.clear();
    s.status_tip_shown.clear();
    s.detached_head_warned = false;
    st.onboarding.hints_shown_this_session = 0;
    fx.extend(super::prefs::save_settings(st));
    let text = st.settings.lang().tr("hints.reset.toast");
    st.toast(true, text, now);
}

/// A repository loaded in the active tab. Opened: its run of commits starts again, a tip from the
/// repository before it in this tab counts as ignored, and Connect is offered when its hosting
/// service has no account. A refresh
/// after a commit offers Tidy when it applies.
pub(super) fn loaded(st: &mut AppState, kind: LoadKind, fx: &mut Vec<Effect>) {
    if kind == LoadKind::Open {
        st.session.hint_commits = Default::default();
        let changed = hints::ignore(st);
        save_if(st, changed, fx);
        if hints::unconnected_service(st).is_some() {
            hints::offer(st, HintId::Connect);
        }
        return;
    }
    if !std::mem::take(&mut st.session.hint_commits.tidy_check) {
        return;
    }
    let run = st.session.hint_commits;
    if st.snap.as_ref().is_some_and(|s| hints::tidy_due(s, run)) {
        hints::offer(st, HintId::TidyCommits);
    }
}

/// After every message: tips turned off, or the Connect tip under an opened PULL REQUESTS
/// section, hide the one showing; one whose place is gone (its tab left or closed, its
/// repository closed, the left panel hidden) or that the app quits with counts as ignored.
pub(super) fn settle(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.onboarding.hint.is_none() {
        return;
    }
    let covered = st.onboarding.hint.is_some_and(|h| h.id == HintId::Connect) && hints::connect_covered(st);
    if !st.settings.hints || covered {
        // Off, or the PULL REQUESTS section opened over the Connect tip: hidden, not ignored.
        st.onboarding.hint = None;
    } else if hints::place_gone(st) || st.cues.quitting {
        let changed = hints::ignore(st);
        save_if(st, changed, fx);
    }
}
