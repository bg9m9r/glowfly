//! Auto-stashes waiting to go back survive a restart: the record written while an operation
//! holds the stash, the offer when the repository is opened again (Put back, Leave in stash
//! list, Cancel), a stash dropped meanwhile, an operation still in progress, and the common case
//! that writes nothing.

use super::auto_stash_flow_tests::{mine_is_back, stopped_merge};
use super::auto_stash_tests::up;
use super::cherry_pick_tests::feature_repo;
use super::tests::{drive, git, skipped, state};
use crate::effect::{Effect, StashEffect};
use crate::msg::{Msg, RepoMsg};
use crate::runner;
use crate::state::auto_stash::{saved_gone, saved_left, saved_title};
use crate::state::{AppState, Modal};
use glowfly_core::i18n::Lang;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MESSAGE: &str = "Auto-stash before merging feature into main";

fn file(dir: &Path) -> PathBuf {
    glowfly_core::stash_ops::saved_path(dir).unwrap()
}

fn stash_count(dir: &Path) -> usize {
    glowfly_core::stash_ops::list(dir).unwrap().len()
}

fn open(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    st
}

/// A merge that stopped holding the auto-stash, its record written; then the app "closed".
fn saved_then_closed(name: &str) -> PathBuf {
    let (st, dir) = stopped_merge(name);
    let text = std::fs::read_to_string(file(&dir)).expect("the record is written");
    let oid = glowfly_core::stash_ops::list(&dir).unwrap()[0].oid.to_string();
    assert!(text.contains(&oid) && text.contains("\"kind\": \"merge\""), "{text}");
    assert_eq!(glowfly_core::stash_ops::read_saved(&dir).len(), 1);
    drop(st);
    dir
}

fn offered(st: &AppState) -> Option<&str> {
    match &st.modal {
        Some(Modal::ConfirmChoice { title, body, .. }) if title == saved_title(Lang::En) => Some(body),
        _ => None,
    }
}

fn logged(st: &AppState, text: &str) -> bool {
    st.log.iter().any(|l| l.text == text)
}

#[test]
fn a_stopped_operation_survives_a_restart() {
    let dir = saved_then_closed("ass-restart");
    git(&dir, &["merge", "--abort"]);
    let mut st = open(&dir);
    let body = offered(&st).expect("the offer");
    assert!(body.contains(MESSAGE), "{body}");
    drive(&mut st, Msg::ConfirmAccepted);
    mine_is_back(&st, &dir);
    assert!(!file(&dir).exists(), "the record is removed");
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn leave_in_stash_list_forgets_the_record() {
    let dir = saved_then_closed("ass-leave");
    git(&dir, &["merge", "--abort"]);
    let mut st = open(&dir);
    assert!(offered(&st).is_some());
    drive(&mut st, Msg::ConfirmAlt);
    assert!(!file(&dir).exists());
    assert_eq!(stash_count(&dir), 1, "the stash stays");
    assert!(logged(&st, &saved_left(Lang::En, MESSAGE)));
    drop(st);
    assert!(open(&dir).modal.is_none(), "not offered again after a restart");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dropped_stash_is_not_offered() {
    let dir = saved_then_closed("ass-dropped");
    git(&dir, &["merge", "--abort"]);
    git(&dir, &["stash", "drop", "-q"]);
    let st = open(&dir);
    assert!(st.modal.is_none());
    assert!(!file(&dir).exists(), "the stale record is dropped");
    assert!(logged(&st, &saved_gone(Lang::En, MESSAGE)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_offer_waits_for_a_stopped_operation() {
    let dir = saved_then_closed("ass-wait");
    let mut st = open(&dir);
    assert!(st.modal.is_none(), "the merge is still in progress");
    git(&dir, &["merge", "--abort"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(offered(&st).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancel_asks_again_next_open() {
    let dir = saved_then_closed("ass-cancel");
    git(&dir, &["merge", "--abort"]);
    let mut st = open(&dir);
    assert!(offered(&st).is_some());
    drive(&mut st, Msg::CloseModal);
    assert!(file(&dir).exists(), "nothing changes");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.modal.is_none(), "not asked again this session");
    drop(st);
    assert!(offered(&open(&dir)).is_some(), "asked again at the next open");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `finish`, also returning the name of every effect raised along the way.
fn finish_naming(st: &mut AppState, fx: Vec<Effect>) -> Vec<&'static str> {
    let (mut queue, mut names, mut fx) = (Vec::new(), Vec::new(), fx);
    loop {
        for effect in fx {
            names.push(effect.name());
            if !skipped(&effect) {
                queue.extend(runner::execute(effect));
            }
        }
        let Some(m) = queue.pop() else { return names };
        fx = super::update(st, m, Instant::now());
    }
}

#[test]
fn the_common_case_writes_nothing() {
    let (mut st, dir, _) = feature_repo("ass-common", &[("feature work", "feature\n")], None);
    std::fs::write(dir.join("a.txt"), "one\nmine\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(up(&mut st, Msg::git("Checkout", &["checkout", "feature", "--"])).is_empty());
    let fx = up(&mut st, Msg::AutoStash(crate::msg::AutoStashMsg::Stash));
    let names = finish_naming(&mut st, fx);
    mine_is_back(&st, &dir);
    let save = Effect::Stash(StashEffect::SaveAutoStashes { cwd: dir.clone(), saved: Vec::new() }).name();
    assert!(!names.contains(&save), "{names:?}");
    assert!(!file(&dir).exists());
    let _ = std::fs::remove_dir_all(&dir);
}
