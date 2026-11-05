//! The views' edits (`EditMsg`), each set back on the state it was drawn from: an edit for a
//! rename, terminal session, blame or welcome that has since gone is dropped, and only the
//! fields the view writes are taken (the matches, the template, the read and save stay update's).

use crate::effect::Effect;
use crate::msg::EditMsg;
use crate::state::AppState;

pub(super) fn handle(st: &mut AppState, msg: EditMsg, fx: &mut Vec<Effect>) {
    match msg {
        EditMsg::Commit(f) => st.session.draft.apply(f),
        EditMsg::Welcome(f) => {
            if let Some(w) = &mut st.onboarding.welcome {
                w.apply(f);
            }
        }
        EditMsg::TerminalRename(r) => {
            if let Some(open) = st.terminal.renaming.as_mut().filter(|open| open.id == r.id) {
                *open = r;
            }
        }
        EditMsg::TerminalSelection { id, selection } => {
            if let Some(s) = st.terminal.sessions.iter_mut().find(|s| s.id == id) {
                s.selection = selection;
            }
        }
        EditMsg::Blame(f) => {
            if let Some(v) = st.session.blame.view.as_mut().filter(|v| v.path == f.path && v.at == f.at) {
                v.apply(*f);
            }
        }
        EditMsg::FileNav(nav) => {
            if let Some(v) = &mut st.session.history.view {
                v.nav.apply_view(*nav);
            }
        }
        EditMsg::Settings(s) => {
            st.settings = *s;
            super::prefs::settings_edited(st, fx);
        }
    }
}
