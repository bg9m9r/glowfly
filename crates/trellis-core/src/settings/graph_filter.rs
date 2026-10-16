//! Refs hidden from or soloed in a repository's commit graph, saved per working directory.

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
