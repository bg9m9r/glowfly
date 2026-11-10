//! Choosing what to do next: the actions that apply on the current screen, each with its weight.
//! An action applies when the widget that does it is shown and enabled; the arguments it needs
//! (a branch, a path, a summary) come from the repository reading and the generator, never from
//! the order of the screen. Branch menus only open on a right-click, so a branch's actions apply
//! when its row is shown and the step that performs them skips a missing menu item.

use super::super::find::Region;
use super::super::harness::Journey;
use super::action::{Action, Outside};
use super::probe::Reading;
use super::rng::Rng;
use super::scene::{PATHS, Scene};
use super::shadow::Shadow;
use git2::RepositoryState;
use glowfly_core::i18n::Lang;

/// The labels of the buttons a dialog may offer; Cancel is the way out of any of them.
const MODAL_BUTTONS: [&str; 12] = [
    "Cancel",
    "Stash and check out",
    "Stash and pull",
    "Create branch",
    "Delete",
    "Delete anyway",
    "Discard",
    "Discard all",
    "Restore",
    "Reset",
    "Drop commit",
    "Try again",
];

/// An element of `items` chosen by `rng`, or None when it is empty.
fn pick<'a, T>(rng: &mut Rng, items: &'a [T]) -> Option<&'a T> {
    (!items.is_empty()).then(|| &items[rng.below(items.len())])
}

/// A branch name `b<n>` that `r` doesn't have yet.
fn fresh_branch(rng: &mut Rng, r: &Reading) -> String {
    loop {
        let name = format!("b{}", rng.below(1000));
        if !r.branches.contains_key(&name) {
            return name;
        }
    }
}

/// The actions that apply now with their weights, in the order of the vocabulary. `active` is
/// the tab's repository (`app` or `lib`) and `r` its reading. While a dialog is open only its
/// buttons apply. The shadow is not needed yet: the markers are minted when an edit is made.
pub(super) fn applicable(j: &Journey, scene: &Scene, active: &str, r: &Reading, rng: &mut Rng, _shadow: &mut Shadow) -> Vec<(Action, u32)> {
    if j.modal_open() {
        return MODAL_BUTTONS
            .iter()
            .filter(|label| j.usable(label, true, Region::Anywhere))
            .map(|label| (Action::Confirm((*label).to_owned()), if *label == "Cancel" { 1 } else { 5 }))
            .collect();
    }
    use Action::*;
    let mut out: Vec<(Action, u32)> = Vec::new();
    let calm = r.state == RepositoryState::Clean && r.conflicted.is_empty();
    let unstaged: Vec<&String> = r.changed.iter().filter(|p| !r.staged.contains(*p) && !r.conflicted.contains(*p)).collect();
    let staged: Vec<&String> = r.staged.iter().collect();
    let details = |label: &str| j.shown(label, true, Region::Details);
    let toolbar = |label: &str| j.usable(label, true, Region::Toolbar);
    let others: Vec<&String> = r
        .branches
        .keys()
        .filter(|b| r.branch.as_ref() != Some(*b))
        .filter(|b| j.shown(&j.local_branch(b), true, Region::Sidebar))
        .collect();

    if j.shown("Uncommitted changes", true, Region::Central) {
        out.push((SelectWip, 2));
    }
    let can_stage = details("Stage all");
    if can_stage && let Some(p) = pick(rng, &unstaged) {
        out.push((Stage((*p).clone()), 6));
    }
    let can_unstage = details("Unstage all");
    if can_unstage && let Some(p) = pick(rng, &staged) {
        out.push((Unstage((*p).clone()), 2));
    }
    if can_stage && !unstaged.is_empty() {
        out.push((StageAll, 6));
    }
    if can_unstage {
        out.push((UnstageAll, 2));
    }
    if j.shown("Stage hunk", true, Region::Central)
        && let Some(p) = pick(rng, &unstaged)
    {
        out.push((StageHunk((*p).clone()), 6));
    }
    let button = Lang::En.trn("commit.button.commit", r.staged.len(), &[("push", &""), ("skip", &"")]);
    if !r.staged.is_empty() && r.conflicted.is_empty() && j.shown(&button, false, Region::Details) {
        out.push((Commit(format!("msg {}", rng.below(1000))), 8));
    }
    if r.head.is_some() && r.conflicted.is_empty() && j.shown("Amend last commit", false, Region::Details) {
        out.push((Amend, 1));
    }
    if toolbar("Create a branch") {
        out.push((CreateBranch(fresh_branch(rng, r)), 3));
    }
    for b in &others {
        out.push((Checkout((*b).clone()), 4));
    }
    for b in others.iter().filter(|b| b.as_str() != "main") {
        out.push((DeleteBranch((*b).clone()), 1));
    }
    if calm {
        for b in &others {
            out.push((Merge((*b).clone()), 3));
        }
        for b in &others {
            out.push((Rebase((*b).clone()), 2));
        }
    }
    if toolbar("Stash all changes") {
        out.push((Stash, 3));
    }
    if toolbar("Unstash the latest stash") {
        out.push((Unstash, 3));
    }
    if r.stashes > 0 && j.labels(Region::Sidebar).iter().any(|l| l.contains("WIP on")) {
        out.push((DropStash, 1));
    }
    if toolbar("Undo the last action") {
        out.push((Undo, 4));
    }
    if toolbar("Redo the undone action") {
        out.push((Redo, 2));
    }
    if can_stage && let Some(p) = pick(rng, &unstaged) {
        out.push((Discard((*p).clone()), 1));
    }
    for p in &r.conflicted {
        out.push((ResolveOurs(p.clone()), 4));
    }
    for p in &r.conflicted {
        out.push((ResolveTheirs(p.clone()), 4));
    }
    if r.state != RepositoryState::Clean && r.conflicted.is_empty() {
        out.push((Continue, 4));
    }
    if r.state != RepositoryState::Clean {
        out.push((Abort, 1));
    }
    if toolbar("Refresh the repository") {
        out.push((Refresh, 2));
    }
    let other_tab = if active == "app" { "lib" } else { "app" };
    if j.shown(other_tab, false, Region::Tabs) {
        out.push((SwitchTab(other_tab.to_owned()), 1));
        out.push((CloseTab(other_tab.to_owned()), 1));
    }
    out.extend(outside(scene, active, r, rng, calm));
    out
}

/// The changes from outside the app that apply to the repository `active`.
fn outside(scene: &Scene, active: &str, r: &Reading, rng: &mut Rng, calm: bool) -> Vec<(Action, u32)> {
    let dir = if active == "lib" { &scene.lib } else { &scene.app };
    let (present, absent): (Vec<&str>, Vec<&str>) = PATHS.iter().partition(|p| dir.join(p).exists());
    let lines = |rng: &mut Rng| (rng.below(3) + 1) as u8;
    let mut out = Vec::new();
    if let Some(p) = pick(rng, &present) {
        out.push((Action::Outside(Outside::Write { path: (*p).to_owned(), lines: lines(rng) }), 6));
    }
    if let Some(p) = pick(rng, &present) {
        out.push((Action::Outside(Outside::Append { path: (*p).to_owned(), lines: lines(rng) }), 6));
    }
    if let Some(p) = pick(rng, &absent) {
        out.push((Action::Outside(Outside::Create { path: (*p).to_owned(), lines: lines(rng) }), 6));
    }
    if let Some(p) = pick(rng, &present) {
        out.push((Action::Outside(Outside::Delete { path: (*p).to_owned() }), 1));
    }
    if calm && !r.changed.is_empty() {
        out.push((Action::Outside(Outside::GitCommit(format!("msg {}", rng.below(1000)))), 1));
    }
    let others: Vec<&String> = r.branches.keys().filter(|b| r.branch.as_ref() != Some(*b)).collect();
    if calm && let Some(b) = pick(rng, &others) {
        out.push((Action::Outside(Outside::GitCheckout((*b).clone())), 1));
    }
    out.push((Action::Outside(Outside::GitBranch(fresh_branch(rng, r))), 1));
    out
}

#[cfg(test)]
#[path = "choose_tests.rs"]
mod tests;
