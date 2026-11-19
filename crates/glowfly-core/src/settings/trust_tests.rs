//! Tests for listing, stamping and revoking trust decisions (`settings::trust`).

use super::*;
use crate::settings::{RepoTrust, SavedTrust, Settings};
use std::path::{MAIN_SEPARATOR, PathBuf};

fn at(name: &str) -> PathBuf {
    crate::paths::temp_dir().join("glowfly-trust-list").join(name)
}

fn not_trusted() -> Option<SavedTrust> {
    Some(SavedTrust { trust: RepoTrust::NotTrusted, print: None })
}

#[test]
fn stamped_decisions_list_their_time_and_source() {
    let mut s = Settings::default();
    let (repo, tree) = (at("repo"), at("repo-tree"));
    s.set_trust(&repo, Some(SavedTrust::trusted(None)));
    s.stamp_trust(&repo, Some(100), None);
    s.set_trust(&tree, Some(SavedTrust::trusted(None)));
    s.stamp_trust(&tree, Some(200), Some(&repo));
    let list = s.trust_decisions();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0], TrustDecision { path: repo.clone(), trust: RepoTrust::Trusted, at: Some(100), from: None, heirs: 1 });
    assert_eq!(list[1], TrustDecision { path: tree.clone(), trust: RepoTrust::Trusted, at: Some(200), from: Some(repo.clone()), heirs: 0 });

    s.set_trust(&tree, None);
    assert!(s.repo_trust_times.len() == 1 && s.repo_trust_from.is_empty(), "removing a decision removes its time and source");
    s.stamp_trust(&repo, None, None);
    assert!(s.repo_trust_times.is_empty());
}

#[test]
fn revoking_cascades_to_every_heir() {
    let mut s = Settings::default();
    let (repo, tree, sub, other) = (at("cascade"), at("cascade-tree"), at("cascade-tree-sub"), at("other"));
    for p in [&repo, &tree, &sub] {
        s.set_trust(p, Some(SavedTrust::trusted(None)));
    }
    s.set_trust(&other, not_trusted());
    s.stamp_trust(&tree, Some(1), Some(&repo));
    s.stamp_trust(&sub, Some(2), Some(&tree));
    assert_eq!(s.heirs_of(&repo).len(), 2);
    assert_eq!(s.heirs_of(&tree), vec![sub.clone()]);

    // Another case is another folder.
    let upper = at("Cascade");
    assert!(s.revoke_trust(&upper).is_empty());
    assert_eq!(s.repo_trust.len(), 4);

    // Another spelling of the same folder (a trailing separator) is the same decision.
    let spelled = PathBuf::from(format!("{}{MAIN_SEPARATOR}", repo.display()));
    let mut gone = s.revoke_trust(&spelled);
    gone.sort();
    let mut want = vec![repo.clone(), tree.clone(), sub.clone()];
    want.sort();
    assert_eq!(gone, want);
    assert_eq!(s.trust_decisions().iter().map(|d| d.path.clone()).collect::<Vec<_>>(), vec![other.clone()]);
    assert!(s.repo_trust_from.is_empty() && s.repo_trust_times.is_empty());
    assert!(s.revoke_trust(&repo).is_empty(), "nothing left to revoke");
}

#[test]
fn heirs_survive_cycles_and_orphans() {
    let mut s = Settings::default();
    let (a, b, gone) = (at("a"), at("b"), at("gone"));
    s.set_trust(&a, Some(SavedTrust::trusted(None)));
    s.set_trust(&b, Some(SavedTrust::trusted(None)));
    s.stamp_trust(&a, Some(1), Some(&b));
    s.stamp_trust(&b, Some(1), Some(&a));
    assert_eq!(s.heirs_of(&a), vec![b.clone()]);
    assert_eq!(s.revoke_trust(&a).len(), 2);

    // A source without a decision of its own is still named; it has no row, so no heirs.
    let c = at("c");
    s.set_trust(&c, Some(SavedTrust::trusted(None)));
    s.stamp_trust(&c, Some(1), Some(&gone));
    let list = s.trust_decisions();
    assert_eq!((list[0].from.clone(), list[0].heirs), (Some(gone.clone()), 0));
}

#[test]
fn decisions_sort_ignoring_case_first() {
    let mut s = Settings::default();
    for name in ["b", "B", "a", "C"] {
        s.set_trust(&at(name), Some(SavedTrust::trusted(None)));
    }
    let names: Vec<String> = s.trust_decisions().iter().map(|d| d.path.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["a", "B", "b", "C"]);
}

#[test]
fn old_files_load_without_times_or_sources() {
    let s: Settings = serde_json::from_str(r#"{"repo_trust": {"/w/a": "Trusted"}}"#).unwrap();
    assert!(s.repo_trust_times.is_empty() && s.repo_trust_from.is_empty());
    let list = s.trust_decisions();
    assert_eq!((list[0].at, list[0].from.clone(), list[0].heirs), (None, None, 0));

    let mut s = Settings::default();
    let (repo, tree) = (at("round"), at("round-tree"));
    s.set_trust(&tree, Some(SavedTrust::trusted(None)));
    s.stamp_trust(&tree, Some(1_700_000_000), Some(&repo));
    let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!((back.repo_trust_times, back.repo_trust_from), (s.repo_trust_times, s.repo_trust_from));
}

#[test]
fn remembered_places_live_and_end_with_the_trusted_decision() {
    let (repo, tree) = (PathBuf::from("/work/app"), PathBuf::from("/work/app-tree"));
    let places = |s: &Settings, p: &PathBuf| s.remembered_places(p);
    let (a, b) = ("//nas/team/hooks".to_owned(), "//nas/team/ca.pem".to_owned());
    let mut s = Settings::default();
    // Nothing is kept without a Trusted decision.
    s.remember_places(&repo, std::slice::from_ref(&a));
    assert!(places(&s, &repo).is_empty() && s.repo_trust_places.is_empty());
    s.set_trust(&repo, Some(SavedTrust { trust: RepoTrust::NotTrusted, print: None }));
    s.remember_places(&repo, std::slice::from_ref(&a));
    assert!(s.repo_trust_places.is_empty());

    s.set_trust(&repo, Some(SavedTrust::trusted(None)));
    s.remember_places(&repo, std::slice::from_ref(&a));
    s.remember_places(&repo, &[b.clone(), a.clone()]);
    assert_eq!(places(&s, &repo), [b.clone(), a.clone()], "each once, sorted");
    assert!(places(&s, &tree).is_empty(), "another repository has its own");
    // Trusting again (a new print) keeps them; they survive a save and load.
    s.set_trust(&repo, Some(SavedTrust::trusted(None)));
    assert_eq!(places(&s, &repo).len(), 2);
    let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.repo_trust_places, s.repo_trust_places);

    // Don't trust, and removing the decision, each end them.
    s.set_trust(&repo, Some(SavedTrust { trust: RepoTrust::NotTrusted, print: None }));
    assert!(s.repo_trust_places.is_empty());
    s.set_trust(&repo, Some(SavedTrust::trusted(None)));
    assert!(places(&s, &repo).is_empty(), "trusted afresh: nothing remembered");
    s.remember_places(&repo, std::slice::from_ref(&a));
    s.set_trust(&tree, Some(SavedTrust::trusted(None)));
    s.stamp_trust(&tree, Some(1), Some(&repo));
    s.remember_places(&tree, std::slice::from_ref(&b));
    assert_eq!(s.revoke_trust(&repo).len(), 2);
    assert!(s.repo_trust_places.is_empty(), "revoking clears its own and its heirs'");
}
