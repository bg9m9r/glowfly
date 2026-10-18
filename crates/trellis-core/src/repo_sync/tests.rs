use super::*;
use crate::testutil::{git, remote_commit, scratch_repo, set_config, with_origin};

#[test]
fn fetch_then_pull_report_what_changed() {
    let (work, bare) = with_origin("sync-ff");
    remote_commit(&bare, "main", "b.txt", "two");
    assert_eq!(sync(&work, SyncOp::Fetch { prune: true }, true), Outcome::Updated { changed: 1 }, "origin/main moved");
    assert_eq!(sync(&work, SyncOp::Fetch { prune: false }, true), Outcome::UpToDate);
    // The pull fast-forwards main (and HEAD with it).
    assert!(matches!(sync(&work, SyncOp::Pull, true), Outcome::Updated { .. }));
    assert!(work.join("b.txt").exists());
    assert_eq!(sync(&work, SyncOp::Pull, true), Outcome::UpToDate);
}

#[test]
fn nothing_to_fetch_or_pull() {
    let lone = scratch_repo("sync-lone");
    assert_eq!(sync(&lone, SyncOp::Fetch { prune: true }, true), Outcome::NoRemotes);
    assert_eq!(sync(&lone, SyncOp::Pull, true), Outcome::NoRemotes);
    // A branch without an upstream has nothing to pull, though it can fetch.
    let (work, _bare) = with_origin("sync-no-upstream");
    git(&work, &["checkout", "-q", "-b", "topic"]);
    assert_eq!(sync(&work, SyncOp::Pull, true), Outcome::NoUpstream);
    assert_eq!(sync(&work, SyncOp::Fetch { prune: true }, true), Outcome::UpToDate);
    git(&work, &["checkout", "-q", "--detach"]);
    assert_eq!(sync(&work, SyncOp::Pull, true), Outcome::NoUpstream, "no branch checked out");
}

#[test]
fn a_repository_that_runs_programs_is_skipped_unless_trusted() {
    let (work, _bare) = with_origin("sync-untrusted");
    set_config(&work, &[("core.sshCommand", "ssh-wrapper")]);
    assert_eq!(sync(&work, SyncOp::Fetch { prune: true }, false), Outcome::Untrusted);
    assert_eq!(sync(&work, SyncOp::Pull, false), Outcome::Untrusted);
    // The remote is a local folder, so the ssh command never runs.
    assert_eq!(sync(&work, SyncOp::Fetch { prune: true }, true), Outcome::UpToDate);
    // Nothing in a plain repository runs programs: no decision is needed.
    let (plain, _) = with_origin("sync-plain");
    assert_eq!(sync(&plain, SyncOp::Fetch { prune: true }, false), Outcome::UpToDate);
}

#[test]
fn failures_say_why() {
    let gone = crate::paths::temp_dir().join(format!("trellis-sync-gone-{}", std::process::id()));
    assert_eq!(sync(&gone, SyncOp::Pull, true), Outcome::Failed("The folder can't be found.".into()));
    let (work, bare) = with_origin("sync-fails");
    git(&work, &["remote", "set-url", "origin", &bare.with_file_name("no-such-origin.git").display().to_string()]);
    let Outcome::Failed(why) = sync(&work, SyncOp::Fetch { prune: true }, true) else { panic!("fetching a missing remote") };
    assert!(!why.is_empty() && !why.contains('\n'), "{why}");
    // A folder that isn't a repository.
    let plain = crate::paths::temp_dir().join(format!("trellis-sync-plain-folder-{}", std::process::id()));
    std::fs::create_dir_all(&plain).unwrap();
    assert!(matches!(sync(&plain, SyncOp::Fetch { prune: true }, true), Outcome::Failed(_)));
    let _ = std::fs::remove_dir_all(&plain);
}

#[test]
fn commands_fetch_every_remote_and_only_fast_forward() {
    assert_eq!(command(SyncOp::Fetch { prune: true }), ["fetch", "--all", "--prune"]);
    assert_eq!(command(SyncOp::Fetch { prune: false }), ["fetch", "--all"]);
    assert_eq!(command(SyncOp::Pull), ["pull", "--ff-only"]);
}
