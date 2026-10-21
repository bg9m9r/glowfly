use super::*;
use crate::forge::filters::Who;
use std::path::PathBuf;

/// An absolute path on this platform.
fn abs(rest: &str) -> PathBuf {
    if cfg!(windows) { PathBuf::from(format!("C:\\{rest}")) } else { PathBuf::from(format!("/{rest}")) }
}

fn mine() -> Conditions {
    Builtin::MyPrs.conditions()
}

fn names(f: &RepoForgeFilters, kind: FilterKind) -> Vec<String> {
    f.saved_of(kind).map(|s| s.name.clone()).collect()
}

#[test]
fn saved_filters_are_added_changed_moved_and_removed() {
    let mut f = RepoForgeFilters::default();
    let a = f.add(FilterKind::Prs, " Mine with bugs ", mine());
    let issue = f.add(FilterKind::Issues, "Sprint", Conditions::default());
    let b = f.add(FilterKind::Prs, "Drafts", Conditions::default());
    assert!(a != b && b != issue);
    assert_eq!(names(&f, FilterKind::Prs), ["Mine with bugs", "Drafts"]);

    assert!(f.replace(a, "Mine", Conditions { assignee: Who::Me, ..Default::default() }));
    assert_eq!(f.get(a).unwrap().conditions.assignee, Who::Me);
    assert!(!f.replace(99, "x", Conditions::default()));

    // Moving only reorders filters of the same kind, past the issue filter between them.
    assert!(f.move_within_kind(b, true));
    assert_eq!(names(&f, FilterKind::Prs), ["Drafts", "Mine"]);
    assert!(!f.move_within_kind(b, true), "already first");
    assert!(!f.move_within_kind(issue, false), "the only issue filter");
    assert!(f.move_within_kind(b, false));
    assert_eq!(names(&f, FilterKind::Prs), ["Mine", "Drafts"]);

    f.set_choice(FilterKind::Prs, FilterChoice::Saved(a));
    f.set_choice(FilterKind::Issues, FilterChoice::Saved(issue));
    assert_eq!(f.conditions(FilterKind::Prs).2, "Mine");
    assert!(f.remove(a));
    assert_eq!(f.choice(FilterKind::Prs), FilterChoice::default_for(FilterKind::Prs), "removing the shown filter shows the default");
    assert_eq!(f.choice(FilterKind::Issues), FilterChoice::Saved(issue));
    // A removed id isn't handed out again.
    assert!(f.add(FilterKind::Prs, "Next", Conditions::default()) > issue.max(b));
}

#[test]
fn a_missing_or_mismatched_choice_falls_back_to_the_default() {
    let mut f = RepoForgeFilters::default();
    f.set_choice(FilterKind::Prs, FilterChoice::Saved(7));
    assert_eq!(f.choice(FilterKind::Prs), FilterChoice::Builtin(Builtin::AllPrs));
    let issue = f.add(FilterKind::Issues, "Mine", Conditions::default());
    f.set_choice(FilterKind::Prs, FilterChoice::Saved(issue));
    assert_eq!(f.choice(FilterKind::Prs), FilterChoice::Builtin(Builtin::AllPrs), "an issue filter can't show pull requests");
    f.set_choice(FilterKind::Issues, FilterChoice::Builtin(Builtin::MyPrs));
    assert_eq!(
        f.conditions(FilterKind::Issues),
        (FilterChoice::Builtin(Builtin::IssuesAssignedToMe), Builtin::IssuesAssignedToMe.conditions(), "Assigned to me".into())
    );
    // The default is stored as nothing.
    let mut g = RepoForgeFilters::default();
    g.set_choice(FilterKind::Issues, FilterChoice::Builtin(Builtin::IssuesAssignedToMe));
    assert!(g.is_empty());
}

#[test]
fn saved_per_repository_whatever_the_spelling() {
    let mut s = Settings::default();
    let real = abs("work/app");
    let mut f = RepoForgeFilters::default();
    f.add(FilterKind::Prs, "Mine", mine());
    s.set_forge_filters(&real, f.clone());
    assert_eq!(s.forge_filters_for(&real.join(".")), f);
    assert_eq!(s.forge_filters_for(&abs("work/other")), RepoForgeFilters::default());
    s.set_forge_filters(&real, RepoForgeFilters::default());
    assert!(s.repo_forge_filters.is_empty(), "empty filters drop the entry");

    // An entry saved under another name of the repository moves to its resolved key.
    let alias = abs("link/app");
    s.repo_forge_filters.insert(repo_keys::key(&alias), f.clone());
    assert!(s.adopt_repo_keys(&real, &[alias.as_path()]));
    assert_eq!(s.forge_filters_for(&real), f);
    assert_eq!(s.repo_forge_filters.keys().collect::<Vec<_>>(), [&repo_keys::key(&real)]);
}

#[test]
fn settings_without_filters_load() {
    let s: Settings = serde_json::from_str(r#"{"theme":"Light"}"#).unwrap();
    assert!(s.repo_forge_filters.is_empty());
    let mut s = Settings::default();
    let mut f = RepoForgeFilters::default();
    let id = f.add(FilterKind::Issues, "Bugs", Conditions { labels: vec!["bug".into()], ..Default::default() });
    f.set_choice(FilterKind::Issues, FilterChoice::Saved(id));
    s.set_forge_filters(&abs("r"), f.clone());
    let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.forge_filters_for(&abs("r")), f);
}
