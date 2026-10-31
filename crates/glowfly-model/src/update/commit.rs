//! The commit box: committing (optionally staging everything first and pushing afterwards),
//! prefilling the draft from HEAD for an amend, from a merge's MERGE_MSG or from the
//! repository's commit template, the merge banner's Commit merge, and the "Push after
//! committing" setting.

use super::ops::{JobSpec, start_job};
use crate::effect::Effect;
use crate::msg::CommitMsg;
use crate::state::AppState;
use crate::state::commit::{commit_blocked, commit_merge_blocked, pushes_after, unencodable_text};
use glowfly_core::undo::UndoKind;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: CommitMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        CommitMsg::Commit => commit(st, false, now, fx),
        CommitMsg::StageAllAndCommit => commit(st, true, now, fx),
        CommitMsg::PrefillAmend => {
            // Pre-fill from HEAD's message, when the user hasn't written anything yet.
            if st.session.draft.is_blank()
                && let Some(m) = st.snap.as_ref().and_then(|s| s.head_message.clone())
            {
                st.session.draft.fill_from(&m);
            }
        }
        CommitMsg::AppendToDescription(text) => {
            if !st.session.draft.description.trim().is_empty() {
                st.session.draft.description.push_str("\n\n");
            }
            st.session.draft.description.push_str(&text);
        }
        CommitMsg::SetPushAfterCommit(on) if st.settings.push_after_commit != on => {
            st.settings.push_after_commit = on;
            fx.extend(super::prefs::save_settings(st));
        }
        CommitMsg::SetPushAfterCommit(_) => {}
        CommitMsg::CommitMerge => {
            if let Some(why) = commit_merge_blocked(st) {
                st.toast(false, why, now);
                return;
            }
            if st.session.draft.is_blank()
                && let Some(m) = st.snap.as_ref().and_then(|s| s.merge_message.clone())
            {
                st.session.draft.fill_merge(&m);
            }
            commit(st, false, now, fx);
        }
    }
}

/// Commits the draft, after `git add --all` when `stage_all`, its message encoded into the
/// repository's commit encoding. Refused with a toast (not the log, so a keypress doesn't pop
/// it open) when the draft or repository isn't ready or the encoding can't hold the message.
pub(super) fn commit(st: &mut AppState, stage_all: bool, now: Instant, fx: &mut Vec<Effect>) {
    if let Some(why) = commit_blocked(st, stage_all) {
        st.toast(false, why, now);
        return;
    }
    let Some(snap) = &st.snap else { return };
    // git writes the file's bytes as they are and labels them with i18n.commitEncoding.
    let bytes = match snap.commit_encoding.encode(&(st.session.draft.message() + "\n")) {
        Ok(b) => b.into_owned(),
        Err(e) => {
            let why = unencodable_text(st.settings.lang(), &e);
            st.toast(false, why, now);
            return;
        }
    };
    let file = snap.git_dir.join("GLOWFLY_COMMIT_EDITMSG");
    let cmds = st.session.draft.commit_cmds(&file, stage_all);
    let amend = st.session.draft.amend;
    let label = if amend {
        "Amend"
    } else if stage_all {
        "Stage all and commit"
    } else {
        "Commit"
    };
    let spec = JobSpec {
        clear_commit: true,
        then_push: pushes_after(st),
        write_file: Some((file, bytes)),
        record: Some(UndoKind::Commit { amend }),
        ..JobSpec::new(label.into(), cmds)
    };
    start_job(st, spec, fx);
}

/// After a full load: the merge's message (`merge_message`), then the commit template.
pub(super) fn prefill(st: &mut AppState) {
    merge_message(st);
    apply_template(st);
}

/// Fills a blank, non-amend draft from MERGE_MSG when a merge, cherry-pick or revert stops (or
/// swaps in a new message while the old one is untouched), and clears the untouched message
/// once the operation ends any other way than a Glowfly commit. A draft the user wrote is never replaced.
fn merge_message(st: &mut AppState) {
    let m = st.snap.as_ref().and_then(|s| s.merge_message.clone());
    let d = &mut st.session.draft;
    match (m, d.merge_msg.clone()) {
        (Some(m), held) if !d.amend => {
            let fresh = held.is_none() && d.is_blank();
            let changed = held.as_ref().is_some_and(|h| *h != m) && d.is_untouched_merge_msg();
            if fresh || changed {
                d.fill_merge(&m);
            }
        }
        (None, Some(_)) => {
            if d.is_untouched_merge_msg() {
                d.summary.clear();
                d.description.clear();
            }
            d.merge_msg = None;
        }
        _ => {}
    }
}

/// Prefills an empty, non-amend draft from the repository's commit template once (a draft the
/// user cleared stays empty until the next commit), and swaps in a changed template while the
/// draft still holds the old one untouched.
pub(super) fn apply_template(st: &mut AppState) {
    let Some(t) = st.snap.as_ref().and_then(|s| s.commit_template.as_ref()) else { return };
    let d = &mut st.session.draft;
    if d.amend {
        return;
    }
    let fresh = d.template.is_none() && d.is_empty();
    let changed = d.is_untouched_template() && d.template.as_ref() != Some(t);
    if fresh || changed {
        d.apply_template(t);
    }
}
