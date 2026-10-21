//! Headless tests for the Edit submodule dialog: opening it prefilled, refusing what git can't
//! do, the commands Save runs, and an edit end to end.

use super::submodule_tests::{PATH, cleanup, last_log, opened, sub};
use super::tests::{drive, finish, git};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, RepoMsg};
use crate::state::submodules::edit::EditForm;
use crate::state::{AppState, Modal};
use crate::submodule::SubmoduleMsg;
use glowfly_core::git_version::GitInfo;
use glowfly_core::submodule_ops::{set_branch_cmd, set_url_cmds};
use std::time::Instant;

fn dialog(st: &mut AppState) {
    update(st, Msg::Submodule(SubmoduleMsg::EditDialog(PATH.into())), Instant::now());
}

fn form(st: &mut AppState) -> &mut EditForm {
    match st.modal.as_mut() {
        Some(Modal::EditSubmodule(f)) => f,
        _ => panic!("expected the Edit submodule dialog"),
    }
}

fn save(st: &mut AppState) -> Vec<Effect> {
    update(st, Msg::Submodule(SubmoduleMsg::Edit), Instant::now())
}

fn cmds(fx: &[Effect]) -> Vec<Vec<String>> {
    match fx {
        [Effect::RunGit { cmds, .. }] => cmds.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn opens_prefilled_and_refuses_what_git_cant_do() {
    let (mut st, parent, lib) = opened("sme-open");
    git(&parent, &["config", "-f", ".gitmodules", "submodule.libs/lib.branch", "dev"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    dialog(&mut st);
    let f = form(&mut st);
    assert_eq!(f.url, sub_url(&lib));
    assert_eq!(f.branch, "dev");
    assert!(!f.changed());
    st.modal = None;

    st.git.info = Some(GitInfo { path: None, version: Some((2, 20, 0)), chosen: false });
    dialog(&mut st);
    assert!(st.modal.is_none());
    let (ok, text) = last_log(&st);
    assert!(!ok && text.contains("needs git 2.25"), "{text}");
    st.git.info = None;

    st.snap.as_mut().unwrap().submodules[0].url = None;
    dialog(&mut st);
    assert!(st.modal.is_none());
    assert!(last_log(&st).1.contains(".gitmodules has no entry"), "{:?}", last_log(&st));
    cleanup(&[&parent, &lib]);
}

fn sub_url(lib: &std::path::Path) -> String {
    format!("../{}", lib.file_name().unwrap().to_string_lossy())
}

#[test]
fn save_runs_the_changed_parts() {
    let (mut st, parent, lib) = opened("sme-save");
    git(&parent, &["config", "-f", ".gitmodules", "submodule.libs/lib.branch", "dev"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let url = "https://example.com/o/lib.git";

    dialog(&mut st);
    form(&mut st).url = url.into();
    let fx = save(&mut st);
    assert_eq!(cmds(&fx), set_url_cmds(PATH, url));
    assert!(st.modal.is_none(), "closed once the job started");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Edit submodule libs/lib"));
    st.job = None;

    dialog(&mut st);
    form(&mut st).branch = String::new();
    assert_eq!(cmds(&save(&mut st)), [set_branch_cmd(PATH, None)]);
    st.job = None;

    dialog(&mut st);
    let f = form(&mut st);
    f.url = url.into();
    f.branch = "main".into();
    let mut want = set_url_cmds(PATH, url);
    want.push(set_branch_cmd(PATH, Some("main")));
    assert_eq!(cmds(&save(&mut st)), want);
    st.job = None;

    dialog(&mut st);
    form(&mut st).url = "-x".into();
    assert!(save(&mut st).is_empty());
    assert_eq!(last_log(&st), (false, "URLs can't start with a dash.".to_owned()));
    assert!(matches!(st.modal, Some(Modal::EditSubmodule(_))), "stays open");
    cleanup(&[&parent, &lib]);
}

#[test]
fn edit_end_to_end() {
    let (mut st, parent, lib) = opened("sme-e2e");
    if !st.git.has(glowfly_core::git_version::Feature::SubmoduleEdit)
        || glowfly_core::gitcli::version(&parent).is_some_and(|v| (v.0, v.1) < (2, 25))
    {
        eprintln!("git is older than 2.25, skipping");
        return cleanup(&[&parent, &lib]);
    }
    let url = "https://example.com/o/lib.git";
    dialog(&mut st);
    form(&mut st).url = url.into();
    let fx = save(&mut st);
    finish(&mut st, fx);
    let t = st.toasts.last().expect("a toast");
    assert!(t.ok && t.text == "Edit submodule libs/lib succeeded", "{}", t.text);
    let sm = sub(&st);
    assert_eq!(sm.url.as_deref(), Some(url));
    assert_eq!(sm.clone_url.as_deref(), Some(url), "synced into this repository's config");
    cleanup(&[&parent, &lib]);
}
