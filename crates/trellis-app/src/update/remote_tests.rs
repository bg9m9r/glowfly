//! Headless tests for the REMOTE section: the Add/Edit remote dialog's prefill, refusals and
//! jobs, Fetch one remote, Remove after a confirmation, the undo history, the forge remote
//! after a rename and the palette entries. Commands are checked as emitted, and end to end
//! against a repository with a bare `origin`.

use super::branch_tests::with_origin;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::remote::RemoteMsg;
use crate::state::palette::{PaletteCommand, entries};
use crate::state::remotes::RemoteForm;
use crate::state::{AppState, Modal};
use std::path::Path;
use std::time::Instant;
use trellis_core::gitcli::args;
use trellis_core::repo::RemoteInfo;
use trellis_core::undo::Action;

fn run(st: &mut AppState, m: RemoteMsg) -> Vec<Effect> {
    update(st, Msg::Remote(m), Instant::now())
}

fn form(st: &mut AppState) -> &mut RemoteForm {
    match st.modal.as_mut() {
        Some(Modal::Remote(f)) => f,
        _ => panic!("no remote dialog"),
    }
}

/// The single RunGit job's commands in `fx`.
fn git_cmds(fx: &[Effect]) -> Vec<Vec<String>> {
    match fx {
        [Effect::RunGit { cmds, .. }] => cmds.clone(),
        other => panic!("unexpected effects {other:?}"),
    }
}

fn v(lines: &[&[&str]]) -> Vec<Vec<String>> {
    lines.iter().map(|l| args(l)).collect()
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn last_toast(st: &AppState) -> String {
    st.toasts.last().map(|t| t.text.clone()).unwrap_or_default()
}

fn info<'a>(st: &'a AppState, name: &str) -> Option<&'a RemoteInfo> {
    st.snap.as_ref().unwrap().remote_info.iter().find(|r| r.name == name)
}

fn branches(st: &AppState, remote: &str) -> Option<Vec<String>> {
    st.snap.as_ref().unwrap().remotes.iter().find(|(r, _)| r == remote).map(|(_, b)| b.iter().map(|(n, _)| n.clone()).collect())
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// Ends the running job as failed, without running it.
fn drop_job(st: &mut AppState) {
    drive(st, Msg::JobDone { ok: false, output: String::new(), undo: None });
}

/// Submits the open dialog and runs the job to completion.
fn submit(st: &mut AppState) {
    drive(st, Msg::Remote(RemoteMsg::Submit));
}

#[test]
fn add_dialog_prefill() {
    let mut st = state();
    assert!(run(&mut st, RemoteMsg::AddDialog).is_empty());
    assert!(st.modal.is_none(), "no repository, no dialog");

    let dir = scratch_repo("remote-prefill");
    drive(&mut st, Msg::Open(dir.clone()));
    run(&mut st, RemoteMsg::AddDialog);
    let f = form(&mut st);
    assert_eq!(f.name, "origin");
    assert!(f.fetch_after && !f.is_edit());

    let (mut st2, work, bare) = with_origin("remote-prefill-origin");
    run(&mut st2, RemoteMsg::AddDialog);
    assert_eq!(form(&mut st2).name, "");
    cleanup(&[&dir, &work, &bare]);
}

#[test]
fn add_remote_end_to_end() {
    let (mut st, work, bare) = with_origin("remote-add");
    run(&mut st, RemoteMsg::AddDialog);
    let f = form(&mut st);
    f.name = "mirror".into();
    f.url = bare.display().to_string();
    f.fetch_after = true;
    submit(&mut st);
    assert!(st.modal.is_none());
    assert_eq!(last_toast(&st), "Add remote mirror succeeded");
    assert!(info(&st, "mirror").is_some());
    let mut b = branches(&st, "mirror").unwrap();
    b.sort();
    assert_eq!(b, vec!["feature".to_owned(), "main".to_owned()]);
    cleanup(&[&work, &bare]);
}

#[test]
fn add_with_push_url_no_fetch() {
    let (mut st, work, bare) = with_origin("remote-add-push");
    let fill = |st: &mut AppState| {
        run(st, RemoteMsg::AddDialog);
        let f = form(st);
        (f.name, f.url, f.push_url, f.fetch_after) =
            ("x".into(), " https://example.invalid/x.git ".into(), "https://example.invalid/p.git".into(), false);
    };
    fill(&mut st);
    let fx = run(&mut st, RemoteMsg::Submit);
    assert_eq!(
        git_cmds(&fx),
        v(&[
            &["remote", "add", "x", "https://example.invalid/x.git"],
            &["remote", "set-url", "--push", "x", "https://example.invalid/p.git"]
        ])
    );
    drop_job(&mut st);
    fill(&mut st);
    submit(&mut st);
    assert_eq!(info(&st, "x").unwrap().push_urls, vec!["https://example.invalid/p.git".to_owned()]);
    assert_eq!(branches(&st, "x"), Some(Vec::new()));
    cleanup(&[&work, &bare]);
}

#[test]
fn add_refusals_keep_dialog_open() {
    let (mut st, work, bare) = with_origin("remote-add-refusals");
    let cases = [
        ("", "u", "", "Enter a remote name."),
        ("origin", "u", "", "A remote named origin already exists."),
        ("ORIGIN", "u", "", "A remote named origin already exists."),
        ("bad name", "u", "", "Remote names can't contain spaces."),
        ("ok", "", "", "Enter a fetch URL."),
        ("ok", "-x", "", "URLs can't start with a dash."),
        ("ok", "u", "-x", "URLs can't start with a dash."),
    ];
    for (name, url, push, message) in cases {
        run(&mut st, RemoteMsg::AddDialog);
        let f = form(&mut st);
        (f.name, f.url, f.push_url) = (name.into(), url.into(), push.into());
        assert!(run(&mut st, RemoteMsg::Submit).is_empty(), "{name:?} {url:?}");
        assert_eq!(last_log(&st), message);
        assert!(matches!(st.modal, Some(Modal::Remote(_))));
    }
    // Busy: the dialog stays open.
    let f = form(&mut st);
    (f.name, f.url, f.push_url) = ("ok".into(), "u".into(), String::new());
    assert!(!update(&mut st, Msg::git("Fetch", &["fetch"]), Instant::now()).is_empty());
    assert!(run(&mut st, RemoteMsg::Submit).is_empty());
    assert!(last_log(&st).contains("Another operation is still running."), "{}", last_log(&st));
    assert!(matches!(st.modal, Some(Modal::Remote(_))));
    cleanup(&[&work, &bare]);
}

#[test]
fn edit_rename_and_push_url() {
    let (mut st, work, bare) = with_origin("remote-edit-rename");
    let fill = |st: &mut AppState| {
        run(st, RemoteMsg::EditDialog("origin".into()));
        let f = form(st);
        f.name = "upstream".into();
        f.push_url = "C:/elsewhere.git".into();
    };
    run(&mut st, RemoteMsg::EditDialog("origin".into()));
    let f = form(&mut st);
    assert_eq!((f.name.as_str(), f.url.as_str(), f.push_url.as_str()), ("origin", bare.display().to_string().as_str(), ""));
    assert!(f.is_edit() && !f.changed());
    fill(&mut st);
    let fx = run(&mut st, RemoteMsg::Submit);
    assert_eq!(
        git_cmds(&fx),
        v(&[&["remote", "rename", "origin", "upstream"], &["remote", "set-url", "--push", "upstream", "C:/elsewhere.git"]])
    );
    assert!(st.modal.is_none());
    drop_job(&mut st);

    fill(&mut st);
    submit(&mut st);
    assert_eq!(last_toast(&st), "Edit remote origin succeeded");
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.remote_info.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["upstream"]);
    assert_eq!(snap.remote_info[0].push_urls, vec!["C:/elsewhere.git".to_owned()]);
    let main = snap.locals.iter().find(|b| b.name == "main").unwrap();
    assert_eq!(main.tracking.as_ref().unwrap().remote, "upstream");
    assert!(branches(&st, "upstream").is_some_and(|b| b.contains(&"main".to_owned())));
    cleanup(&[&work, &bare]);
}

#[test]
fn edit_refuses_case_only_rename() {
    // git remote rename origin Origin stops halfway on Windows and macOS, so it never runs.
    let (mut st, work, bare) = with_origin("remote-edit-case");
    run(&mut st, RemoteMsg::EditDialog("origin".into()));
    form(&mut st).name = "Origin".into();
    assert!(run(&mut st, RemoteMsg::Submit).is_empty());
    assert_eq!(last_log(&st), "Changing only the letter case isn't supported. Rename to another name first, then to this one.");
    assert!(matches!(st.modal, Some(Modal::Remote(_))));
    assert!(st.job.is_none());
    assert!(info(&st, "origin").is_some());
    cleanup(&[&work, &bare]);
}

#[test]
fn edit_clears_push_url() {
    let (mut st, work, bare) = with_origin("remote-edit-clear");
    git(&work, &["remote", "set-url", "--push", "origin", "https://h/x+y(1).git"]);
    drive(&mut st, Msg::Refresh);
    run(&mut st, RemoteMsg::EditDialog("origin".into()));
    let f = form(&mut st);
    assert_eq!(f.push_url, "https://h/x+y(1).git");
    f.push_url.clear();
    let fx = run(&mut st, RemoteMsg::Submit);
    assert_eq!(git_cmds(&fx), v(&[&["remote", "set-url", "--delete", "--push", "origin", r"^https://h/x\+y\(1\)\.git$"]]));
    drop_job(&mut st);
    run(&mut st, RemoteMsg::EditDialog("origin".into()));
    form(&mut st).push_url.clear();
    submit(&mut st);
    assert!(info(&st, "origin").unwrap().push_urls.is_empty());
    cleanup(&[&work, &bare]);
}

#[test]
fn edit_unchanged_and_gone() {
    let (mut st, work, bare) = with_origin("remote-edit-gone");
    run(&mut st, RemoteMsg::EditDialog("origin".into()));
    assert!(run(&mut st, RemoteMsg::Submit).is_empty());
    assert!(st.modal.is_none(), "nothing changed: the dialog just closes");

    assert!(run(&mut st, RemoteMsg::EditDialog("nope".into())).is_empty());
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), "Remote nope no longer exists.");

    run(&mut st, RemoteMsg::EditDialog("origin".into()));
    form(&mut st).url = "https://example.invalid/new.git".into();
    git(&work, &["remote", "remove", "origin"]);
    drive(&mut st, Msg::Refresh);
    st.log.clear();
    assert!(run(&mut st, RemoteMsg::Submit).is_empty());
    assert_eq!(last_log(&st), "Remote origin no longer exists.");
    assert!(st.job.is_none());
    cleanup(&[&work, &bare]);
}

#[test]
fn remove_asks_then_removes() {
    let (mut st, work, bare) = with_origin("remote-remove");
    assert!(run(&mut st, RemoteMsg::RemoveDialog("origin".into())).is_empty());
    let Some(Modal::Confirm { title, body, button, then }) = &st.modal else { panic!("no confirmation") };
    assert_eq!(title, "Remove remote origin");
    assert_eq!(button, "Remove remote");
    for part in ["origin/main", "origin/feature", "the local branches", "main", "feature", "Nothing changes on the server."] {
        assert!(body.contains(part), "{part}: {body}");
    }
    assert!(matches!(&**then, Msg::Remote(RemoteMsg::Remove(n)) if n == "origin"));
    drive(&mut st, Msg::ConfirmAccepted);
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.remotes.is_empty() && snap.remote_info.is_empty());
    assert!(snap.locals.iter().all(|b| b.tracking.is_none()));
    assert_eq!(last_toast(&st), "Remove remote origin succeeded");

    assert!(run(&mut st, RemoteMsg::Remove("nope".into())).is_empty());
    assert_eq!(last_log(&st), "Remote nope no longer exists.");
    cleanup(&[&work, &bare]);
}

#[test]
fn fetch_one_remote() {
    let (mut st, work, bare) = with_origin("remote-fetch");
    super::branch_tests::remote_commit(&bare, "main", "new.txt");
    let fx = run(&mut st, RemoteMsg::Fetch("origin".into()));
    assert_eq!(git_cmds(&fx), v(&[&["fetch", "--prune", "origin"]]));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Fetch origin"));
    drop_job(&mut st);
    drive(&mut st, Msg::Remote(RemoteMsg::Fetch("origin".into())));
    let bare_main = git2::Repository::open_bare(&bare).unwrap().refname_to_id("refs/heads/main").unwrap();
    let snap = st.snap.as_ref().unwrap();
    let (_, b) = snap.remotes.iter().find(|(r, _)| r == "origin").unwrap();
    assert_eq!(b.iter().find(|(n, _)| n == "main").map(|(_, o)| *o), Some(bare_main));

    assert!(run(&mut st, RemoteMsg::Fetch("nope".into())).is_empty());
    assert_eq!(last_log(&st), "Remote nope no longer exists.");
    // Listed through its push URL alone.
    git(&work, &["config", "remote.nourl.pushurl", "https://example.invalid/p.git"]);
    drive(&mut st, Msg::Refresh);
    assert!(run(&mut st, RemoteMsg::Fetch("nourl".into())).is_empty());
    assert_eq!(last_log(&st), "Can't fetch nourl: it has no fetch URL.");
    cleanup(&[&work, &bare]);
}

#[test]
fn remote_jobs_and_undo() {
    let keeps = |a: &[&str]| super::undo::keeps_history(&[args(a)]);
    assert!(keeps(&["remote", "add", "x", "u"]));
    assert!(keeps(&["remote", "set-url", "--push", "x", "u"]));
    assert!(keeps(&["fetch", "--prune", "origin"]));
    assert!(!keeps(&["remote", "rename", "a", "b"]));
    assert!(!keeps(&["remote", "remove", "a"]));

    let (mut st, work, bare) = with_origin("remote-undo");
    st.undo.record(Action::DeleteBranch { name: "gone".into(), oid: git2::Oid::zero(), upstream: None });
    run(&mut st, RemoteMsg::AddDialog);
    let f = form(&mut st);
    (f.name, f.url, f.fetch_after) = ("other".into(), "https://example.invalid/o.git".into(), false);
    submit(&mut st);
    assert_eq!(last_toast(&st), "Add remote other succeeded");
    assert_eq!(st.undo.undo.len(), 1, "adding a remote keeps the history");
    drive(&mut st, Msg::Remote(RemoteMsg::Remove("other".into())));
    assert_eq!(last_toast(&st), "Remove remote other succeeded");
    assert!(st.undo.undo.is_empty(), "removing a remote invalidates it");
    cleanup(&[&work, &bare]);
}

#[test]
fn rename_follows_forge_remote() {
    let dir = scratch_repo("remote-forge");
    git(&dir, &["remote", "add", "origin", "https://github.com/owner/repo.git"]);
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    let host = st.forge.as_ref().expect("forge detected").host.clone();
    assert!(host.is_some());
    assert_eq!(st.forge.as_ref().unwrap().remote, "origin");
    run(&mut st, RemoteMsg::EditDialog("origin".into()));
    form(&mut st).name = "upstream".into();
    submit(&mut st);
    assert_eq!(last_toast(&st), "Edit remote origin succeeded");
    let fs = st.forge.as_ref().unwrap();
    assert_eq!(fs.remote, "upstream");
    assert_eq!(fs.host, host);
    cleanup(&[&dir]);
}

#[test]
fn palette_remote_entries() {
    let (mut st, work, bare) = with_origin("remote-palette");
    let all = entries(&st);
    let labels: Vec<&str> = all.iter().map(|e| e.label.as_str()).collect();
    assert!(labels.contains(&"Add remote…"), "{labels:?}");
    let fetch = all.iter().find(|e| e.label == "Fetch origin").expect("Fetch origin");
    assert_eq!(fetch.detail.as_deref(), Some(bare.display().to_string().as_str()));
    let pos = |l: &str| labels.iter().position(|x| *x == l).unwrap();
    assert!(pos("Fetch all remotes") < pos("Fetch origin"));

    let fx = update(&mut st, Msg::RunPaletteCommand(PaletteCommand::Remote(RemoteMsg::Fetch("origin".into()))), Instant::now());
    assert_eq!(git_cmds(&fx), v(&[&["fetch", "--prune", "origin"]]));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Fetch origin"));
    drop_job(&mut st);
    update(&mut st, Msg::RunPaletteCommand(PaletteCommand::Remote(RemoteMsg::AddDialog)), Instant::now());
    assert!(matches!(st.modal, Some(Modal::Remote(_))));
    cleanup(&[&work, &bare]);
}
