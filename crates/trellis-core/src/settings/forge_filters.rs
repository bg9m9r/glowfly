//! A repository's pull request and issue filters: the ones the user saved, in their order, and
//! the filter each sidebar section shows. Saved per repository through `repo_keys`.

use super::{Settings, repo_keys};
use crate::forge::{Builtin, Conditions, FilterChoice, FilterKind, SavedFilter};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RepoForgeFilters {
    #[serde(default)]
    saved: Vec<SavedFilter>,
    /// None shows the kind's default.
    #[serde(default)]
    prs: Option<FilterChoice>,
    #[serde(default)]
    issues: Option<FilterChoice>,
    /// The last id given out, so a removed filter's id isn't reused.
    #[serde(default)]
    next_id: u64,
}

impl RepoForgeFilters {
    /// Nothing saved and both sections on their defaults.
    pub fn is_empty(&self) -> bool {
        self.saved.is_empty() && self.prs.is_none() && self.issues.is_none()
    }

    pub fn get(&self, id: u64) -> Option<&SavedFilter> {
        self.saved.iter().find(|s| s.id == id)
    }

    /// The filter `kind`'s section shows: the kind's default when none was chosen, or the chosen
    /// saved filter is gone (removed in another window, or the file was edited by hand).
    pub fn choice(&self, kind: FilterKind) -> FilterChoice {
        let chosen = match kind {
            FilterKind::Prs => self.prs,
            FilterKind::Issues => self.issues,
        };
        match chosen {
            Some(FilterChoice::Builtin(b)) if b.kind() == kind => FilterChoice::Builtin(b),
            Some(FilterChoice::Saved(id)) if self.get(id).is_some_and(|s| s.kind == kind) => FilterChoice::Saved(id),
            _ => FilterChoice::default_for(kind),
        }
    }

    /// The shown filter, its conditions and its name.
    pub fn conditions(&self, kind: FilterKind) -> (FilterChoice, Conditions, String) {
        let choice = self.choice(kind);
        let (c, label) = self.describe(choice).unwrap_or_else(|| {
            let b = Builtin::default_for(kind);
            (b.conditions(), b.label().to_owned())
        });
        (choice, c, label)
    }

    /// The conditions and name of `choice`, if it exists.
    pub fn describe(&self, choice: FilterChoice) -> Option<(Conditions, String)> {
        match choice {
            FilterChoice::Builtin(b) => Some((b.conditions(), b.label().to_owned())),
            FilterChoice::Saved(id) => self.get(id).map(|s| (s.conditions.clone(), s.name.clone())),
        }
    }

    /// The saved filters of `kind`, in their order.
    pub fn saved_of(&self, kind: FilterKind) -> impl Iterator<Item = &SavedFilter> {
        self.saved.iter().filter(move |s| s.kind == kind)
    }

    /// Saves a new filter at the end of the list; returns its id.
    pub fn add(&mut self, kind: FilterKind, name: &str, conditions: Conditions) -> u64 {
        let id = self.next_id.max(self.saved.iter().map(|s| s.id).max().unwrap_or(0)) + 1;
        self.next_id = id;
        self.saved.push(SavedFilter { id, name: name.trim().to_owned(), kind, conditions });
        id
    }

    /// Changes a saved filter in place; false when there is none with `id`.
    pub fn replace(&mut self, id: u64, name: &str, conditions: Conditions) -> bool {
        let Some(s) = self.saved.iter_mut().find(|s| s.id == id) else { return false };
        s.name = name.trim().to_owned();
        s.conditions = conditions;
        true
    }

    /// Removes a saved filter; a section showing it goes back to its default.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.saved.len();
        self.saved.retain(|s| s.id != id);
        for chosen in [&mut self.prs, &mut self.issues] {
            if *chosen == Some(FilterChoice::Saved(id)) {
                *chosen = None;
            }
        }
        self.saved.len() != before
    }

    /// Moves a saved filter one place up or down among the filters of its own kind; false when it
    /// is already first (last) or doesn't exist.
    pub fn move_within_kind(&mut self, id: u64, up: bool) -> bool {
        let Some(i) = self.saved.iter().position(|s| s.id == id) else { return false };
        let kind = self.saved[i].kind;
        let other = if up {
            self.saved[..i].iter().rposition(|s| s.kind == kind)
        } else {
            self.saved[i + 1..].iter().position(|s| s.kind == kind).map(|j| i + 1 + j)
        };
        let Some(j) = other else { return false };
        self.saved.swap(i, j);
        true
    }

    /// Shows `choice` in `kind`'s section (the default is stored as nothing).
    pub fn set_choice(&mut self, kind: FilterKind, choice: FilterChoice) {
        let stored = (choice != FilterChoice::default_for(kind)).then_some(choice);
        match kind {
            FilterKind::Prs => self.prs = stored,
            FilterKind::Issues => self.issues = stored,
        }
    }
}

impl Settings {
    /// The pull request and issue filters of the repository at `real`.
    pub fn forge_filters_for(&self, real: &Path) -> RepoForgeFilters {
        repo_keys::get(&self.repo_forge_filters, real).cloned().unwrap_or_default()
    }

    /// Saves the filters of the repository at `real`; empty ones remove its entry.
    pub fn set_forge_filters(&mut self, real: &Path, f: RepoForgeFilters) {
        repo_keys::set(&mut self.repo_forge_filters, real, (!f.is_empty()).then_some(f));
    }
}

#[cfg(test)]
#[path = "forge_filters_tests.rs"]
mod tests;
