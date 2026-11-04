//! Headless tests for clones from a host with a connected account (#524): an https clone sends
//! that host's SSH-named submodules over https too, and an SSH clone that fails for a key or an
//! unknown host key offers Clone over HTTPS as <user>, which runs the clone again over https.

use super::tests::state;
use super::update;
use glowfly_core::clone_ops::CloneOptions;
use glowfly_core::forge::{Account, Connection, Provider};
use glowfly_core::gitcli::args;
use glowfly_model::effect::Effect;
use glowfly_model::msg::{CloneMsg, JobMsg, Msg, PrefsMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::clone::CloneRequest;
use glowfly_model::state::prefs::PrefPage;
use glowfly_model::state::toasts::{Toast, ToastAction};
use std::time::Instant;

const REWRITES: [&str; 4] =
    ["-c", "url.https://github.com/.insteadOf=git@github.com:", "-c", "url.https://github.com/.insteadOf=ssh://git@github.com/"];

fn work() -> &'static str {
    if cfg!(windows) { "C:/work" } else { "/work" }
}

fn up(st: &mut AppState, m: Msg) -> Vec<Effect> {
    update(st, m, Instant::now())
}

/// A state with a GitHub connection for `user` at github.com.
fn connected(user: &str) -> AppState {
    let mut st = state();
    let account = Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() };
    st.settings.connections = vec![Connection { account, user: user.into(), method: "Git Credential Manager".into() }];
    st
}

fn start(st: &mut AppState, url: &str, submodules: bool) -> Vec<Vec<String>> {
    let options = CloneOptions { submodules, ..CloneOptions::default() };
    let req = CloneRequest { url: url.into(), parent: work().into(), name: "app".into(), options };
    match up(st, Msg::Clone(CloneMsg::Start(req))).as_slice() {
        [Effect::RunGit { cmds, .. }] => cmds.clone(),
        other => panic!("unexpected effects {other:?}"),
    }
}

fn fail<'a>(st: &'a mut AppState, output: &str) -> &'a Toast {
    up(st, Msg::Job(JobMsg::JobDone { ok: false, output: output.into(), undo: None }));
    st.toasts.last().expect("a toast")
}

/// The message a toast button runs: the fix behind a `LogsMsg::Fix` or `AltFix` proxy.
fn runs<'a>(st: &'a AppState, a: &'a ToastAction) -> &'a Msg {
    match *a.msg {
        Msg::Logs(glowfly_model::logs::LogsMsg::Fix(id)) => &st.logs.fix(id).expect("the fix is kept").msg,
        Msg::Logs(glowfly_model::logs::LogsMsg::AltFix(id)) => &st.logs.alt_fix(id).expect("the second fix is kept").msg,
        _ => &a.msg,
    }
}

const DENIED: &str = "Cloning into 'app'...\ngit@github.com: Permission denied (publickey).\n\
fatal: Could not read from remote repository.\n\nPlease make sure you have the correct access rights\nand the repository exists.";

#[test]
fn an_https_clone_from_a_connected_host_fetches_its_ssh_submodules_over_https() {
    let mut st = connected("ada");
    let url = "https://github.com/ada/app.git";
    let cmds = start(&mut st, url, true);
    let mut want = args(&REWRITES);
    want.extend(args(&["clone", "--recurse-submodules", "--", url, "app"]));
    assert_eq!(cmds, [want]);

    // SSH stays the user's choice: an SSH clone is left as it is.
    st.job = None;
    let ssh = "git@github.com:ada/app.git";
    assert_eq!(start(&mut st, ssh, true), [args(&["clone", "--recurse-submodules", "--", ssh, "app"])]);
    // As is a host with no connected account.
    st.job = None;
    let other = "https://gitlab.example.com/a/b.git";
    assert_eq!(start(&mut st, other, true), [args(&["clone", "--recurse-submodules", "--", other, "app"])]);
}

#[test]
fn a_denied_ssh_clone_offers_https_with_the_account() {
    let mut st = connected("ada");
    start(&mut st, "git@github.com:ada/app.git", false);
    fail(&mut st, DENIED);
    let t = st.toasts.last().unwrap();
    assert_eq!(t.text, "The server didn't accept your SSH key.");
    let https = t.action.as_ref().expect("the https offer");
    assert_eq!(https.label, "Clone over HTTPS as ada");
    let mut want = args(&REWRITES);
    want.extend(args(&["clone", "--", "https://github.com/ada/app.git", "app"]));
    assert!(matches!(runs(&st, https), Msg::Job(JobMsg::Retry(r)) if r.spec.cmds == [want.clone()]));
    let keys = t.alt.as_ref().expect("SSH keys beside it");
    assert_eq!(keys.label, "SSH keys");
    assert!(matches!(runs(&st, keys), Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh))));

    // The offer runs that clone.
    let Msg::Job(JobMsg::Retry(r)) = runs(&st, https) else { unreachable!() };
    let r = r.clone();
    match up(&mut st, Msg::Job(JobMsg::Retry(r))).as_slice() {
        [Effect::RunGit { cmds, .. }] => assert_eq!(cmds, &[want]),
        other => panic!("unexpected effects {other:?}"),
    }
}

#[test]
fn an_unknown_host_key_offers_https_alone() {
    let mut st = connected("a-very-long-user-name-indeed");
    start(&mut st, "ssh://git@github.com/ada/app.git", false);
    fail(&mut st, "Host key verification failed.\nfatal: Could not read from remote repository.");
    let t = st.toasts.last().unwrap();
    let https = t.action.as_ref().expect("the https offer");
    assert_eq!(https.label, "Clone over HTTPS as a-very-long-user-na…", "a long name is cut");
    assert!(t.alt.is_none());
}

#[test]
fn no_offer_without_a_connection_or_once_the_folder_is_made() {
    let mut st = state();
    start(&mut st, "git@github.com:ada/app.git", false);
    let t = fail(&mut st, DENIED);
    assert_eq!(t.action.as_ref().map(|a| a.label.as_str()), Some("SSH keys"));

    // The clone got as far as its submodules: the folder is there, so it isn't cloned again.
    let mut st = connected("ada");
    start(&mut st, "git@github.com:ada/app.git", true);
    let submodule = format!("Submodule 'core' (git@github.com:ada/lib.git) registered for path 'core'\n{DENIED}");
    let t = fail(&mut st, &submodule);
    assert_eq!(t.action.as_ref().map(|a| a.label.as_str()), Some("SSH keys"));
    assert!(t.alt.is_none());
}
