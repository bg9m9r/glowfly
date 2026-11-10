//! A TLS failure's help: a revocation check Windows couldn't finish offers Relax check for this
//! repository and for all repositories, each asking first, then sets
//! http.schannelCheckRevoke to best-effort and fetches again; an old git gets only why; and a
//! loud certificate or revocation failure has the runner read the TLS settings its Details show.

use super::auto_stash_tests::{git_cmds, up};
use super::jobs::start_job;
use super::tests::{clean_tree, drive, scratch_repo, state};
use glowfly_core::error::ReadFailed;
use glowfly_core::git_version::GitInfo;
use glowfly_core::gitcli::args;
use glowfly_core::tls_config::{Origin, Scope, Setting, TlsFailure, TlsSettings};
use glowfly_model::effect::{Effect, TlsEffect};
use glowfly_model::msg::{JobMsg, Msg, RepoMsg, TlsMsg};
use glowfly_model::state::job_spec::JobSpec;
use glowfly_model::state::logs::EntryId;
use glowfly_model::state::tls_help;
use glowfly_model::state::toasts::Toast;
use glowfly_model::state::{AppState, Modal};
use std::path::PathBuf;

const REVOCATION: &str = "$ git fetch\nfatal: unable to access 'https://git.corp.example/team/app.git/': schannel: next InitializeSecurityContext failed: CRYPT_E_NO_REVOCATION_CHECK (0x80092012) - The revocation function was unable to check revocation for the certificate.";
const CERTIFICATE: &str = "$ git fetch\nfatal: unable to access 'https://ada:tok@git.corp.example/team/app.git/': SSL certificate problem: unable to get local issuer certificate";

/// A clean repository with a Fetch running.
fn fetching(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    clean_tree(&dir);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    up(&mut st, Msg::git("Fetch", &["fetch"]));
    assert!(st.job.is_some());
    (st, dir)
}

fn fail(st: &mut AppState, output: &str) -> Vec<Effect> {
    up(st, Msg::Job(JobMsg::JobDone { ok: false, output: output.into(), undo: None }))
}

fn last_toast(st: &AppState) -> &Toast {
    st.toasts.last().expect("a toast")
}

fn buttons(t: &Toast) -> Vec<&str> {
    [&t.action, &t.alt, &t.details].into_iter().flatten().map(|a| a.label.as_str()).collect()
}

/// The settings read asked for, with its run and URL.
fn read_of(fx: &[Effect]) -> Option<(EntryId, Option<String>, TlsFailure)> {
    fx.iter().find_map(|e| match e {
        Effect::Tls(TlsEffect::ReadSettings { entry, url, failure, .. }) => Some((*entry, url.clone(), *failure)),
        _ => None,
    })
}

/// The confirmed fix's write.
fn set_of(fx: Vec<Effect>) -> (PathBuf, bool, Effect) {
    let found = fx.into_iter().find_map(|e| match &e {
        Effect::Tls(TlsEffect::SetBestEffort { cwd, global, .. }) => Some((cwd.clone(), *global, e)),
        _ => None,
    });
    found.expect("the config write")
}

#[test]
fn revocation_offers_best_effort_and_sets_it() {
    let (mut st, dir) = fetching("tls-local");
    let fx = fail(&mut st, REVOCATION);
    let t = last_toast(&st);
    assert!(t.text.starts_with("Windows couldn't finish checking whether the certificate of git.corp.example"), "{}", t.text);
    assert_eq!(buttons(t), ["Relax check for this repository", "Relax check for all repositories", "Details"]);
    let (_, url, failure) = read_of(&fx).expect("the settings are read");
    assert_eq!((url.as_deref(), failure), (Some("https://git.corp.example/team/app.git/"), TlsFailure::Revocation));

    let id = t.id;
    up(&mut st, Msg::ToastAction(id));
    let Some(Modal::Confirm { title, body, button, .. }) = &st.modal else { panic!("it asks first: {:?}", st.modal.is_some()) };
    assert_eq!((title.as_str(), button.as_str()), ("Relax the revocation check?", "Use best-effort"));
    assert!(body.contains("best-effort") && body.contains("false") && body.ends_with("Then Fetch runs again."), "{body}");
    let (cwd, global, write) = set_of(up(&mut st, Msg::ConfirmAccepted));
    assert!(!global);
    assert_eq!(cwd, st.workdir().unwrap(), "the job's own repository");

    let done = glowfly_runner::testing::execute(write).expect("the write's result");
    let fx = up(&mut st, done);
    let set = glowfly_core::testkit::git_out(&dir, &["config", "--local", "--get", "http.schannelCheckRevoke"]);
    assert_eq!(set.trim(), "best-effort");
    assert!(st.log.iter().any(|e| e.text == "Set http.schannelCheckRevoke to best-effort for this repository."), "the note is logged");
    assert_eq!(git_cmds(&fx), vec![args(&["fetch"])], "then the fetch runs again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn global_fix_is_offered_but_not_run_in_tests() {
    let (mut st, dir) = fetching("tls-global");
    fail(&mut st, REVOCATION);
    let id = last_toast(&st).id;
    up(&mut st, Msg::ToastAlt(id));
    assert!(matches!(&st.modal, Some(Modal::Confirm { body, .. }) if body.contains("global settings")), "it asks first");
    // Asserted only: running it would write the developer's own global config.
    let (cwd, global, _) = set_of(up(&mut st, Msg::ConfirmAccepted));
    assert!(global);
    assert!(!cwd.starts_with(&dir), "run from outside any repository: {}", cwd.display());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn old_git_gets_no_best_effort() {
    let (mut st, dir) = fetching("tls-old-git");
    st.git.info = Some(GitInfo { version: Some((2, 40, 1)), ..GitInfo::default() });
    fail(&mut st, REVOCATION);
    let t = last_toast(&st);
    assert_eq!(buttons(t), ["Details"]);
    assert!(t.text.contains("needs Git 2.43 or newer (you have 2.40.1)"), "{}", t.text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn certificate_details_show_the_settings() {
    let (mut st, dir) = fetching("tls-certificate");
    let fx = fail(&mut st, CERTIFICATE);
    let (entry, url, failure) = read_of(&fx).expect("the settings are read");
    assert_eq!((url.as_deref(), failure), (Some("https://git.corp.example/team/app.git/"), TlsFailure::Certificate), "no credentials");
    let at = |value: &str, scope: Scope, path: &str| {
        Some(Setting { value: value.into(), origin: Origin::File { scope: Some(scope), path: path.into() }, url: None })
    };
    let settings = TlsSettings {
        backend: at("schannel", Scope::System, "C:/Program Files/Git/etc/gitconfig"),
        ca_info: at("certs/corp.pem", Scope::Global, "gitconfig"),
        ..TlsSettings::default()
    };
    up(&mut st, Msg::Tls(TlsMsg::SettingsRead { entry, failure, result: Ok(Box::new(settings)) }));
    let note = tls_help::note(&st.logs, entry).expect("kept for the run");
    let lines = tls_help::lines(st.settings.lang(), note);
    assert!(lines.iter().any(|l| l.warn && l.text.starts_with("Git ignores http.sslCAInfo here")), "{lines:?}");
    // A read that failed says so.
    up(&mut st, Msg::Tls(TlsMsg::SettingsRead { entry, failure, result: Err(ReadFailed::Said { said: "fatal: bad".into() }) }));
    let lines = tls_help::lines(st.settings.lang(), tls_help::note(&st.logs, entry).unwrap());
    assert_eq!(lines.len(), 1);

    // A quiet job's failure reads nothing.
    let spec = JobSpec { quiet: true, ..JobSpec::new(glowfly_model::state::notify::AUTO_FETCH.into(), vec![args(&["fetch"])]) };
    start_job(&mut st, spec, &mut Vec::new());
    assert!(read_of(&fail(&mut st, CERTIFICATE)).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
