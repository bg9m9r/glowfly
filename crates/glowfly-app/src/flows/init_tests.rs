//! Headless tests for the Init dialog: opening it, checking its folder by an effect, suggesting a
//! copyright holder, its rules and the request it builds, the job it starts, and an init end to
//! end.

use super::clone::PARENT_CHECK_DELAY;
use super::tests::{drive, state};
use super::update;
use crate::effect::OsEffect;
use crate::effect::{Effect, InitEffect};
use crate::msg::{FolderPurpose, InitMsg, Msg, PrefsMsg, RepoMsg};
use crate::state::init::{InitForm, InitRequest};
use crate::state::trust::{Created, recorded};
use crate::state::{AppState, Modal};
use glowfly_core::init_ops::{FolderFacts, GitignoreTemplate, INITIAL_COMMIT, InitOutcome, InitPlan, License};
use glowfly_core::settings::{RepoTrust, Settings};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn run(st: &mut AppState, m: Msg) -> Vec<Effect> {
    update(st, m, Instant::now())
}

fn form(st: &mut AppState) -> &mut InitForm {
    match st.modal.as_mut() {
        Some(Modal::Init(f)) => f,
        _ => panic!("no init dialog"),
    }
}

fn work() -> &'static str {
    if cfg!(windows) { r"C:\work" } else { "/work" }
}

fn facts(exists: bool) -> FolderFacts {
    FolderFacts { path: PathBuf::from(work()).join("app"), exists, repo: false, has_files: false, gitignore: false, license: None }
}

/// A form for `<work>/app` whose check found `found`.
fn checked(found: Option<FolderFacts>) -> InitForm {
    let folder = PathBuf::from(work()).join("app").display().to_string();
    InitForm { folder_checked: Some((folder.clone(), found)), ..InitForm::new(folder, String::new()) }
}

fn checks(fx: &[Effect]) -> Vec<(String, Duration)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Init(InitEffect::CheckFolder { folder, delay }) => Some((folder.clone(), *delay)),
            _ => None,
        })
        .collect()
}

#[test]
fn init_opens_the_dialog_instead_of_a_folder_picker() {
    let mut st = AppState::new(
        Settings { default_branch: "trunk".into(), ..Settings::default() },
        vec![PathBuf::from("C:/work/app")],
        Instant::now(),
    );
    let fx = run(&mut st, Msg::Repo(RepoMsg::InitRepo));
    assert!(!fx.iter().any(|e| matches!(e, Effect::Os(OsEffect::PickFolder { .. }))), "{fx:?}");
    let f = form(&mut st);
    assert_eq!(Path::new(&f.folder), Path::new("C:/work"));
    assert_eq!(f.branch, "trunk");
    assert!(f.commit && f.gitignore.is_none() && f.license.is_none());
    assert_eq!(checks(&fx), [(f.folder.clone(), Duration::ZERO)]);
    assert!(fx.iter().any(|e| matches!(e, Effect::Init(InitEffect::ReadAuthor { .. }))), "{fx:?}");

    // Edits ask again after a pause; an answer for older text is dropped.
    form(&mut st).folder = format!("{} ", work());
    let fx = run(&mut st, Msg::Init(InitMsg::FolderEdited));
    assert_eq!(checks(&fx), [(work().to_owned(), PARENT_CHECK_DELAY)]);
    run(&mut st, Msg::Init(InitMsg::FolderChecked { folder: "elsewhere".into(), found: Some(facts(true)) }));
    assert_eq!(form(&mut st).checked(), None);
    run(&mut st, Msg::Init(InitMsg::FolderChecked { folder: work().into(), found: Some(facts(true)) }));
    assert_eq!(form(&mut st).facts(), Some(&facts(true)));

    // Browse fills the field and checks it at once; no git runs.
    let picked = PathBuf::from(work()).join("picked");
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::FolderPicked { purpose: FolderPurpose::InitRepo, path: Some(picked.clone()) }));
    assert_eq!(Path::new(&form(&mut st).folder), picked);
    assert_eq!(checks(&fx).len(), 1);
    assert!(checks(&fx)[0].1.is_zero());
    assert!(!fx.iter().any(|e| matches!(e, Effect::RunGit { .. } | Effect::Init(InitEffect::Run { .. }))), "{fx:?}");
    assert!(st.job.is_none());
}

#[test]
fn the_author_fills_the_holder_until_the_user_types() {
    let mut st = state();
    st.modal = Some(Modal::Init(checked(Some(facts(true)))));
    run(&mut st, Msg::Init(InitMsg::AuthorRead(Some("Ada".into()))));
    assert_eq!(form(&mut st).holder, "Ada");
    run(&mut st, Msg::Init(InitMsg::AuthorRead(Some("Ada Lovelace".into()))));
    assert_eq!(form(&mut st).holder, "Ada Lovelace", "an untouched suggestion is replaced");
    form(&mut st).holder = "Grace".into();
    run(&mut st, Msg::Init(InitMsg::AuthorRead(Some("Ada".into()))));
    assert_eq!(form(&mut st).holder, "Grace", "never what the user typed");
    run(&mut st, Msg::Init(InitMsg::AuthorRead(None)));
    assert_eq!(form(&mut st).holder, "Grace");
}

#[test]
fn form_rules() {
    let relative = InitForm::new("src/app".into(), String::new());
    assert_eq!(relative.problems().folder.as_deref(), Some("Enter the full path of the folder (it may start with ~)."));
    let unchecked = InitForm::new(work().into(), String::new());
    let p = unchecked.problems();
    assert!(p.blocked && p.folder.is_none(), "unchecked text blocks without a message");
    let p = checked(None).problems();
    assert_eq!(p.folder.as_deref(), Some("Its parent folder doesn't exist."));
    assert!(p.blocked);
    assert!(!checked(Some(facts(false))).problems().blocked);
    let p = InitForm { branch: "a..b".into(), ..checked(Some(facts(true))) }.problems();
    assert_eq!(p.branch.as_deref(), Some("Not a valid branch name."));
    assert!(p.blocked);
    let p = InitForm { license: Some(License::Mit), ..checked(Some(facts(true))) }.problems();
    assert_eq!(p.holder.as_deref(), Some("Enter the name for the copyright line."));
    let p = InitForm { license: Some(License::Mit), holder: "a\nb".into(), ..checked(Some(facts(true))) }.problems();
    assert_eq!(p.holder.as_deref(), Some("Use a single line of text."));
    assert!(!InitForm { license: Some(License::Apache2), ..checked(Some(facts(true))) }.problems().blocked, "no holder needed");
}

#[test]
fn existing_files_are_kept_and_commit_needs_a_new_file() {
    let chosen = |found: FolderFacts| InitForm {
        gitignore: Some(GitignoreTemplate::Rust),
        license: Some(License::Mit),
        holder: "Ada".into(),
        ..checked(Some(found))
    };
    let names = |f: &InitForm| f.files(2026).into_iter().map(|n| n.name).collect::<Vec<_>>();
    assert_eq!(names(&chosen(facts(true))), [".gitignore", "LICENSE"]);
    let both = chosen(FolderFacts { gitignore: true, license: Some("COPYING".into()), ..facts(true) });
    assert!(names(&both).is_empty());
    assert!(!both.can_commit(2026), "nothing to commit");
    assert_eq!(both.license_kept().as_deref(), Some("This folder already has COPYING; it is kept."));
    let repo = chosen(FolderFacts { repo: true, ..facts(true) });
    assert!(!repo.can_commit(2026), "never commits in an existing repository");
    assert_eq!(repo.request(2026).unwrap().plan.commit, None);
    let plain = InitForm { commit: true, ..checked(Some(facts(true))) };
    assert!(!plain.can_commit(2026) && plain.request(2026).unwrap().plan.commit.is_none());

    let req = InitForm { branch: " dev ".into(), ..chosen(facts(false)) }.request(2026).unwrap();
    assert_eq!(req.folder, facts(false).path);
    assert_eq!(req.plan.branch.as_deref(), Some("dev"));
    assert_eq!(req.plan.commit.as_deref(), Some(INITIAL_COMMIT));
    assert!(req.plan.files[1].contents.contains("Copyright (c) 2026 Ada"));
    assert_eq!(InitForm { commit: false, ..chosen(facts(true)) }.request(2026).unwrap().plan.commit, None);
}

fn request() -> InitRequest {
    InitRequest { folder: PathBuf::from(work()).join("app"), plan: InitPlan { branch: None, files: Vec::new(), commit: None } }
}

#[test]
fn start_runs_one_init_job() {
    let mut st = state();
    st.modal = Some(Modal::Init(checked(Some(facts(true)))));
    let req = request();
    let fx = run(&mut st, Msg::Init(InitMsg::Start(req.clone())));
    let [Effect::Init(InitEffect::Run { folder, plan })] = fx.as_slice() else { panic!("unexpected effects {fx:?}") };
    assert_eq!((folder, plan), (&req.folder, &req.plan));
    let job = st.job.as_ref().unwrap();
    assert_eq!(job.label, "New repository");
    assert_eq!(job.then_open, Some((req.folder.clone(), Created::Init)));
    assert!(st.modal.is_none());

    let mut st = state();
    let fx = run(&mut st, Msg::Init(InitMsg::Start(InitRequest { folder: "app".into(), ..request() })));
    assert!(fx.is_empty() && st.job.is_none());
}

fn loads(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. }))
}

#[test]
fn a_failed_commit_still_opens_the_repository() {
    let mut st = state();
    run(&mut st, Msg::Init(InitMsg::Start(request())));
    let output = "$ git init\nInitialized empty Git repository in /work/app/.git/\nWrote LICENSE.\n$ git add -- LICENSE\n$ git commit -m Initial commit -- LICENSE\nAuthor identity unknown".into();
    let o = InitOutcome { initialized: true, ok: false, failed: Some("the commit".into()), output };
    let fx = run(&mut st, Msg::Init(InitMsg::Finished(o)));
    let toast = st.toasts.last().unwrap();
    assert!(!toast.ok);
    assert_eq!(toast.text, "Created the repository, but the commit failed: Author identity unknown");
    assert!(loads(&fx), "{fx:?}");
    assert_eq!(st.trust.created, [request().folder], "git made a new repository");

    let mut st = state();
    run(&mut st, Msg::Init(InitMsg::Start(request())));
    let o = InitOutcome { initialized: false, ok: false, failed: None, output: "$ git init\nfatal: cannot mkdir".into() };
    let fx = run(&mut st, Msg::Init(InitMsg::Finished(o)));
    assert!(!loads(&fx) && st.trust.created.is_empty() && st.job.is_none(), "{fx:?}");
}

#[test]
fn init_end_to_end_opens_and_trusts_the_new_repository() {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-init-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut st = state();
    st.modal = Some(Modal::Init(InitForm::new(dir.join("new").display().to_string(), "main".into())));
    // The folder check, as the dialog would get it.
    let fx = run(&mut st, Msg::Init(InitMsg::FolderEdited));
    super::tests::finish(&mut st, fx);
    let f = form(&mut st);
    f.gitignore = Some(GitignoreTemplate::Rust);
    f.license = Some(License::Mit);
    f.holder = "Ada".into();
    assert_eq!(f.facts().map(|f| f.exists), Some(false));
    let req = f.request(2026).expect("a complete form");
    st.modal = None;
    drive(&mut st, Msg::Init(InitMsg::Start(req)));
    let new = dir.join("new");
    let log: Vec<&String> = st.log.iter().map(|l| &l.text).collect();
    assert!(new.join(".gitignore").is_file() && new.join("LICENSE").is_file(), "{log:?}");
    let snap = st.snap.as_ref().unwrap_or_else(|| panic!("the repository is open: {log:?}"));
    assert_eq!(snap.commits.len(), 1, "{log:?}");
    assert_eq!(snap.commits[0].summary, INITIAL_COMMIT);
    assert!(st.toasts.last().unwrap().ok, "{log:?}");
    assert!(st.modal.is_none(), "trusted: no trust prompt");
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    assert_eq!(recorded(&st.settings, &real), Some(RepoTrust::Trusted), "git made it new");
    let _ = std::fs::remove_dir_all(&dir);
}
