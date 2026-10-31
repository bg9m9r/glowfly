//! A Finish stopped at a merge conflict, remembered for its tab with the steps still to run,
//! and the texts of the window banner, the Git Flow panel row, the in-progress banner's line and
//! the palette entry that offer to continue it (update/gitflow_resume.rs keeps it current). The
//! step texts come from core in English and fill the translated sentences.

use super::AppState;
use glowfly_core::gitflow::{FinishResume, ResumeGone};
use glowfly_core::i18n::Lang;

/// The remembered steps and what the last snapshot showed. Views read only `ready` and
/// `seen_merge`, so no history search runs per frame.
#[derive(Clone, Debug, PartialEq)]
pub struct StoppedFinish {
    pub resume: FinishResume,
    /// A snapshot with the merge in progress was seen (so a later clean one at the old tip is an
    /// abort, not a stale snapshot).
    pub seen_merge: bool,
    /// The merge is committed: Continue finish is offered.
    pub ready: bool,
}

/// The steps still to run: "tag main as v1.2.0, check out develop, …".
pub fn rest_text(r: &FinishResume) -> String {
    r.plan.steps.iter().map(|s| s.what.as_str()).collect::<Vec<_>>().join(", ")
}

/// The banner and panel row once the merge is committed.
pub fn ready_text(t: Lang, r: &FinishResume) -> String {
    t.trf("gitflow.resume.ready", &[("into", &r.into), ("branch", &r.plan.branch), ("steps", &rest_text(r))])
}

/// The in-progress banner's line (and the panel row) while the merge is in progress.
pub fn waiting_text(t: Lang, r: &FinishResume) -> String {
    t.trf("gitflow.resume.waiting", &[("branch", &r.plan.branch), ("steps", &rest_text(r))])
}

/// The log line when the steps no longer apply.
pub fn gone_text(t: Lang, r: &FinishResume, why: &ResumeGone) -> String {
    t.trf("gitflow.resume.dropped", &[("branch", &r.plan.branch), ("why", &gone_reason(t, why))])
}

/// Why the steps no longer apply, to fill `gone_text` (no full stop).
pub fn gone_reason(t: Lang, why: &ResumeGone) -> String {
    match why {
        ResumeGone::NotSetUp => t.tr("gitflow.resume.why.not_set_up").to_owned(),
        ResumeGone::SettingsChanged => t.tr("gitflow.resume.why.settings_changed").to_owned(),
        ResumeGone::BranchGone(b) => t.trf("gitflow.resume.why.branch_gone", &[("branch", b)]),
        ResumeGone::BaseGone(b) => t.trf("gitflow.resume.why.base_gone", &[("branch", b)]),
        ResumeGone::BranchMoved(b) => t.trf("gitflow.resume.why.branch_moved", &[("branch", b)]),
        ResumeGone::Aborted(into) => t.trf("gitflow.resume.why.aborted", &[("branch", into)]),
    }
}

/// Continue finish before the merge is committed.
pub fn not_committed_text(t: Lang, into: &str) -> String {
    t.trf("gitflow.resume.not_committed", &[("into", &into)])
}

/// "1 step left" / "4 steps left".
pub fn steps_left(t: Lang, n: usize) -> String {
    t.trn("gitflow.palette.steps_left", n, &[])
}

/// The stopped Finish when Continue finish can run (its merge is committed).
pub fn offered(st: &AppState) -> Option<&StoppedFinish> {
    st.session.gitflow_resume.as_ref().filter(|r| r.ready)
}

/// The palette entry's label: "Continue finishing release/1.2.0".
pub fn palette_label(t: Lang, r: &FinishResume) -> String {
    t.trf("gitflow.palette.continue", &[("branch", &r.plan.branch)])
}
