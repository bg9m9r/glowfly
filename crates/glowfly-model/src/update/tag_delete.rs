//! The Delete tag dialog: opening it at the commit the snapshot has for the tag, and deleting the
//! tag here and from the ticked remotes in one job. The local delete comes first and is recorded
//! for undo (also when a later remote delete fails); the job's `expect` runs nothing if the tag
//! moved on disk after the snapshot. Remote deletes are never undone.

use super::ops::{self, JobSpec, Started};
use crate::effect::Effect;
use crate::state::tag_delete::DeleteTagForm;
use crate::state::tags::{RemoteTagJob, RemoteTagOp};
use crate::state::{AppState, Modal};
use glowfly_core::tag_ops::{delete_remote_tag_cmd, delete_tag_cmd, is_valid_tag_name};
use glowfly_core::undo::UndoKind;

/// The job's label with nothing ticked (a label `job_spec/labels.rs` translates).
const LOCAL_LABEL: &str = "Delete tag";
/// The job's label with remotes ticked.
const REMOTE_LABEL: &str = "Delete local and remote tag";

/// Opens the Delete tag dialog for `tag`, with every remote unticked; a tag that is gone is
/// refused with a log line.
pub(super) fn open(st: &mut AppState, tag: String) {
    let Some(snap) = st.snap.as_ref() else { return };
    match DeleteTagForm::new(snap, &tag) {
        Some(form) => st.modal = Some(Modal::DeleteTag(form)),
        None => {
            let text = st.settings.lang().trf("tag.delete.gone", &[("tag", &tag)]);
            st.log(false, text);
        }
    }
}

/// Checks the request against the current snapshot (the dialog's may be stale) and starts one
/// job: `tag -d`, then a delete per remote in list order. A refusal is logged and keeps the
/// dialog open; it closes once the job starts (or waits for the trust prompt).
pub(super) fn delete(st: &mut AppState, tag: String, at: git2::Oid, remotes: Vec<String>, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let t = st.settings.lang();
    let refusal = if !is_valid_tag_name(&tag) {
        Some(t.tr("tag.delete.invalid").to_owned())
    } else {
        match snap.tags.iter().find(|(n, _)| *n == tag) {
            None => Some(t.trf("tag.delete.gone", &[("tag", &tag)])),
            Some((_, now)) if *now != at => {
                let sha = now.to_string();
                Some(t.trf("tag.delete.moved", &[("tag", &tag), ("sha", &&sha[..7])]))
            }
            Some(_) => None,
        }
    };
    if let Some(text) = refusal {
        st.log(false, text);
        return;
    }
    let mut targets: Vec<String> = Vec::new();
    for r in remotes {
        if !targets.contains(&r) {
            targets.push(r);
        }
    }
    if let Some(r) = targets.iter().find(|r| !snap.remotes.iter().any(|(n, _)| n == *r)) {
        let text = t.trf("tag.delete.no_remote", &[("remote", r)]);
        st.log(false, text);
        return;
    }
    if ops::busy(st, false) {
        return;
    }
    let mut cmds = vec![delete_tag_cmd(&tag)];
    cmds.extend(targets.iter().map(|r| delete_remote_tag_cmd(r, &tag)));
    let label = if targets.is_empty() { LOCAL_LABEL } else { REMOTE_LABEL };
    let spec = JobSpec {
        record: Some(UndoKind::DeleteTag { name: tag.clone(), remotes: targets.clone() }),
        expect: Some((format!("refs/tags/{tag}"), at)),
        ..JobSpec::new(label.to_owned(), cmds)
    };
    if ops::start_job(st, spec, fx) == Started::No {
        return;
    }
    if matches!(st.modal, Some(Modal::DeleteTag(_))) {
        st.modal = None;
    }
    // Also for a job waiting for the trust prompt; dropping it clears this.
    if !targets.is_empty() {
        st.tag_job = Some(RemoteTagJob { label: label.to_owned(), op: RemoteTagOp::Delete, tag, remotes: targets, local: true });
    }
}
