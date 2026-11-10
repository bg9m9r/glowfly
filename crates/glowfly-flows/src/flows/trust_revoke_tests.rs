//! Preferences > Trusted repositories with the runner: revoking the open repository holds git
//! there again without asking over Preferences, revoking a repository takes the worktrees made
//! from it along, and the folder check finds a deleted folder for Forget missing folders.

use super::tests::{clean_tree, drive, finish, state};
use super::trust_inherit_tests::{cleanup, create_worktree, wt_folder};
use super::trust_print_tests::trusted_repo;
use super::trust_tests::{asking, real, run};
use glowfly_core::paths::resolve;
use glowfly_core::settings::SavedTrust;
use glowfly_model::effect::{Effect, LoadEffect, SaveEffect};
use glowfly_model::msg::{Msg, PrefsMsg, TrustMsg};
use glowfly_model::state::trust::Standing;
use glowfly_model::state::trust_list::{TrustPage, rows};
use glowfly_model::state::{Modal, PrefPage};

#[test]
fn revoking_the_open_repository_holds_it_without_asking_over_preferences() {
    let (mut st, dir) = trusted_repo("revoke-open");
    // So the pull below doesn't stop at the auto-stash question.
    clean_tree(&dir);
    let real = real(&st);
    drive(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Trust)));
    let fx = run(&mut st, Msg::Trust(TrustMsg::Revoke { real: real.clone() }));
    assert!(st.settings.saved_trust(&real).is_none());
    assert!(st.held());
    assert_eq!(st.trust_standing(), Some(Standing::Undecided), "the toolbar chip shows Untrusted");
    assert!(fx.iter().any(|e| matches!(e, Effect::Save(SaveEffect::Settings(_)))), "{fx:?}");
    assert!(fx.iter().any(|e| matches!(e, Effect::Load(LoadEffect::Repo { opts, .. }) if !opts.trusted)), "{fx:?}");
    finish(&mut st, fx);
    assert!(!asking(&st), "no prompt over Preferences");

    drive(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs));
    assert!(st.modal.is_none(), "{:?}", st.modal.as_ref().map(|m| m.kind()));
    let fx = run(&mut st, Msg::git("Pull", &["pull", "--ff"]));
    assert!(fx.is_empty(), "nothing runs: {fx:?}");
    let Some(Modal::Trust(p)) = &st.modal else { panic!("the trust prompt: {:?}", st.modal.as_ref().map(|m| m.kind())) };
    assert_eq!(p.held.as_deref(), Some("Pull"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revoking_a_repository_revokes_the_worktree_made_from_it() {
    let (mut st, dir) = trusted_repo("revoke-heirs");
    let folder = wt_folder(&dir);
    create_worktree(&mut st, &folder, false);
    let (repo, tree) = (real(&st), resolve(&folder));
    let listed = rows(&st.settings, &TrustPage::default());
    let parent = listed.iter().find(|r| r.path == repo).expect("the repository's row");
    assert_eq!(parent.heirs, 1);
    let heir = listed.iter().find(|r| r.path == tree).expect("the worktree's row");
    assert!(heir.at.is_some(), "{heir:?}");
    assert_eq!(heir.from_name.as_deref(), Some(parent.name.as_str()));

    drive(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Trust)));
    drive(&mut st, Msg::Trust(TrustMsg::Revoke { real: repo.clone() }));
    assert!(st.settings.saved_trust(&tree).is_none() && st.settings.saved_trust(&repo).is_none());
    let text = st.log.iter().rev().find(|l| l.text.starts_with("Revoked trust")).map(|l| l.text.clone()).unwrap_or_default();
    assert!(text.contains("the repository passed on from it"), "{text}");
    cleanup(&dir, &folder);
}

#[test]
fn a_deleted_folder_is_found_missing_and_forgotten() {
    let base = glowfly_core::paths::temp_dir().join(format!("glowfly-revoke-missing-{}", std::process::id()));
    let (here, gone) = (base.join("here"), base.join("gone"));
    std::fs::create_dir_all(&here).unwrap();
    std::fs::create_dir_all(&gone).unwrap();
    let mut st = state();
    for p in [&here, &gone] {
        st.settings.set_trust(p, Some(SavedTrust::trusted(None)));
    }
    std::fs::remove_dir_all(&gone).unwrap();
    drive(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Trust)));
    let page = &st.prefs.as_ref().expect("Preferences").trust;
    let listed = rows(&st.settings, page);
    assert_eq!(listed.iter().map(|r| (r.name.as_str(), r.missing)).collect::<Vec<_>>(), [("gone", true), ("here", false)]);

    drive(&mut st, Msg::Trust(TrustMsg::ForgetMissing));
    assert!(st.settings.saved_trust(&gone).is_none());
    assert!(st.settings.saved_trust(&here).is_some());
    let _ = std::fs::remove_dir_all(&base);
}
