//! Headless tests for removing a submodule: what is refused before anything runs, the
//! confirmation built from the runner's facts, the job (and the trust prompt holding it), each
//! way it can end, and a removal end to end.

use super::submodule_tests::{PATH, cleanup, last_log, opened, sub};
use super::tests::{drive, finish};
use super::trust_held_tests::held_repo;
use super::update;
use crate::effect::{Effect, SubmoduleEffect};
use crate::msg::{Msg, RepoMsg};
use crate::runner;
use crate::state::submodules::remove::RemoveForm;
use crate::state::{AppState, Modal};
use crate::submodule::{RemoveRequest, SubmoduleMsg};
use git2::Oid;
use glowfly_core::repo::{SubmoduleInfo, SubmoduleState};
use glowfly_core::submodule_ops::{RemovalFacts, RemoveOutcome, RemoveReport};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn sm(m: SubmoduleMsg) -> Msg {
    Msg::Submodule(m)
}

fn facts(info: SubmoduleInfo, module_dir: Option<PathBuf>) -> Box<RemovalFacts> {
    Box::new(RemovalFacts { info, unpushed: Vec::new(), stashes: 0, module_dir })
}

fn store(parent: &Path) -> PathBuf {
    parent.join(".git").join("modules").join("libs").join("lib")
}

#[test]
fn remove_dialog_checks_first_and_refuses_what_git_would() {
    let (mut st, parent, lib) = opened("sm-rm-dialog");
    let workdir = st.workdir().unwrap();
    let fx = run(&mut st, sm(SubmoduleMsg::RemoveDialog(PATH.into())));
    match &fx[..] {
        [e @ Effect::Submodule(SubmoduleEffect::CheckRemove { workdir: w, path })] => {
            assert_eq!((w, path.as_str()), (&workdir, PATH));
            assert_eq!(e.scope(), Some(&workdir), "the check is scoped to its tab");
        }
        other => panic!("{other:?}"),
    }
    assert!(st.modal.is_none());

    // An unknown path.
    assert!(run(&mut st, sm(SubmoduleMsg::RemoveDialog("libs/gone".into()))).is_empty());
    assert!(last_log(&st).1.contains("no longer in this repository"));

    // No .gitmodules entry.
    st.snap.as_mut().unwrap().submodules[0].url = None;
    assert!(run(&mut st, sm(SubmoduleMsg::RemoveDialog(PATH.into()))).is_empty());
    assert_eq!(last_log(&st), (false, "Can't remove libs/lib: .gitmodules has no entry for it, so git can't deinitialize it.".into()));

    // .gitmodules edited and not staged.
    let gm = parent.join(".gitmodules");
    let text = std::fs::read_to_string(&gm).unwrap();
    std::fs::write(&gm, format!("{text}# edited\n")).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(run(&mut st, sm(SubmoduleMsg::RemoveDialog(PATH.into()))).is_empty());
    assert_eq!(last_log(&st), (false, "Can't remove libs/lib: Stage or discard the changes to .gitmodules first.".into()));
    cleanup(&[&parent, &lib]);
}

#[test]
fn checked_opens_the_confirmation() {
    let (mut st, parent, lib) = opened("sm-rm-checked");
    let checked = |result| sm(SubmoduleMsg::RemoveChecked { path: PATH.into(), result });
    run(&mut st, checked(Ok(None)));
    assert!(last_log(&st).1.contains("no longer in this repository") && st.modal.is_none());
    run(&mut st, checked(Err("boom".into())));
    assert_eq!(last_log(&st), (false, "Could not read submodule libs/lib: boom".into()));

    let mut info = sub(&st);
    info.state = SubmoduleState::Dirty;
    let mut f = facts(info, Some(store(&parent)));
    let id = Oid::from_str(&"c".repeat(40)).unwrap();
    f.unpushed = vec![(id, "local work".into())];
    run(&mut st, checked(Ok(Some(f))));
    let Some(Modal::RemoveSubmodule(form)) = &st.modal else { panic!("expected the confirmation") };
    let lines = form.deleted_lines();
    assert!(lines[0].contains("the working folder libs/lib"), "{lines:?}");
    assert!(lines[1].contains(".gitmodules"), "{lines:?}");
    assert!(lines[2].contains(&glowfly_core::native_path::shown(&store(&parent))), "{lines:?}");
    let w = form.warnings();
    assert!(w[0].contains("uncommitted changes") && w[1].contains("ccccccc local work"), "{w:?}");
    assert_eq!(sub(&st).state, SubmoduleState::Dirty, "the row is read again");
    cleanup(&[&parent, &lib]);
}

/// Opens the confirmation for the fixture's submodule from made-up facts.
fn confirming(st: &mut AppState) {
    let f = facts(sub(st), None);
    st.modal = Some(Modal::RemoveSubmodule(RemoveForm::from_facts(&f)));
}

#[test]
fn remove_starts_the_job_and_closes() {
    let (mut st, parent, lib) = opened("sm-rm-confirm");
    confirming(&mut st);
    let fx = run(&mut st, sm(SubmoduleMsg::Remove));
    let want = RemoveRequest { repo: st.workdir().unwrap(), path: PATH.into(), name: PATH.into() };
    assert!(matches!(&fx[..], [Effect::Submodule(SubmoduleEffect::Remove(r))] if *r == want), "{fx:?}");
    assert_eq!(fx[0].scope(), None, "it ends the job whichever tab is active");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Remove submodule libs/lib"));
    assert!(st.modal.is_none());

    // A second one while that runs: said so, and the dialog stays.
    confirming(&mut st);
    assert!(run(&mut st, sm(SubmoduleMsg::Remove)).is_empty());
    assert_eq!(last_log(&st), (false, "Another operation is still running.".into()));
    assert!(matches!(st.modal, Some(Modal::RemoveSubmodule(_))));
    cleanup(&[&parent, &lib]);
}

/// With work it would lose, Remove waits for the acknowledgement.
#[test]
fn remove_with_warnings_waits_for_the_acknowledgement() {
    let (mut st, parent, lib) = opened("sm-rm-ack");
    let mut f = facts(sub(&st), None);
    f.stashes = 2;
    let form = RemoveForm::from_facts(&f);
    assert!(form.needs_ack() && !form.acked);
    st.modal = Some(Modal::RemoveSubmodule(form));
    assert!(run(&mut st, sm(SubmoduleMsg::Remove)).is_empty(), "no job before the tick");
    assert!(st.job.is_none());
    assert!(matches!(st.modal, Some(Modal::RemoveSubmodule(_))), "the dialog stays");
    if let Some(Modal::RemoveSubmodule(form)) = st.modal.as_mut() {
        form.acked = true;
    }
    let fx = run(&mut st, sm(SubmoduleMsg::Remove));
    assert!(matches!(&fx[..], [Effect::Submodule(SubmoduleEffect::Remove(_))]), "{fx:?}");
    assert!(st.modal.is_none());
    cleanup(&[&parent, &lib]);
}

#[test]
fn remove_in_an_untrusted_repository_asks_first() {
    let (mut st, dir) = held_repo("sm-rm-held");
    let id = Oid::from_str(&"a".repeat(40)).unwrap();
    let info = SubmoduleInfo {
        name: PATH.into(),
        path: PATH.into(),
        url: Some("../lib.git".into()),
        recorded: id,
        checked_out: Some(id),
        state: SubmoduleState::InSync,
        left_behind: Vec::new(),
        clone_url: None,
        branch: None,
        added: false,
    };
    st.snap.as_mut().unwrap().submodules.push(info);
    confirming(&mut st);
    let fx = run(&mut st, sm(SubmoduleMsg::Remove));
    assert!(fx.is_empty(), "{fx:?}");
    match &st.modal {
        Some(Modal::Trust(p)) => assert_eq!(p.held.as_deref(), Some("Remove submodule libs/lib")),
        _ => panic!("expected the trust prompt"),
    }
    cleanup(&[&dir]);
}

fn removed(st: &mut AppState, outcome: RemoveOutcome, output: &str) -> Vec<Effect> {
    let request = RemoveRequest { repo: st.workdir().unwrap(), path: PATH.into(), name: PATH.into() };
    run(st, sm(SubmoduleMsg::Removed { request, report: RemoveReport { outcome, output: output.into() } }))
}

/// Starts the removal job, so a `Removed` has a job to end.
fn started(st: &mut AppState) {
    confirming(st);
    run(st, sm(SubmoduleMsg::Remove));
    assert!(st.job.is_some());
}

#[test]
fn each_outcome_has_its_message() {
    let (mut st, parent, lib) = opened("sm-rm-outcomes");
    started(&mut st);
    let fx = removed(&mut st, RemoveOutcome::Removed, "$ git submodule deinit -f -- :(literal)libs/lib");
    assert!(st.job.is_none());
    let t = st.toasts.last().unwrap();
    assert!(t.ok && t.text == "Removed submodule libs/lib. Commit to record the removal.", "{}", t.text);
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. } | Effect::LoadRepo { .. })), "the tab reloads: {fx:?}");

    started(&mut st);
    let dir = store(&parent);
    removed(&mut st, RemoveOutcome::ModuleLeft { dir: dir.clone(), error: "it is a link".into() }, "");
    assert!(st.job.is_none());
    let n = st.toasts.len();
    let (git_part, left) = (&st.toasts[n - 2], &st.toasts[n - 1]);
    assert!(git_part.ok && git_part.text == "Removed submodule libs/lib from the index and .gitmodules.", "{}", git_part.text);
    let shown = glowfly_core::native_path::shown(&dir);
    assert!(!left.ok && left.text.starts_with(&format!("Could not delete the stored module at {shown}: it is a link.")), "{}", left.text);
    assert_eq!(last_log(&st), (false, left.text.clone()));

    started(&mut st);
    removed(&mut st, RemoveOutcome::RmFailed, "$ git --literal-pathspecs rm -f -- libs/lib\nfatal: please stage your changes");
    let t = st.toasts.last().unwrap();
    assert!(!t.ok && t.text.contains("but git rm failed: fatal: please stage your changes."), "{}", t.text);
    assert!(st.job.is_none());

    started(&mut st);
    removed(&mut st, RemoveOutcome::RmFailed, "$ git rm -f -- libs/lib\nfatal: Unable to create '/r/.git/index.lock': File exists.");
    let t = st.toasts.last().unwrap();
    let says = "but git rm failed: Another git program is using this repository (index.lock exists). Wait for it to finish, then try \
                again. If none is running, delete that file. It is still in .gitmodules";
    assert!(t.text.contains(says), "{}", t.text);

    started(&mut st);
    removed(&mut st, RemoveOutcome::DeinitFailed, "$ git submodule deinit\nerror: no such submodule");
    let t = st.toasts.last().unwrap();
    assert!(!t.ok && t.text == "Remove submodule libs/lib failed: error: no such submodule", "{}", t.text);
    assert!(st.job.is_none());

    // Without a job (one that already ended), nothing happens.
    let toasts = st.toasts.len();
    assert!(removed(&mut st, RemoveOutcome::Removed, "").is_empty());
    assert_eq!(st.toasts.len(), toasts);
    cleanup(&[&parent, &lib]);
}

#[test]
fn a_removal_ends_its_job_after_a_tab_switch() {
    let (mut st, parent, lib) = opened("sm-rm-switched");
    started(&mut st);
    // The tab changed while it ran: the job's repository is no longer the active one.
    st.job.as_mut().unwrap().workdir = Some(PathBuf::from("C:/another/repo"));
    let fx = removed(&mut st, RemoveOutcome::Removed, "");
    assert!(st.job.is_none());
    assert!(st.toasts.last().unwrap().ok);
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })), "the other tab reloads when selected: {fx:?}");
    cleanup(&[&parent, &lib]);
}

#[test]
fn removes_a_submodule_end_to_end() {
    let (mut st, parent, lib) = opened("sm-rm-e2e");
    assert!(store(&parent).exists());
    let fx = run(&mut st, sm(SubmoduleMsg::RemoveDialog(PATH.into())));
    for e in fx {
        let m = runner::execute(e).expect("the check answers");
        drive(&mut st, m);
    }
    assert!(matches!(st.modal, Some(Modal::RemoveSubmodule(_))));
    let fx = run(&mut st, sm(SubmoduleMsg::Remove));
    finish(&mut st, fx);
    let t = st.toasts.last().unwrap();
    assert!(t.ok && t.text == "Removed submodule libs/lib. Commit to record the removal.", "{}", t.text);
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.submodules.is_empty(), "{:?}", snap.submodules);
    assert!(snap.staged.iter().any(|f| f.path == ".gitmodules"), "{:?}", snap.staged.iter().map(|f| &f.path).collect::<Vec<_>>());
    assert!(!store(&parent).exists());
    assert!(!parent.join("libs").join("lib").exists());
    cleanup(&[&parent, &lib]);
}
