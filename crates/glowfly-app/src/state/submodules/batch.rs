//! Updating several submodules in one job: Initialize all, Update all (which asks before leaving
//! commits behind) and the update after a checkout, pull, merge, rebase or reset (which leaves
//! out any submodule where it would), with their texts, and what this session remembers about
//! them.

use super::{count_commits, named_commits};
use crate::submodule::Batch;
use glowfly_core::repo::SubmoduleInfo;
use glowfly_core::submodule_ops::batch_update_cmds;
use std::path::PathBuf;

/// What the SUBMODULES section remembers for this session (not saved, not per tab).
#[derive(Default)]
pub struct SubmoduleSession {
    /// The workdir and label of a started job that, once it succeeds, updates the submodules.
    pub after_job: Option<(PathBuf, String)>,
    /// Resolved workdirs whose uninitialized submodules were offered (or initialized) this session.
    pub offered: Vec<PathBuf>,
}

/// What a batch does with a list of submodules.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchPlan {
    /// The paths to update.
    pub targets: Vec<String>,
    /// The targets are every submodule listed, so no pathspecs are needed.
    pub all: bool,
    /// Update all: targets whose update would leave commits behind (asked about first).
    pub stranding: Vec<SubmoduleInfo>,
    /// After a job: left out, because updating would leave commits behind.
    pub skipped: Vec<SubmoduleInfo>,
}

/// Has a URL and no checkout (uninitialized, or its folder is gone).
fn needs_init(sm: &SubmoduleInfo) -> bool {
    sm.url.is_some() && !sm.is_initialized()
}

/// Has a URL and is checked out at another commit than the recorded one.
fn needs_update(sm: &SubmoduleInfo) -> bool {
    sm.url.is_some() && sm.is_out_of_sync()
}

pub fn plan(batch: Batch, list: &[SubmoduleInfo]) -> BatchPlan {
    let mut p = BatchPlan::default();
    for sm in list {
        let wanted = match batch {
            Batch::InitAll => needs_init(sm),
            Batch::UpdateAll | Batch::Auto => needs_init(sm) || needs_update(sm),
        };
        if !wanted {
            continue;
        }
        if !sm.left_behind.is_empty() {
            match batch {
                Batch::Auto => {
                    p.skipped.push(sm.clone());
                    continue;
                }
                Batch::UpdateAll => p.stranding.push(sm.clone()),
                Batch::InitAll => {}
            }
        }
        p.targets.push(sm.path.clone());
    }
    p.all = !p.targets.is_empty() && p.targets.len() == list.len();
    p
}

/// The job's commands. The update after a job leaves `--checkout` out, so each submodule's
/// `submodule.<name>.update` setting is honoured; the menu's batches check out the recorded
/// commits, as their labels say.
pub fn cmds(batch: Batch, plan: &BatchPlan) -> Vec<Vec<String>> {
    batch_update_cmds((!plan.all).then_some(plan.targets.as_slice()), batch != Batch::Auto)
}

/// The job's label, in the toast and the activity log.
pub fn job_label(batch: Batch) -> &'static str {
    match batch {
        Batch::InitAll => "Initialize submodules",
        Batch::UpdateAll | Batch::Auto => "Update submodules",
    }
}

pub fn init_all_blocked(list: &[SubmoduleInfo]) -> Option<&'static str> {
    (!list.iter().any(needs_init)).then_some("Every submodule with a URL is checked out already.")
}

pub fn update_all_blocked(list: &[SubmoduleInfo]) -> Option<&'static str> {
    (!list.iter().any(|s| needs_init(s) || needs_update(s))).then_some("Every submodule with a URL is at its recorded commit already.")
}

/// What is logged when a batch finds nothing to do.
pub fn nothing_to_do(batch: Batch) -> &'static str {
    match batch {
        Batch::InitAll => "Every submodule is checked out already.",
        Batch::UpdateAll | Batch::Auto => "Every submodule is at its recorded commit already.",
    }
}

/// The confirmation's button when Update all would leave commits behind.
pub const CONFIRM_ALL_BUTTON: &str = "Update all and leave commits behind";

/// The confirmation before Update all leaves the commits of `stranding` behind, naming them.
pub fn confirm_all_body(stranding: &[SubmoduleInfo]) -> String {
    let mut out = String::from(
        "Updating checks out the commits this repository records. No branch or tag inside these submodules contains the \
         commits checked out now, so updating leaves them behind:",
    );
    for sm in stranding {
        out.push_str(&format!("\n\n{}: {}", sm.path, count_commits(sm.left_behind.len())));
        out.push_str(&named_commits(&sm.left_behind));
    }
    out.push_str(
        "\n\nAfterwards only each submodule's reflog finds them, until it expires. To keep them, cancel, open the submodule \
         and create a branch.",
    );
    out
}

/// Logged for a submodule the update after a job left out.
pub fn skipped_line(sm: &SubmoduleInfo) -> String {
    format!(
        "Submodule {} was left as it is: updating it would leave {} on no branch or tag behind. Update to recorded commit in \
         its menu asks before doing that.",
        sm.path,
        count_commits(sm.left_behind.len())
    )
}

/// The toast offering to initialize `n` submodules of the repository called `name`.
pub fn offer_text(name: &str, n: usize) -> String {
    if n == 1 {
        format!("1 submodule in {name} isn't checked out yet.")
    } else {
        format!("{n} submodules in {name} aren't checked out yet.")
    }
}

#[cfg(test)]
#[path = "batch_tests.rs"]
mod tests;
