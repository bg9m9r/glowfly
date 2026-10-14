//! The commit box: committing (optionally staging everything first and pushing afterwards),
//! prefilling the draft from HEAD for an amend or from the repository's commit template, and
//! the "Push after committing" setting.

use super::ops::{JobSpec, start_job};
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::commit::{commit_blocked, pushes_after};
use std::time::Instant;
use trellis_core::undo::UndoKind;

pub(super) fn handle(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        Msg::Commit => commit(st, false, now, fx),
        Msg::StageAllAndCommit => commit(st, true, now, fx),
        Msg::PrefillAmend => {
            // Pre-fill from HEAD's message, when the user hasn't written anything yet.
            if st.draft.is_blank()
                && let Some(m) = st.snap.as_ref().and_then(|s| s.head_message.clone())
            {
                st.draft.fill_from(&m);
            }
        }
        Msg::AppendToDescription(text) => {
            if !st.draft.description.trim().is_empty() {
                st.draft.description.push_str("\n\n");
            }
            st.draft.description.push_str(&text);
        }
        Msg::SetPushAfterCommit(on) if st.settings.push_after_commit != on => {
            st.settings.push_after_commit = on;
            fx.push(super::prefs::save_settings(st));
        }
        _ => {}
    }
}

/// Commits the draft, after `git add --all` when `stage_all`. Refused with a toast (not the
/// log, so a keypress doesn't pop it open) when the draft or repository isn't ready.
fn commit(st: &mut AppState, stage_all: bool, now: Instant, fx: &mut Vec<Effect>) {
    if let Some(why) = commit_blocked(st, stage_all) {
        st.toast(false, why, now);
        return;
    }
    let Some(snap) = &st.snap else { return };
    let file = snap.git_dir.join("TRELLIS_COMMIT_EDITMSG");
    let cmds = st.draft.commit_cmds(&file, stage_all);
    let amend = st.draft.amend;
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
        write_file: Some((file, st.draft.message() + "\n")),
        record: Some(UndoKind::Commit { amend }),
        ..JobSpec::new(label.into(), cmds)
    };
    start_job(st, spec, fx);
}

/// Prefills an empty, non-amend draft from the repository's commit template once (a draft the
/// user cleared stays empty until the next commit), and swaps in a changed template while the
/// draft still holds the old one untouched.
pub(super) fn apply_template(st: &mut AppState) {
    let Some(t) = st.snap.as_ref().and_then(|s| s.commit_template.as_ref()) else { return };
    let d = &mut st.draft;
    if d.amend {
        return;
    }
    let fresh = d.template.is_none() && d.is_empty();
    let changed = d.is_untouched_template() && d.template.as_ref() != Some(t);
    if fresh || changed {
        d.apply_template(t);
    }
}
