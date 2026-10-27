//! Headless tests for View on service and the fork dialogs: which remotes qualify and why not,
//! the requests sent, results matched to the open dialog, the suggested remote name, the
//! add-remote jobs, and a fork created after switching tabs. The hosting service is answered by
//! hand; adding a fork runs for real against a bare repository.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::OsEffect;
use crate::effect::{Effect, ForgeError};
use crate::msg::{Msg, PaletteMsg, RepoMsg};
use crate::remote::{ForkRequest, RemoteEffect, RemoteMsg};
use crate::state::forks::{ForkCreate, ForkPicker, ForkStep};
use crate::state::palette::{PaletteCommand, entries};
use crate::state::{AppState, ForgeState, Modal};
use glowfly_core::forge::{Auth, CreatedFork, Fork, ForkList, ForkLookup, Provider, Target, parse_remote};
use glowfly_core::gitcli::args;
use std::path::{Path, PathBuf};
use std::time::Instant;

const ORIGIN: &str = "https://github.com/team/app.git";

fn run(st: &mut AppState, m: RemoteMsg) -> Vec<Effect> {
    update(st, Msg::Remote(m), Instant::now())
}

/// A repository with these remotes, opened.
fn repo(name: &str, remotes: &[(&str, &str)]) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    for (n, url) in remotes {
        git(&dir, &["remote", "add", n, url]);
    }
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn workdir(st: &AppState) -> PathBuf {
    st.snap.as_ref().unwrap().workdir.clone()
}

fn target() -> Target {
    parse_remote(ORIGIN, &[]).unwrap()
}

fn fork(owner: &str) -> Fork {
    Fork {
        owner: owner.into(),
        full_name: format!("{owner}/app"),
        https_url: format!("https://github.com/{owner}/app.git"),
        ssh_url: format!("git@github.com:{owner}/app.git"),
        web_url: format!("https://github.com/{owner}/app"),
        private: false,
    }
}

fn picker(st: &mut AppState) -> &mut ForkPicker {
    match st.modal.as_mut() {
        Some(Modal::AddFork(p)) => p,
        _ => panic!("the Add a fork dialog is not open"),
    }
}

fn creator(st: &mut AppState) -> &mut ForkCreate {
    match st.modal.as_mut() {
        Some(Modal::CreateFork(f)) => f,
        _ => panic!("the Fork dialog is not open"),
    }
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn git_job(fx: &[Effect]) -> (PathBuf, Vec<Vec<String>>) {
    match fx {
        [Effect::RunGit { cwd, cmds, .. }] => (cwd.clone(), cmds.clone()),
        other => panic!("unexpected effects {other:?}"),
    }
}

fn v(lines: &[&[&str]]) -> Vec<Vec<String>> {
    lines.iter().map(|l| args(l)).collect()
}

fn loaded(st: &mut AppState, forks: Vec<Fork>) {
    let workdir = workdir(st);
    run(st, RemoteMsg::ForksLoaded { workdir, target: target(), result: Box::new(Ok(ForkList { forks, truncated: false })) });
}

fn found(st: &mut AppState, user: &str, mine: Option<Fork>, own_repository: bool) {
    let (workdir, user) = (workdir(st), user.to_owned());
    run(st, RemoteMsg::MyForkFound { workdir, target: target(), result: Box::new(Ok(ForkLookup { user, mine, own_repository })) });
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn view_opens_the_repository_page() {
    let (mut st, dir) = repo("fork-view", &[("origin", ORIGIN), ("backup", "D:/backups/app.git")]);
    let fx = run(&mut st, RemoteMsg::View("origin".into()));
    assert!(matches!(&fx[..], [Effect::Os(OsEffect::OpenUrl(u))] if u == "https://github.com/team/app"), "{fx:?}");
    assert!(run(&mut st, RemoteMsg::View("backup".into())).is_empty());
    assert!(last_log(&st).contains("folder on disk"), "{}", last_log(&st));
    assert!(run(&mut st, RemoteMsg::View("gone".into())).is_empty());
    assert_eq!(last_log(&st), "Remote gone no longer exists.");
    cleanup(&[&dir]);
}

#[test]
fn fork_dialog_asks_for_the_forks() {
    let remotes = [("origin", ORIGIN), ("dc", "https://bb.acme.io/scm/PROJ/app.git"), ("az", "https://dev.azure.com/org/proj/_git/app")];
    let (mut st, dir) = repo("fork-dialog", &remotes);
    let fx = run(&mut st, RemoteMsg::ForkDialog("origin".into()));
    let p = picker(&mut st);
    assert!(p.loading && p.target == target() && p.fetch_after && p.remote == "origin");
    let want = |fx: &[Effect], auth: bool| {
        matches!(fx, [Effect::Remote(RemoteEffect::ListForks(ForkRequest { target: t, auth: a, connected: false, .. }))]
            if *t == target() && a.is_some() == auth)
    };
    assert!(want(&fx, false), "{fx:?}");
    // A credential from this session is passed along.
    let mut fs = ForgeState::new(Some(target()), "origin".into(), None);
    fs.auth.insert(target().account().key(), Auth::from_token(Provider::GitHub, None, "tok", "test"));
    st.session.forge = Some(fs);
    assert!(want(&run(&mut st, RemoteMsg::ForkDialog("origin".into())), true));

    st.modal = None;
    assert!(run(&mut st, RemoteMsg::ForkDialog("dc".into())).is_empty());
    assert!(last_log(&st).contains("Preferences > Integrations"), "{}", last_log(&st));
    assert!(run(&mut st, RemoteMsg::ForkDialog("az".into())).is_empty());
    assert!(last_log(&st).contains("isn't supported yet"), "{}", last_log(&st));
    assert!(st.modal.is_none());
    cleanup(&[&dir]);
}

#[test]
fn fork_lists_are_matched_to_the_dialog() {
    let (mut st, dir) = repo("fork-loaded", &[("origin", ORIGIN)]);
    run(&mut st, RemoteMsg::ForkDialog("origin".into()));
    let w = workdir(&st);
    let other = parse_remote("https://github.com/team/other", &[]).unwrap();
    let ok = || Box::new(Ok(ForkList { forks: vec![fork("ada")], truncated: false }));
    run(&mut st, RemoteMsg::ForksLoaded { workdir: w.clone(), target: other, result: ok() });
    run(&mut st, RemoteMsg::ForksLoaded { workdir: "C:/elsewhere".into(), target: target(), result: ok() });
    assert!(picker(&mut st).loading, "results for another repository or target are dropped");
    run(&mut st, RemoteMsg::ForksLoaded { workdir: w.clone(), target: target(), result: ok() });
    let p = picker(&mut st);
    assert!(!p.loading && p.list.forks.len() == 1);

    let err = ForgeError { message: "the server said \u{201c}Bad credentials\u{201d}".into(), auth: true };
    let fx = run(&mut st, RemoteMsg::RetryForks);
    assert!(matches!(&fx[..], [Effect::Remote(RemoteEffect::ListForks(_))]) && picker(&mut st).loading);
    run(&mut st, RemoteMsg::ForksLoaded { workdir: w.clone(), target: target(), result: Box::new(Err(err)) });
    let text = crate::state::forks::error_text(picker(&mut st).error.as_ref().unwrap());
    assert!(text.contains("Bad credentials") && text.contains("Sign in again in Preferences > Integrations."), "{text}");
    assert!(matches!(&run(&mut st, RemoteMsg::RetryForks)[..], [Effect::Remote(RemoteEffect::ListForks(_))]));
    assert!(picker(&mut st).error.is_none());
    cleanup(&[&dir]);
}

#[test]
fn picking_suggests_a_remote_name() {
    let (mut st, dir) = repo("fork-pick", &[("origin", ORIGIN), ("linus", "https://github.com/linus/app.git")]);
    run(&mut st, RemoteMsg::ForkDialog("origin".into()));
    loaded(&mut st, vec![fork("ada"), fork("grace"), fork("linus"), fork("origin")]);
    run(&mut st, RemoteMsg::PickFork(0));
    assert_eq!((picker(&mut st).picked, picker(&mut st).name.as_str()), (Some(0), "ada"));
    run(&mut st, RemoteMsg::PickFork(1));
    assert_eq!(picker(&mut st).name, "grace", "follows the pick");
    picker(&mut st).name = "mine".into();
    run(&mut st, RemoteMsg::PickFork(0));
    assert_eq!((picker(&mut st).picked, picker(&mut st).name.as_str()), (Some(0), "mine"), "a typed name stays");
    picker(&mut st).name = picker(&mut st).suggested.clone();
    run(&mut st, RemoteMsg::PickFork(3));
    assert_eq!(picker(&mut st).name, "origin-fork");
    run(&mut st, RemoteMsg::PickFork(2));
    assert_eq!(picker(&mut st).picked, Some(3), "a fork that already is a remote can't be picked");
    run(&mut st, RemoteMsg::PickFork(9));
    assert_eq!(picker(&mut st).picked, Some(3));
    cleanup(&[&dir]);
}

#[test]
fn add_fork_runs_one_job() {
    let (mut st, dir) = repo("fork-add", &[("origin", ORIGIN), ("linus", "git@github.com:linus/app")]);
    run(&mut st, RemoteMsg::ForkDialog("origin".into()));
    loaded(&mut st, vec![fork("ada"), fork("linus")]);
    run(&mut st, RemoteMsg::PickFork(0));
    picker(&mut st).fetch_after = false;
    let (_, cmds) = git_job(&run(&mut st, RemoteMsg::AddFork));
    assert_eq!(cmds, v(&[&["remote", "add", "ada", "https://github.com/ada/app.git"]]));
    assert!(st.modal.is_none(), "closed once the job starts");

    // While that job runs, another add keeps the dialog open.
    run(&mut st, RemoteMsg::ForkDialog("origin".into()));
    loaded(&mut st, vec![fork("ada"), fork("linus")]);
    run(&mut st, RemoteMsg::PickFork(0));
    assert!(run(&mut st, RemoteMsg::AddFork).is_empty());
    assert!(st.modal.is_some() && last_log(&st).contains("still running"), "{}", last_log(&st));
    st.job = None;
    let (_, cmds) = git_job(&run(&mut st, RemoteMsg::AddFork));
    assert_eq!(cmds, v(&[&["remote", "add", "ada", "https://github.com/ada/app.git"], &["fetch", "--prune", "ada"]]));
    st.job = None;

    // A taken name, or a fork that is already a remote: logged, the dialog stays.
    run(&mut st, RemoteMsg::ForkDialog("origin".into()));
    loaded(&mut st, vec![fork("ada"), fork("linus")]);
    run(&mut st, RemoteMsg::PickFork(0));
    picker(&mut st).name = "Origin".into();
    assert!(run(&mut st, RemoteMsg::AddFork).is_empty());
    assert!(last_log(&st).contains("already exists"), "{}", last_log(&st));
    picker(&mut st).picked = Some(1);
    picker(&mut st).name = "lt".into();
    assert!(run(&mut st, RemoteMsg::AddFork).is_empty());
    assert_eq!(last_log(&st), "linus/app is already the remote linus.");
    assert!(st.modal.is_some());
    cleanup(&[&dir]);
}

#[test]
fn an_ssh_origin_adds_the_ssh_url() {
    let (mut st, dir) = repo("fork-add-ssh", &[("origin", "git@github.com:team/app.git")]);
    run(&mut st, RemoteMsg::ForkDialog("origin".into()));
    loaded(&mut st, vec![fork("ada")]);
    run(&mut st, RemoteMsg::PickFork(0));
    let (_, cmds) = git_job(&run(&mut st, RemoteMsg::AddFork));
    assert_eq!(cmds[0], args(&["remote", "add", "ada", "git@github.com:ada/app.git"]));
    cleanup(&[&dir]);
}

#[test]
fn add_fork_end_to_end() {
    let (mut st, dir) = repo("fork-e2e", &[("origin", ORIGIN)]);
    let bare = dir.with_file_name(format!("{}-fork.git", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&bare);
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    git(&dir, &["push", "-q", &bare.display().to_string(), "main"]);
    run(&mut st, RemoteMsg::ForkDialog("origin".into()));
    let mut ada = fork("ada");
    ada.https_url = bare.display().to_string();
    loaded(&mut st, vec![ada]);
    run(&mut st, RemoteMsg::PickFork(0));
    drive(&mut st, Msg::Remote(RemoteMsg::AddFork));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.remote_info.iter().any(|r| r.name == "ada"));
    let branches = snap.remotes.iter().find(|(r, _)| r == "ada").map(|(_, b)| b.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>());
    assert_eq!(branches, Some(vec!["main".to_owned()]));
    cleanup(&[&dir, &bare]);
}

#[test]
fn fork_dialog_steps() {
    let (mut st, dir) = repo("fork-steps", &[("origin", ORIGIN)]);
    let fx = run(&mut st, RemoteMsg::ForkOnService("origin".into()));
    assert!(matches!(&fx[..], [Effect::Remote(RemoteEffect::FindMyFork(r))] if r.target == target()), "{fx:?}");
    assert_eq!(creator(&mut st).step, ForkStep::Checking);
    found(&mut st, "team", None, true);
    assert_eq!(creator(&mut st).step, ForkStep::OwnRepo { user: "team".into() });
    assert!(run(&mut st, RemoteMsg::CreateFork).is_empty(), "nothing to fork");

    run(&mut st, RemoteMsg::ForkOnService("origin".into()));
    found(&mut st, "ada", Some(fork("ada")), false);
    assert!(matches!(&creator(&mut st).step, ForkStep::Mine { fork, .. } if fork.full_name == "ada/app"));
    assert_eq!(creator(&mut st).name, "ada");
    let (_, cmds) = git_job(&run(&mut st, RemoteMsg::AddMyFork));
    assert_eq!(cmds, v(&[&["remote", "add", "ada", "https://github.com/ada/app.git"], &["fetch", "--prune", "ada"]]));
    assert!(st.modal.is_none());
    st.job = None;

    run(&mut st, RemoteMsg::ForkOnService("origin".into()));
    found(&mut st, "ada", None, false);
    assert_eq!((creator(&mut st).step.clone(), creator(&mut st).name.as_str()), (ForkStep::Ready { user: "ada".into() }, "ada"));
    let fx = run(&mut st, RemoteMsg::CreateFork);
    assert!(matches!(&fx[..], [Effect::Remote(RemoteEffect::CreateFork { name, .. })] if name == "ada"), "{fx:?}");
    assert_eq!(creator(&mut st).step, ForkStep::Creating { user: "ada".into() });
    assert!(run(&mut st, RemoteMsg::CreateFork).is_empty(), "no second fork");
    cleanup(&[&dir]);
}

fn created(st: &mut AppState, workdir: PathBuf, name: &str, ready: bool) -> Vec<Effect> {
    let result = Box::new(Ok(CreatedFork { fork: fork("ada"), ready }));
    run(st, RemoteMsg::ForkCreated { workdir, target: target(), name: name.into(), result })
}

#[test]
fn a_created_fork_is_added_and_fetched() {
    let (mut st, dir) = repo("fork-created", &[("origin", ORIGIN)]);
    let w = workdir(&st);
    run(&mut st, RemoteMsg::ForkOnService("origin".into()));
    found(&mut st, "ada", None, false);
    run(&mut st, RemoteMsg::CreateFork);
    let fx = created(&mut st, w.clone(), "ada", true);
    assert!(st.modal.is_none());
    assert!(st.log.iter().any(|l| l.text == "Created the fork ada/app on GitHub (https://github.com/ada/app)."));
    let (cwd, cmds) = git_job(&fx);
    assert_eq!(cwd, w);
    assert_eq!(cmds, v(&[&["remote", "add", "ada", "https://github.com/ada/app.git"], &["fetch", "--prune", "ada"]]));
    st.job = None;

    let (_, cmds) = git_job(&created(&mut st, w.clone(), "ada", false));
    assert_eq!(cmds, v(&[&["remote", "add", "ada", "https://github.com/ada/app.git"]]));
    assert_eq!(last_log(&st), "GitHub is still preparing the fork; fetch ada in a minute.");
    st.job = None;

    // The name was taken meanwhile: nothing runs.
    assert!(created(&mut st, w.clone(), "Origin", true).is_empty());
    assert_eq!(last_log(&st), "Created the fork ada/app, but the remote name Origin is taken now. Add it with Add a fork of origin….");

    // A failure goes back to the dialog, or to the log once it is closed.
    run(&mut st, RemoteMsg::ForkOnService("origin".into()));
    found(&mut st, "ada", None, false);
    run(&mut st, RemoteMsg::CreateFork);
    let err = || Box::new(Err(ForgeError { message: "name already exists".into(), auth: false }));
    run(&mut st, RemoteMsg::ForkCreated { workdir: w.clone(), target: target(), name: "ada".into(), result: err() });
    let f = creator(&mut st);
    assert_eq!(
        (f.step.clone(), f.error.as_ref().map(|e| e.message.as_str())),
        (ForkStep::Ready { user: "ada".into() }, Some("name already exists"))
    );
    st.modal = None;
    run(&mut st, RemoteMsg::ForkCreated { workdir: w, target: target(), name: "ada".into(), result: err() });
    assert_eq!(last_log(&st), "Forking team/app on GitHub failed: name already exists");
    cleanup(&[&dir]);
}

#[test]
fn a_fork_created_after_switching_tabs_is_added_to_its_repository() {
    let (mut st, a) = repo("fork-tabs-a", &[("origin", ORIGIN)]);
    let w = workdir(&st);
    let b = scratch_repo("fork-tabs-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert_ne!(workdir(&st), w, "the other tab is active");
    let (cwd, cmds) = git_job(&created(&mut st, w.clone(), "ada", true));
    assert_eq!(cwd, w);
    assert_eq!(cmds[0], args(&["remote", "add", "ada", "https://github.com/ada/app.git"]));
    assert_eq!(st.job.as_ref().and_then(|j| j.workdir.clone()), Some(w));
    cleanup(&[&a, &b]);
}

#[test]
fn palette_offers_the_remote_actions() {
    let (st, dir) = repo("fork-palette", &[("origin", ORIGIN), ("backup", "D:/backups/app.git")]);
    let all = entries(&st);
    let labels: Vec<&str> = all.iter().map(|e| e.label.as_str()).collect();
    for want in ["View origin on GitHub", "Add a fork of origin…", "Fork origin (team/app) on GitHub…"] {
        assert!(labels.contains(&want), "{want}: {labels:?}");
    }
    assert!(!labels.iter().any(|l| l.contains("backup") && !l.starts_with("Fetch")), "{labels:?}");
    let view = all.iter().find(|e| e.label == "View origin on GitHub").unwrap();
    assert_eq!(view.command, PaletteCommand::Remote(RemoteMsg::View("origin".into())));
    let mut st = st;
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(view.command.clone())), Instant::now());
    assert!(matches!(&fx[..], [Effect::Os(OsEffect::OpenUrl(_))]), "{fx:?}");
    cleanup(&[&dir]);
}
