//! Predicted merge conflicts of the active tab's checked-out branch: the last automatic
//! result, the one check allowed to run at a time with one queued behind it, and the texts the
//! toolbar chip, its popover and the report dialog show, worked out without drawing.

use crate::conflict_check::CheckRequest;
use git2::RepositoryState;
use trellis_core::conflict_check::{MergeTarget, Prediction, TargetKind, target_for};
use trellis_core::repo::Snapshot;
use trellis_core::text::printable;

#[cfg(test)]
mod tests;

/// The most paths a report lists before "and K more".
pub const SHOWN_FILES: usize = 50;
/// Why the head branch menu's check is off.
pub const NO_TARGET: &str = "No upstream with new commits or default branch to compare with.";
pub use crate::state::rebase::JOB_RUNNING;

/// A finished check.
#[derive(Clone, Debug, PartialEq)]
pub struct ConflictReport {
    pub request: CheckRequest,
    pub result: Result<Prediction, String>,
}

/// The active tab's automatic checks (parked with the tab).
#[derive(Default)]
pub struct ConflictChecks {
    last: Option<ConflictReport>,
    /// F5 asked for the last result to be checked again (it stays shown until then).
    stale: bool,
    running: bool,
    /// A check was asked for while one ran: ask again when it returns.
    queued: bool,
    /// Bumped by `clear` and parking: replies to checks started before are ignored.
    generation: u64,
    /// The error last logged, so one that repeats (an old git) is logged once.
    logged_error: Option<String>,
}

/// What `ConflictChecks::finish` did with a reply.
#[derive(Debug, Default, PartialEq)]
pub struct Finished {
    /// The reply was current and is now the last result.
    pub accepted: bool,
    /// A check was asked for meanwhile: ask again now.
    pub queued: bool,
    /// The reply's error, when it differs from the one logged last.
    pub new_error: Option<String>,
}

impl ConflictChecks {
    /// Whether `req` needs a check: nothing is known for it, or F5 asked again.
    pub fn wants(&self, req: &CheckRequest) -> bool {
        self.stale || self.last.as_ref().is_none_or(|l| l.request != *req)
    }

    /// Starts a check and returns the generation it must carry; None while one is running (it is
    /// asked for again when that one returns).
    pub fn start(&mut self) -> Option<u64> {
        if self.running {
            self.queued = true;
            return None;
        }
        self.running = true;
        Some(self.generation)
    }

    /// Takes a reply to a check started at `generation`; ignored when cleared or parked since.
    pub fn finish(&mut self, generation: u64, report: ConflictReport) -> Finished {
        if generation != self.generation {
            return Finished::default();
        }
        self.running = false;
        let queued = std::mem::take(&mut self.queued);
        // An F5 that came while this check ran is still owed a check.
        self.stale = self.stale && queued;
        let new_error = match &report.result {
            Err(e) if self.logged_error.as_deref() == Some(e) => None,
            Err(e) => {
                self.logged_error = Some(e.clone());
                Some(e.clone())
            }
            Ok(_) => {
                self.logged_error = None;
                None
            }
        };
        self.last = Some(report);
        Finished { accepted: true, queued, new_error }
    }

    /// Keeps a result the user asked for (Check again) as the last one.
    pub fn store_manual(&mut self, report: ConflictReport) {
        self.last = Some(report);
        self.stale = false;
    }

    /// F5: check again, still showing the last result until the new one arrives.
    pub fn invalidate(&mut self) {
        self.stale = true;
    }

    /// Whether there is nothing to clear.
    pub fn is_empty(&self) -> bool {
        self.last.is_none() && !self.running && !self.queued && !self.stale && self.logged_error.is_none()
    }

    /// Drops everything; replies still in flight are ignored when they arrive.
    pub fn clear(&mut self) {
        self.last = None;
        self.stale = false;
        self.running = false;
        self.queued = false;
        self.logged_error = None;
        self.generation += 1;
    }

    /// For parking: keeps the result, drops the check in flight (its reply is dropped, and the
    /// tab asks again on return). Both the parked and the left-behind value get a newer
    /// generation, so a reply that slips through is ignored.
    pub fn parked(&mut self) -> ConflictChecks {
        self.generation += 1;
        let parked = ConflictChecks {
            last: self.last.take(),
            stale: self.stale,
            running: false,
            queued: false,
            generation: self.generation,
            logged_error: self.logged_error.take(),
        };
        self.clear();
        parked
    }

    /// The last result when it is for `current` (the inputs on screen).
    pub fn shown(&self, current: Option<&CheckRequest>) -> Option<&ConflictReport> {
        self.last.as_ref().filter(|l| Some(&l.request) == current)
    }
}

/// The automatic check's inputs for `snap`: a checked-out branch with a commit, no merge,
/// rebase or cherry-pick in progress, and a target to compare with.
pub fn current_request(snap: &Snapshot) -> Option<CheckRequest> {
    let branch = snap.head_branch.clone()?;
    let ours = snap.head_oid?;
    if snap.state != RepositoryState::Clean {
        return None;
    }
    Some(CheckRequest { branch, ours, target: target_for(snap)? })
}

/// Whether `target` is a remote-tracking branch of `snap` (not a local branch).
pub fn is_remote(snap: &Snapshot, target: &MergeTarget) -> bool {
    !snap.locals.iter().any(|b| b.name == target.name)
        && snap.remotes.iter().any(|(r, bs)| bs.iter().any(|(b, _)| format!("{r}/{b}") == target.name))
}

/// "1 file", "3 files".
pub fn file_count(n: usize) -> String {
    if n == 1 { "1 file".into() } else { format!("{n} files") }
}

/// The one sentence a result gets (dialog, toast, activity log).
pub fn headline(report: &ConflictReport) -> String {
    let (ours, theirs) = (report.request.branch.as_str(), report.request.target.name.as_str());
    match &report.result {
        Ok(Prediction::Conflicts(paths)) if paths.is_empty() => format!("Merging {theirs} into {ours} would stop with conflicts."),
        Ok(Prediction::Conflicts(paths)) => {
            format!("Merging {theirs} into {ours} would stop with conflicts in {}:", file_count(paths.len()))
        }
        Ok(Prediction::Clean) => format!("{theirs} merges into {ours} without conflicts."),
        Ok(Prediction::UpToDate) => format!("{ours} already contains everything on {theirs}."),
        Ok(Prediction::FastForward) => format!("{ours} has no commits that {theirs} lacks, so merging is a fast-forward."),
        Err(e) => format!("Couldn't check {ours} against {theirs}: {}", printable(e)),
    }
}

/// The toolbar chip's hover text (for a Conflicts result).
pub fn chip_tooltip(report: &ConflictReport) -> String {
    let (ours, theirs) = (report.request.branch.as_str(), report.request.target.name.as_str());
    let n = match &report.result {
        Ok(Prediction::Conflicts(paths)) if !paths.is_empty() => format!(" in {}", file_count(paths.len())),
        _ => String::new(),
    };
    format!("Merging {theirs} into {ours} would stop with conflicts{n}. Click for details.")
}

/// Where the target came from, and for a remote one that its last fetched state counts. None
/// for a branch the user picked.
pub fn target_note(target: &MergeTarget, branch: &str, remote: bool) -> Option<String> {
    let name = target.name.as_str();
    let mut note = match target.kind {
        TargetKind::Upstream => format!("{name} is the upstream of {branch}."),
        TargetKind::DefaultBranch => format!("{name} is this repository's default branch."),
        TargetKind::Chosen => return None,
    };
    if remote {
        note.push_str(&format!(" Compared with the last fetched state of {name}."));
    }
    Some(note)
}

/// The paths a report lists (at most SHOWN_FILES, made printable), and "and K more" for the rest.
pub fn file_list(paths: &[String]) -> (Vec<String>, Option<String>) {
    let shown = paths.iter().take(SHOWN_FILES).map(|p| printable(p)).collect();
    let more = (paths.len() > SHOWN_FILES).then(|| format!("and {} more", paths.len() - SHOWN_FILES));
    (shown, more)
}

/// The Merge and Rebase actions a result offers: only for a real merge (Conflicts, Clean).
pub fn offers_actions(report: &ConflictReport) -> bool {
    matches!(report.result, Ok(Prediction::Conflicts(_) | Prediction::Clean))
}
