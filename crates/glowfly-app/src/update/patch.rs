//! Patch files: saving commits, one file of a commit or uncommitted changes (a dialog, then a
//! write that only reads the repository, so it is no job), copying a commit's patch, showing a
//! saved one, and applying a patch file (`patch_apply.rs`: read it, the Apply patch dialog,
//! then one `RunGit` job). The Create patch dialog for uncommitted changes is in
//! `patch_changes.rs`.

#[path = "patch_apply.rs"]
mod apply;
#[path = "patch_changes.rs"]
mod changes;

#[cfg(test)]
#[path = "patch_apply_tests.rs"]
mod apply_tests;
#[cfg(test)]
#[path = "patch_changes_tests.rs"]
mod changes_tests;
#[cfg(test)]
#[path = "patch_copy_tests.rs"]
mod copy_tests;

use crate::effect::OsEffect;
use crate::effect::{Effect, UiEffect};
use crate::msg::Msg;
use crate::patch::{PatchEffect, PatchMsg};
use crate::state::AppState;
use crate::state::files::SHOW_IN_FOLDER;
use crate::state::patch::{default_name, export_blocker, export_done};
use crate::state::toasts::ToastAction;
use glowfly_core::patch_ops::{self, PatchSource, SaveReport};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, m: PatchMsg, now: Instant, fx: &mut Vec<Effect>) {
    match m {
        PatchMsg::Export(oids) => {
            let Some(ordered) = ordered(st, oids) else { return };
            let Some(name) = st.snap.as_ref().and_then(|s| default_name(s, &ordered)) else { return };
            let Some(workdir) = st.workdir() else { return };
            fx.push(Effect::Patch(PatchEffect::PickSave {
                workdir,
                what: PatchSource::Commits { oids: ordered, paths: Vec::new() },
                name,
            }));
        }
        PatchMsg::ExportFiles { oid, paths } => {
            let Some(ordered) = ordered(st, vec![oid]) else { return };
            let Some(c) = st.snap.as_ref().and_then(|s| s.row_of.get(&oid).map(|&r| &s.commits[r])) else { return };
            let name = patch_ops::files_file_name(&c.summary, &paths);
            let Some(workdir) = st.workdir() else { return };
            fx.push(Effect::Patch(PatchEffect::PickSave { workdir, what: PatchSource::Commits { oids: ordered, paths }, name }));
        }
        PatchMsg::ChangesDialog { scope, path } => changes::dialog(st, scope, path),
        PatchMsg::SaveChanges { scope, paths, untracked } => changes::save(st, scope, paths, untracked, now, fx),
        PatchMsg::SaveTo { dest: None, .. } => {}
        PatchMsg::SaveTo { workdir, what, dest: Some(dest) } => fx.push(Effect::Patch(PatchEffect::Export { workdir, what, dest })),
        PatchMsg::Exported { dest, result } => match result {
            Ok(r) => {
                let action = ToastAction {
                    label: SHOW_IN_FOLDER.into(),
                    msg: Box::new(Msg::Patch(PatchMsg::ShowSaved(dest.clone()))),
                    workdir: None,
                };
                st.toast_action(true, export_done(&dest, &r), action, now);
                let what = match r {
                    SaveReport::Commits(r) if r.written == 1 => "1 commit".to_owned(),
                    SaveReport::Commits(r) => format!("{} commits", r.written),
                    SaveReport::Changes { files: 1 } => "the changes to 1 file".to_owned(),
                    SaveReport::Changes { files } => format!("the changes to {files} files"),
                };
                st.log(true, format!("Created {} from {what}", dest.display()));
            }
            Err(e) => {
                let text = format!("Could not create the patch: {e}");
                st.toast(false, &text, now);
                st.log(false, text);
            }
        },
        PatchMsg::ShowSaved(path) => fx.push(Effect::Os(OsEffect::RevealPath(path))),
        PatchMsg::Copy(oids) => {
            let Some(oids) = ordered(st, oids) else { return };
            let Some(workdir) = st.workdir() else { return };
            fx.push(Effect::Patch(PatchEffect::Copy { workdir, oids }));
        }
        PatchMsg::Copied(Ok((text, report))) => {
            // The first part's "From <sha>" line names the commit.
            let short = text.strip_prefix("From ").and_then(|r| r.get(..7)).unwrap_or("the commit").to_owned();
            let done = match report.written {
                1 => format!("Copied the patch of {short} to the clipboard"),
                n => format!("Copied the patches of {n} commits to the clipboard"),
            };
            fx.push(Effect::Ui(UiEffect::CopyText(text)));
            st.toast(true, done, now);
        }
        PatchMsg::Copied(Err(e)) => st.toast(false, e, now),
        PatchMsg::PickToApply | PatchMsg::ApplyFrom { .. } | PatchMsg::Inspected { .. } | PatchMsg::Apply(_) => {
            apply::handle(st, m, now, fx)
        }
    }
}

/// `oids` that are graph rows, oldest first, unless there are none or a merge is among them
/// (said in the log).
fn ordered(st: &mut AppState, oids: Vec<git2::Oid>) -> Option<Vec<git2::Oid>> {
    let snap = st.snap.as_ref()?;
    let ordered = crate::state::selection::in_graph_order(oids, snap, true);
    if ordered.is_empty() {
        return None;
    }
    if let Some(why) = export_blocker(snap, &ordered) {
        st.log(false, why.text(st.settings.lang()));
        return None;
    }
    Some(ordered)
}

/// The headline of the job that just ended, when it is a patch job (see `patch_apply.rs`).
pub(super) fn headline(st: &mut AppState, ok: bool, output: &str) -> Option<String> {
    apply::headline(st, ok, output)
}
