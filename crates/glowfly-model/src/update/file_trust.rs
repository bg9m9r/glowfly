//! The trust gate for working-tree file work (`Effect::Files`): every such effect is pushed
//! here. Work that starts git in the repository (`FilesEffect::runs_git`: a whole file staged,
//! discarded, restored or written from the index, a conflict marked, a staged rename) lets the
//! programs the repository's settings name run, so while git is held there it waits for the
//! trust prompt, which names it, and runs on Trust and run (`trust::released`, checked again by
//! the runner); Cancel or Don't trust drops it. In a repository git runs in unasked it is
//! pushed as it is, and the runner checks the repository again just before it starts
//! (`trust_seal::read_in`). Work that only uses libgit2 is never held.

use super::jobs::Started;
use crate::effect::{Effect, FilesEffect, LocalOp};
use crate::state::AppState;
use crate::state::trust::WaitingFor;

/// Pushes `effect`, or holds it for the trust prompt while git is held in the active
/// repository and the effect starts git. `Held`: nothing was pushed; it runs once the
/// repository is trusted, and a conflict change's sender marks its view as for one sent at
/// once (`dropped` clears that if it never runs). `No`: no prompt could open (another dialog
/// is open); the log says why nothing ran.
pub(super) fn push(st: &mut AppState, effect: FilesEffect, fx: &mut Vec<Effect>) -> Started {
    if !effect.runs_git() || st.runs_git_unasked() {
        fx.push(Effect::Files(effect));
        return Started::Now;
    }
    let t = st.settings.lang();
    let label = label(t, &effect).to_owned();
    st.log(false, held_text(st, &label));
    super::trust::hold_waiting(st, WaitingFor::Read(Effect::Files(effect)), Some(label));
    if st.trust.waiting.is_some() { Started::Held } else { Started::No }
}

/// Waiting file work was dropped (update/trust_waiting.rs): a conflict change no longer counts
/// as being saved in the open conflict view.
pub(super) fn dropped(st: &mut AppState, effect: &Effect) {
    let conflict = matches!(
        effect,
        Effect::Files(FilesEffect::LocalOp {
            op: LocalOp::ResolveConflict { .. }
                | LocalOp::SavePartialResolution { .. }
                | LocalOp::ResolveWithSide { .. }
                | LocalOp::MarkResolved(_),
            ..
        })
    );
    if let Some(c) = st.session.conflict.view.as_mut().filter(|_| conflict) {
        c.saving = false;
        c.step_on_resolve = false;
        c.pending_save = None;
    }
}

/// Why `label` didn't run while git is held in the repository.
pub(super) fn held_text(st: &AppState, label: &str) -> String {
    st.settings.lang().trf("trust.files.held", &[("label", &label)])
}

/// What the trust prompt says waits.
fn label(t: glowfly_core::i18n::Lang, effect: &FilesEffect) -> &'static str {
    match effect {
        FilesEffect::Restore { .. } => t.tr("trust.files.restore"),
        FilesEffect::Work { .. } => t.tr("trust.files.rename"),
        FilesEffect::DiscardPaths { .. } | FilesEffect::LocalOp { op: LocalOp::DiscardFile(_), .. } => t.tr("trust.files.discard"),
        FilesEffect::LocalOp { op: LocalOp::StageFile(_) | LocalOp::StagePaths(_) | LocalOp::StageAll, .. } => t.tr("trust.files.stage"),
        FilesEffect::LocalOp { op: LocalOp::MarkConflicted(_), .. } => t.tr("trust.files.mark_conflicted"),
        FilesEffect::LocalOp {
            op:
                LocalOp::ResolveConflict { .. }
                | LocalOp::SavePartialResolution { .. }
                | LocalOp::ResolveWithSide { .. }
                | LocalOp::MarkResolved(_),
            ..
        } => t.tr("trust.files.conflict"),
        // The rest starts no git, so it is never held.
        FilesEffect::LocalOp { op: LocalOp::UnstagePaths(_) | LocalOp::Hunk { .. } | LocalOp::Lines { .. }, .. }
        | FilesEffect::LoadTracked { .. }
        | FilesEffect::Revert { .. }
        | FilesEffect::DeletePaths { .. } => t.tr("trust.files.change"),
    }
}
