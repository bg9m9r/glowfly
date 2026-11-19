//! Copy > Message (`RepoMsg::CopyMessage`): the snapshot keeps a commit's body flattened to one
//! line for search, so the whole message is read from the repository (`CommitEffect::CopyMessage`)
//! and copied as the details panel shows it; a commit that can't be read says so in a toast.
//! Tests in `update/tests/copy_message_tests.rs` and `flows/copy_message_tests.rs`.

use crate::effect::{CommitEffect, Effect, UiEffect};
use crate::state::AppState;
use crate::state::error_words::typed;
use git2::Oid;
use glowfly_core::error::ReadFailed;
use std::time::Instant;

/// Asks for commit `oid`'s whole message.
pub(super) fn read(st: &AppState, oid: Oid, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    fx.push(Effect::Commit(CommitEffect::CopyMessage { workdir, oid }));
}

/// Copies the message read, or says why it can't be.
pub(super) fn copied(st: &mut AppState, result: Result<String, ReadFailed>, now: Instant, fx: &mut Vec<Effect>) {
    match result {
        Ok(text) => fx.push(Effect::Ui(UiEffect::CopyText(text))),
        Err(e) => {
            let t = st.settings.lang();
            st.toast(false, t.trf("commits.copy_message.failed", &[("error", &typed::read(t, &e))]), now)
        }
    }
}
