//! A Finish stopped at a merge conflict, remembered for its tab with the steps still to run,
//! and the texts of the window banner, the Git Flow panel row, the in-progress banner's line and
//! the palette entry that offer to continue it (update/gitflow_resume.rs keeps it current).

use super::AppState;
use trellis_core::gitflow::{FinishResume, ResumeGone};

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
pub fn ready_text(r: &FinishResume) -> String {
    format!("{} can go on: the merge into {} is committed. Still to do: {}.", r.plan.label(), r.into, rest_text(r))
}

/// The in-progress banner's line (and the panel row) while the merge is in progress.
pub fn waiting_text(r: &FinishResume) -> String {
    format!("This merge belongs to {}. Commit it, then Continue finish runs: {}.", r.plan.label(), rest_text(r))
}

/// The log line when the steps no longer apply.
pub fn gone_text(r: &FinishResume, why: &ResumeGone) -> String {
    format!("{} can't continue: {}. Its remaining steps were dropped.", r.plan.label(), why.message())
}

/// Continue finish before the merge is committed.
pub fn not_committed_text(into: &str) -> String {
    format!("Commit the merge into {into} first, then Continue finish.")
}

/// "1 step left" / "4 steps left".
pub fn steps_left(n: usize) -> String {
    if n == 1 { "1 step left".into() } else { format!("{n} steps left") }
}

/// The stopped Finish when Continue finish can run (its merge is committed).
pub fn offered(st: &AppState) -> Option<&StoppedFinish> {
    st.gitflow_resume.as_ref().filter(|r| r.ready)
}

/// The palette entry's label: "Continue finishing Git Flow release 1.2.0".
pub fn palette_label(r: &FinishResume) -> String {
    format!("Continue finishing Git Flow {} {}", r.plan.kind.noun(), r.plan.short)
}
