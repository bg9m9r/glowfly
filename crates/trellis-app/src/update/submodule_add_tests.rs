//! Headless tests for the Add submodule dialog: opening it, the debounced folder check and which
//! answers are kept, the job it starts (busy, a switched tab, the trust prompt), the palette
//! entry, and adding a local repository end to end.

use super::submodule_add::ADD_CHECK_DELAY;
use super::submodule_tests::{cleanup, last_log};
use super::tests::{drive, finish, scratch_repo, state};
use super::trust_held_tests::held_repo;
use super::trust_tests::real;
use super::update;
use crate::effect::{Effect, SubmoduleEffect};
use crate::msg::{Msg, PaletteMsg, RepoMsg, TrustMsg};
use crate::runner;
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::submodules::add::{AddForm, TAB_GONE};
use crate::state::{AppState, Modal, RunningJob};
use crate::submodule::SubmoduleMsg;
use git2::Oid;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo::{SubmoduleInfo, SubmoduleState};
use trellis_core::submodule_ops::add_cmd;

const URL: &str = "https://example.com/team/lib.git";

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn sm(m: SubmoduleMsg) -> Msg {
    Msg::Submodule(m)
}

/// A scratch repository opened in a fresh state.
fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn form(st: &mut AppState) -> &mut AddForm {
    match st.modal.as_mut() {
        Some(Modal::AddSubmodule(f)) => f,
        _ => panic!("expected the Add submodule dialog"),
    }
}

/// Opens the dialog and fills it in; `checked` is the disk check's answer for `path`.
fn filled(st: &mut AppState, path: &str, checked: Option<Result<Vec<String>, String>>) {
    run(st, sm(SubmoduleMsg::AddDialog));
    let f = form(st);
    f.url = URL.into();
    f.path = path.into();
    f.path_edited = true;
    f.branch = "dev".into();
    f.checked = checked.map(|r| (path.to_owned(), r));
}

fn checks(fx: &[Effect]) -> Vec<(PathBuf, String, std::time::Duration)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Submodule(SubmoduleEffect::CheckAddPath { repo, path, delay }) => Some((repo.clone(), path.clone(), *delay)),
            _ => None,
        })
        .collect()
}

fn info(path: &str) -> SubmoduleInfo {
    let id = Oid::from_str(&"a".repeat(40)).unwrap();
    SubmoduleInfo {
        name: path.into(),
        path: path.into(),
        url: Some("../x.git".into()),
        recorded: id,
        checked_out: Some(id),
        state: SubmoduleState::InSync,
        left_behind: Vec::new(),
        clone_url: None,
        branch: None,
        added: false,
    }
}

#[test]
fn dialog_opens_for_the_active_repository() {
    let mut st = state();
    assert!(run(&mut st, sm(SubmoduleMsg::AddDialog)).is_empty());
    assert!(st.modal.is_none(), "nothing opens without a repository");

    let (mut st, dir) = opened("sm-add-dialog");
    run(&mut st, sm(SubmoduleMsg::AddDialog));
    let workdir = st.workdir().unwrap();
    let f = form(&mut st);
    assert_eq!((f.repo.clone(), f.url.as_str(), f.path.as_str(), f.path_edited), (workdir, "", "", false));
    cleanup(&[&dir]);
}

#[test]
fn edits_ask_once_per_folder() {
    let (mut st, dir) = opened("sm-add-edited");
    filled(&mut st, "libs/lib", None);
    let workdir = st.workdir().unwrap();
    let fx = run(&mut st, sm(SubmoduleMsg::AddEdited));
    assert_eq!(checks(&fx), [(workdir, "libs/lib".to_owned(), ADD_CHECK_DELAY)]);
    assert_eq!(fx.len(), 1, "{fx:?}");
    assert!(run(&mut st, sm(SubmoduleMsg::AddEdited)).is_empty(), "the same folder isn't asked again");
    // "./libs//lib/" is the same folder.
    form(&mut st).path = "./libs//lib/".into();
    assert!(run(&mut st, sm(SubmoduleMsg::AddEdited)).is_empty());

    // Refused from the text or the snapshot: nothing to ask.
    form(&mut st).path = "../x".into();
    assert!(run(&mut st, sm(SubmoduleMsg::AddEdited)).is_empty());
    st.snap.as_mut().unwrap().submodules.push(info("vendor/zlib"));
    for p in ["vendor/zlib", "vendor/zlib/x", "vendor"] {
        form(&mut st).path = p.into();
        assert!(run(&mut st, sm(SubmoduleMsg::AddEdited)).is_empty(), "{p}");
        let f = form(&mut st).clone();
        assert!(f.rel(st.snap.as_ref().unwrap()).is_err(), "{p}");
    }
    cleanup(&[&dir]);
}

#[test]
fn only_the_answer_for_the_current_folder_is_kept() {
    let (mut st, dir) = opened("sm-add-checked");
    filled(&mut st, "libs/lib", None);
    let workdir = st.workdir().unwrap();
    let answer = |repo: PathBuf, path: &str, result| sm(SubmoduleMsg::AddPathChecked { repo, path: path.into(), result });
    run(&mut st, answer(workdir.clone(), "libs/old", Ok(Vec::new())));
    assert_eq!(form(&mut st).checked, None, "an older folder text");
    run(&mut st, answer(PathBuf::from("C:/elsewhere"), "libs/lib", Ok(Vec::new())));
    assert_eq!(form(&mut st).checked, None, "another repository");
    run(&mut st, answer(workdir, "libs/lib", Err("no".into())));
    assert_eq!(form(&mut st).checked, Some(("libs/lib".into(), Err("no".into()))));
    cleanup(&[&dir]);
}

#[test]
fn add_runs_the_job_and_closes() {
    let (mut st, dir) = opened("sm-add-run");
    filled(&mut st, "libs/lib", Some(Ok(Vec::new())));
    let fx = run(&mut st, sm(SubmoduleMsg::Add));
    match &fx[..] {
        [Effect::RunGit { cmds, prompts: true, .. }] => assert_eq!(cmds[0], add_cmd(URL, "libs/lib", Some("dev"))),
        other => panic!("{other:?}"),
    }
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Add submodule libs/lib"));
    assert!(st.modal.is_none());
    cleanup(&[&dir]);
}

#[test]
fn add_waits_for_a_good_form() {
    let (mut st, dir) = opened("sm-add-refused");
    // The check is still on its way, or refused the folder.
    filled(&mut st, "libs/lib", None);
    assert!(run(&mut st, sm(SubmoduleMsg::Add)).is_empty());
    form(&mut st).checked = Some(("libs/lib".into(), Err("libs/lib already exists.".into())));
    assert!(run(&mut st, sm(SubmoduleMsg::Add)).is_empty());
    // An empty URL.
    filled(&mut st, "libs/lib", Some(Ok(Vec::new())));
    form(&mut st).url = " ".into();
    assert!(run(&mut st, sm(SubmoduleMsg::Add)).is_empty());
    assert!(matches!(st.modal, Some(Modal::AddSubmodule(_))) && st.job.is_none());

    // Another job is running: said so, and the dialog stays.
    filled(&mut st, "libs/lib", Some(Ok(Vec::new())));
    st.job = Some(RunningJob {
        label: "Fetch".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
        fetches: false,
    });
    assert!(run(&mut st, sm(SubmoduleMsg::Add)).is_empty());
    assert_eq!(last_log(&st), (false, "Another operation is still running.".to_owned()));
    assert!(matches!(st.modal, Some(Modal::AddSubmodule(_))));
    st.job = None;

    // The dialog's tab is no longer the active one.
    form(&mut st).repo = PathBuf::from("C:/another/repo");
    assert!(run(&mut st, sm(SubmoduleMsg::Add)).is_empty());
    assert_eq!(last_log(&st), (false, TAB_GONE.to_owned()));
    assert!(st.modal.is_none());
    cleanup(&[&dir]);
}

#[test]
fn add_in_an_untrusted_repository_asks_first() {
    let (mut st, dir) = held_repo("sm-add-held");
    filled(&mut st, "libs/lib", Some(Ok(Vec::new())));
    let fx = run(&mut st, sm(SubmoduleMsg::Add));
    assert!(!fx.iter().any(|e| matches!(e, Effect::RunGit { .. })), "{fx:?}");
    match &st.modal {
        Some(Modal::Trust(p)) => assert_eq!(p.held.as_deref(), Some("Add submodule libs/lib")),
        _ => panic!("expected the trust prompt"),
    }
    let real = real(&st);
    let fx = run(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }));
    let gits: Vec<&Effect> = fx.iter().filter(|e| matches!(e, Effect::RunGit { .. })).collect();
    assert!(matches!(gits[..], [Effect::RunGit { cmds, .. }] if cmds[0] == add_cmd(URL, "libs/lib", Some("dev"))), "{fx:?}");
    cleanup(&[&dir]);
}

#[test]
fn palette_offers_add_submodule() {
    let st = state();
    assert!(!entries(&st).iter().any(|e| e.label == "Add submodule…"), "not without a repository");
    let (mut st, dir) = opened("sm-add-palette");
    assert!(st.snap.as_ref().unwrap().submodules.is_empty());
    let entry = entries(&st).into_iter().find(|e| e.label == "Add submodule…").expect("listed without submodules");
    assert_eq!(entry.command.recall_key(), Some("add-submodule"));
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Submodule(SubmoduleMsg::AddDialog))));
    assert!(matches!(st.modal, Some(Modal::AddSubmodule(_))));
    assert_eq!(st.palette_recent.keys(), ["add-submodule"]);
    cleanup(&[&dir]);
}

#[test]
fn adds_a_local_repository_end_to_end() {
    let lib = scratch_repo("sm-add-e2e-lib");
    let (mut st, dir) = opened("sm-add-e2e");
    run(&mut st, sm(SubmoduleMsg::AddDialog));
    let f = form(&mut st);
    f.url = lib.display().to_string();
    f.path = "libs/lib".into();
    f.path_edited = true;
    let fx = run(&mut st, sm(SubmoduleMsg::AddEdited));
    for e in fx {
        let m = runner::execute(e).expect("the check answers");
        drive(&mut st, m);
    }
    assert_eq!(form(&mut st).checked, Some(("libs/lib".into(), Ok(Vec::new()))));
    let fx = run(&mut st, sm(SubmoduleMsg::Add));
    assert!(st.modal.is_none());
    finish(&mut st, fx);
    let t = st.toasts.last().expect("a toast");
    assert!(t.ok && t.text == "Add submodule libs/lib succeeded", "{}", t.text);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.submodules.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(), ["libs/lib"]);
    let staged: Vec<&str> = snap.staged.iter().map(|f| f.path.as_str()).collect();
    assert!(staged.contains(&".gitmodules") && staged.contains(&"libs/lib"), "{staged:?}");
    cleanup(&[&dir, &lib]);
}
