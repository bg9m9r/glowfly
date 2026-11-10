//! The six rules every step of an exploration session must keep, each a check that returns what
//! is wrong. They hold for any sequence of actions, so none of them needs to know what the
//! session did.

use super::super::find::Region;
use super::super::fixture::git_output;
use super::super::harness::Journey;
use super::probe::Reading;
use super::reach;
use super::shadow::Shadow;
use glowfly_model::state::Selection;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Rule {
    NoCrash,
    Sound,
    NoLostWork,
    ScreenAgrees,
    UndoReturns,
    ErrorsReported,
}

impl Rule {
    pub fn name(self) -> &'static str {
        match self {
            Rule::NoCrash => "no-crash",
            Rule::Sound => "sound",
            Rule::NoLostWork => "no-lost-work",
            Rule::ScreenAgrees => "screen-agrees",
            Rule::UndoReturns => "undo-returns",
            Rule::ErrorsReported => "errors-reported",
        }
    }
}

/// A rule that failed, at which step, and how.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Broken {
    pub rule: Rule,
    pub step: usize,
    pub detail: String,
}

/// Rule 1: no effect's worker panicked.
pub(super) fn no_crash(j: &Journey) -> Result<(), String> {
    let panicked = j.app().panicked_effects();
    if panicked.is_empty() { Ok(()) } else { Err(format!("effects panicked: {panicked:?}")) }
}

/// Rule 2: the repository passes `git fsck`.
pub(super) fn sound(repo: &Path) -> Result<(), String> {
    let (ok, out) = git_output(repo, &["fsck", "--no-progress"]);
    if ok { Ok(()) } else { Err(format!("git fsck failed:\n{out}")) }
}

/// Rule 3: every marker and commit summary the session wrote is still reachable.
pub(super) fn no_lost_work(repo: &Path, name: &str, shadow: &Shadow) -> Result<(), String> {
    let missing = reach::missing(repo, shadow.live(name), shadow.summaries(name));
    if missing.is_empty() { Ok(()) } else { Err(format!("work lost from {name}: {missing:?}")) }
}

/// Rule 4: the screen shows what the repository holds (`r`, read from disk just now). The head
/// and branch names are read off the widgets' names. The changed-file set and the stash count
/// have no reliable names on screen, so they are compared with the active tab's snapshot, which
/// is what the lists on screen are drawn from.
pub(super) fn screen_agrees(j: &Journey, r: &Reading) -> Result<(), String> {
    let mut wrong = Vec::new();
    let sidebar = j.labels(Region::Sidebar);
    if let Some(branch) = &r.branch {
        let head = format!("{branch}, checked out");
        if !sidebar.contains(&head) {
            wrong.push(format!("the sidebar doesn't say {head:?}"));
        }
        let caption = format!("Branch: {branch}");
        let comma = format!("{caption},");
        if !j.labels(Region::Toolbar).iter().any(|l| *l == caption || l.starts_with(&comma)) {
            wrong.push(format!("the toolbar doesn't say {caption:?}"));
        }
    }
    // The sidebar draws only the rows in view, so with many branches some are off screen: then
    // only the checked-out branch (checked above) is held to be there.
    if r.branches.len() <= 8 {
        for name in r.branches.keys() {
            let head = format!("{name}, checked out");
            if !sidebar.iter().any(|l| l == name || *l == head) {
                wrong.push(format!("the sidebar doesn't list branch {name:?}"));
            }
        }
    }
    let st = j.state();
    // Details draws "View conflicts" only while a file is still conflicted, and the conflict view,
    // once open, takes the Details panel's place.
    let details_shown = st.session.selection == Selection::Wip && st.session.conflict.view.is_none();
    if !r.conflicted.is_empty() && details_shown && !j.shown("View conflicts", false, Region::Details) {
        wrong.push("the working copy has conflicts but Details has no \"View conflicts\"".to_owned());
    }
    if let Some(snap) = &st.snap {
        let staged: BTreeSet<String> = snap.staged.iter().map(|f| f.path.clone()).collect();
        let changed: BTreeSet<String> = staged.iter().cloned().chain(snap.unstaged.iter().map(|f| f.path.clone())).collect();
        if staged != r.staged {
            wrong.push(format!("staged files: screen {staged:?}, disk {:?}", r.staged));
        }
        if changed != r.changed {
            wrong.push(format!("changed files: screen {changed:?}, disk {:?}", r.changed));
        }
        if snap.stashes.len() != r.stashes {
            wrong.push(format!("stashes: screen {}, disk {}", snap.stashes.len(), r.stashes));
        }
    }
    if wrong.is_empty() { Ok(()) } else { Err(wrong.join("; ")) }
}

/// Rule 5: after an undo, the head, the branches and the changed paths are what they were before
/// the undone action (the spec's "working tree content unchanged"; markers are rule 3's).
pub(super) fn undo_returns(before_undone: &Reading, after_undo: &Reading) -> Result<(), String> {
    let mut wrong = Vec::new();
    if before_undone.head != after_undo.head {
        wrong.push(format!("head {:?} became {:?}", before_undone.head, after_undo.head));
    }
    if before_undone.branch != after_undo.branch {
        wrong.push(format!("branch {:?} became {:?}", before_undone.branch, after_undo.branch));
    }
    if before_undone.branches != after_undo.branches {
        wrong.push(format!("branches {:?} became {:?}", before_undone.branches, after_undo.branches));
    }
    if before_undone.changed != after_undo.changed {
        wrong.push(format!("changed paths {:?} became {:?}", before_undone.changed, after_undo.changed));
    }
    if wrong.is_empty() { Ok(()) } else { Err(format!("undo did not return: {}", wrong.join("; "))) }
}

/// Rule 6: a failed action says so (a failure toast or a logged error), and a successful one
/// shows no failure toast. `took` is `probe::took_effect`; None never fails.
pub(super) fn errors_reported(took: Option<bool>, toasts: &[(bool, String)], errors: &[String]) -> Result<(), String> {
    let failure_toast = toasts.iter().any(|(ok, _)| !ok);
    match took {
        Some(false) if !failure_toast && errors.is_empty() => Err("the action failed and nothing said so".to_owned()),
        Some(true) if failure_toast => Err(format!("the action worked but a failure was shown: {toasts:?}")),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
