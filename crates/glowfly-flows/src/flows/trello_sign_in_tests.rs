//! Sign in with Trello in an open repository: the token Trello hands back connects the account,
//! and the page's offer makes Trello the repository's tracker, loading its boards.

use super::tests::{drive, scratch_repo, state};
use super::update;
use glowfly_core::forge::{Account, Provider};
use glowfly_core::settings::{RepoTrust, TrackerChoice};
use glowfly_model::effect::{AccountEffect, Effect};
use glowfly_model::msg::{ForgeMsg, Msg, PrefsMsg, RepoMsg};
use glowfly_model::state::trello::SignInStep;
use glowfly_model::state::{AppState, PrefPage};
use std::time::Instant;

const KEY: &str = "0123456789abcdef0123456789abcdef";
const TOKEN: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn acct() -> Account {
    Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() }
}

fn run(st: &mut AppState, msg: ForgeMsg, now: Instant) -> Vec<Effect> {
    update(st, Msg::Forge(msg), now)
}

#[test]
fn signing_in_connects_trello_and_offers_it_to_the_repository() {
    let dir = scratch_repo("trello-sign-in");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir)));
    let real = st.snap.as_ref().expect("opened").real_workdir.clone();
    glowfly_model::state::trust::record(&mut st.settings, &real, RepoTrust::Trusted);
    update(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)), Instant::now());
    st.prefs.as_mut().unwrap().trello_sign_in.key = Some(KEY.into());

    let at = Instant::now();
    let fx = run(&mut st, ForgeMsg::StartTrelloSignIn, at);
    assert!(fx.iter().any(|e| matches!(e, Effect::Account(AccountEffect::TrelloSignIn { .. }))));
    let fx = run(&mut st, ForgeMsg::TrelloSignInEnded { started: at, key: KEY.into(), result: Ok(TOKEN.into()) }, at);
    let Some(Effect::Account(AccountEffect::Connect { account, user, token, .. })) =
        fx.into_iter().find(|e| matches!(e, Effect::Account(AccountEffect::Connect { .. })))
    else {
        panic!("the token is verified");
    };
    assert_eq!((&account, user.as_deref(), token.as_deref()), (&acct(), Some(KEY), Some(TOKEN)));
    let token = Some((user, token.unwrap()));
    let fx = run(&mut st, ForgeMsg::Connected { account, method: "access token".into(), token, result: Ok("Ada".into()) }, at);
    assert!(fx.iter().any(|e| matches!(e, Effect::Account(AccountEffect::SaveToken { token, .. }) if token == TOKEN)), "saved");
    assert!(st.settings.connections.iter().any(|c| c.account == acct()));
    let p = st.prefs.as_ref().unwrap();
    assert!(p.trello_sign_in.offer_tracker && p.trello_sign_in.step == SignInStep::Idle);

    // "Use Trello for this repository": all boards, and the Board picker's list loads.
    let all = TrackerChoice::Trello { account: acct(), board: String::new(), board_name: String::new() };
    let fx = run(&mut st, ForgeMsg::SetTracker(all.clone()), at);
    assert_eq!(st.settings.tracker_for(&real), all);
    assert!(fx.iter().any(|e| matches!(e, Effect::Account(AccountEffect::TrelloBoards { account, .. }) if *account == acct())));
    assert!(st.prefs.as_ref().unwrap().trello_boards.loading);
}
