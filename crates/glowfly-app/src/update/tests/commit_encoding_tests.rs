//! Headless tests for commit messages in a repository with `i18n.commitEncoding` set to a
//! legacy encoding (#242): commit and amend round trips, and the refusals for characters the
//! encoding can't hold, in the commit box and the rebase panel.

use super::rebase_tests::with_panel;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{CommitMsg, Msg, RebaseMsg, RepoMsg, StageMsg};
use crate::state::AppState;
use glowfly_core::commit_encoding::MessageEncoding;
use glowfly_core::rebase::{RebaseAction, RebaseBase, TodoStep};
use std::path::Path;
use std::time::Instant;

fn head_bytes(dir: &Path) -> Vec<u8> {
    let repo = git2::Repository::open(dir).unwrap();
    let c = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(c.message_encoding(), Some("windows-1252"));
    c.message_bytes().to_vec()
}

fn head_message(st: &AppState) -> String {
    st.snap.as_ref().unwrap().head_message.clone().unwrap()
}

#[test]
fn commit_and_two_amends_keep_a_windows_1252_message() {
    let dir = scratch_repo("commit-enc-amend");
    git(&dir, &["config", "i18n.commitEncoding", "windows-1252"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    st.session.draft.summary = "Café für alle".into();
    st.session.draft.description = "5 € each.".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert_eq!(head_bytes(&dir), b"Caf\xe9 f\xfcr alle\n\n5 \x80 each.\n");
    for round in 1..=2 {
        st.session.draft.amend = true;
        drive(&mut st, Msg::Commit(CommitMsg::PrefillAmend));
        assert_eq!(st.session.draft.summary, "Café für alle", "prefill {round}");
        drive(&mut st, Msg::Commit(CommitMsg::Commit));
        assert_eq!(head_message(&st), "Café für alle\n\n5 € each.", "after amend {round}");
    }
    assert_eq!(head_bytes(&dir), b"Caf\xe9 f\xfcr alle\n\n5 \x80 each.\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn characters_the_encoding_lacks_refuse_the_commit() {
    let dir = scratch_repo("commit-enc-refuse");
    git(&dir, &["config", "i18n.commitEncoding", "windows-1252"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    st.session.draft.summary = "Done ✓".into();
    assert!(update(&mut st, Msg::Commit(CommitMsg::Commit), Instant::now()).is_empty(), "nothing runs");
    let toast = st.toasts.last().expect("a toast");
    assert!(!toast.ok);
    assert_eq!(
        toast.text,
        "This repository writes commit messages in windows-1252 (i18n.commitEncoding), which can't hold \"✓\". Remove them from the message."
    );
    assert_eq!(st.session.draft.summary, "Done ✓", "the draft is kept");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_utf8_repository_writes_any_character() {
    let dir = scratch_repo("commit-enc-utf8");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    st.session.draft.summary = "Done ✓ 日本".into();
    let fx = update(&mut st, Msg::Commit(CommitMsg::Commit), Instant::now());
    let written = fx.iter().find_map(|e| match e {
        Effect::RunGit { write_file: Some((_, b)), .. } => Some(b.clone()),
        _ => None,
    });
    assert_eq!(written.as_deref(), Some("Done ✓ 日本\n".as_bytes()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_rebase_reword_the_encoding_cannot_hold_blocks_start() {
    let (mut st, dir, _) = with_panel("rebase-enc-refuse", RebaseBase::FromCommit(git2::Oid::zero()));
    st.snap.as_mut().unwrap().commit_encoding = MessageEncoding::named(Some("windows-1252"));
    let p = st.session.rebase.view.as_mut().unwrap();
    let item = p.items.iter_mut().find_map(TodoStep::commit_mut).unwrap();
    item.action = RebaseAction::Reword;
    item.new_message = Some("c1 ✓".into());
    let lang = st.settings.lang();
    let why = st.session.rebase.view.as_ref().unwrap().message_blocker(st.snap.as_ref().unwrap(), lang);
    assert!(why.as_deref().is_some_and(|w| w.contains("\"✓\"")), "{why:?}");
    let fx = update(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: true, autostash: false }), Instant::now());
    assert!(fx.is_empty(), "nothing starts: {fx:?}");
    assert!(st.log.last().is_some_and(|l| l.text.contains("windows-1252")));

    let p = st.session.rebase.view.as_mut().unwrap();
    p.items.iter_mut().find_map(TodoStep::commit_mut).unwrap().new_message = Some("c1, café".into());
    assert_eq!(p.message_blocker(st.snap.as_ref().unwrap(), lang), None);
    let _ = std::fs::remove_dir_all(&dir);
}
