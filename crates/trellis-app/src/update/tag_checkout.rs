//! Checking out a tag (detached): one git job named after the tag, its headline, and the
//! one-time detached-HEAD warning with a "Create branch…" action after the first success.

use super::ops::{self, JobSpec};
use crate::effect::Effect;
use crate::msg::{BranchMsg, Msg};
use crate::state::AppState;
use crate::state::tags::{DETACHED_WARNING, TagCheckoutJob, checkout_headline};
use crate::state::toasts::ToastAction;
use trellis_core::gitcli::args;
use trellis_core::tag_ops::is_valid_tag_name;
use trellis_core::undo::UndoKind;

/// Checks `tag` against the snapshot and starts `git checkout refs/tags/<tag>`. A missing tag
/// is logged and starts nothing.
pub(super) fn checkout(st: &mut AppState, tag: String, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    if !is_valid_tag_name(&tag) {
        st.log(false, "Not a valid tag name.");
        return;
    }
    let oid = snap.tags.iter().find(|(n, _)| *n == tag).map(|(_, o)| *o);
    if oid.is_none() && !snap.other_tags.contains(&tag) {
        st.log(false, format!("Tag {tag} no longer exists."));
        return;
    }
    if ops::busy(st, false) {
        return;
    }
    let cmd = args(&["checkout", &format!("refs/tags/{tag}")]);
    let label = format!("Checkout tag {tag}");
    let record = UndoKind::classify(&cmd);
    let workdir = st.workdir();
    // Also for a job waiting for the trust prompt; dropping it clears this.
    if ops::start_job(st, JobSpec { record, ..JobSpec::new(label.clone(), vec![cmd]) }, fx) != ops::Started::No {
        st.tag_checkout = Some(TagCheckoutJob { label, tag, oid, workdir });
    }
}

/// The headline of the job that just ended, when it is the pending tag checkout, and the job
/// again when this success is the first one (so `warned` runs after the toast). None on
/// failure, which keeps the usual failure text. Always clears the pending job.
pub(super) fn headline(st: &mut AppState, ok: bool) -> Option<(String, Option<TagCheckoutJob>)> {
    let pending = st.tag_checkout.take()?;
    if !ok || st.job.as_ref().is_none_or(|j| j.label != pending.label) {
        return None;
    }
    if st.settings.detached_head_warned {
        Some((checkout_headline(&pending.tag, false), None))
    } else {
        Some((checkout_headline(&pending.tag, true), Some(pending)))
    }
}

/// After the first tag checkout's toast: gives it the "Create branch…" action (at the tag's
/// commit, else HEAD), logs the warning, and saves that it was shown.
pub(super) fn warned(st: &mut AppState, pending: TagCheckoutJob, fx: &mut Vec<Effect>) {
    let start = pending.oid.map(|o| o.to_string());
    let msg = Box::new(Msg::Branch(BranchMsg::NewBranch { start, name: String::new() }));
    st.set_last_toast_action(ToastAction { label: "Create branch…".into(), msg, workdir: pending.workdir });
    st.log(true, DETACHED_WARNING);
    st.settings.detached_head_warned = true;
    fx.push(super::prefs::save_settings(st));
}
