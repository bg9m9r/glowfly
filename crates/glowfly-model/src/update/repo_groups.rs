//! Repository groups: the New group, Rename group and Add repositories dialogs, colouring,
//! ordering and removing groups, moving favorites between them, adding repositories to one, and
//! resetting every group's colour to the default (with Undo).
//! Every change saves the settings; a message that names a group that is gone (the list changed
//! under a menu) does nothing. A Create or Rename whose name is usable closes the dialog; a
//! refused one leaves it open.

use crate::effect::Effect;
use crate::msg::{GroupMsg, Msg};
use crate::state::repo_group_picker::AddPicker;
use crate::state::repo_groups::{GroupForm, favorite_entry, member_entries, members, prune, ungrouped_entries};
use crate::state::{AppState, Modal, ToastAction};
use glowfly_core::paths;
use glowfly_core::settings::{RepoColour, RepoGroup, clean_group_name, default_colours, new_group_colour};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: GroupMsg, now: Instant, fx: &mut Vec<Effect>) {
    let changed = match msg {
        GroupMsg::NewDialog { repo } => {
            st.modal = Some(Modal::RepoGroup(GroupForm::new(&st.settings, None, repo)));
            false
        }
        GroupMsg::RenameDialog(g) => {
            if g < st.settings.repo_groups.len() {
                st.modal = Some(Modal::RepoGroup(GroupForm::new(&st.settings, Some(g), None)));
            }
            false
        }
        GroupMsg::Create { name, repo } => {
            let made = create(st, &name, repo.as_deref());
            if made {
                close_dialog(st);
            }
            made
        }
        GroupMsg::Rename { group, name } => {
            let name = clean_group_name(&name);
            let usable = !name_taken(st, &name, Some(group));
            if usable {
                close_dialog(st);
            }
            match st.settings.repo_groups.get_mut(group) {
                Some(g) if usable && g.name != name => {
                    g.name = name;
                    true
                }
                _ => false,
            }
        }
        GroupMsg::SetColour { group, colour } => st.settings.repo_groups.get_mut(group).is_some_and(|g| g.set_hue(colour)),
        GroupMsg::MoveGroup { from, to } => {
            let groups = &mut st.settings.repo_groups;
            let moves = from != to && from < groups.len() && to < groups.len();
            if moves {
                let g = groups.remove(from);
                groups.insert(to, g);
            }
            moves
        }
        GroupMsg::Remove(g) => remove(st, g, now),
        GroupMsg::MoveRepo { path, group, index } => move_repo(st, &path, group, index),
        GroupMsg::AddDialog(g) => {
            if g < st.settings.repo_groups.len() {
                st.modal = Some(Modal::AddToGroup(AddPicker::new(st, g)));
            }
            false
        }
        GroupMsg::AddRepos { group, paths } => add_repos(st, group, &paths),
        GroupMsg::ResetColours => reset_colours(st, now),
        GroupMsg::RestoreColours(saved) => restore_colours(st, &saved),
    };
    if changed {
        fx.extend(super::prefs::save_settings(st));
    }
}

/// Puts each of `paths` at the end of `group`, starring it first if need be; one already in
/// the group stays put. Closes the Add repositories picker. Returns whether anything changed.
fn add_repos(st: &mut AppState, group: usize, paths: &[PathBuf]) -> bool {
    if matches!(st.modal, Some(Modal::AddToGroup(_))) {
        st.modal = None;
    }
    if group >= st.settings.repo_groups.len() {
        return false;
    }
    let mut changed = false;
    for p in paths {
        let here = members(&st.settings, group).iter().any(|m| st.same_folder(m, p));
        if !here {
            changed |= move_repo(st, p, Some(group), usize::MAX);
        }
    }
    changed
}

/// Closes the New group or Rename group dialog once its name is taken up; a refused name
/// leaves it open with its message.
fn close_dialog(st: &mut AppState) {
    if matches!(st.modal, Some(Modal::RepoGroup(_))) {
        st.modal = None;
    }
}

/// Makes the group (in the New group color setting's colour) and moves `repo` into it. A blank name makes
/// nothing.
fn create(st: &mut AppState, name: &str, repo: Option<&Path>) -> bool {
    let name = clean_group_name(name);
    if name_taken(st, &name, None) {
        return false;
    }
    let colour = new_group_colour(st.settings.new_group_colour, &st.settings.repo_groups);
    st.settings.repo_groups.push(RepoGroup::new(&name, colour));
    if let Some(p) = repo {
        move_repo(st, p, Some(st.settings.repo_groups.len() - 1), usize::MAX);
    }
    true
}

/// Whether `name` can't be a group's: blank, or another group's than `except` (case ignored).
/// The dialog says so first (`GroupForm::problem`); this keeps any other sender honest.
fn name_taken(st: &AppState, name: &str, except: Option<usize>) -> bool {
    let low = name.to_lowercase();
    name.is_empty() || st.settings.repo_groups.iter().enumerate().any(|(i, g)| Some(i) != except && g.name.to_lowercase() == low)
}

/// Removes group `g`; its repositories stay favorites, moved after the ungrouped ones in their
/// group's order. Says so in a toast.
fn remove(st: &mut AppState, g: usize, now: Instant) -> bool {
    if g >= st.settings.repo_groups.len() {
        return false;
    }
    let moved = member_entries(&st.settings, g);
    let group = st.settings.repo_groups.remove(g);
    let favs = &mut st.settings.favorite_repos;
    favs.retain(|f| !moved.contains(f));
    favs.extend(moved);
    let t = st.settings.lang();
    st.toast(true, t.trf("repo_group.removed.toast", &[("name", &group.name)]), now);
    true
}

/// Puts the repository at `p` in `group` (None: no group) at `index` among that section's rows
/// (past the end: last). Returns whether anything changed.
fn move_repo(st: &mut AppState, p: &Path, group: Option<usize>, index: usize) -> bool {
    if group.is_some_and(|g| g >= st.settings.repo_groups.len()) {
        return false;
    }
    let before = (st.settings.favorite_repos.clone(), st.settings.repo_groups.clone());
    let entry = match favorite_entry(st, p) {
        Some(f) => f,
        None => {
            let f = paths::encode(p);
            st.settings.favorite_repos.push(f.clone());
            f
        }
    };
    prune(&mut st.settings);
    let s = &mut st.settings;
    for g in &mut s.repo_groups {
        g.repos.retain(|r| *r != entry);
    }
    match group {
        Some(g) => {
            let repos = &mut s.repo_groups[g].repos;
            repos.insert(index.min(repos.len()), entry);
        }
        None => {
            // The ungrouped rows are `favorite_repos` without the grouped entries: put it
            // before the one now at `index`, or after them all.
            s.favorite_repos.retain(|f| *f != entry);
            let next = ungrouped_entries(s).into_iter().nth(index);
            let at = next.and_then(|n| s.favorite_repos.iter().position(|f| *f == n)).unwrap_or(s.favorite_repos.len());
            s.favorite_repos.insert(at, entry);
        }
    }
    (st.settings.favorite_repos.clone(), st.settings.repo_groups.clone()) != before
}

/// Gives every group its default colour (`default_colours`) and toasts with an Undo that puts
/// the old ones back. Returns whether anything changed: nothing does when all are at default.
fn reset_colours(st: &mut AppState, now: Instant) -> bool {
    let groups = &mut st.settings.repo_groups;
    let before: Vec<(String, Option<RepoColour>)> = groups.iter().map(|g| (g.name.clone(), g.hue())).collect();
    let mut changed = false;
    for (g, c) in groups.iter_mut().zip(default_colours(st.settings.new_group_colour, before.len())) {
        changed |= g.set_hue(c);
    }
    if changed {
        let t = st.settings.lang();
        let msg = Box::new(Msg::Groups(GroupMsg::RestoreColours(before)));
        let action = ToastAction { label: t.tr("repo_group.colours_reset.undo").to_owned(), msg, workdir: None };
        st.toast_action(true, t.tr("repo_group.colours_reset.toast"), action, now);
    }
    changed
}

/// Puts back the colours in `saved` on the groups still called those names (case ignored).
fn restore_colours(st: &mut AppState, saved: &[(String, Option<RepoColour>)]) -> bool {
    let mut changed = false;
    for g in &mut st.settings.repo_groups {
        let low = g.name.to_lowercase();
        if let Some((_, c)) = saved.iter().find(|(name, _)| name.to_lowercase() == low) {
            changed |= g.set_hue(*c);
        }
    }
    changed
}
