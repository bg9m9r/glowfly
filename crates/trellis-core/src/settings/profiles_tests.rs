//! Profiles in settings files, seeding and syncing from git, and the name checks.

use super::*;
use crate::forge::{Account, Connection, Provider};

fn git(name: &str, email: &str, key: Option<&str>) -> Identity {
    Identity { name: name.into(), email: email.into(), signing_key: key.map(str::to_owned) }
}

fn profile(id: u32, name: &str, email: &str) -> Profile {
    Profile {
        id: ProfileId(id),
        name: name.into(),
        author_name: name.into(),
        author_email: email.into(),
        signing_key: None,
        ..Profile::default()
    }
}

/// "Work" (1, current) and "Personal" (2).
fn two() -> Settings {
    Settings {
        profiles: vec![profile(1, "Work", "ada@work.example"), profile(2, "Personal", "ada@home.example")],
        active_profile: Some(ProfileId(1)),
        ..Settings::default()
    }
}

#[test]
fn old_files_load_without_profiles() {
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.profiles.is_empty());
    assert_eq!(s.active_profile, None);
}

#[test]
fn profiles_round_trip() {
    let mut s = two();
    s.profiles[1].signing_key = Some("ABCD".into());
    let text = serde_json::to_string(&s).unwrap();
    let back: Settings = serde_json::from_str(&text).unwrap();
    assert_eq!(back.profiles, s.profiles);
    assert_eq!(back.active_profile, Some(ProfileId(1)));
    // No key, no field.
    let one = serde_json::to_value(&s.profiles[0]).unwrap();
    assert!(one.get("signing_key").is_none(), "{one}");
    assert_eq!(one["id"], serde_json::json!(1));
}

#[test]
fn seeding_makes_default_from_git() {
    let mut s = Settings::default();
    assert!(s.seed_or_sync_profiles(&git("Ada", "ada@example.com", Some("ABCD"))));
    assert_eq!(s.profiles.len(), 1);
    let p = &s.profiles[0];
    assert_eq!((p.id, p.name.as_str(), p.author_name.as_str()), (ProfileId(1), DEFAULT_PROFILE_NAME, "Ada"));
    assert_eq!(p.author_email, "ada@example.com");
    assert_eq!(p.signing_key.as_deref(), Some("ABCD"));
    assert_eq!(s.active_profile, Some(ProfileId(1)));
    assert!(!s.seed_or_sync_profiles(&git("Ada", "ada@example.com", Some("ABCD"))), "nothing new");

    let mut s = Settings::default();
    s.seed_or_sync_profiles(&git("Ada", "ada@example.com", None));
    assert_eq!(s.profiles[0].signing_key, None);
}

#[test]
fn sync_updates_the_current_profile_only() {
    let mut s = two();
    assert!(s.seed_or_sync_profiles(&git("Ada L", "ada@new.example", Some("KEY"))));
    assert_eq!((s.profiles[0].author_name.as_str(), s.profiles[0].author_email.as_str()), ("Ada L", "ada@new.example"));
    assert_eq!(s.profiles[0].signing_key, None, "a profile without a key doesn't take git's");
    assert_eq!(s.profiles[1], profile(2, "Personal", "ada@home.example"));

    s.profiles[0].signing_key = Some("OLD".into());
    assert!(s.seed_or_sync_profiles(&git("Ada L", "ada@new.example", Some("NEW"))));
    assert_eq!(s.profiles[0].signing_key.as_deref(), Some("NEW"));
    assert!(s.seed_or_sync_profiles(&git("Ada L", "ada@new.example", None)));
    assert_eq!(s.profiles[0].signing_key, None, "git's key is gone");
    assert!(!s.seed_or_sync_profiles(&git("Ada L", "ada@new.example", None)));
}

#[test]
fn a_stale_current_id_falls_back_to_the_first() {
    let mut s = two();
    s.active_profile = Some(ProfileId(9));
    assert!(s.seed_or_sync_profiles(&git("Work", "ada@work.example", None)));
    assert_eq!(s.active_profile, Some(ProfileId(1)));
}

#[test]
fn adding_checks_the_name() {
    let mut s = two();
    assert_eq!(s.add_profile("  Client  ", Identity::default(), Vec::new()), Ok(ProfileId(3)));
    assert_eq!(s.profiles[2].name, "Client");
    assert_eq!(s.add_profile("   ", Identity::default(), Vec::new()), Err(ProfileProblem::Blank));
    assert_eq!(s.add_profile("personal", Identity::default(), Vec::new()), Err(ProfileProblem::Taken));
    assert_eq!(s.active_profile, Some(ProfileId(1)), "a new profile isn't made current");

    let mut empty = Settings::default();
    assert_eq!(empty.add_profile("First", git("Ada", "a@b", None), Vec::new()), Ok(ProfileId(1)));
    assert_eq!(empty.profiles[0].author_name, "Ada");
    // Ids grow from the largest, not the count.
    s.delete_profile(ProfileId(2)).unwrap();
    assert_eq!(s.add_profile("Again", Identity::default(), Vec::new()), Ok(ProfileId(4)));
}

#[test]
fn renaming_checks_the_name() {
    let mut s = two();
    assert_eq!(s.rename_profile(ProfileId(2), "WORK"), Err(ProfileProblem::Taken));
    assert_eq!(s.rename_profile(ProfileId(2), " "), Err(ProfileProblem::Blank));
    assert_eq!(s.rename_profile(ProfileId(2), "PERSONAL"), Ok(()), "a new casing of its own name");
    assert_eq!(s.profiles[1].name, "PERSONAL");
    assert_eq!(s.rename_profile(ProfileId(2), " Home "), Ok(()));
    assert_eq!(s.profiles[1].name, "Home");
    assert_eq!(s.rename_profile(ProfileId(7), "Other"), Err(ProfileProblem::Missing));
}

#[test]
fn deleting_refuses_the_current_profile() {
    let mut s = two();
    assert_eq!(s.delete_profile(ProfileId(1)), Err(ProfileProblem::Current));
    assert_eq!(s.delete_profile(ProfileId(7)), Err(ProfileProblem::Missing));
    assert_eq!(s.delete_profile(ProfileId(2)).map(|p| p.name), Ok("Personal".to_owned()));
    assert_eq!(s.profiles.len(), 1);
    // The last one stays, even when no profile is marked current.
    s.active_profile = None;
    assert_eq!(s.delete_profile(ProfileId(1)), Err(ProfileProblem::Current));
}

#[test]
fn adding_stores_its_accounts_but_no_preferences() {
    let mut s = two();
    let gh = Connection {
        account: Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() },
        user: "ada".into(),
        method: "access token".into(),
    };
    let empty = s.add_profile("Empty", Identity::default(), Vec::new()).unwrap();
    let copied = s.add_profile("Copied", Identity::default(), vec![gh.clone()]).unwrap();
    assert_eq!(s.profile(empty).unwrap().connections, Some(Vec::new()));
    assert_eq!(s.profile(copied).unwrap().connections, Some(vec![gh]));
    assert!(s.profile(empty).unwrap().prefs.is_none() && s.profile(copied).unwrap().prefs.is_none());
}
