//! Whether the commit box can commit now, and if not the first missing step it names: another
//! job, an amend that can't run, conflicts, staging, the template, the summary.

use super::super::AppState;
use super::CommitDraft;
use super::stop::{amend_blocked_in, edit_stop};
use git2::RepositoryState;

/// Why committing (or staging everything and committing) can't start now; None when it can.
/// It names only the first missing step: staging (a clean tree, or nothing staged), then the
/// summary. While merging, every conflict must be resolved and an empty Staged list still
/// commits (a merge whose result equals HEAD); during a merge, cherry-pick or revert an amend
/// can't run.
pub fn commit_blocked(st: &AppState, stage_all: bool) -> Option<String> {
    let t = st.settings.lang();
    let Some(snap) = &st.snap else { return Some(t.tr("commit.blocked.no_repo").to_owned()) };
    let d = &st.session.draft;
    let merging = snap.state == RepositoryState::Merge;
    let conflicted = snap.unstaged.iter().any(|f| f.status == '!');
    let why = if st.job.is_some() {
        Some(t.tr("commit.blocked.job_running"))
    } else if let Some(why) = amend_blocked_in(t, snap).filter(|_| d.amend) {
        Some(why)
    } else if merging && conflicted {
        Some(t.tr("commit.blocked.merge_conflicts"))
    } else if let Some(step) = staging_step(st, stage_all, conflicted) {
        return Some(step);
    } else if d.is_untouched_template() {
        Some(t.tr("commit.blocked.untouched_template"))
    } else if d.summary.trim().is_empty() {
        Some(t.tr("commit.blocked.no_summary"))
    } else if d.amend
        && edit_stop(snap).is_some()
        && snap.staged.is_empty()
        && (!stage_all || snap.unstaged.is_empty())
        && same_as_head(d, snap)
    {
        // Amend and continue would change nothing: Continue does that.
        Some(t.tr("commit.blocked.nothing_to_amend"))
    } else {
        None
    };
    why.map(str::to_owned)
}

/// What is left to stage before committing (not for a merge or an amend, which commit with
/// nothing staged): the tree is clean, nothing is staged (naming the Stage all and commit key
/// when it could run), or staging everything would mark conflicted files resolved.
fn staging_step(st: &AppState, stage_all: bool, conflicted: bool) -> Option<String> {
    let t = st.settings.lang();
    let snap = st.snap.as_ref()?;
    if stage_all && conflicted {
        // Staging a conflicted file marks it resolved.
        return Some(t.tr("commit.blocked.conflicts").to_owned());
    }
    if snap.state == RepositoryState::Merge || st.session.draft.amend || !snap.staged.is_empty() {
        return None;
    }
    if snap.unstaged.is_empty() {
        return Some(t.tr("commit.blocked.clean").to_owned());
    }
    if stage_all {
        return None;
    }
    match st.keymap.hint(crate::keymap::BindingId::StageAllAndCommit).filter(|_| !conflicted) {
        Some(key) => Some(t.trf("commit.blocked.nothing_staged_key", &[("key", &key)])),
        None => Some(t.tr("commit.blocked.nothing_staged").to_owned()),
    }
}

/// Whether the draft's message is HEAD's (as the commit box splits it).
fn same_as_head(d: &CommitDraft, snap: &glowfly_core::repo::Snapshot) -> bool {
    let mut head = CommitDraft::default();
    head.fill_from(snap.head_message.as_deref().unwrap_or_default());
    head.message() == d.message()
}

/// Why the merge banner's Commit merge can't run now: conflicts left, another job, an amend.
pub fn commit_merge_blocked(st: &AppState) -> Option<String> {
    let t = st.settings.lang();
    let conflicted = st.snap.as_ref().map_or(0, |s| s.unstaged.iter().filter(|f| f.status == '!').count());
    if conflicted > 0 {
        Some(t.trn("details.banner.commit_merge.conflicts", conflicted, &[]))
    } else if st.job.is_some() {
        Some(t.tr("commit.blocked.job_running").to_owned())
    } else if st.session.draft.amend {
        Some(t.tr("commit.blocked.amend_merge").to_owned())
    } else {
        None
    }
}

/// The commit button's text: Amend and continue (or Commit and continue) at a rebase stopped to
/// edit; else plain "Commit" while it can't commit (not for an amend or a merge, which name
/// their action), the caption saying what is missing; else `CommitDraft::button_label`.
pub fn commit_label(st: &AppState) -> String {
    let t = st.settings.lang();
    let d = &st.session.draft;
    let Some(snap) = &st.snap else { return t.tr("commit.button.plain").to_owned() };
    let merging = snap.state == RepositoryState::Merge;
    if edit_stop(snap).is_some() {
        d.continue_label(t)
    } else if commit_blocked(st, false).is_some() && !d.amend && !merging {
        t.tr("commit.button.plain").to_owned()
    } else {
        d.button_label(t, snap.staged.len(), super::pushes_after(st), merging)
    }
}
