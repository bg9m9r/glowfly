//! Commit signature checks for the details panel's badge. A failed check isn't logged: the
//! badge's tooltip says what went wrong.

use crate::effect::Effect;
use crate::state::{AppState, Selection};
use git2::Oid;
use trellis_core::signature_check::SignatureCheck;

/// Asks for a check of `oid` when it is the commit on screen, it is signed, nothing is known or
/// on its way, and git may run unasked here (trust): the check runs the repo's gpg.program.
pub(super) fn want(st: &mut AppState, oid: Oid, fx: &mut Vec<Effect>) {
    if st.selection != Selection::Commit(oid) || !st.runs_git_unasked() || !st.signatures.wanted(oid) {
        return;
    }
    let Some(format) = st.detail.as_ref().filter(|d| d.oid == oid).and_then(|d| d.signature) else { return };
    let Some(workdir) = st.workdir() else { return };
    let generation = st.signatures.start(oid);
    fx.push(Effect::VerifySignature { workdir, oid, format, generation });
}

/// Forgets what is known about `oid`'s signature and checks it again (after importing a key or
/// editing the allowed signers file outside Trellis).
pub(super) fn again(st: &mut AppState, oid: Oid, fx: &mut Vec<Effect>) {
    st.signatures.forget(oid);
    want(st, oid, fx);
}

pub(super) fn checked(st: &mut AppState, oid: Oid, generation: u64, result: Result<SignatureCheck, String>) {
    st.signatures.finish(oid, generation, result);
}
