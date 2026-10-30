//! Repository groups: the New group and Rename group dialogs, colouring, ordering and removing
//! groups, and moving favorites between them. Every change saves the settings; a message that
//! names a group that is gone (the list changed under a menu) does nothing.

use crate::effect::Effect;
use crate::msg::GroupMsg;
use crate::state::repo_groups::{GroupForm, favorite_entry, member_entries, prune, ungrouped_entries};
use crate::state::{AppState, Modal};
use glowfly_core::paths;
use glowfly_core::settings::{RepoGroup, clean_group_name, new_group_colour};
use std::path::Path;
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
        GroupMsg::Create { name, repo } => create(st, &name, repo.as_deref()),
        GroupMsg::Rename { group, name } => {
            let name = clean_group_name(&name);
            let usable = !name_taken(st, &name, Some(group));
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
    };
    if changed {
        fx.push(super::prefs::save_settings(st));
    }
}

/// Makes the group (with the next free colour) and moves `repo` into it. A blank name makes
/// nothing.
fn create(st: &mut AppState, name: &str, repo: Option<&Path>) -> bool {
    let name = clean_group_name(name);
    if name_taken(st, &name, None) {
        return false;
    }
    let colour = new_group_colour(&st.settings.repo_groups);
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
