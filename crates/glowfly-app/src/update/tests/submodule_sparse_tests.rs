//! Headless tests for submodules in a sparse checkout: Initialize all and the update after a job
//! leave out the ones whose folders the cone leaves out (git would check them out all the same),
//! naming them in the log once.

use super::submodule_batch_tests::{checks_all, job_cmds, step};
use super::submodule_tests::{PATH, cleanup, opened};
use super::tests::{drive, finish, git};
use super::update;
use crate::msg::{Msg, RepoMsg};
use crate::state::AppState;
use crate::submodule::{Batch, SubmoduleMsg};
use glowfly_core::repo::{SubmoduleInfo, SubmoduleState};
use glowfly_core::submodule_ops::batch_update_cmds;
use std::path::PathBuf;
use std::time::Instant;

const FAR: &str = "far/lib";

/// `opened` plus a second submodule at `far/lib`, both uninitialized, with a cone sparse
/// checkout of `libs` only.
fn sparse_two(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (mut st, parent, lib) = opened(name);
    let url = format!("../{}", lib.file_name().unwrap().to_string_lossy());
    git(&parent, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, FAR]);
    git(&parent, &["commit", "-q", "-m", "add far"]);
    git(&parent, &["submodule", "deinit", "-q", "-f", "--all"]);
    git(&parent, &["sparse-checkout", "set", "--cone", "libs"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, parent, lib)
}

fn at<'a>(st: &'a AppState, path: &str) -> &'a SubmoduleInfo {
    st.snap.as_ref().unwrap().submodules.iter().find(|s| s.path == path).expect("listed")
}

fn noted(st: &AppState) -> usize {
    st.log.iter().filter(|l| l.ok && l.text.starts_with("Left out the submodules the sparse checkout leaves out: far/lib.")).count()
}

#[test]
fn initialize_all_leaves_out_the_submodules_outside_the_cone() {
    let (mut st, parent, lib) = sparse_two("smb-sparse-init");
    assert!(at(&st, FAR).outside_sparse && !at(&st, PATH).outside_sparse);
    let fx = update(&mut st, Msg::Submodule(SubmoduleMsg::Batch { batch: Batch::InitAll, confirmed: false }), Instant::now());
    let fx = step(&mut st, fx);
    assert_eq!(job_cmds(&st, &fx, "Initialize submodules"), Some(batch_update_cmds(Some(&[PATH.to_owned()]), true)));
    assert_eq!(noted(&st), 1);
    finish(&mut st, fx);
    assert_eq!(at(&st, PATH).state, SubmoduleState::InSync);
    assert!(!at(&st, FAR).is_initialized());
    assert!(!parent.join(FAR).join(".git").exists(), "nothing checked out outside the cone");

    // Again: nothing to do, and the note isn't repeated.
    let fx = update(&mut st, Msg::Submodule(SubmoduleMsg::Batch { batch: Batch::InitAll, confirmed: false }), Instant::now());
    let fx = step(&mut st, fx);
    assert!(st.job.is_none(), "{fx:?}");
    assert_eq!(noted(&st), 1, "once per repository");
    cleanup(&[&parent, &lib]);
}

#[test]
fn the_update_after_a_job_leaves_them_out_too() {
    let (mut st, parent, lib) = sparse_two("smb-sparse-auto");
    st.settings.submodules.keep_updated = true;
    let fx = update(&mut st, Msg::git("Checkout", &["checkout", "-q", "-b", "other"]), Instant::now());
    let fx = step(&mut st, fx);
    assert!(checks_all(&fx, Batch::Auto), "{fx:?}");
    let fx = step(&mut st, fx);
    assert_eq!(job_cmds(&st, &fx, "Update submodules"), Some(batch_update_cmds(Some(&[PATH.to_owned()]), false)));
    finish(&mut st, fx);
    assert!(!at(&st, FAR).is_initialized());
    cleanup(&[&parent, &lib]);
}
