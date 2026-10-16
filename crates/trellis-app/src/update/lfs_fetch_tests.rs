//! Headless tests for the LFS download that follows a successful auto-fetch (update/lfs.rs):
//! when it starts, that it stays quiet, and when it doesn't start.

use super::branch_tests::with_origin;
use super::notify::{AUTO_FETCH, AUTO_FETCH_LFS};
use super::tests::{drive, finish, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{JobMsg, Msg, RepoMsg};
use crate::runner;
use crate::state::lfs::LfsInstall;
use crate::state::trust;
use crate::state::{AppState, RunningJob};
use std::path::Path;
use std::time::{Duration, Instant};
use trellis_core::gitcli::args;
use trellis_core::settings::RepoTrust;

const ATTRIBUTES: &str = "*.bin filter=lfs diff=lfs merge=lfs -text\n";

/// Whether `fx` starts `git lfs fetch`.
fn lfs_fetch(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::RunGit { cmds, .. } if *cmds == [args(&["lfs", "fetch"])]))
}

/// Makes the open repository store `*.bin` in LFS (reloaded, so the snapshot has the pattern),
/// with git-lfs installed.
fn use_lfs(st: &mut AppState, dir: &Path) {
    std::fs::write(dir.join(".gitattributes"), ATTRIBUTES).unwrap();
    drive(st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.snap.as_ref().is_some_and(|s| !s.lfs_patterns.is_empty()), "the reload read the pattern");
    st.lfs_install = LfsInstall::Installed("3.7.1".into());
}

/// Ends a pretend job labelled `label` in the active tab, without running anything.
fn end(st: &mut AppState, label: &str, quiet: bool, ok: bool) -> Vec<Effect> {
    st.job = Some(RunningJob {
        label: label.into(),
        workdir: st.workdir(),
        quiet,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        keeps_undo: true,
    });
    update(st, Msg::Job(JobMsg::JobDone { ok, output: String::new(), undo: None }), Instant::now())
}

#[test]
fn auto_fetch_in_an_lfs_repository_starts_a_quiet_lfs_download() {
    let (mut st, work, bare) = with_origin("lfs-auto-fetch-follow");
    use_lfs(&mut st, &work);
    st.settings.auto_fetch_minutes = 1;
    let tick = st.last_fetch + Duration::from_secs(61);
    let mut fx = update(&mut st, Msg::Tick(tick), Instant::now());
    assert!(matches!(&fx[..], [Effect::RunGit { .. }]), "{fx:?}");
    let done = runner::execute(fx.remove(0)).expect("the fetch's result");
    let fx = update(&mut st, done, Instant::now());
    let quiet = fx.iter().any(|e| matches!(e, Effect::RunGit { cmds, prompts: false, .. } if *cmds == [args(&["lfs", "fetch"])]));
    assert!(quiet, "{fx:?}");
    let job = st.job.as_ref().expect("the download is the active job");
    assert_eq!((job.label.as_str(), job.quiet), (AUTO_FETCH_LFS, true));

    finish(&mut st, fx);
    assert!(st.job.is_none(), "nothing follows the download");
    assert!(st.log.iter().any(|l| l.text.starts_with(AUTO_FETCH_LFS)), "logged");
    assert!(st.toasts.iter().all(|t| t.ok), "never a failure toast");
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn no_lfs_download_unless_the_repository_uses_lfs_and_git_may_run() {
    let dir = scratch_repo("lfs-auto-fetch-skip");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    use_lfs(&mut st, &dir);
    assert!(lfs_fetch(&end(&mut st, AUTO_FETCH, true, true)), "starts when everything holds");
    st.job = None;

    assert!(!lfs_fetch(&end(&mut st, AUTO_FETCH, true, false)), "the fetch failed");
    assert!(!lfs_fetch(&end(&mut st, "Fetch", false, true)), "a manual fetch has its own LFS button");
    let fx = end(&mut st, AUTO_FETCH_LFS, true, true);
    assert!(fx.iter().all(|e| !matches!(e, Effect::RunGit { .. })) && st.job.is_none(), "the download's end starts nothing");
    for missing in [LfsInstall::Missing, LfsInstall::Unknown, LfsInstall::Checking] {
        st.lfs_install = missing;
        assert!(!lfs_fetch(&end(&mut st, AUTO_FETCH, true, true)), "git-lfs isn't known to be installed");
    }
    st.lfs_install = LfsInstall::Installed("3.7.1".into());

    let head = st.snap.as_mut().unwrap().head_oid.take();
    assert!(!lfs_fetch(&end(&mut st, AUTO_FETCH, true, true)), "an unborn branch has no files to download");
    st.snap.as_mut().unwrap().head_oid = head;

    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    trust::record(&mut st.settings, &real, RepoTrust::NotTrusted);
    assert!(!lfs_fetch(&end(&mut st, AUTO_FETCH, true, true)), "git is held in a repository that isn't trusted");
    st.settings.repo_trust.clear();

    st.snap.as_mut().unwrap().lfs_patterns.clear();
    assert!(!lfs_fetch(&end(&mut st, AUTO_FETCH, true, true)), "no LFS patterns");
    assert!(st.job.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
