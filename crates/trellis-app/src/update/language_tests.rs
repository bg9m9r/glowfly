//! Headless tests for the Language setting: text built in update follows it (the commit
//! refusal, the welcome's identity check, the favorite shortcut's toast and the question about a
//! missing repository), and choosing a language on Preferences > General is saved like any
//! other edit made in place.

use super::tests::{drive, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{CommitMsg, Msg, PrefsMsg, RecentMsg, RepoMsg};
use crate::onboarding::OnboardingMsg;
use crate::state::onboarding::{Welcome, WelcomeStep};
use crate::state::{Modal, PrefPage, PrefsState};
use std::time::Instant;
use trellis_core::i18n::Lang;
use trellis_core::identity::Identity;

#[test]
fn the_commit_refusal_toast_is_in_the_chosen_language() {
    let mut st = state();
    st.settings.language = "de".into();
    st.snap = None;
    update(&mut st, Msg::Commit(CommitMsg::Commit), Instant::now());
    let text = st.toasts.last().map(|t| t.text.clone()).expect("a refusal toast");
    assert_eq!(text, Lang::De.tr("commit.blocked.no_repo"));
    assert_ne!(text, Lang::En.tr("commit.blocked.no_repo"));
}

#[test]
fn choosing_a_language_saves_it() {
    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::Language));
    update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now());
    st.settings.language = "es".into();
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now());
    let saved = fx.iter().find_map(|e| match e {
        Effect::SaveSettings(s) => Some(s.clone()),
        _ => None,
    });
    let saved = saved.expect("the settings are saved");
    assert_eq!(saved.language, "es");
    assert!(serde_json::to_string(&*saved).unwrap().contains(r#""language":"es""#));
    assert_eq!(st.settings.lang(), Lang::Es, "takes effect at once");
}

#[test]
fn the_welcome_identity_check_is_in_the_chosen_language() {
    let mut st = state();
    st.settings.language = "de".into();
    let mut w = Welcome::new();
    w.identity.seed(Identity::default());
    w.identity.email = "ada@example.com".into();
    st.onboarding.welcome = Some(w);
    update(&mut st, Msg::Onboarding(OnboardingMsg::SaveIdentity), Instant::now());
    let w = st.onboarding.welcome.as_ref().unwrap();
    assert_eq!(w.step, WelcomeStep::Identity);
    assert_eq!(w.identity.error.as_deref(), Some(Lang::De.tr("welcome.identity.name_blank")));
}

#[test]
fn the_missing_favorite_toast_is_in_the_chosen_language() {
    let mut st = state();
    st.settings.language = "es".into();
    update(&mut st, Msg::Recent(RecentMsg::OpenFavorite(5)), Instant::now());
    let text = st.toasts.last().map(|t| t.text.clone()).expect("a toast");
    assert_eq!(text, Lang::Es.trf("repos.no_favorite.toast", &[("number", &6)]));
    assert!(text.contains('6') && !text.contains('{'), "{text}");
}

#[test]
fn the_missing_repository_question_is_in_the_chosen_language() {
    let path = trellis_core::paths::temp_dir().join(format!("trellis-app-test-lang-forget-{}-gone", std::process::id()));
    let mut st = state();
    st.settings.language = "de".into();
    st.recent = vec![path.clone()];
    drive(&mut st, Msg::Repo(RepoMsg::Open(path.clone())));
    let Some(Modal::Confirm { title, body, button, .. }) = &st.modal else { panic!("asks to remove it") };
    assert_eq!(title, Lang::De.tr("repos.forget.title"));
    assert_eq!(button, Lang::De.tr("repos.remove.button"));
    assert!(!body.contains('{'), "{body}");
}

#[test]
fn the_issue_heading_follows_the_tracker_and_the_language() {
    use crate::state::ForgeState;
    use trellis_core::forge::{Account, Provider, Target};
    let gh = Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    let trello = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
    let board = Target::trello(&trello, "5f1a2b3c4d5e6f7a8b9c0d1e", "Website");
    let issues = ForgeState::new(Some(gh.clone()), "origin".into(), Some(gh.clone()));
    let cards = ForgeState::new(Some(gh), "origin".into(), Some(board));
    assert_eq!((issues.issues_header(Lang::En), cards.issues_header(Lang::En)), ("ISSUES", "CARDS"));
    assert_eq!((cards.issues_header(Lang::De), issues.issues_header(Lang::Es)), ("KARTEN", "INCIDENCIAS"));
}

#[test]
fn a_multi_selections_blocked_reasons_follow_a_language_switch_without_a_reselect() {
    use crate::msg::SelectionMsg;
    use crate::state::Selection;
    use crate::state::selection::CommitBlock;
    let (mut st, f) = super::selection_tests::fixture("lang-multi-blocked");
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    let blocked = |st: &crate::state::AppState| match &st.selection {
        Selection::Commits(m) => m.pick_blocked,
        other => panic!("not a multi-selection: {other:?}"),
    };
    assert_eq!(blocked(&st), Some(CommitBlock::PickOnHead));
    st.settings.language = "de".into();
    let why = blocked(&st).expect("still blocked").text(st.settings.lang());
    assert_eq!(why, Lang::De.tr("commits.blocked.pick_on_head"));
    assert_ne!(why, CommitBlock::PickOnHead.text(Lang::En));
    let _ = std::fs::remove_dir_all(&f.dir);
}
