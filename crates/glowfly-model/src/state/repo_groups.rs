//! Repository groups (`Settings::repo_groups`): which favorites each group shows, in what order
//! the favorites are shown (the groups' members first, group by group, then the ungrouped ones;
//! the favorite keys follow it), a repository's group and its colour, and the New group and
//! Rename group form (`Modal::RepoGroup`). A group's entries are copies of `favorite_repos`
//! entries; one that is no longer a favorite is ignored here and dropped by `prune`.

use super::AppState;
use glowfly_core::i18n::Lang;
use glowfly_core::paths;
use glowfly_core::settings::{RepoColour, Settings, clean_group_name};
use std::path::{Path, PathBuf};

/// Whether the saved entry `f` is in some group.
fn grouped(s: &Settings, f: &str) -> bool {
    s.repo_groups.iter().any(|g| g.repos.iter().any(|r| r == f))
}

/// The saved entries of the favorites group `g` shows, in its order.
pub fn member_entries(s: &Settings, g: usize) -> Vec<String> {
    let Some(group) = s.repo_groups.get(g) else { return Vec::new() };
    let mut out: Vec<String> = Vec::new();
    for r in group.repos.iter().filter(|r| s.favorite_repos.contains(r)) {
        // A favorite listed by two groups (an edited file) shows in the first only.
        if s.repo_groups.iter().position(|o| o.repos.contains(r)) == Some(g) && !out.contains(r) {
            out.push(r.clone());
        }
    }
    out
}

/// The favorites group `g` shows, in its order.
pub fn members(s: &Settings, g: usize) -> Vec<PathBuf> {
    member_entries(s, g).iter().map(|r| paths::decode(r)).collect()
}

/// The saved entries of the favorites in no group, in the order the user put them.
pub fn ungrouped_entries(s: &Settings) -> Vec<String> {
    s.favorite_repos.iter().filter(|f| !grouped(s, f)).cloned().collect()
}

/// The favorites in no group, in the order the user put them.
pub fn ungrouped(s: &Settings) -> Vec<PathBuf> {
    ungrouped_entries(s).iter().map(|f| paths::decode(f)).collect()
}

/// Every favorite in the order shown: each group's members, then the ungrouped ones.
pub fn shown_favorites(s: &Settings) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = (0..s.repo_groups.len()).flat_map(|g| members(s, g)).collect();
    out.extend(ungrouped(s));
    out
}

/// The group the repository at `p` is in, if it is a grouped favorite.
pub fn group_of(st: &AppState, p: &Path) -> Option<usize> {
    let s = &st.settings;
    (0..s.repo_groups.len()).find(|&g| members(s, g).iter().any(|m| st.same_folder(m, p)))
}

/// The colour of the group the repository at `p` is in.
pub fn group_hue(st: &AppState, p: &Path) -> Option<RepoColour> {
    group_of(st, p).and_then(|g| st.settings.repo_groups[g].hue())
}

/// Whether any repository or group has a colour (rows keep a swatch's room then).
pub fn any_colour(s: &Settings) -> bool {
    !s.repo_colours.is_empty() || s.repo_groups.iter().any(|g| g.hue().is_some())
}

/// The saved favorite entry for `p` (its spelling in `favorite_repos`), if it is a favorite.
pub fn favorite_entry(st: &AppState, p: &Path) -> Option<String> {
    st.settings.favorite_repos.iter().find(|f| st.same_folder(&paths::decode(f), p)).cloned()
}

/// Drops every group entry that is no longer a favorite, and a second listing of one favorite.
/// Returns whether anything went.
pub fn prune(s: &mut Settings) -> bool {
    let favs = s.favorite_repos.clone();
    let mut seen: Vec<String> = Vec::new();
    let mut changed = false;
    for g in &mut s.repo_groups {
        let before = g.repos.len();
        g.repos.retain(|r| {
            let keep = favs.contains(r) && !seen.contains(r);
            if keep {
                seen.push(r.clone());
            }
            keep
        });
        changed |= g.repos.len() != before;
    }
    changed
}

/// The New group and Rename group dialog (`Modal::RepoGroup`).
pub struct GroupForm {
    /// The group being renamed; None makes a new one.
    pub group: Option<usize>,
    /// The repository that moves into the new group (its row's New group…).
    pub repo: Option<PathBuf>,
    pub text: String,
    /// The other groups' names, to catch a name already used.
    pub taken: Vec<String>,
}

impl GroupForm {
    /// The form for a new group (with `repo` moving into it) or for renaming group `group`.
    pub fn new(s: &Settings, group: Option<usize>, repo: Option<PathBuf>) -> Self {
        let text = group.and_then(|g| s.repo_groups.get(g)).map(|g| g.name.clone()).unwrap_or_default();
        let taken = s.repo_groups.iter().enumerate().filter(|(i, _)| Some(*i) != group).map(|(_, g)| g.name.clone()).collect();
        Self { group, repo, text, taken }
    }

    /// The name as it would be saved.
    pub fn name(&self) -> String {
        clean_group_name(&self.text)
    }

    /// Why the name can't be used: None when it can, Some("") when it is empty (the button is
    /// off without a message), else the message.
    pub fn problem(&self, t: Lang) -> Option<String> {
        let name = self.name();
        if name.is_empty() {
            return Some(String::new());
        }
        let low = name.to_lowercase();
        self.taken.iter().any(|n| n.to_lowercase() == low).then(|| t.trf("repo_group.name.taken", &[("name", &name)]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::settings::RepoGroup;

    fn settings(favs: &[&str], groups: &[(&str, &[&str])]) -> Settings {
        let mut s = Settings { favorite_repos: favs.iter().map(|f| f.to_string()).collect(), ..Settings::default() };
        for (name, repos) in groups {
            let mut g = RepoGroup::new(name, None);
            g.repos = repos.iter().map(|r| r.to_string()).collect();
            s.repo_groups.push(g);
        }
        s
    }

    fn paths(list: &[&str]) -> Vec<PathBuf> {
        list.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn groups_come_first_and_ignore_entries_that_are_not_favorites() {
        let s = settings(&["/w/a", "/w/b", "/w/c", "/w/d"], &[("x", &["/w/c", "/w/gone"]), ("y", &["/w/a", "/w/c"])]);
        assert_eq!(members(&s, 0), paths(&["/w/c"]));
        assert_eq!(members(&s, 1), paths(&["/w/a"]), "a favorite shows in its first group only");
        assert_eq!(ungrouped(&s), paths(&["/w/b", "/w/d"]));
        assert_eq!(shown_favorites(&s), paths(&["/w/c", "/w/a", "/w/b", "/w/d"]));
        assert!(members(&s, 5).is_empty());
    }

    #[test]
    fn prune_drops_stale_and_doubled_entries() {
        let mut s = settings(&["/w/a", "/w/c"], &[("x", &["/w/c", "/w/gone"]), ("y", &["/w/a", "/w/c"])]);
        assert!(prune(&mut s));
        assert_eq!(s.repo_groups[0].repos, ["/w/c"]);
        assert_eq!(s.repo_groups[1].repos, ["/w/a"]);
        assert!(!prune(&mut s), "nothing left to drop");
    }

    #[test]
    fn the_form_checks_the_name() {
        let s = settings(&[], &[("Work", &[]), ("Home", &[])]);
        let mut f = GroupForm::new(&s, None, None);
        assert_eq!(f.problem(Lang::En).as_deref(), Some(""), "empty");
        f.text = " work ".into();
        assert_eq!(f.problem(Lang::En).as_deref(), Some("There's already a group called work."));
        f.text = "Clients".into();
        assert_eq!(f.problem(Lang::En), None);
        let f = GroupForm::new(&s, Some(0), None);
        assert_eq!((f.text.as_str(), f.problem(Lang::En)), ("Work", None), "its own name is not taken");
    }
}
