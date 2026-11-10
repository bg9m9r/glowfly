//! Preferences > Trusted repositories without the runner (update/trust_revoke.rs): the folder
//! check the page starts, its result, revoking and forgetting decisions with their heirs, and
//! the time and source a decision keeps.

use super::tests::state;
use super::update;
use crate::effect::{Effect, LoadEffect, SaveEffect, TrustEffect};
use crate::msg::{FolderCheck, Msg, PrefsMsg, TrustMsg};
use crate::state::{AppState, PrefPage};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{RepoTrust, SavedTrust};
use std::path::PathBuf;
use std::time::Instant;

fn at(name: &str) -> PathBuf {
    glowfly_core::paths::temp_dir().join("glowfly-trust-revoke").join(name)
}

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// A state with `repo` trusted, `tree` inherited from it and `other` not trusted.
fn decided() -> (AppState, PathBuf, PathBuf, PathBuf) {
    let mut st = state();
    let (repo, tree, other) = (at("repo"), at("repo-tree"), at("other"));
    st.settings.set_trust(&repo, Some(SavedTrust::trusted(None)));
    st.settings.set_trust(&tree, Some(SavedTrust::trusted(None)));
    st.settings.stamp_trust(&tree, Some(10), Some(&repo));
    st.settings.set_trust(&other, Some(SavedTrust { trust: RepoTrust::NotTrusted, print: None }));
    (st, repo, tree, other)
}

fn saves(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Save(SaveEffect::Settings(_))))
}

fn missing(st: &AppState) -> Option<Vec<PathBuf>> {
    st.prefs.as_ref().and_then(|p| p.trust.missing.clone())
}

#[test]
fn opening_the_page_checks_every_decided_folder() {
    let (mut st, repo, tree, other) = decided();
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Trust)));
    let checked: Vec<&Vec<PathBuf>> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::Trust(TrustEffect::CheckFolders(p)) => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(checked, [&vec![other, repo, tree]]);
    assert_eq!(missing(&st), None, "no marks while the check runs");

    let mut empty = state();
    let fx = run(&mut empty, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Trust)));
    assert!(!fx.iter().any(|e| matches!(e, Effect::Trust(_))), "nothing to check");
    assert_eq!(missing(&empty), Some(Vec::new()));
}

#[test]
fn the_check_marks_missing_folders_only_while_preferences_is_open() {
    let (mut st, repo, tree, _) = decided();
    let checks = vec![
        FolderCheck { path: repo.clone(), present: true, folder: None },
        FolderCheck { path: tree.clone(), present: false, folder: None },
    ];
    run(&mut st, Msg::Trust(TrustMsg::FoldersChecked(checks.clone())));
    assert!(st.prefs.is_none());
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Trust)));
    run(&mut st, Msg::Trust(TrustMsg::FoldersChecked(checks)));
    assert_eq!(missing(&st), Some(vec![tree]));
}

#[test]
fn forget_missing_removes_the_missing_decisions_and_their_heirs() {
    let (mut st, repo, tree, other) = decided();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Trust)));
    let checks = [(&repo, false), (&tree, true), (&other, true)]
        .into_iter()
        .map(|(p, present)| FolderCheck { path: p.clone(), present, folder: None })
        .collect();
    run(&mut st, Msg::Trust(TrustMsg::FoldersChecked(checks)));
    let fx = run(&mut st, Msg::Trust(TrustMsg::ForgetMissing));
    assert!(saves(&fx));
    assert_eq!(st.settings.trust_decisions().into_iter().map(|d| d.path).collect::<Vec<_>>(), vec![other]);
    assert_eq!(missing(&st), Some(Vec::new()));
    let want = Lang::En.trn("trust.forgot_missing", 1, &[]);
    assert_eq!(st.log.last().map(|l| l.text.clone()), Some(want));
    assert!(run(&mut st, Msg::Trust(TrustMsg::ForgetMissing)).is_empty(), "nothing left to forget");
}

#[test]
fn revoke_removes_a_closed_repository_and_its_heir() {
    let (mut st, repo, tree, other) = decided();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Trust)));
    let fx = run(&mut st, Msg::Trust(TrustMsg::Revoke { real: repo.clone() }));
    assert!(saves(&fx));
    assert!(!fx.iter().any(|e| matches!(e, Effect::Load(LoadEffect::Repo { .. }))), "nothing open to reload");
    assert!(st.settings.saved_trust(&repo).is_none() && st.settings.saved_trust(&tree).is_none());
    assert!(st.settings.saved_trust(&other).is_some());
    let want = Lang::En.trn("trust.revoked_with", 1, &[("name", &"repo")]);
    assert_eq!(st.log.last().map(|l| l.text.clone()), Some(want));
    assert!(st.trust.asked.contains(&repo), "a reload doesn't ask over Preferences");

    // Forgetting a Don't trust decision says so.
    run(&mut st, Msg::Trust(TrustMsg::Revoke { real: other.clone() }));
    assert_eq!(st.log.last().map(|l| l.text.clone()), Some(Lang::En.trf("trust.forgot", &[("name", &"other")])));

    let logged = st.log.len();
    assert!(run(&mut st, Msg::Trust(TrustMsg::Revoke { real: at("unknown") })).is_empty());
    assert_eq!(st.log.len(), logged, "an unknown path does nothing");
}

#[test]
fn deciding_stamps_the_time_and_makes_it_the_users_own() {
    let (mut st, _, tree, _) = decided();
    st.cues.now = 1_772_600_767;
    run(&mut st, Msg::Trust(TrustMsg::Decide { real: tree.clone(), trusted: false }));
    let d = st.settings.trust_decisions().into_iter().find(|d| d.path == tree).unwrap();
    assert_eq!((d.trust, d.at, d.from), (RepoTrust::NotTrusted, Some(1_772_600_767), None));
}
