//! Headless tests for auto-fetch and remotes that need a sign-in (update/sign_in.rs): pausing a
//! remote once, skipping it, the banner's fetch and "Not now", and resuming after a fetch works.

use super::notify::AUTO_FETCH;
use super::tests::{git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{CredentialMsg, JobMsg, Msg};
use crate::state::sign_in::FetchRun;
use crate::state::{AppState, RunningJob};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use trellis_core::auth_failure::AuthNeed;
use trellis_core::gitcli::args;
use trellis_core::repo::{self, LoadOptions};

const NEEDS_USERNAME: &str =
    "$ git fetch --all --prune\nfatal: could not read Username for 'https://github.com': terminal prompts disabled";

/// A repository with `remotes` (name, URL) opened in a fresh state; `skip` are skipFetchAll.
fn with_remotes(name: &str, remotes: &[(&str, &str)], skip: &[&str]) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    for (r, url) in remotes {
        git(&dir, &["remote", "add", r, url]);
    }
    for r in skip {
        git(&dir, &["config", &format!("remote.{r}.skipFetchAll"), "true"]);
    }
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.settings.auto_fetch_minutes = 1;
    (st, dir)
}

fn origin(name: &str) -> (AppState, PathBuf) {
    with_remotes(name, &[("origin", "https://github.com/me/app.git")], &[])
}

/// The git commands of the jobs in `fx`, with whether they may prompt.
fn runs(fx: &[Effect]) -> Vec<(Vec<Vec<String>>, bool)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::RunGit { cmds, prompts, .. } => Some((cmds.clone(), *prompts)),
            _ => None,
        })
        .collect()
}

/// The next auto-fetch tick's git commands.
fn tick(st: &mut AppState) -> Vec<Vec<String>> {
    let at = st.last_fetch + Duration::from_secs(61);
    let fx = update(st, Msg::Tick(at), Instant::now());
    runs(&fx).into_iter().flat_map(|(c, _)| c).collect()
}

/// The next auto-fetch tick's git commands, leaving no job running.
fn peek(st: &mut AppState) -> Vec<Vec<String>> {
    let cmds = tick(st);
    st.job = None;
    st.sign_in.fetching = None;
    cmds
}

fn done(st: &mut AppState, ok: bool, output: &str) -> Vec<Effect> {
    update(st, Msg::Job(JobMsg::JobDone { ok, output: output.into(), undo: None }), Instant::now())
}

/// Runs an auto-fetch (whatever it covers) that fails with `output`.
fn auto_fetch_fails(st: &mut AppState, output: &str) {
    assert!(!tick(st).is_empty(), "an auto-fetch started");
    done(st, false, output);
}

fn sign_in(st: &mut AppState, msg: CredentialMsg) -> Vec<Effect> {
    update(st, Msg::Credentials(msg), Instant::now())
}

fn paused(st: &AppState, dir: &Path, remote: &str) -> bool {
    st.sign_in.is_paused(dir, remote)
}

#[test]
fn a_remote_that_needs_a_sign_in_is_paused_and_skipped() {
    let (mut st, dir) = origin("sign-in-pause");
    assert_eq!(tick(&mut st), [args(&["fetch", "--all", "--prune"])]);
    let lines = st.log.len();
    done(&mut st, false, NEEDS_USERNAME);
    assert!(paused(&st, &dir, "origin"));
    let n = &st.sign_in.paused[0];
    assert_eq!((n.host.as_deref(), n.need), (Some("github.com"), AuthNeed::Https));
    let logged: Vec<&str> = st.log[lines..].iter().map(|l| l.text.as_str()).collect();
    assert!(logged.contains(&"Auto-fetch skipped origin: it needs you to sign in."), "{logged:?}");
    assert!(st.view.show_log, "said where the user sees it");

    // The same failure again (an auto-fetch already under way) adds nothing.
    let lines = st.log.len();
    st.job = Some(RunningJob {
        label: AUTO_FETCH.into(),
        workdir: st.workdir(),
        quiet: true,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
    });
    let remotes = vec![("origin".to_owned(), "https://github.com/me/app.git".to_owned())];
    st.sign_in.fetching = Some(FetchRun { workdir: dir.clone(), remotes, auto: true });
    done(&mut st, false, NEEDS_USERNAME);
    assert_eq!(st.sign_in.paused.len(), 1);
    assert!(!st.log[lines..].iter().any(|l| l.text.starts_with("Auto-fetch skipped")), "no second notice");

    assert!(peek(&mut st).is_empty(), "every remote is paused: no auto-fetch");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn auto_fetch_names_only_the_active_remotes() {
    let remotes = [("origin", "https://github.com/me/app.git"), ("upstream", "https://github.com/team/app.git")];
    let (mut st, dir) = with_remotes("sign-in-active", &remotes, &[]);
    let out = "Fetching origin\nfatal: could not read Username for 'https://github.com': terminal prompts disabled\n\
               error: could not fetch origin\nFetching upstream";
    auto_fetch_fails(&mut st, out);
    assert!(paused(&st, &dir, "origin") && !paused(&st, &dir, "upstream"));
    assert_eq!(peek(&mut st), [args(&["fetch", "--prune", "--", "upstream"])]);
    let _ = std::fs::remove_dir_all(&dir);

    let three = [("a", "https://a.example/r.git"), ("b", "https://b.example/r.git"), ("c", "https://c.example/r.git")];
    let (mut st, dir) = with_remotes("sign-in-three", &three, &["c"]);
    auto_fetch_fails(&mut st, "Fetching a\nfatal: Authentication failed for 'https://a.example/r.git/'\nFetching b");
    assert!(paused(&st, &dir, "a"));
    assert_eq!(peek(&mut st), [args(&["fetch", "--prune", "--", "b"])], "c is skipFetchAll");
    st.settings.auto_prune = false;
    git(&dir, &["remote", "add", "d", "https://d.example/r.git"]);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    assert_eq!(peek(&mut st), [args(&["fetch", "--multiple", "--", "b", "d"])]);
    st.settings.auto_prune = true;
    assert_eq!(peek(&mut st), [args(&["fetch", "--multiple", "--prune", "--", "b", "d"])]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_banners_fetch_can_prompt_and_ends_the_pause() {
    let (mut st, dir) = origin("sign-in-fetch");
    auto_fetch_fails(&mut st, NEEDS_USERNAME);
    let ignored = [
        CredentialMsg::SignInFetch { workdir: trellis_core::paths::temp_dir().join("elsewhere"), remote: "origin".into() },
        CredentialMsg::SignInFetch { workdir: dir.clone(), remote: "gone".into() },
    ];
    for msg in ignored {
        assert!(runs(&sign_in(&mut st, msg)).is_empty());
        assert!(st.job.is_none());
    }
    let fx = sign_in(&mut st, CredentialMsg::SignInFetch { workdir: dir.clone(), remote: "origin".into() });
    assert_eq!(runs(&fx), [(vec![args(&["fetch", "--prune", "--", "origin"])], true)]);
    let job = st.job.as_ref().unwrap();
    assert_eq!((job.label.as_str(), job.quiet), ("Fetch origin", false));
    done(&mut st, true, "$ git fetch --prune -- origin");
    assert!(!paused(&st, &dir, "origin"));
    assert!(st.log.iter().any(|l| l.text == "Auto-fetch includes origin again."));
    assert_eq!(peek(&mut st), [args(&["fetch", "--all", "--prune"])]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fetch_all_also_ends_the_pause() {
    let (mut st, dir) = origin("sign-in-fetch-all");
    auto_fetch_fails(&mut st, NEEDS_USERNAME);
    let fx = update(&mut st, Msg::git("Fetch all", &["fetch", "--all", "--prune"]), Instant::now());
    assert_eq!(runs(&fx).len(), 1);
    done(&mut st, false, NEEDS_USERNAME);
    assert!(paused(&st, &dir, "origin"), "a failed fetch the user started pauses nothing new and ends nothing");
    update(&mut st, Msg::git("Fetch all", &["fetch", "--all", "--prune"]), Instant::now());
    done(&mut st, true, "");
    assert!(!paused(&st, &dir, "origin"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn not_now_hides_the_line_but_keeps_the_pause() {
    let (mut st, dir) = origin("sign-in-dismiss");
    auto_fetch_fails(&mut st, NEEDS_USERNAME);
    let remotes = st.snap.as_ref().unwrap().remote_info.clone();
    assert_eq!(st.sign_in.shown(&dir, &remotes).len(), 1);
    assert!(st.sign_in.shown(&dir, &[]).is_empty(), "a removed remote has no line");
    sign_in(&mut st, CredentialMsg::DismissSignIn { workdir: dir.clone(), remote: "origin".into() });
    assert!(st.sign_in.shown(&dir, &remotes).is_empty());
    assert!(paused(&st, &dir, "origin"));
    assert!(peek(&mut st).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_sign_in_failures_pause() {
    let changed = "@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@\n\
                   @    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @\n\
                   Host key verification failed.\nfatal: Could not read from remote repository.";
    for output in ["fatal: unable to access 'https://github.com/me/app.git/': Could not resolve host: github.com", changed] {
        let (mut st, dir) = origin("sign-in-other");
        auto_fetch_fails(&mut st, output);
        assert!(st.sign_in.paused.is_empty(), "{output}");
        let _ = std::fs::remove_dir_all(&dir);
    }
    let (mut st, dir) = with_remotes("sign-in-host-key", &[("origin", "git@github.com:me/app.git")], &[]);
    auto_fetch_fails(&mut st, "Host key verification failed.\nfatal: Could not read from remote repository.");
    let n = &st.sign_in.paused[0];
    assert_eq!((n.remote.as_str(), n.need), ("origin", AuthNeed::HostKey));
    assert!(st.log.iter().any(|l| l.text == "Auto-fetch skipped origin: its SSH host key needs confirming."));
    let _ = std::fs::remove_dir_all(&dir);
}
