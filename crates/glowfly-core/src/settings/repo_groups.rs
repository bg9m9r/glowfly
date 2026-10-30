//! Repository groups: named sets of favorite repositories (the repository menu and the welcome
//! screen), each with an optional colour. A group lists some of `favorite_repos` in its own
//! order; every favorite stays in `favorite_repos`, so a file read by an older version still
//! lists them all, and removing a group leaves its repositories as plain favorites. The colour
//! is saved as a `RepoColour::id` name, so a name this version doesn't know reads as none.

use super::RepoColour;
use serde::{Deserialize, Serialize};

/// The most characters a group's name keeps.
pub const MAX_GROUP_NAME: usize = 60;

/// One repository group.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RepoGroup {
    /// The name shown on its header; never empty once made through the app.
    pub name: String,
    /// The group's colour as a `RepoColour::id` name; None (or an unknown name) is none.
    pub colour: Option<String>,
    /// Its repositories, as `paths::encode`d favorite paths, in the order shown.
    pub repos: Vec<String>,
}

impl RepoGroup {
    /// A new, empty group called `name`, coloured `colour`.
    pub fn new(name: &str, colour: Option<RepoColour>) -> Self {
        Self { name: clean_name(name), colour: colour.map(|c| c.id().to_owned()), repos: Vec::new() }
    }

    /// The group's colour, if it has one this version knows.
    pub fn hue(&self) -> Option<RepoColour> {
        self.colour.as_deref().and_then(RepoColour::from_id)
    }

    /// Sets the colour; returns whether it changed.
    pub fn set_hue(&mut self, c: Option<RepoColour>) -> bool {
        let wanted = c.map(|c| c.id().to_owned());
        if self.colour == wanted {
            return false;
        }
        self.colour = wanted;
        true
    }
}

/// `name` as a group keeps it: on one line, trimmed, at most `MAX_GROUP_NAME` characters.
pub fn clean_name(name: &str) -> String {
    let one_line: String = name.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    one_line.trim().chars().take(MAX_GROUP_NAME).collect::<String>().trim_end().to_owned()
}

/// The order new groups take their hues in: far apart first, so the first few groups differ.
pub const GROUP_ORDER: [RepoColour; 8] = [
    RepoColour::Rose,
    RepoColour::Teal,
    RepoColour::Plum,
    RepoColour::Fern,
    RepoColour::Sky,
    RepoColour::Iris,
    RepoColour::Mint,
    RepoColour::Slate,
];

/// The colour a new group gets: the first hue in `GROUP_ORDER` no group in `groups` uses; None
/// once all are taken. (A default group colour setting, #3, would take its place.)
pub fn new_group_colour(groups: &[RepoGroup]) -> Option<RepoColour> {
    GROUP_ORDER.into_iter().find(|c| !groups.iter().any(|g| g.hue() == Some(*c)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn old_files_load_without_groups() {
        let old: Settings = serde_json::from_str(r#"{"favorite_repos":["/w/a"]}"#).unwrap();
        assert!(old.repo_groups.is_empty());
        assert_eq!(old.favorite_repos, ["/w/a"]);
    }

    #[test]
    fn groups_round_trip_and_unknown_hues_read_as_none() {
        let mut s = Settings::default();
        let mut g = RepoGroup::new("  Work  ", Some(RepoColour::Teal));
        g.repos.push("/w/a".into());
        s.repo_groups.push(g.clone());
        let text = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.repo_groups, [g]);
        assert_eq!(back.repo_groups[0].name, "Work");
        let junk: Settings = serde_json::from_str(r#"{"repo_groups":[{"name":"X","colour":"turquoise"},{}]}"#).unwrap();
        assert_eq!(junk.repo_groups[0].hue(), None, "an unknown name is no colour");
        assert_eq!(junk.repo_groups[1], RepoGroup::default(), "missing fields default");
    }

    #[test]
    fn names_are_cleaned_and_hues_handed_out_in_order() {
        assert_eq!(clean_name(" a\nb\t"), "a b");
        assert_eq!(clean_name(&"x".repeat(80)).chars().count(), MAX_GROUP_NAME);
        let mut groups = vec![RepoGroup::new("a", Some(RepoColour::Rose)), RepoGroup::new("b", None)];
        assert_eq!(new_group_colour(&groups), Some(RepoColour::Teal));
        assert!(groups[1].set_hue(Some(RepoColour::Teal)));
        assert!(!groups[1].set_hue(Some(RepoColour::Teal)));
        assert_eq!(new_group_colour(&groups), Some(RepoColour::Plum));
        let all: Vec<RepoGroup> = RepoColour::ALL.iter().map(|&c| RepoGroup::new("g", Some(c))).collect();
        assert_eq!(new_group_colour(&all), None);
        assert!(RepoColour::ALL.iter().all(|c| GROUP_ORDER.contains(c)), "every hue once");
    }
}
