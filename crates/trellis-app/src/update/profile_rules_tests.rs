//! Headless tests for profile folder rules: adding (picked folder or the open repository) after
//! the runner's check, removing, the one-at-a-time include syncs (waiting for a switch, and holding one),
//! deleting and editing a ruled profile, the identity reads with their one repair, and Use
//! <profile> here behind its confirmation. No effect runs: syncs write the global git config.

use super::profiles_tests::{PERSONAL, WORK, profile, run, saves, two_profiles};
use super::tests::{finish_keeping, scratch_repo};
use crate::effect::Effect;
use crate::msg::{FolderPurpose, Msg, PrefsMsg, RepoMsg};
use crate::profile::{IdentityField, ProfileEffect, ProfileMsg};
use crate::state::{AppState, Modal};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::paths;
use trellis_core::repo::LoadOptions;
use trellis_core::repo_identity::{IdentitySource, RepoIdentity};
use trellis_core::settings::FolderRule;

fn rule(rest: &str) -> FolderRule {
    FolderRule { path: paths::encode(&paths::temp_dir().join("trellis-rules-app").join(rest)), ..FolderRule::default() }
}

/// The SyncIncludes effects in `fx`, by `place`.
fn syncs(fx: &[Effect]) -> Vec<bool> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Profile(ProfileEffect::SyncIncludes { place, .. }) => Some(*place),
            _ => None,
        })
        .collect()
}

fn reads(fx: &[Effect]) -> Vec<(PathBuf, usize)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Profile(ProfileEffect::ReadIdentity { git_dir, rules, .. }) => Some((git_dir.clone(), rules.len())),
            _ => None,
        })
        .collect()
}

fn rules_error(st: &AppState) -> Option<&str> {
    st.prefs.as_ref().and_then(|p| p.profiles.rules_error.as_deref())
}

/// `two_profiles` with a repository loaded as the active tab's snapshot.
fn with_repo(name: &str) -> (AppState, PathBuf) {
    let mut st = two_profiles();
    let dir = scratch_repo(name);
    st.snap = Some(trellis_core::repo::load(&dir, &LoadOptions::all(50)).unwrap());
    (st, dir)
}

fn git_dir(st: &AppState) -> PathBuf {
    st.snap.as_ref().unwrap().git_dir.clone()
}

fn identity(source: IdentitySource, expected: Option<trellis_core::settings::ProfileId>) -> RepoIdentity {
    RepoIdentity { name: "Ada".into(), email: "x@y.example".into(), source, expected, ..RepoIdentity::default() }
}

#[test]
fn adding_asks_for_a_folder_or_uses_the_open_repository() {
    let mut st = two_profiles();
    let fx = profile(&mut st, ProfileMsg::AddFolder(WORK));
    assert!(matches!(&fx[..], [Effect::PickFolder { purpose: FolderPurpose::ProfileFolder(WORK), .. }]), "{fx:?}");
    let picked = paths::temp_dir().join("work");
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::FolderPicked { purpose: FolderPurpose::ProfileFolder(WORK), path: Some(picked.clone()) }));
    match &fx[..] {
        [Effect::Profile(ProfileEffect::CheckFolder { id: WORK, path, repository: false })] => assert_eq!(path, &picked),
        other => panic!("expected a folder check, got {other:?}"),
    }
    assert!(profile(&mut st, ProfileMsg::AddThisRepo(WORK)).is_empty(), "no repository open");
    let (mut st, dir) = with_repo("rules-this-repo");
    let common = st.snap.as_ref().unwrap().common.dir.clone();
    let fx = profile(&mut st, ProfileMsg::AddThisRepo(WORK));
    match &fx[..] {
        [Effect::Profile(ProfileEffect::CheckFolder { id: WORK, path, repository: true })] => assert_eq!(path, &common),
        other => panic!("expected a folder check, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_checked_folder_is_stored_and_synced() {
    let mut st = two_profiles();
    let fx = profile(&mut st, ProfileMsg::FolderChecked { id: WORK, result: Ok(rule("work")) });
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.settings.profile(WORK).unwrap().folders, vec![rule("work")]);
    match fx.iter().find(|e| matches!(e, Effect::Profile(ProfileEffect::SyncIncludes { .. }))) {
        Some(Effect::Profile(ProfileEffect::SyncIncludes { plan, place: true, .. })) => {
            assert_eq!(plan.entries.len(), 1);
            assert_eq!(plan.entries[0].1, WORK);
            assert_eq!(plan.files.len(), 1);
        }
        other => panic!("expected a placing sync, got {other:?}"),
    }
    assert!(st.includes.running);
    assert_eq!(rules_error(&st), None);
}

#[test]
fn refusals_fill_the_error_line() {
    let mut st = two_profiles();
    st.settings.add_folder_rule(PERSONAL, rule("home")).unwrap();
    let fx = profile(&mut st, ProfileMsg::FolderChecked { id: WORK, result: Ok(rule("home")) });
    assert!(fx.is_empty(), "no save, no sync: {fx:?}");
    assert_eq!(rules_error(&st), Some("The profile Personal already has this folder."));
    profile(&mut st, ProfileMsg::FolderChecked { id: WORK, result: Err("That folder doesn't exist.".into()) });
    assert_eq!(rules_error(&st), Some("That folder doesn't exist."));
}

#[test]
fn syncs_run_one_at_a_time() {
    let mut st = two_profiles();
    let fx = profile(&mut st, ProfileMsg::FolderChecked { id: WORK, result: Ok(rule("a")) });
    assert_eq!(syncs(&fx), [true]);
    // An identity edit while it runs waits, then a removal widens what it waits for.
    let mut fx = Vec::new();
    super::profiles::set_identity(&mut st, WORK, IdentityField::AuthorName, "Ada Lovelace", &mut fx);
    assert!(syncs(&fx).is_empty());
    assert_eq!(st.includes.again, Some(false));
    let fx = profile(&mut st, ProfileMsg::FolderChecked { id: WORK, result: Ok(rule("b")) });
    assert!(syncs(&fx).is_empty());
    assert_eq!(st.includes.again, Some(true));
    let fx = profile(&mut st, ProfileMsg::IncludesSynced(Ok(())));
    assert_eq!(syncs(&fx), [true], "the deferred sync, placing");
    assert_eq!(st.includes.again, None);
    // A failure is told.
    let fx = profile(&mut st, ProfileMsg::IncludesSynced(Err("Could not update: locked".into())));
    assert!(syncs(&fx).is_empty() && !st.includes.running);
    assert!(st.toasts.iter().any(|t| !t.ok && t.text.contains("locked")));
}

#[test]
fn a_switch_holds_the_sync_until_it_ends() {
    let mut st = two_profiles();
    profile(&mut st, ProfileMsg::Switch(PERSONAL));
    let fx = profile(&mut st, ProfileMsg::FolderChecked { id: WORK, result: Ok(rule("a")) });
    assert!(syncs(&fx).is_empty());
    let fx = profile(&mut st, ProfileMsg::Applied { id: PERSONAL, result: Ok(super::profiles_tests::values("Ada L", "ada@home.example")) });
    assert_eq!(syncs(&fx), [true]);
}

#[test]
fn a_sync_holds_the_switch_until_it_ends() {
    let applies = |fx: &[Effect]| fx.iter().filter(|e| matches!(e, Effect::Profile(ProfileEffect::Apply { id: PERSONAL, .. }))).count();
    let mut st = two_profiles();
    let fx = profile(&mut st, ProfileMsg::FolderChecked { id: WORK, result: Ok(rule("a")) });
    assert_eq!(syncs(&fx), [true]);
    // Chosen while the placing sync writes the global config: busy, but nothing written yet.
    let fx = profile(&mut st, ProfileMsg::Switch(PERSONAL));
    assert_eq!(applies(&fx), 0, "{fx:?}");
    assert_eq!(st.profile_switch, Some(PERSONAL));
    let fx = profile(&mut st, ProfileMsg::Switch(WORK));
    assert_eq!(applies(&fx), 0, "a second switch is refused while the first waits");
    // A sync asked for meanwhile waits for the switch.
    let fx = profile(&mut st, ProfileMsg::FolderChecked { id: WORK, result: Ok(rule("b")) });
    assert!(syncs(&fx).is_empty());
    let fx = profile(&mut st, ProfileMsg::IncludesSynced(Ok(())));
    assert_eq!(applies(&fx), 1, "{fx:?}");
    assert!(syncs(&fx).is_empty(), "not alongside the switch: {fx:?}");
    assert!(!st.includes.running && st.includes.held_switch.is_none());
    let fx = profile(&mut st, ProfileMsg::Applied { id: PERSONAL, result: Ok(super::profiles_tests::values("Ada L", "ada@home.example")) });
    assert_eq!(syncs(&fx), [true]);
    assert_eq!(st.settings.active_profile, Some(PERSONAL));
}

#[test]
fn removing_deleting_and_editing_a_ruled_profile() {
    let mut st = two_profiles();
    st.settings.add_folder_rule(PERSONAL, rule("home")).unwrap();
    st.settings.add_folder_rule(PERSONAL, rule("home2")).unwrap();
    // Editing the identity of a ruled profile that isn't current rewrites its file only.
    let mut fx = Vec::new();
    super::profiles::set_identity(&mut st, PERSONAL, IdentityField::AuthorEmail, "ada@new.example", &mut fx);
    assert_eq!(syncs(&fx), [false]);
    assert!(!fx.iter().any(|e| matches!(e, Effect::SetGitConfig { .. })), "{fx:?}");
    profile(&mut st, ProfileMsg::IncludesSynced(Ok(())));
    // An unruled profile's edit syncs nothing.
    let mut fx = Vec::new();
    super::profiles::set_identity(&mut st, WORK, IdentityField::AuthorName, "Ada W", &mut fx);
    assert!(syncs(&fx).is_empty());
    let fx = profile(&mut st, ProfileMsg::RemoveFolder { id: PERSONAL, path: rule("home2").path });
    assert_eq!((saves(&fx), syncs(&fx)), (1, vec![true]));
    profile(&mut st, ProfileMsg::IncludesSynced(Ok(())));
    profile(&mut st, ProfileMsg::AskDelete(PERSONAL));
    match &st.modal {
        Some(Modal::ConfirmDanger { body, .. }) => assert!(body.contains("folder rule is removed from your Git config"), "{body}"),
        _ => panic!("expected the confirmation"),
    }
    let fx = profile(&mut st, ProfileMsg::Delete(PERSONAL));
    assert_eq!(syncs(&fx), [true]);
}

#[test]
fn identity_is_read_on_open_focus_and_closing_preferences() {
    let mut st = two_profiles();
    st.settings.add_folder_rule(WORK, rule("work")).unwrap();
    let dir = scratch_repo("rules-open");
    let fx = run(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let kept = finish_keeping(&mut st, fx);
    let git_dir = git_dir(&st);
    assert_eq!(reads(&kept), [(git_dir.clone(), 1)]);
    let fx = run(&mut st, Msg::Repo(RepoMsg::WindowFocused));
    assert_eq!(reads(&fx), [(git_dir.clone(), 1)]);
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs));
    assert_eq!(reads(&fx), [(git_dir, 1)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_rule_git_didnt_apply_is_placed_again_once() {
    let (mut st, dir) = with_repo("rules-heal");
    st.settings.add_folder_rule(WORK, rule("work")).unwrap();
    let gd = git_dir(&st);
    let read = ProfileMsg::IdentityRead { git_dir: gd.clone(), result: Ok(identity(IdentitySource::Elsewhere, Some(WORK))) };
    let fx = profile(&mut st, read.clone());
    assert_eq!(syncs(&fx), [true]);
    assert!(st.includes.healed.contains(&gd));
    let fx = profile(&mut st, ProfileMsg::IncludesSynced(Ok(())));
    assert_eq!(reads(&fx).len(), 1, "read again after the sync");
    assert!(syncs(&profile(&mut st, read)).is_empty(), "only once");
    let now = &st.identities[&gd];
    assert!(crate::state::repo_identity::not_applied(now, &st.settings, true));
    // The repository's own identity is the user's: reported, never removed unasked.
    let own = RepoIdentity { local_email: Some("x@y.example".into()), ..identity(IdentitySource::Repository, Some(WORK)) };
    let fx = profile(&mut st, ProfileMsg::IdentityRead { git_dir: gd.clone(), result: Ok(own) });
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(crate::state::repo_identity::shadowed(&st.identities[&gd], &st.settings), Some(WORK));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn using_a_profile_here_asks_first() {
    let (mut st, dir) = with_repo("rules-use-here");
    let gd = git_dir(&st);
    let own = RepoIdentity {
        local_name: Some("Me".into()),
        local_email: Some("x@y.example".into()),
        ..identity(IdentitySource::Repository, Some(WORK))
    };
    st.identities.insert(gd.clone(), own);
    let fx = profile(&mut st, ProfileMsg::AskUseHere(WORK));
    assert!(fx.is_empty());
    match &st.modal {
        Some(Modal::ConfirmDanger { title, body, button, then, .. }) => {
            assert_eq!(title, "Use profile here");
            assert!(body.contains("(Me <x@y.example>)") && body.contains("profile Work"), "{body}");
            assert_eq!(button, "Remove and use Work");
            assert!(matches!(&**then, Msg::Profile(ProfileMsg::UseHere { id: WORK, git_dir }) if *git_dir == gd));
        }
        _ => panic!("expected the confirmation"),
    }
    let elsewhere = profile(&mut st, ProfileMsg::UseHere { id: WORK, git_dir: Path::new("other").to_path_buf() });
    assert!(elsewhere.is_empty(), "another repository's confirmation does nothing");
    let fx = profile(&mut st, ProfileMsg::UseHere { id: WORK, git_dir: gd.clone() });
    assert!(matches!(&fx[..], [Effect::Profile(ProfileEffect::ClearLocalIdentity { git_dir, .. })] if *git_dir == gd), "{fx:?}");
    let cleared = identity(IdentitySource::Profile(WORK), Some(WORK));
    profile(&mut st, ProfileMsg::LocalCleared { git_dir: gd.clone(), result: Ok(cleared.clone()) });
    assert_eq!(st.identities[&gd], cleared);
    let now = Instant::now();
    super::update(&mut st, Msg::Profile(ProfileMsg::LocalCleared { git_dir: gd, result: Err("locked".into()) }), now);
    assert!(st.toasts.iter().any(|t| !t.ok && t.text == "locked") && st.log.iter().any(|l| l.text == "locked"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rule_results_are_dropped_for_an_inactive_tab() {
    let msgs = [
        ProfileMsg::AddFolder(WORK),
        ProfileMsg::AddThisRepo(WORK),
        ProfileMsg::RemoveFolder { id: WORK, path: String::new() },
        ProfileMsg::AskUseHere(WORK),
        ProfileMsg::UseHere { id: WORK, git_dir: PathBuf::new() },
        ProfileMsg::FolderChecked { id: WORK, result: Err(String::new()) },
        ProfileMsg::IncludesSynced(Ok(())),
        ProfileMsg::IdentityRead { git_dir: PathBuf::new(), result: Err(String::new()) },
        ProfileMsg::LocalCleared { git_dir: PathBuf::new(), result: Err(String::new()) },
    ];
    for m in msgs {
        assert!(m.on_inactive().is_none());
    }
}
