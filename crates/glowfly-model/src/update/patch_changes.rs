//! The Create patch dialog for uncommitted changes (`Modal::SavePatch`): opening it on a scope
//! (or one file), and Save…: checked again against the snapshot, then the save dialog. Saving
//! runs `git diff` in the working tree, so an untrusted repository is asked about first.

use crate::effect::Effect;
use crate::msg::{Msg, TrustMsg};
use crate::patch::PatchEffect;
use crate::state::patch::changes::{SaveChangesForm, blocked, named_paths, request};
use crate::state::{AppState, Modal};
use glowfly_core::patch_ops::{self, PatchSource, WipScope};
use std::time::Instant;

/// Logged instead of saving while git is held in the repository.
pub const HELD: &str = "Trust this repository to create a patch from its working tree: git may run its filters.";

/// Opens the dialog on `scope`, with only `path` ticked under Only selected files when given.
pub(super) fn dialog(st: &mut AppState, scope: WipScope, path: Option<String>) {
    if st.snap.is_some() {
        st.modal = Some(Modal::SavePatch(SaveChangesForm::new(scope, path)));
    }
}

/// Save…: asks where to save the changes the dialog chose, then closes it.
pub(super) fn save(st: &mut AppState, scope: WipScope, paths: Option<Vec<String>>, untracked: bool, now: Instant, fx: &mut Vec<Effect>) {
    let Some(real) = st.snap.as_ref().map(|s| s.real_workdir.clone()) else { return };
    if !st.runs_git_unasked() {
        st.log(false, HELD);
        fx.extend(super::super::update_sealed(st, Msg::Trust(TrustMsg::Ask { real }), now));
        return;
    }
    let Some(snap) = &st.snap else { return };
    let form = SaveChangesForm::of(scope, paths, untracked);
    if let Some(why) = blocked(&form, snap) {
        st.log(false, why);
        return;
    }
    let Ok(req) = request(&form, snap) else { return };
    let name = patch_ops::changes_file_name(snap.head_branch.as_deref(), &named_paths(&req));
    let Some(workdir) = st.workdir() else { return };
    fx.push(Effect::Patch(PatchEffect::PickSave { workdir, what: PatchSource::Changes(req), name }));
    if matches!(st.modal, Some(Modal::SavePatch(_))) {
        st.modal = None;
    }
}
