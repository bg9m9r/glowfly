//! Delete lock file and Try again for a working-tree op that failed on a lock (stage, unstage,
//! discard, a hunk or lines; `LockMsg::LocalLocked`). The failure is a note of two lines, the
//! lock sentence and libgit2's or git's own words, folded open with Try again kept on it and on
//! its toast; the runner then checks the lock (update/lock_file.rs puts Delete lock file first
//! when it is stale). Try again sends the op once more the way it was first sent: whole-file ops
//! through their `StageMsg`, so every check before them applies again, and a hunk or lines as the
//! same `LocalOp`, whose `expected` hunk core checks again. It runs only in the op's tab, while
//! that is active and still shows the op's repository.

use crate::effect::{Effect, LocalOp, LockEffect};
use crate::msg::{LockMsg, Msg, StageMsg};
use crate::state::error_words::typed;
use crate::state::lock_retry::{LocalRetry, LockRetry};
use crate::state::{AppState, ToastAction};
use glowfly_core::error::WriteFailed;
use glowfly_core::i18n::Lang;
use glowfly_core::paths::same_spelling;
use std::path::PathBuf;
use std::time::Instant;

/// `op` in `workdir` failed on lock `lock`: logs the note with Try again on it and its toast,
/// asks whether the lock may be deleted, and settles as a failed local op does.
pub(super) fn failed(
    st: &mut AppState,
    workdir: PathBuf,
    op: LocalOp,
    lock: String,
    error: WriteFailed,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let t = st.settings.lang();
    let words = typed::write(t, &error);
    let said = match &error {
        WriteFailed::Locked { said, .. } => said.clone(),
        _ => words.clone(),
    };
    let Some(tab) = st.tabs.get(st.active_tab).map(|tab| tab.id) else {
        return super::local_done::done(st, Err(error), now, fx);
    };
    let retry = LockRetry::Local(Box::new(LocalRetry { tab, workdir: workdir.clone(), op }));
    let id = st.log_noted(false, format!("{words}\n{said}"));
    // Open from the start, like a failed run, so its buttons show.
    st.logs.open.insert(id);
    let proxy = st.logs.keep_fix(id, button(t, &retry));
    // The end-of-update announcement gives it Details beside Try again.
    st.toast_action(false, words, proxy, now);
    fx.push(Effect::Lock(LockEffect::Check { workdir, lock, entry: id }));
    super::local_done::settle_failed(st, now, fx);
}

/// The Try again button that runs `retry`.
pub(super) fn button(t: Lang, retry: &LockRetry) -> ToastAction {
    match retry {
        LockRetry::Job(r) => super::job_retry::button(t, r),
        LockRetry::Local(l) => ToastAction {
            label: t.tr("job_retry.button").to_owned(),
            msg: Box::new(Msg::Lock(LockMsg::Again(l.clone()))),
            workdir: Some(l.workdir.clone()),
        },
    }
}

/// Why `retry` can't run now: its tab is closed, isn't the active one, or shows another
/// repository. None when it may.
pub(super) fn refusal(st: &AppState, retry: &LockRetry) -> Option<String> {
    match retry {
        LockRetry::Job(r) => super::job_retry::refusal(st, r),
        LockRetry::Local(l) => local_refusal(st, l),
    }
}

/// `refusal` for a working-tree op.
fn local_refusal(st: &AppState, l: &LocalRetry) -> Option<String> {
    let t = st.settings.lang();
    if !st.tabs.iter().any(|tab| tab.id == l.tab) {
        return Some(t.tr("lock.again.tab_closed").to_owned());
    }
    if st.tabs.get(st.active_tab).map(|tab| tab.id) != Some(l.tab) {
        return Some(t.tr("lock.again.switch_back").to_owned());
    }
    let here = st.workdir().is_some_and(|a| same_spelling(&l.workdir, &a));
    (!here).then(|| t.tr("lock.again.moved").to_owned())
}

/// Try again: sends the op once more, unless `refusal` says why not (in a toast).
pub(super) fn again(st: &mut AppState, l: LocalRetry, now: Instant, fx: &mut Vec<Effect>) {
    if let Some(why) = local_refusal(st, &l) {
        return st.toast(false, why, now);
    }
    let stage = |st: &mut AppState, msg: StageMsg, fx: &mut Vec<Effect>| super::ops::stage(st, msg, now, fx);
    match l.op {
        LocalOp::StageFile(p) => {
            // A folder row's stage is the same op: sent as Stage folder, which refuses while a
            // conflicted file lies under it.
            let file = st.snap.as_ref().is_some_and(|s| s.unstaged.iter().any(|f| f.path == p));
            let msg = if file { StageMsg::StageFile(p) } else { StageMsg::StageFolder(p) };
            stage(st, msg, fx)
        }
        LocalOp::StagePaths(p) => stage(st, StageMsg::StagePaths(p), fx),
        LocalOp::StageAll => stage(st, StageMsg::StageAll, fx),
        LocalOp::UnstagePaths(p) => stage(st, StageMsg::UnstagePaths(p), fx),
        LocalOp::DiscardFile(p) => stage(st, StageMsg::DiscardFile(p), fx),
        op @ (LocalOp::Hunk { .. } | LocalOp::Lines { .. }) => {
            // As the first send did: the diff's hunks are stale until the reload after it lands.
            st.session.diff.generation += 1;
            let _ = super::ops::local(st, op, fx);
        }
        op @ (LocalOp::ResolveConflict { .. }
        | LocalOp::SavePartialResolution { .. }
        | LocalOp::ResolveWithSide { .. }
        | LocalOp::MarkResolved(_)
        | LocalOp::MarkConflicted(_)
        | LocalOp::TakeTrivialPicks(_)) => {
            // The runner never offers these (`LocalOp::retryable`).
            tracing::debug!(?op, "a conflict change isn't tried again");
        }
    }
}
