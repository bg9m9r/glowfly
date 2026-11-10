//! Performing one action: through the app's widgets for what a user does in the window, and on
//! disk or with the git CLI for what happens outside it. A widget or menu item that is missing or
//! disabled, or an outside change that failed twice, makes the step `Skipped`; it never panics.

use super::super::find::{Pick, Region};
use super::super::harness::Journey;
use super::action::Action;
use super::perform_outside::perform_outside;
use super::probe;
use super::scene::Scene;
use super::shadow::Shadow;
use eframe::egui::Key;
use glowfly_core::i18n::Lang;
use std::path::Path;

#[derive(Debug)]
pub(super) enum Done {
    Performed,
    /// The widget or menu item wasn't there or was disabled, or an outside change failed twice.
    Skipped(String),
}

fn skipped(why: impl Into<String>) -> Done {
    Done::Skipped(why.into())
}

/// Clicks the enabled widget `label` (exact) in `region`, or says it wasn't there.
fn press(j: &mut Journey, label: &str, region: Region) -> Result<(), Done> {
    press_pick(j, label, true, region, Pick::First)
}

fn press_pick(j: &mut Journey, label: &str, exact: bool, region: Region, pick: Pick<'_>) -> Result<(), Done> {
    if j.try_click(label, exact, region, pick) { Ok(()) } else { Err(skipped(format!("no enabled {label:?} in {region:?}"))) }
}

/// Right-clicks the row `row` in `region` and picks `item` from its menu; closes the menu when
/// the item isn't in it.
fn row_menu(j: &mut Journey, row: &str, region: Region, item: &str) -> Result<(), Done> {
    if !j.shown(row, true, region) {
        return Err(skipped(format!("no row {row:?} in {region:?}")));
    }
    j.right_click(row, region, Pick::First);
    if j.try_click(item, true, Region::Anywhere, Pick::First) {
        return Ok(());
    }
    j.press(Key::Escape);
    Err(skipped(format!("no enabled menu item {item:?} on {row:?}")))
}

/// A dialog's button; its buttons are disabled for a moment after it opens.
fn dialog_button(j: &mut Journey, label: &str) -> Result<(), Done> {
    j.settle();
    press_pick(j, label, true, Region::Anywhere, Pick::Last)
}

/// Performs `action` on the tab `active` (`app` or `lib`) of the scene.
pub(super) fn perform(j: &mut Journey, scene: &Scene, active: &str, shadow: &mut Shadow, action: &Action) -> Done {
    let dir = if active == "lib" { &scene.lib } else { &scene.app };
    if let Action::Outside(o) = action {
        return perform_outside(j, dir, active, shadow, o);
    }
    match ui(j, dir, active, shadow, action) {
        Ok(()) => Done::Performed,
        Err(done) => done,
    }
}

fn ui(j: &mut Journey, dir: &Path, active: &str, shadow: &mut Shadow, action: &Action) -> Result<(), Done> {
    use Action::*;
    let current = || probe::read(dir).branch.ok_or_else(|| skipped("no branch is checked out"));
    match action {
        SelectWip => press(j, "Uncommitted changes", Region::Central),
        Stage(p) => press_pick(j, "Stage", true, Region::Details, Pick::InRowOf(p)),
        Unstage(p) => press_pick(j, "Unstage", true, Region::Details, Pick::InRowOf(p)),
        StageAll => press(j, "Stage all", Region::Details),
        UnstageAll => press(j, "Unstage all", Region::Details),
        StageHunk(p) => {
            if !j.shown("Stage hunk", true, Region::Central) {
                press(j, p, Region::Details)?;
            }
            press(j, "Stage hunk", Region::Central)
        }
        Commit(text) => commit(j, dir, active, shadow, text),
        // The checkbox turns the commit button into one with the same name.
        Amend => {
            press_pick(j, "Amend last commit", false, Region::Details, Pick::First)?;
            press_pick(j, "Amend last commit", false, Region::Details, Pick::Last)
        }
        CreateBranch(name) => {
            press(j, "Create a branch", Region::Toolbar)?;
            if !j.try_type("Branch name", name) {
                return Err(skipped("no Branch name field"));
            }
            dialog_button(j, "Create branch")
        }
        Checkout(b) => branch_item(j, b, "Checkout"),
        DeleteBranch(b) => branch_item(j, b, "Delete…"),
        Merge(b) => branch_item(j, b, &format!("Merge into {}", current()?)),
        Rebase(b) => branch_item(j, b, &format!("Rebase {} onto this branch", current()?)),
        Stash => press(j, "Stash all changes", Region::Toolbar),
        Unstash => press(j, "Unstash the latest stash", Region::Toolbar),
        DropStash => {
            let Some(row) = j.labels(Region::Sidebar).into_iter().find(|l| l.contains("WIP on")) else {
                return Err(skipped("no stash in the sidebar"));
            };
            row_menu(j, &row, Region::Sidebar, "Delete…")
        }
        Undo => press(j, "Undo the last action", Region::Toolbar),
        Redo => press(j, "Redo the undone action", Region::Toolbar),
        Discard(p) => {
            row_menu(j, p, Region::Details, "Discard changes…")?;
            dialog_button(j, "Discard")
        }
        ResolveOurs(_) => resolve(j, "Take every conflict from ours"),
        ResolveTheirs(_) => resolve(j, "Take every conflict from theirs"),
        // A rebase continues with "Continue"; a merge is finished by committing it.
        Continue => press(j, "Continue", Region::Details).or_else(|_| press(j, "Commit merge", Region::Details)),
        Abort => press_pick(j, "Abort ", false, Region::Details, Pick::First),
        Refresh => press(j, "Refresh the repository", Region::Toolbar),
        SwitchTab(name) => press(j, name, Region::Tabs),
        CloseTab(name) => press(j, &format!("Close tab {name}"), Region::Tabs),
        Confirm(label) => dialog_button(j, label),
        Outside(_) => unreachable!("outside actions are performed on disk"),
    }
}

/// Types the summary and presses the commit button, which names the staged count. The summary is
/// recorded only when the head commit has it afterwards.
fn commit(j: &mut Journey, dir: &Path, active: &str, shadow: &mut Shadow, text: &str) -> Result<(), Done> {
    if !j.shown("Commit summary", false, Region::Details) {
        press(j, "Uncommitted changes", Region::Central)?;
    }
    if !j.try_type("Commit summary", text) {
        return Err(skipped("no Commit summary field"));
    }
    let n = j.state().snap.as_ref().map_or(0, |s| s.staged.len());
    let label = Lang::En.trn("commit.button.commit", n, &[("push", &""), ("skip", &"")]);
    press_pick(j, &label, false, Region::Details, Pick::First)?;
    if probe::read(dir).head_summary.as_deref() == Some(text) {
        shadow.summary(active, text);
    }
    Ok(())
}

/// Picks `item` from the menu of local branch `name`.
fn branch_item(j: &mut Journey, name: &str, item: &str) -> Result<(), Done> {
    let row = j.local_branch(name);
    row_menu(j, &row, Region::Sidebar, item)
}

/// Opens the conflict view, takes one side for the whole file and marks it resolved.
fn resolve(j: &mut Journey, take: &str) -> Result<(), Done> {
    if !j.shown("View conflicts", true, Region::Details) {
        press(j, "View uncommitted changes", Region::Details)?;
    }
    press(j, "View conflicts", Region::Details)?;
    press_pick(j, take, false, Region::Central, Pick::First)?;
    press(j, "Mark resolved", Region::Central)
}

#[cfg(test)]
#[path = "perform_tests.rs"]
mod tests;
