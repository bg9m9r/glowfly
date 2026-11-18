//! Refs hidden from or soloed in a repository's commit graph, saved per working directory.
//! Renames and remote removals made in Glowfly rewrite the names (`rename`, `rename_remote`,
//! `remove_remote`); a rename made elsewhere is only noticed by the next load, which drops them.

use serde::{Deserialize, Serialize};

/// Which refs a repository's graph leaves out (`hidden`) or shows exclusively (`solo`, plus
/// HEAD). Full ref names: refs/heads/x, refs/remotes/r/x, refs/tags/x. A ref is never in both
/// lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphFilter {
    pub hidden: Vec<String>,
    pub solo: Vec<String>,
}

/// A change to a `GraphFilter`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterOp {
    Hide(String),
    Show(String),
    Solo(String),
    Unsolo(String),
    /// Clear the solo set.
    StopSoloing,
    /// Clear the hidden list.
    ShowAll,
}

impl GraphFilter {
    pub fn is_empty(&self) -> bool {
        self.hidden.is_empty() && self.solo.is_empty()
    }

    pub fn is_hidden(&self, full: &str) -> bool {
        self.hidden.iter().any(|r| r == full)
    }

    pub fn is_soloed(&self, full: &str) -> bool {
        self.solo.iter().any(|r| r == full)
    }

    /// Applies `op`; returns whether anything changed. Hiding a soloed ref stops soloing it and
    /// soloing a hidden ref shows it, so the lists stay disjoint.
    pub fn apply(&mut self, op: FilterOp) -> bool {
        match op {
            FilterOp::Hide(r) => {
                if self.is_hidden(&r) {
                    return false;
                }
                self.solo.retain(|s| *s != r);
                self.hidden.push(r);
                true
            }
            FilterOp::Show(r) => remove(&mut self.hidden, &r),
            FilterOp::Solo(r) => {
                if self.is_soloed(&r) {
                    return false;
                }
                self.hidden.retain(|h| *h != r);
                self.solo.push(r);
                true
            }
            FilterOp::Unsolo(r) => remove(&mut self.solo, &r),
            FilterOp::StopSoloing => !std::mem::take(&mut self.solo).is_empty(),
            FilterOp::ShowAll => !std::mem::take(&mut self.hidden).is_empty(),
        }
    }

    /// Keeps only refs for which `exists` holds. Returns (hidden changed, solo changed).
    pub fn prune(&mut self, exists: impl Fn(&str) -> bool) -> (bool, bool) {
        let (h, s) = (self.hidden.len(), self.solo.len());
        self.hidden.retain(|r| exists(r));
        self.solo.retain(|r| exists(r));
        (self.hidden.len() != h, self.solo.len() != s)
    }

    /// Moves the entry `old` to the name `new` in whichever list holds it, keeping its place (a
    /// renamed branch stays hidden or shown on its own). An entry already saved as `new` gives
    /// way to the moved one, so the lists stay disjoint and free of duplicates. Names are
    /// compared exactly; nothing happens for equal or empty names or an unlisted `old`. Returns
    /// (hidden changed, solo changed).
    pub fn rename(&mut self, old: &str, new: &str) -> (bool, bool) {
        if old.is_empty() || new.is_empty() || old == new {
            return (false, false);
        }
        self.remap(|r| (r == old).then(|| new.to_owned()))
    }

    /// Moves every entry under `refs/remotes/{old}/` to `refs/remotes/{new}/`, as git moves
    /// every ref under that prefix when a remote is renamed (a remote whose name nests under
    /// `old` follows too; `refs/remotes/{old}2/` does not). Same collision rule and result as
    /// `rename`.
    pub fn rename_remote(&mut self, old: &str, new: &str) -> (bool, bool) {
        if old.is_empty() || new.is_empty() || old == new {
            return (false, false);
        }
        let from = format!("refs/remotes/{old}/");
        let to = format!("refs/remotes/{new}/");
        self.remap(|r| r.strip_prefix(&from).map(|rest| format!("{to}{rest}")))
    }

    /// Drops every entry under `refs/remotes/{name}/`, as git removes the remote's
    /// remote-tracking refs. Returns how many hidden and how many solo entries went.
    pub fn remove_remote(&mut self, name: &str) -> (usize, usize) {
        if name.is_empty() {
            return (0, 0);
        }
        let prefix = format!("refs/remotes/{name}/");
        let (h, s) = (self.hidden.len(), self.solo.len());
        self.hidden.retain(|r| !r.starts_with(&prefix));
        self.solo.retain(|r| !r.starts_with(&prefix));
        (h - self.hidden.len(), s - self.solo.len())
    }

    /// Replaces each entry `to` maps to a new name (one pass over the original lists). A moved
    /// entry wins over an unmoved one of the same name, and appears once.
    fn remap(&mut self, to: impl Fn(&str) -> Option<String>) -> (bool, bool) {
        let landed: Vec<String> = self.hidden.iter().chain(&self.solo).filter_map(|r| to(r)).collect();
        let place = |list: &mut Vec<String>| {
            let mut out: Vec<String> = Vec::with_capacity(list.len());
            for r in list.iter() {
                let name = match to(r) {
                    Some(n) => n,
                    None if landed.contains(r) => continue,
                    None => r.clone(),
                };
                if !out.contains(&name) {
                    out.push(name);
                }
            }
            std::mem::replace(list, out)
        };
        let (h, s) = (place(&mut self.hidden), place(&mut self.solo));
        (h != self.hidden, s != self.solo)
    }

    /// A full ref name without refs/heads/, refs/remotes/ or refs/tags/, for labels.
    pub fn short(full: &str) -> &str {
        full.strip_prefix("refs/heads/")
            .or_else(|| full.strip_prefix("refs/remotes/"))
            .or_else(|| full.strip_prefix("refs/tags/"))
            .unwrap_or(full)
    }
}

fn remove(list: &mut Vec<String>, r: &str) -> bool {
    let n = list.len();
    list.retain(|x| x != r);
    list.len() != n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    use std::path::Path;

    fn s(r: &str) -> String {
        r.to_owned()
    }

    #[test]
    fn apply_ops_are_exclusive_and_idempotent() {
        let mut f = GraphFilter::default();
        assert!(f.apply(FilterOp::Hide(s("refs/heads/a"))));
        assert!(!f.apply(FilterOp::Hide(s("refs/heads/a"))));
        assert!(f.apply(FilterOp::Solo(s("refs/heads/a"))));
        assert_eq!(f, GraphFilter { hidden: vec![], solo: vec![s("refs/heads/a")] });
        assert!(!f.apply(FilterOp::Solo(s("refs/heads/a"))));
        assert!(f.apply(FilterOp::Hide(s("refs/heads/a"))));
        assert_eq!(f, GraphFilter { hidden: vec![s("refs/heads/a")], solo: vec![] });
        assert!(!f.apply(FilterOp::Unsolo(s("refs/heads/a"))));
        assert!(f.apply(FilterOp::Show(s("refs/heads/a"))));
        assert!(!f.apply(FilterOp::Show(s("refs/heads/a"))));
        assert!(f.is_empty());
        assert!(!f.apply(FilterOp::StopSoloing));
        assert!(!f.apply(FilterOp::ShowAll));

        // Soloing is additive; stopping clears every solo at once.
        assert!(f.apply(FilterOp::Solo(s("refs/heads/a"))));
        assert!(f.apply(FilterOp::Solo(s("refs/remotes/origin/b"))));
        assert!(f.is_soloed("refs/remotes/origin/b") && !f.is_hidden("refs/remotes/origin/b"));
        assert!(f.apply(FilterOp::Unsolo(s("refs/heads/a"))));
        assert!(f.apply(FilterOp::StopSoloing));
        assert!(f.apply(FilterOp::Hide(s("refs/heads/c"))));
        assert!(f.apply(FilterOp::ShowAll));
        assert!(f.is_empty());
        assert_eq!(GraphFilter::short("refs/remotes/origin/b"), "origin/b");
        assert_eq!(GraphFilter::short("refs/heads/a/b"), "a/b");
        assert_eq!(GraphFilter::short("refs/tags/v1.0"), "v1.0");
    }

    #[test]
    fn prune_drops_missing_refs() {
        let mut f = GraphFilter { hidden: vec![s("refs/heads/gone"), s("refs/heads/main")], solo: vec![s("refs/remotes/origin/gone")] };
        let exists = |r: &str| r == "refs/heads/main";
        assert_eq!(f.prune(exists), (true, true));
        assert_eq!(f, GraphFilter { hidden: vec![s("refs/heads/main")], solo: vec![] });
        assert_eq!(f.prune(exists), (false, false));
    }

    fn lists(hidden: &[&str], solo: &[&str]) -> GraphFilter {
        GraphFilter { hidden: hidden.iter().map(|r| s(r)).collect(), solo: solo.iter().map(|r| s(r)).collect() }
    }

    #[test]
    fn rename_moves_an_entry_in_place() {
        let mut f = lists(&["a", "old", "b"], &[]);
        assert_eq!(f.rename("old", "new"), (true, false));
        assert_eq!(f, lists(&["a", "new", "b"], &[]));
        let mut f = lists(&["a"], &["old"]);
        assert_eq!(f.rename("old", "new"), (false, true));
        assert_eq!(f, lists(&["a"], &["new"]));
        let before = lists(&["a", "old"], &["c"]);
        for (old, new) in [("old", "old"), ("gone", "new"), ("", "new"), ("old", "")] {
            let mut f = before.clone();
            assert_eq!(f.rename(old, new), (false, false), "{old:?} -> {new:?}");
            assert_eq!(f, before);
        }
    }

    #[test]
    fn rename_wins_over_an_entry_under_the_new_name() {
        let mut f = lists(&["old"], &["new"]);
        assert_eq!(f.rename("old", "new"), (true, true));
        assert_eq!(f, lists(&["new"], &[]));
        let mut f = lists(&["new", "old"], &[]);
        assert_eq!(f.rename("old", "new"), (true, false));
        assert_eq!(f, lists(&["new"], &[]));
        let mut f = lists(&["old", "new"], &[]);
        assert_eq!(f.rename("old", "new"), (true, false));
        assert_eq!(f, lists(&["new"], &[]));
    }

    #[test]
    fn rename_remote_rewrites_the_prefix_only() {
        let mut f = lists(
            &["refs/remotes/origin/x", "refs/remotes/origin2/x", "refs/heads/origin/x"],
            &["refs/remotes/origin/a/b", "refs/tags/origin/x"],
        );
        assert_eq!(f.rename_remote("origin", "upstream"), (true, true));
        assert_eq!(
            f,
            lists(
                &["refs/remotes/upstream/x", "refs/remotes/origin2/x", "refs/heads/origin/x"],
                &["refs/remotes/upstream/a/b", "refs/tags/origin/x"]
            )
        );
        assert_eq!(f.rename_remote("origin", "upstream"), (false, false), "nothing left under the old name");

        let mut f = lists(&["refs/remotes/team/origin/x"], &[]);
        assert_eq!(f.rename_remote("team/origin", "origin"), (true, false));
        assert_eq!(f, lists(&["refs/remotes/origin/x"], &[]));

        // Each entry is mapped once: a result under the new prefix isn't matched again.
        let mut f = lists(&["refs/remotes/a/b/x", "refs/remotes/b/y"], &[]);
        assert_eq!(f.rename_remote("a", "b"), (true, false));
        assert_eq!(f, lists(&["refs/remotes/b/b/x", "refs/remotes/b/y"], &[]));

        // An entry under a nested remote name follows, as git moves every ref under the prefix.
        let mut f = lists(&["refs/remotes/a/b/x"], &[]);
        assert_eq!(f.rename_remote("a", "c"), (true, false));
        assert_eq!(f, lists(&["refs/remotes/c/b/x"], &[]));

        let mut f = lists(&["refs/remotes/origin/x"], &[]);
        assert_eq!(f.rename_remote("origin", "origin"), (false, false));
        assert_eq!(f.rename_remote("", "z"), (false, false));
        assert_eq!(f.rename_remote("origin", ""), (false, false));
        assert_eq!(f, lists(&["refs/remotes/origin/x"], &[]));
    }

    #[test]
    fn remove_remote_drops_and_counts() {
        let mut f = lists(
            &["refs/remotes/origin/a", "refs/heads/origin/a", "refs/remotes/other/a", "refs/remotes/origin/b"],
            &["refs/remotes/origin/c", "refs/tags/origin/a"],
        );
        assert_eq!(f.remove_remote("origin"), (2, 1));
        assert_eq!(f, lists(&["refs/heads/origin/a", "refs/remotes/other/a"], &["refs/tags/origin/a"]));
        assert_eq!(f.remove_remote("origin"), (0, 0));
        assert_eq!(f.remove_remote(""), (0, 0));
        assert_eq!(f.remove_remote("oth"), (0, 0), "the prefix ends in a slash");
        assert_eq!(f.hidden.len(), 2);
    }

    #[test]
    fn settings_persist_graph_filters_per_repo() {
        let mut st = Settings::default();
        let f = GraphFilter { hidden: vec![s("refs/heads/a")], solo: vec![s("refs/remotes/origin/b")] };
        st.set_graph_filter(Path::new("C:/a"), f.clone());
        assert_eq!(st.graph_filter_for(Path::new("C:/a")), f);
        assert_eq!(st.graph_filter_for(Path::new("C:/b")), GraphFilter::default());

        let back: Settings = serde_json::from_str(&serde_json::to_string(&st).unwrap()).unwrap();
        assert_eq!(back.graph_filter_for(Path::new("C:/a")), f);

        st.set_graph_filter(Path::new("C:/a"), GraphFilter::default());
        assert!(st.graph_filters.is_empty(), "empty filters are not stored");

        // Files written before graph filters existed load with none.
        let old: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
        assert!(old.graph_filters.is_empty());
    }
}
