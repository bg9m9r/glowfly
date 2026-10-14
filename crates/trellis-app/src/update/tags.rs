//! The New tag dialog: opening it, creating the tag (and pushing it in the same job), and the
//! push checkbox's setting.

use super::ops::{self, JobSpec};
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::branches::default_remote;
use crate::state::tags::{TagForm, tag_problem};
use crate::state::{AppState, Modal};
use trellis_core::tag_ops::{push_tag_cmd, tag_cmd};

pub(super) fn handle(st: &mut AppState, msg: Msg, fx: &mut Vec<Effect>) {
    match msg {
        Msg::NewTag { at, annotated } => {
            let remote = st.snap.as_ref().and_then(default_remote).unwrap_or_default();
            st.modal = Some(Modal::NewTag(TagForm { at, name: String::new(), message: String::new(), annotated, remote }));
        }
        Msg::CreateTag { at, name, message, push } => create(st, &at, &name, &message, push, fx),
        Msg::SetPushNewTags(on) if st.settings.push_new_tags != on => {
            st.settings.push_new_tags = on;
            fx.push(super::prefs::save_settings(st));
        }
        _ => {}
    }
}

/// Checks the request again (the dialog's state may be stale) and starts one job: the tag, then
/// the push. The dialog closes only when the job starts; otherwise it stays open as typed.
fn create(st: &mut AppState, at: &str, name: &str, message: &str, push: Option<String>, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let (name, message) = (name.trim(), message.trim());
    if let Some(p) = tag_problem(Some(snap), name) {
        st.log(false, p.message(name));
        return;
    }
    if let Some(r) = push.as_deref()
        && !snap.remotes.iter().any(|(n, _)| n == r)
    {
        st.log(false, format!("There is no remote named {r}."));
        return;
    }
    if ops::busy(st, false) {
        return;
    }
    let mut cmds = vec![tag_cmd(name, at, message)];
    let label = match push {
        Some(r) => {
            cmds.push(push_tag_cmd(&r, name));
            format!("Create tag and push to {r}")
        }
        None => "Create tag".to_owned(),
    };
    // Tags aren't undoable, so nothing is recorded.
    let before = fx.len();
    ops::start_job(st, JobSpec::new(label, cmds), fx);
    if fx.len() > before && matches!(st.modal, Some(Modal::NewTag(_))) {
        st.modal = None;
    }
}
