//! Copy hunk as patch (`DiffMsg::CopyHunkPatch`): the hunk is read again from the repository
//! (`Effect::CopyHunkPatch`), so the patch keeps the file's line endings and bytes, and copied
//! when it is UTF-8 text; a file in another encoding can't go to the clipboard as text.

use crate::effect::{Effect, HunkPatchRequest, UiEffect};
use crate::state::AppState;
use crate::state::error_words::typed;
use glowfly_core::error::ReadFailed;
use std::time::Instant;

/// Asks for hunk `index` of the diff on screen, unless that diff changed since the menu showed it.
pub(super) fn read(st: &mut AppState, index: usize, generation: u64, fx: &mut Vec<Effect>) {
    if st.hunk_ops_gen() != Some(generation) {
        let t = st.settings.lang();
        st.log(false, t.tr("diff.copy_hunk.stale"));
        return;
    }
    let Some((target, d)) = st.session.diff.view.as_ref() else { return };
    let Some(expected) = d.hunks.get(index).cloned() else { return };
    let target = target.clone();
    let Some(workdir) = st.workdir() else { return };
    let ignore_whitespace = st.settings.diff_ignore_whitespace;
    fx.push(Effect::CopyHunkPatch(HunkPatchRequest { workdir, target, index, expected, ignore_whitespace }));
}

/// Copies the patch read, or says why it can't be.
pub(super) fn copied(st: &mut AppState, result: Result<Vec<u8>, ReadFailed>, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    match result.map(String::from_utf8) {
        Ok(Ok(text)) => fx.push(Effect::Ui(UiEffect::CopyText(text))),
        Ok(Err(_)) => st.toast(false, t.tr("diff.row_menu.not_text"), now),
        Err(e) => st.toast(false, t.trf("diff.copy_hunk.failed", &[("error", &typed::read(t, &e))]), now),
    }
}
