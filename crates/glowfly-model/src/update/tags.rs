//! The New tag dialog: opening it, creating the tag (and pushing it in the same job), and the
//! push checkbox's setting. Also the TAGS row menu's remote push and delete: one job with a
//! command per remote, and the headline that names the remote when it ends. Annotating a
//! lightweight tag is in `tag_annotate.rs`.

use super::ops::{self, JobSpec};
use crate::effect::Effect;
use crate::state::branches::default_remote;
use crate::state::tags::{RemoteTagJob, RemoteTagOp, TagForm, job_label, tag_problem};
use crate::state::{AppState, Modal};
use crate::tag::TagMsg;
use glowfly_core::tag_ops::{delete_remote_tag_cmd, is_valid_tag_name, push_tag_cmd, tag_cmd};
use glowfly_core::undo::UndoKind;

pub(super) fn handle(st: &mut AppState, msg: TagMsg, fx: &mut Vec<Effect>) {
    match msg {
        TagMsg::NewTag { at, annotated } => {
            let remote = st.snap.as_ref().and_then(default_remote).unwrap_or_default();
            st.modal = Some(Modal::NewTag(TagForm { at, annotated, remote, ..TagForm::default() }));
        }
        TagMsg::CreateTag { at, name, message, push } => create(st, &at, &name, &message, push, fx),
        TagMsg::SetPushNewTags(on) if st.settings.push_new_tags != on => {
            st.settings.push_new_tags = on;
            fx.extend(super::prefs::save_settings(st));
        }
        TagMsg::SetPushNewTags(_) => {}
        TagMsg::Push { tag, remotes } => start_remote(st, RemoteTagOp::Push, tag, remotes, fx),
        TagMsg::DeleteRemote { tag, remotes } => start_remote(st, RemoteTagOp::Delete, tag, remotes, fx),
        TagMsg::OpenAnnotate { tag } => super::tag_annotate::open(st, tag),
        TagMsg::Annotate { tag, at, message } => super::tag_annotate::annotate(st, &tag, &at, &message, fx),
        TagMsg::Checkout { tag } => super::tag_checkout::checkout(st, tag, fx),
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
    // Undo deletes a local tag; one also pushed would stay on the remote, so that isn't recorded.
    let record = push.is_none().then(|| UndoKind::CreateTag(name.into()));
    let mut cmds = vec![tag_cmd(name, at, message)];
    let label = match push {
        Some(r) => {
            cmds.push(push_tag_cmd(&r, name));
            format!("Create tag and push to {r}")
        }
        None => "Create tag".to_owned(),
    };
    let before = fx.len();
    ops::start_job(st, JobSpec { record, ..JobSpec::new(label, cmds) }, fx);
    if fx.len() > before && matches!(st.modal, Some(Modal::NewTag(_))) {
        st.modal = None;
    }
}

/// Checks the request against the current snapshot and starts one job with a command per
/// remote. The remotes come from the message, so one added after a confirmation is never
/// touched. A refusal is logged (an empty list silently) and starts nothing.
fn start_remote(st: &mut AppState, op: RemoteTagOp, tag: String, remotes: Vec<String>, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    if !is_valid_tag_name(&tag) {
        st.log(false, "Not a valid tag name.");
        return;
    }
    let mut targets: Vec<String> = Vec::new();
    for r in remotes {
        if !targets.contains(&r) {
            targets.push(r);
        }
    }
    if targets.is_empty() {
        return;
    }
    if let Some(r) = targets.iter().find(|r| !snap.remotes.iter().any(|(n, _)| n == *r)) {
        let text = format!("There is no remote named {r}.");
        st.log(false, text);
        return;
    }
    let local = snap.tags.iter().any(|(n, _)| *n == tag) || snap.other_tags.contains(&tag);
    if op == RemoteTagOp::Push && !local {
        st.log(false, format!("Tag {tag} no longer exists."));
        return;
    }
    if ops::busy(st, false) {
        return;
    }
    let cmds = targets
        .iter()
        .map(|r| match op {
            RemoteTagOp::Push => push_tag_cmd(r, &tag),
            RemoteTagOp::Delete => delete_remote_tag_cmd(r, &tag),
        })
        .collect();
    let label = job_label(op, &tag, &targets);
    // Remote tags aren't undoable, so nothing is recorded.
    // Also for a job waiting for the trust prompt; dropping it clears this.
    if ops::start_job(st, JobSpec::new(label.clone(), cmds), fx) != ops::Started::No {
        st.tag_job = Some(RemoteTagJob { label, op, tag, remotes: targets });
    }
}

/// The headline of the job that just ended, when it is the pending remote tag job. Always
/// clears the pending job.
pub(super) fn headline(st: &mut AppState, ok: bool, output: &str) -> Option<String> {
    let pending = st.tag_job.take()?;
    if st.job.as_ref().is_none_or(|j| j.label != pending.label) {
        return None;
    }
    Some(if ok { pending.done() } else { pending.failed(output) })
}
