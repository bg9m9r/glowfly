//! Undo and redo of a removed remote and a renamed one, through the runner: the removal comes
//! back with every URL and refspec, the toast's Fetch brings its remote-tracking branches back,
//! and a rename made in the Edit remote dialog is renamed back.

use super::branch_tests::with_origin;
use super::tests::drive;
use glowfly_core::i18n::Lang;
use glowfly_model::msg::{JobMsg, Msg};
use glowfly_model::remote::RemoteMsg;
use glowfly_model::state::{AppState, Modal};
use std::path::Path;

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// Every `remote.<name>.*` value in the repository's config, in order.
fn section(dir: &Path, name: &str) -> Vec<(String, String)> {
    let repo = git2::Repository::open(dir).unwrap();
    let cfg = repo.config().unwrap().open_level(git2::ConfigLevel::Local).unwrap().snapshot().unwrap();
    let prefix = format!("remote.{name}.");
    let mut out = Vec::new();
    let mut it = cfg.entries(None).unwrap();
    while let Some(e) = it.next() {
        let e = e.unwrap();
        if let Some(key) = e.name().and_then(|n| n.strip_prefix(&prefix)) {
            out.push((key.to_owned(), glowfly_core::config_entry::utf8(e).unwrap_or("true").to_owned()));
        }
    }
    out
}

fn has_ref(dir: &Path, name: &str) -> bool {
    git2::Repository::open(dir).unwrap().find_reference(name).is_ok()
}

fn undo_label(st: &AppState) -> String {
    st.session.undo.undo_entry(Lang::En, true).0
}

#[test]
fn a_removed_remote_comes_back_and_its_fetch_brings_its_branches() {
    let (mut st, work, bare) = with_origin("undo-flow-remote-remove");
    let url = bare.display().to_string();
    glowfly_model::testing::git(&work, &["config", "--add", "remote.origin.url", &url]);
    glowfly_model::testing::git(&work, &["config", "--add", "remote.origin.fetch", "+refs/heads/main:refs/remotes/origin/main-only"]);
    drive(&mut st, Msg::Repo(glowfly_model::msg::RepoMsg::Refresh));
    let before = section(&work, "origin");
    assert_eq!(before.iter().filter(|(k, _)| k == "url").count(), 2);
    assert!(has_ref(&work, "refs/remotes/origin/main"));

    drive(&mut st, Msg::Remote(RemoteMsg::RemoveDialog("origin".into())));
    assert!(matches!(st.modal, Some(Modal::ConfirmDanger { .. })), "asks first");
    drive(&mut st, Msg::ConfirmAccepted);
    assert!(section(&work, "origin").is_empty() && !has_ref(&work, "refs/remotes/origin/main"));
    assert_eq!(undo_label(&st), "Undo removal of remote origin");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(section(&work, "origin"), before, "every value back, in order");
    let toast = st.toasts.last().expect("a toast");
    assert_eq!(toast.text, "Remote origin is back. Fetch it to bring back its remote-tracking branches.");
    let fetch = toast.action.as_ref().expect("the Fetch button");
    assert_eq!(fetch.label, "Fetch origin");
    assert!(matches!(&*fetch.msg, Msg::Remote(RemoteMsg::Fetch(n)) if n == "origin"));
    let id = toast.id;
    drive(&mut st, Msg::ToastAction(id));
    assert!(has_ref(&work, "refs/remotes/origin/main"), "the fetch brought the remote branches back");
    let main = st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == "main").and_then(|b| b.tracking.clone());
    assert_eq!(main.map(|t| t.remote), Some("origin".into()), "main tracks it again");

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(section(&work, "origin").is_empty(), "removed again");
    cleanup(&[&work, &bare]);
}

#[test]
fn a_rename_in_the_edit_dialog_is_renamed_back() {
    let (mut st, work, bare) = with_origin("undo-flow-remote-rename");
    drive(&mut st, Msg::Remote(RemoteMsg::EditDialog("origin".into())));
    let Some(Modal::Remote(form)) = st.modal.as_mut() else { panic!("no remote dialog") };
    form.name = "upstream".into();
    drive(&mut st, Msg::Remote(RemoteMsg::Submit));
    assert!(has_ref(&work, "refs/remotes/upstream/main"));
    assert_eq!(undo_label(&st), "Undo rename of remote origin to upstream");
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(has_ref(&work, "refs/remotes/origin/main") && !has_ref(&work, "refs/remotes/upstream/main"));
    assert!(st.snap.as_ref().unwrap().remote_info.iter().any(|r| r.name == "origin"));
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(has_ref(&work, "refs/remotes/upstream/main"));
    cleanup(&[&work, &bare]);
}
