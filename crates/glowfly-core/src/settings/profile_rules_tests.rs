//! Folder rules in settings files, adding and removing them, and which rule a git dir gets.

use super::*;
use crate::settings::Profile;

const WORK: ProfileId = ProfileId(1);
const PERSONAL: ProfileId = ProfileId(2);

/// A path under the temporary folder: absolute on every OS, never touched.
fn at(rest: &str) -> PathBuf {
    paths::temp_dir().join("glowfly-rules").join(rest)
}

fn folder(rest: &str) -> FolderRule {
    FolderRule { path: paths::encode(&at(rest)), ..FolderRule::default() }
}

fn two() -> Settings {
    let p = |id, name: &str, email: &str| Profile { id, name: name.into(), author_email: email.into(), ..Profile::default() };
    Settings {
        profiles: vec![p(WORK, "Work", "ada@work.example"), p(PERSONAL, "Personal", "ada@home.example")],
        active_profile: Some(WORK),
        ..Settings::default()
    }
}

#[test]
fn old_files_load_without_folders() {
    let s: Settings = serde_json::from_str(r#"{"profiles": [{"id": 1, "name": "Work"}]}"#).unwrap();
    assert!(s.profiles[0].folders.is_empty());
}

#[test]
fn rules_round_trip_and_skip_empty_fields() {
    let mut s = two();
    s.add_folder_rule(WORK, folder("work")).unwrap();
    s.add_folder_rule(WORK, FolderRule { repository: true, ignore_case: true, ..folder("acme/.git") }).unwrap();
    let text = serde_json::to_string(&s).unwrap();
    let back: Settings = serde_json::from_str(&text).unwrap();
    assert_eq!(back.profiles, s.profiles);
    let work = serde_json::to_value(&s.profiles[0]).unwrap();
    assert!(work["folders"][0].get("repository").is_none(), "{work}");
    assert_eq!(work["folders"][1]["repository"], serde_json::json!(true));
    let personal = serde_json::to_value(&s.profiles[1]).unwrap();
    assert!(personal.get("folders").is_none(), "{personal}");
}

#[test]
fn adding_refuses_duplicates_other_profiles_missing_and_no_email() {
    let mut s = two();
    s.add_folder_rule(PERSONAL, folder("home")).unwrap();
    s.add_folder_rule(WORK, folder("work")).unwrap();
    let trailing = FolderRule { path: format!("{}{}", paths::encode(&at("work")), std::path::MAIN_SEPARATOR), ..FolderRule::default() };
    assert_eq!(s.add_folder_rule(WORK, trailing), Err(RuleProblem::Duplicate));
    let taken = s.add_folder_rule(WORK, folder("home")).unwrap_err();
    assert_eq!(taken, RuleProblem::Taken("Personal".into()));
    assert_eq!(taken.message(), "The profile Personal already has this folder.");
    assert_eq!(s.add_folder_rule(ProfileId(9), folder("x")), Err(RuleProblem::Missing));
    s.profiles[1].author_email = "  ".into();
    assert_eq!(s.add_folder_rule(PERSONAL, folder("other")), Err(RuleProblem::NoEmail));
    assert_eq!(s.profiles[0].folders.len(), 1);
    assert!(s.has_folder_rules());
    assert_eq!(s.rules().len(), 2);
}

#[test]
fn removing_goes_by_spelling() {
    let mut s = two();
    s.add_folder_rule(WORK, folder("work")).unwrap();
    assert!(!s.remove_folder_rule(WORK, &paths::encode(&at("elsewhere"))));
    assert!(!s.remove_folder_rule(PERSONAL, &paths::encode(&at("work"))), "another profile's rule");
    let spelled = format!("{}{}", paths::encode(&at("work")), std::path::MAIN_SEPARATOR);
    assert!(s.remove_folder_rule(WORK, &spelled));
    assert!(!s.has_folder_rules());
}

#[test]
fn the_deepest_rule_wins() {
    let rules = vec![(WORK, folder("a")), (PERSONAL, folder("a/b"))];
    assert_eq!(profile_for_git_dir(&rules, &at("a/b/repo/.git")), Some(PERSONAL));
    assert_eq!(profile_for_git_dir(&rules, &at("a/c/.git")), Some(WORK));
    assert_eq!(profile_for_git_dir(&rules, &at("a")), Some(WORK), "the folder itself");
    assert_eq!(profile_for_git_dir(&rules, &at("elsewhere/.git")), None);
}

#[test]
fn a_repository_rule_covers_its_worktrees() {
    let repo = FolderRule { repository: true, ..folder("work/acme/.git") };
    let rules = vec![(WORK, folder("work")), (PERSONAL, repo.clone())];
    assert_eq!(profile_for_git_dir(&rules, &at("work/acme/.git")), Some(PERSONAL));
    assert_eq!(profile_for_git_dir(&rules, &at("work/acme/.git/worktrees/x")), Some(PERSONAL));
    assert_eq!(shown_folder(&repo), at("work/acme"));
    assert_eq!(shown_folder(&folder("work")), at("work"));
}

#[test]
fn case_follows_the_rule() {
    let folded = FolderRule { ignore_case: true, ..folder("Work") };
    assert!(rule_matches(&folded, &at("WORK/repo/.git")));
    assert!(!rule_matches(&folder("Work"), &at("WORK/repo/.git")));
}

#[test]
fn a_shared_prefix_is_not_inside() {
    assert!(!rule_matches(&folder("work"), &at("workshop/.git")));
    let folded = FolderRule { ignore_case: true, ..folder("work") };
    assert!(!rule_matches(&folded, &at("WORKSHOP/.git")));
}
