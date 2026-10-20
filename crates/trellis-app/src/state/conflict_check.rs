//! Predicted merge conflicts of the active tab's checked-out branch: the last automatic
//! result, the one check allowed to run at a time with one queued behind it, and the texts the
//! toolbar chip, its popover and the report dialog show, worked out without drawing.

use crate::conflict_check::CheckRequest;
use git2::RepositoryState;
use trellis_core::conflict_check::{MergeTarget, Prediction, TargetKind, target_for};
use trellis_core::i18n::Lang;
use trellis_core::repo::Snapshot;
use trellis_core::text::printable;

#[cfg(test)]
mod tests;

/// The most paths a report lists before "and K more".
pub const SHOWN_FILES: usize = 50;

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
    /// F5 asked for the last result to be checked again (it stays shown until then). The next
    /// check to start answers it, so one that came while a check ran is still owed afterwards.
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
        self.stale = false;
        Some(self.generation)
    }

    /// Takes a reply to a check started at `generation`; ignored when cleared or parked since.
    pub fn finish(&mut self, generation: u64, report: ConflictReport) -> Finished {
        if generation != self.generation {
            return Finished::default();
        }
        self.running = false;
        let queued = std::mem::take(&mut self.queued);
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
            // The check dropped here may have been answering an F5: ask again on return.
            stale: self.stale || self.running,
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

/// The one sentence a result gets (dialog, toast, activity log).
pub fn headline(t: Lang, report: &ConflictReport) -> String {
    let (ours, theirs) = (report.request.branch.as_str(), report.request.target.name.as_str());
    let names: [(&str, &dyn std::fmt::Display); 2] = [("ours", &ours), ("theirs", &theirs)];
    match &report.result {
        Ok(Prediction::Conflicts(paths)) if paths.is_empty() => t.trf("conflict_check.headline.conflicts", &names),
        Ok(Prediction::Conflicts(paths)) => t.trn("conflict_check.headline.conflicts_files", paths.len(), &names),
        Ok(Prediction::Clean) => t.trf("conflict_check.headline.clean", &names),
        Ok(Prediction::UpToDate) => t.trf("conflict_check.headline.up_to_date", &names),
        Ok(Prediction::FastForward) => t.trf("conflict_check.headline.fast_forward", &names),
        Err(e) => {
            let error = printable(e);
            t.trf("conflict_check.headline.failed", &[("ours", &ours), ("theirs", &theirs), ("error", &error)])
        }
    }
}

/// The toolbar chip's hover text: what it warns about, where the target came from (`remote`:
/// it is a remote-tracking branch) and what a click shows. Any result gets a fitting text,
/// though the chip is drawn only for conflicts.
pub fn chip_tooltip(t: Lang, report: &ConflictReport, remote: bool) -> String {
    let (ours, theirs) = (report.request.branch.as_str(), report.request.target.name.as_str());
    let names: [(&str, &dyn std::fmt::Display); 2] = [("ours", &ours), ("theirs", &theirs)];
    let listed = matches!(&report.result, Ok(Prediction::Conflicts(paths)) if !paths.is_empty());
    let mut lines = vec![match &report.result {
        Ok(Prediction::Conflicts(paths)) if !paths.is_empty() => t.trn("conflict_check.chip.tip_files", paths.len(), &names),
        Ok(Prediction::Conflicts(_)) => t.trf("conflict_check.chip.tip", &names),
        Ok(Prediction::Clean | Prediction::UpToDate | Prediction::FastForward) | Err(_) => headline(t, report),
    }];
    lines.extend(target_note(t, &report.request.target, ours, remote));
    lines.push(if listed { t.tr("conflict_check.chip.click.files") } else { t.tr("conflict_check.chip.click.details") }.into());
    lines.join("\n")
}

/// Where the target came from, and for a remote one that its last fetched state counts. None
/// for a branch the user picked.
pub fn target_note(t: Lang, target: &MergeTarget, branch: &str, remote: bool) -> Option<String> {
    let args: [(&str, &dyn std::fmt::Display); 2] = [("name", &target.name), ("branch", &branch)];
    let note = match (target.kind, remote) {
        (TargetKind::Upstream, false) => t.trf("conflict_check.note.upstream", &args),
        (TargetKind::Upstream, true) => t.trf("conflict_check.note.upstream_fetched", &args),
        (TargetKind::DefaultBranch, false) => t.trf("conflict_check.note.default", &args),
        (TargetKind::DefaultBranch, true) => t.trf("conflict_check.note.default_fetched", &args),
        (TargetKind::Chosen, _) => return None,
    };
    Some(note)
}

/// The paths a report lists (at most SHOWN_FILES, made printable), and "and K more" for the rest.
pub fn file_list(t: Lang, paths: &[String]) -> (Vec<String>, Option<String>) {
    let shown = paths.iter().take(SHOWN_FILES).map(|p| printable(p)).collect();
    let more = (paths.len() > SHOWN_FILES).then(|| t.trn("conflict_check.more", paths.len() - SHOWN_FILES, &[]));
    (shown, more)
}

/// The Merge and Rebase actions a result offers: only for a real merge (Conflicts, Clean).
pub fn offers_actions(report: &ConflictReport) -> bool {
    matches!(report.result, Ok(Prediction::Conflicts(_) | Prediction::Clean))
}

/// Whether `report` was made for the branch and commit checked out in `snap`. Merge and Rebase
/// act on whatever is checked out, so a report made before a checkout or commit offers neither.
pub fn for_head(report: &ConflictReport, snap: &Snapshot) -> bool {
    snap.head_branch.as_deref() == Some(report.request.branch.as_str()) && snap.head_oid == Some(report.request.ours)
}
