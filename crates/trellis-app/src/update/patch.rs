//! Saving commits as a patch file (a dialog, then a write that only reads the repository, so it
//! is no job) and applying a patch file to the working tree (a dialog, then one `RunGit` job
//! that checks before it applies).

use super::ops::{self, JobSpec};
use crate::effect::Effect;
use crate::patch::{PatchEffect, PatchMsg};
use crate::state::AppState;
use crate::state::patch::{APPLY_LABEL, PendingApply, default_name, display_name, export_blocker, export_done};
use std::time::Instant;
use trellis_core::patch_ops;

pub(super) fn handle(st: &mut AppState, m: PatchMsg, now: Instant, fx: &mut Vec<Effect>) {
    match m {
        PatchMsg::Export(oids) => {
            let Some(snap) = &st.snap else { return };
            let ordered = crate::state::selection::in_graph_order(oids, snap, true);
            if ordered.is_empty() {
                return;
            }
            if let Some(why) = export_blocker(snap, &ordered) {
                st.log(false, why);
                return;
            }
            let Some(name) = default_name(snap, &ordered) else { return };
            let Some(workdir) = st.workdir() else { return };
            fx.push(Effect::Patch(PatchEffect::PickSave { workdir, oids: ordered, name }));
        }
        PatchMsg::SaveTo { dest: None, .. } => {}
        PatchMsg::SaveTo { workdir, oids, dest: Some(dest) } => fx.push(Effect::Patch(PatchEffect::Export { workdir, oids, dest })),
        PatchMsg::Exported { dest, result } => match result {
            Ok(r) => {
                st.toast(true, export_done(&dest, &r), now);
                let commits = if r.written == 1 { "1 commit".to_owned() } else { format!("{} commits", r.written) };
                st.log(true, format!("Created {} from {commits}", dest.display()));
            }
            Err(e) => {
                let text = format!("Could not create the patch: {e}");
                st.toast(false, &text, now);
                st.log(false, text);
            }
        },
        PatchMsg::PickToApply => {
            let Some(workdir) = st.workdir() else { return };
            if ops::busy(st, false) {
                return;
            }
            fx.push(Effect::Patch(PatchEffect::PickOpen { workdir }));
        }
        PatchMsg::ApplyFrom { path: None, .. } => {}
        PatchMsg::ApplyFrom { workdir, path: Some(path) } => apply(st, &workdir, &path, fx),
    }
}

/// Applies the patch file at `path` to `workdir`, the repository the dialog was opened for, if
/// that is still the active tab and no job is running.
fn apply(st: &mut AppState, workdir: &std::path::Path, path: &std::path::Path, fx: &mut Vec<Effect>) {
    if !st.workdir().is_some_and(|w| trellis_core::paths::same_spelling(&w, workdir)) {
        st.log(false, format!("The patch was not applied: {} is no longer the active tab.", display_name(workdir)));
        return;
    }
    // Command lines are Strings; a name that isn't valid Unicode can't be passed on.
    let Some(text) = path.to_str() else {
        st.log(false, format!("Trellis can't pass this file name to git: {}.", path.display()));
        return;
    };
    if ops::busy(st, false) {
        return;
    }
    // Also for a job waiting for the trust prompt; dropping it clears this.
    if ops::start_job(st, JobSpec::new(APPLY_LABEL.into(), patch_ops::apply_cmds(text)), fx) != ops::Started::No {
        st.patch_apply = Some(PendingApply { label: APPLY_LABEL.into(), name: display_name(path) });
    }
}

/// The headline of the job that just ended, when it is the pending Apply patch job. Always
/// clears the pending apply.
pub(super) fn headline(st: &mut AppState, ok: bool, output: &str) -> Option<String> {
    let pending = st.patch_apply.take()?;
    if st.job.as_ref().is_none_or(|j| j.label != pending.label) {
        return None;
    }
    let name = pending.name;
    if ok {
        return Some(format!("Applied {name} to the working tree."));
    }
    let why = patch_ops::failure_summary(output).unwrap_or_else(|| ops::last_line(output).to_owned());
    Some(format!("{name} doesn't apply: {}. Nothing was changed.", why.trim_end_matches('.')))
}
