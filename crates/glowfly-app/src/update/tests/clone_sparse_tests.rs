//! Headless tests for sparse clones: the Clone dialog's folders field, the job a sparse request
//! starts, how its end opens the clone (even when only setting the folders failed), and a sparse
//! clone end to end.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{CloneEffect, Effect};
use crate::msg::{CloneMsg, Msg};
use crate::state::AppState;
use crate::state::clone::{CloneForm, CloneRequest};
use crate::state::trust::Created;
use chrono::NaiveDate;
use glowfly_core::clone_ops::{CloneOptions, CloneOutcome, Sparse};
use glowfly_core::git_version::GitInfo;
use std::path::{Path, PathBuf};
use std::time::Instant;

const URL: &str = "https://example.com/team/app.git";

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()
}

fn work() -> &'static str {
    if cfg!(windows) { "C:/work" } else { "/work" }
}

fn sparse_form(folders: &str) -> CloneForm {
    CloneForm { url: URL.into(), sparse: true, sparse_folders: folders.into(), ..CloneForm::new(work().into()) }
}

fn sparse_request(folders: &[&str]) -> CloneRequest {
    let sparse = Some(Sparse { folders: folders.iter().map(|f| (*f).to_owned()).collect(), partial: false });
    CloneRequest { url: URL.into(), parent: work().into(), name: "app".into(), options: CloneOptions { shallow: None, sparse } }
}

#[test]
fn folders_are_checked_like_the_sparse_checkout_page() {
    let t = today();
    let p = sparse_form("docs\nsrc/*.rs").problems(t, true);
    assert_eq!(
        p.sparse.as_deref(),
        Some("\"src/*.rs\" looks like a pattern. List folders here; patterns can be set after cloning in Preferences > Sparse checkout.")
    );
    assert!(p.blocked);
    let p = sparse_form("../x").problems(t, true);
    assert_eq!(p.sparse.as_deref(), Some("\"../x\" leaves the repository; use folders relative to its root."));
    assert!(p.blocked);
    assert_eq!(sparse_form("").problems(t, true), Default::default(), "an empty list checks out only the root files");
    // With sparse off the field doesn't matter.
    assert_eq!(CloneForm { sparse: false, ..sparse_form("*") }.problems(t, true), Default::default());

    let req = CloneForm { partial: true, ..sparse_form(" ./docs/\nsrc\\app\ndocs\n") }.request(t).unwrap();
    assert_eq!(req.options.sparse, Some(Sparse { folders: vec!["docs".into(), "src/app".into()], partial: true }));
    assert_eq!(sparse_form("*").request(t), None);
    assert_eq!(CloneForm { url: URL.into(), ..CloneForm::new(work().into()) }.request(t).unwrap().options.sparse, None);
}

#[test]
fn a_sparse_clone_is_one_job_that_opens_the_clone() {
    let mut st = state();
    let req = sparse_request(&["docs"]);
    let fx = update(&mut st, Msg::Clone(CloneMsg::Start(req.clone())), Instant::now());
    let [Effect::Clone(CloneEffect::Sparse { parent, cmd, dest, folders })] = fx.as_slice() else { panic!("unexpected effects {fx:?}") };
    assert_eq!(parent, &req.parent);
    assert_eq!(dest, &req.parent.join("app"));
    assert_eq!(cmd, &["clone", "--sparse", "--", URL, "app"]);
    assert_eq!(folders, &["docs"]);
    let job = st.job.as_ref().unwrap();
    assert_eq!(job.label, "Clone");
    assert_eq!(job.then_open, Some((req.parent.join("app"), Created::Clone)));
}

#[test]
fn an_old_git_refuses_a_sparse_clone() {
    let mut st = state();
    st.git.info = Some(GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 30, 1)), chosen: false });
    let fx = update(&mut st, Msg::Clone(CloneMsg::Start(sparse_request(&["docs"]))), Instant::now());
    assert!(fx.is_empty(), "{fx:?}");
    assert!(st.job.is_none());
    let line = &st.log.last().unwrap().text;
    assert!(line.starts_with("Could not clone: ") && line.contains("2.35"), "{line}");
    // A plain clone still runs.
    let req = CloneRequest { options: CloneOptions::default(), ..sparse_request(&[]) };
    let fx = update(&mut st, Msg::Clone(CloneMsg::Start(req)), Instant::now());
    assert!(matches!(fx.as_slice(), [Effect::RunGit { .. }]), "{fx:?}");
}

fn loads(fx: &[Effect], dest: &Path) -> bool {
    fx.iter().any(|e| matches!(e, Effect::LoadRepo { path, .. } if path == dest))
}

#[test]
fn a_clone_whose_folders_failed_still_opens() {
    let mut st = state();
    update(&mut st, Msg::Clone(CloneMsg::Start(sparse_request(&["docs"]))), Instant::now());
    let dest = PathBuf::from(work()).join("app");
    let o = CloneOutcome { cloned: true, ok: false, output: "$ git clone x\n$ git sparse-checkout set\nfatal: no space".into() };
    let fx = update(&mut st, Msg::Clone(CloneMsg::Finished(o)), Instant::now());
    assert!(st.job.is_none());
    let toast = st.toasts.last().unwrap();
    assert!(!toast.ok);
    assert_eq!(toast.text, "Cloned app, but setting its folders failed: fatal: no space. Choose them in Preferences > Sparse checkout.");
    assert!(loads(&fx, &dest), "{fx:?}");
    assert!(st.trust.created.contains(&dest));

    // A cause the classifier knows is said in plain words.
    let mut st = state();
    update(&mut st, Msg::Clone(CloneMsg::Start(sparse_request(&["docs"]))), Instant::now());
    let locked = "$ git clone x\n$ git sparse-checkout set\nfatal: Unable to create '/w/app/.git/index.lock': File exists.";
    let o = CloneOutcome { cloned: true, ok: false, output: locked.into() };
    update(&mut st, Msg::Clone(CloneMsg::Finished(o)), Instant::now());
    let text = &st.toasts.last().unwrap().text;
    assert!(text.starts_with("Cloned app, but setting its folders failed: Another git program is using this repository"), "{text}");
    assert!(text.ends_with("delete that file. Choose them in Preferences > Sparse checkout."), "{text}");

    let mut st = state();
    update(&mut st, Msg::Clone(CloneMsg::Start(sparse_request(&["docs"]))), Instant::now());
    let o = CloneOutcome { cloned: false, ok: false, output: "$ git clone x\nfatal: repository not found".into() };
    let fx = update(&mut st, Msg::Clone(CloneMsg::Finished(o)), Instant::now());
    assert!(st.job.is_none());
    assert!(!loads(&fx, &dest) && st.trust.created.is_empty(), "{fx:?}");
    assert_eq!(st.toasts.last().unwrap().text, "Clone failed: fatal: repository not found");
}

#[test]
fn sparse_clone_end_to_end_checks_out_only_the_chosen_folder() {
    let src = scratch_repo("clone-sparse-src");
    for (dir, file) in [("a", "a/x.txt"), ("b", "b/y.txt")] {
        std::fs::create_dir_all(src.join(dir)).unwrap();
        std::fs::write(src.join(file), "x").unwrap();
    }
    git(&src, &["add", "."]);
    git(&src, &["commit", "-q", "-m", "folders"]);
    let parent = src.with_file_name(format!("{}-into", src.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&parent);
    std::fs::create_dir_all(&parent).unwrap();
    let mut st: AppState = state();
    let sparse = Some(Sparse { folders: vec!["a".into()], partial: false });
    let req = CloneRequest {
        url: src.display().to_string(),
        parent: parent.clone(),
        name: "copy".into(),
        options: CloneOptions { shallow: None, sparse },
    };
    drive(&mut st, Msg::Clone(CloneMsg::Start(req)));
    let dest = parent.join("copy");
    assert!(dest.join("a/x.txt").is_file(), "log: {:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    assert!(dest.join("a.txt").is_file(), "files at the root are checked out");
    assert!(!dest.join("b").exists());
    let same = |p: &Path| p.canonicalize().ok() == dest.canonicalize().ok();
    let snap = st.snap.as_ref().expect("the clone is loaded");
    assert!(same(&snap.workdir));
    assert!(st.toasts.last().unwrap().ok);
    let _ = std::fs::remove_dir_all(&parent);
    let _ = std::fs::remove_dir_all(&src);
}
