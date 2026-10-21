//! Annotating a lightweight tag from the TAGS row menu: opening the dialog at the commit the
//! snapshot has for the tag, and replacing the tag with an annotated one at that commit in one
//! job. The job's `expect` refuses to run if the tag moved on disk after the snapshot.

use super::ops::{self, JobSpec};
use crate::effect::Effect;
use crate::state::tags::{AnnotateForm, annotate_problem};
use crate::state::{AppState, Modal};
use glowfly_core::tag_ops::{annotate_tag_cmd, is_valid_tag_name};

/// Opens the Annotate dialog for lightweight tag `tag`; a tag that is gone is ignored, and an
/// annotated one is refused with a log line.
pub(super) fn open(st: &mut AppState, tag: String) {
    let Some(snap) = st.snap.as_ref() else { return };
    let Some(oid) = snap.tags.iter().find(|(n, _)| *n == tag).map(|(_, o)| *o) else { return };
    if !snap.lightweight_tags.contains(&tag) {
        st.log(false, format!("Tag {tag} is already annotated."));
        return;
    }
    st.modal = Some(Modal::AnnotateTag(AnnotateForm { tag, at: oid.to_string(), message: String::new() }));
}

/// Checks the request against the current snapshot (the dialog's may be stale) and starts the
/// job. A refusal is logged and keeps the dialog open as typed; it closes once the job starts.
/// Tags aren't undoable, so nothing is recorded.
pub(super) fn annotate(st: &mut AppState, tag: &str, at: &str, message: &str, fx: &mut Vec<Effect>) {
    if !is_valid_tag_name(tag) {
        st.log(false, "Not a valid tag name.");
        return;
    }
    let message = message.trim();
    if message.is_empty() {
        st.log(false, "Enter a tag message.");
        return;
    }
    if st.snap.is_none() {
        return;
    }
    if let Some(problem) = annotate_problem(st.snap.as_ref(), tag, at) {
        st.log(false, problem);
        return;
    }
    if ops::busy(st, false) {
        return;
    }
    let Ok(oid) = git2::Oid::from_str(at) else {
        st.log(false, "Not a valid commit.");
        return;
    };
    let before = fx.len();
    let spec = JobSpec {
        expect: Some((format!("refs/tags/{tag}"), oid)),
        ..JobSpec::new(format!("Annotate tag {tag}"), vec![annotate_tag_cmd(tag, at, message)])
    };
    ops::start_job(st, spec, fx);
    if fx.len() > before && matches!(st.modal, Some(Modal::AnnotateTag(_))) {
        st.modal = None;
    }
}
