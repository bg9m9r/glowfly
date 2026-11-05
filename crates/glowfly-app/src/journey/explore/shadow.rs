//! What a session expects to find again: the markers of edits it made (one per edit, written
//! into a file) and the summaries of commits it made, per repository. A marker is live until the
//! session discards it on purpose.

use std::collections::{BTreeMap, BTreeSet};

/// Whether `marker` occurs in `text` as a whole: `edit-9-1` is not in `edit-9-10`.
pub(super) fn contains_marker(text: &str, marker: &str) -> bool {
    text.match_indices(marker).any(|(at, _)| !text[at + marker.len()..].starts_with(|c: char| c.is_ascii_digit()))
}

pub(super) struct Shadow {
    seed: u64,
    next: u64,
    live: BTreeMap<String, BTreeSet<String>>,
    summaries: BTreeMap<String, BTreeSet<String>>,
}

impl Shadow {
    pub fn new(seed: u64) -> Self {
        Self { seed, next: 1, live: BTreeMap::new(), summaries: BTreeMap::new() }
    }

    /// A new marker, `edit-<seed>-<n>` with `n` counting from 1.
    pub fn marker(&mut self) -> String {
        let marker = format!("edit-{}-{}", self.seed, self.next);
        self.next += 1;
        marker
    }

    pub fn add(&mut self, repo: &str, marker: String) {
        self.live.entry(repo.to_owned()).or_default().insert(marker);
    }

    /// Drops every live marker of `repo` that occurs in `text` (work the session threw away).
    pub fn forget_in(&mut self, repo: &str, text: &str) {
        if let Some(set) = self.live.get_mut(repo) {
            set.retain(|m| !contains_marker(text, m));
        }
    }

    pub fn summary(&mut self, repo: &str, s: &str) {
        self.summaries.entry(repo.to_owned()).or_default().insert(s.to_owned());
    }

    pub fn live(&self, repo: &str) -> &BTreeSet<String> {
        static NONE: BTreeSet<String> = BTreeSet::new();
        self.live.get(repo).unwrap_or(&NONE)
    }

    pub fn summaries(&self, repo: &str) -> &BTreeSet<String> {
        static NONE: BTreeSet<String> = BTreeSet::new();
        self.summaries.get(repo).unwrap_or(&NONE)
    }
}

#[cfg(test)]
mod tests {
    use super::Shadow;

    #[test]
    fn markers_count_up() {
        let mut s = Shadow::new(9);
        assert_eq!(s.marker(), "edit-9-1");
        assert_eq!(s.marker(), "edit-9-2");
    }

    #[test]
    fn forget_in_drops_only_present() {
        let mut s = Shadow::new(9);
        s.add("x", "edit-9-1".into());
        s.add("x", "edit-9-2".into());
        s.forget_in("x", "x\nedit-9-2\n");
        assert_eq!(s.live("x").iter().collect::<Vec<_>>(), ["edit-9-1"]);
        assert!(s.live("other").is_empty());
    }

    #[test]
    fn a_marker_is_not_found_in_a_longer_number() {
        let mut s = Shadow::new(9);
        s.add("x", "edit-9-1".into());
        s.forget_in("x", "edit-9-10");
        assert_eq!(s.live("x").len(), 1);
    }

    #[test]
    fn summaries_are_per_repository() {
        let mut s = Shadow::new(1);
        s.summary("a", "msg 1");
        assert!(s.summaries("a").contains("msg 1"));
        assert!(s.summaries("b").is_empty());
    }
}
