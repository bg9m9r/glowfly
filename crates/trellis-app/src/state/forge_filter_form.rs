//! The filter dialog (`Modal::ForgeFilter`): a saved filter's name and conditions while they are
//! edited, and why it can't be saved yet.

use trellis_core::forge::filters::matching;
use trellis_core::forge::{Conditions, FilterKind, Provider, SavedFilter};
use trellis_core::settings::RepoForgeFilters;

/// The longest name a saved filter may have, in characters.
pub const MAX_NAME: usize = 60;

pub struct FilterForm {
    /// The saved filter being changed; None for a new one.
    pub editing: Option<u64>,
    pub kind: FilterKind,
    /// The service the filter applies to (the code host or the tracker), for what it supports.
    pub provider: Option<Provider>,
    pub name: String,
    /// Labels as typed, comma-separated.
    pub labels: String,
    /// Every condition but the labels, which `conditions` reads from `labels`.
    pub c: Conditions,
}

impl FilterForm {
    /// A new filter starting from `start`'s conditions (the filter on show).
    pub fn new(kind: FilterKind, provider: Option<Provider>, start: &Conditions) -> Self {
        FilterForm { editing: None, kind, provider, name: String::new(), labels: start.labels.join(", "), c: start.clone() }
    }

    pub fn edit(saved: &SavedFilter, provider: Option<Provider>) -> Self {
        FilterForm { editing: Some(saved.id), name: saved.name.clone(), ..Self::new(saved.kind, provider, &saved.conditions) }
    }

    /// The labels typed, trimmed, without empty or repeated ones (ignoring case).
    pub fn label_list(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for l in self.labels.split(',').map(str::trim).filter(|l| !l.is_empty()) {
            if !out.iter().any(|have| have.to_lowercase() == l.to_lowercase()) {
                out.push(l.to_owned());
            }
        }
        out
    }

    pub fn conditions(&self) -> Conditions {
        Conditions { labels: self.label_list(), ..self.c.clone() }
    }

    /// "pull request", "merge request", "issue" or "work item".
    pub fn noun(&self) -> &'static str {
        match (self.provider, self.kind) {
            (Some(p), kind) => matching::item_noun(p, kind),
            (None, FilterKind::Prs) => "pull request",
            (None, FilterKind::Issues) => "issue",
        }
    }

    /// Everything that keeps the filter from being saved; empty when it can be.
    pub fn problems(&self, existing: &RepoForgeFilters) -> Vec<String> {
        let mut out = Vec::new();
        let name = self.name.trim();
        if name.is_empty() {
            out.push("Enter a name.".to_owned());
        } else if name.chars().count() > MAX_NAME {
            out.push(format!("Keep the name to {MAX_NAME} characters."));
        } else if existing.saved_of(self.kind).any(|s| Some(s.id) != self.editing && s.name.trim().to_lowercase() == name.to_lowercase()) {
            out.push(format!("Another {} filter is already called \u{201c}{name}\u{201d}.", self.noun()));
        }
        if let Some(p) = self.provider {
            out.extend(matching::problems(p, self.kind, &self.conditions()));
        }
        out
    }

    /// The dialog's heading: "New pull request filter", "Edit issue filter", …
    pub fn title(&self) -> String {
        let verb = if self.editing.is_some() { "Edit" } else { "New" };
        format!("{verb} {} filter", self.noun())
    }

    /// Adds a suggested label to the typed list, unless it is already there.
    pub fn add_label(&mut self, label: &str) {
        if self.label_list().iter().any(|l| l.to_lowercase() == label.to_lowercase()) {
            return;
        }
        let typed = self.labels.trim().trim_end_matches(',').trim_end();
        self.labels = if typed.is_empty() { label.to_owned() } else { format!("{typed}, {label}") };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trellis_core::forge::Builtin;

    #[test]
    fn checks_the_name_and_the_conditions() {
        let mut existing = RepoForgeFilters::default();
        let id = existing.add(FilterKind::Prs, "Mine", Builtin::MyPrs.conditions());
        existing.add(FilterKind::Issues, "Bugs", Default::default());
        let mut f = FilterForm::new(FilterKind::Prs, Some(Provider::GitHub), &Builtin::MyPrs.conditions());
        assert_eq!(f.title(), "New pull request filter");
        assert_eq!(f.problems(&existing), ["Enter a name."]);
        f.name = " mine ".into();
        assert_eq!(f.problems(&existing), ["Another pull request filter is already called \u{201c}mine\u{201d}."]);
        f.name = "Bugs".into();
        assert!(f.problems(&existing).is_empty(), "issue filters have their own names");
        f.name = "x".repeat(MAX_NAME + 1);
        assert_eq!(f.problems(&existing).len(), 1);
        let mut edit = FilterForm::edit(existing.get(id).unwrap(), Some(Provider::GitLab));
        assert_eq!((edit.title().as_str(), edit.name.as_str()), ("Edit merge request filter", "Mine"));
        assert!(edit.problems(&existing).is_empty(), "its own name is fine");
        edit.provider = Some(Provider::Bitbucket);
        edit.labels = "ui".into();
        assert!(edit.problems(&existing)[0].contains("label"), "{:?}", edit.problems(&existing));
    }

    #[test]
    fn labels_are_typed_and_suggested() {
        let mut f = FilterForm::new(FilterKind::Issues, Some(Provider::AzureDevOps), &Default::default());
        f.labels = " bug, ,UI, Bug,".into();
        assert_eq!(f.conditions().labels, ["bug", "UI"]);
        f.add_label("ui");
        assert_eq!(f.labels, " bug, ,UI, Bug,", "already there");
        f.add_label("needs design");
        assert_eq!(f.label_list(), ["bug", "UI", "needs design"]);
        assert_eq!(f.title(), "New work item filter");
    }
}
