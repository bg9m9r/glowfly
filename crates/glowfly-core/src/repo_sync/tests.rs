use super::*;
use crate::settings::SavedTrust;
use crate::testutil::{git, remote_commit, scratch_repo, set_config, with_origin};

/// Trusted, by a decision saved before prints were kept.
const T: Option<&SavedTrust> = Some(&SavedTrust { trust: crate::settings::RepoTrust::Trusted, print: None });

#[test]
fn fetch_then_pull_report_what_changed() {
    let (work, bare) = with_origin("sync-ff");
    remote_commit(&bare, "main", "b.txt", "two");
    assert_eq!(sync(&work, SyncOp::Fetch { prune: true }, T), Outcome::Updated { changed: 1 }, "origin/main moved");
    assert_eq!(sync(&work, SyncOp::Fetch { prune: false }, T), Outcome::UpToDate);
    // The pull fast-forwards main (and HEAD with it).
    assert!(matches!(sync(&work, SyncOp::Pull, T), Outcome::Updated { .. }));
    assert!(work.join("b.txt").exists());
    assert_eq!(sync(&work, SyncOp::Pull, T), Outcome::UpToDate);
}

#[test]
fn nothing_to_fetch_or_pull() {
    let lone = scratch_repo("sync-lone");
    assert_eq!(sync(&lone, SyncOp::Fetch { prune: true }, T), Outcome::NoRemotes);
    assert_eq!(sync(&lone, SyncOp::Pull, T), Outcome::NoRemotes);
    // A branch without an upstream has nothing to pull, though it can fetch.
    let (work, _bare) = with_origin("sync-no-upstream");
    git(&work, &["checkout", "-q", "-b", "topic"]);
    assert_eq!(sync(&work, SyncOp::Pull, T), Outcome::NoUpstream);
    assert_eq!(sync(&work, SyncOp::Fetch { prune: true }, T), Outcome::UpToDate);
    git(&work, &["checkout", "-q", "--detach"]);
    assert_eq!(sync(&work, SyncOp::Pull, T), Outcome::NoUpstream, "no branch checked out");
}

#[test]
fn a_repository_that_runs_programs_is_skipped_unless_trusted() {
    let (work, _bare) = with_origin("sync-untrusted");
    set_config(&work, &[("core.sshCommand", "ssh-wrapper")]);
    assert_eq!(sync(&work, SyncOp::Fetch { prune: true }, None), Outcome::Untrusted);
    assert_eq!(sync(&work, SyncOp::Pull, None), Outcome::Untrusted);
    // Without a print a decision covers nothing that runs (saved before prints were kept).
    assert_eq!(sync(&work, SyncOp::Fetch { prune: true }, T), Outcome::Untrusted);
    // The remote is a local folder, so the ssh command never runs.
    let trusted = SavedTrust::trusted(Some(crate::trust::scan_at(&work).unwrap().print()));
    assert_eq!(sync(&work, SyncOp::Fetch { prune: true }, Some(&trusted)), Outcome::UpToDate);
    // Nothing in a plain repository runs programs: no decision is needed.
    let (plain, _) = with_origin("sync-plain");
    assert_eq!(sync(&plain, SyncOp::Fetch { prune: true }, None), Outcome::UpToDate);
}

#[test]
fn failures_say_why() {
    let gone = crate::paths::temp_dir().join(format!("glowfly-sync-gone-{}", std::process::id()));
    assert_eq!(sync(&gone, SyncOp::Pull, T), Outcome::Failed { reason: "The folder can't be found.".into(), detail: String::new() });
    let (work, bare) = with_origin("sync-fails");
    git(&work, &["remote", "set-url", "origin", &bare.with_file_name("no-such-origin.git").display().to_string()]);
    let Outcome::Failed { reason, detail } = sync(&work, SyncOp::Fetch { prune: true }, T) else { panic!("fetching a missing remote") };
    assert!(!reason.is_empty() && !reason.contains('\n'), "{reason}");
    assert!(detail.contains(&reason) && !detail.contains('\r'), "{detail}");
    // A folder that isn't a repository.
    let plain = crate::paths::temp_dir().join(format!("glowfly-sync-plain-folder-{}", std::process::id()));
    std::fs::create_dir_all(&plain).unwrap();
    assert!(matches!(sync(&plain, SyncOp::Fetch { prune: true }, T), Outcome::Failed { .. }));
    let _ = std::fs::remove_dir_all(&plain);
}

#[test]
fn commands_fetch_every_remote_and_only_fast_forward() {
    assert_eq!(command(SyncOp::Fetch { prune: true }), ["fetch", "--all", "--prune"]);
    assert_eq!(command(SyncOp::Fetch { prune: false }), ["fetch", "--all"]);
    assert_eq!(command(SyncOp::Pull), ["pull", "--ff-only"]);
}

#[test]
fn error_detail_keeps_what_git_showed_last() {
    assert_eq!(error_detail("Receiving objects: 10%\rReceiving objects: 100%\nfatal: boom\n\n"), "Receiving objects: 100%\nfatal: boom");
    let d = error_detail("fatal: unable to access 'https://ada:s3cret@example.com/r.git/'");
    assert!(!d.contains("s3cret") && d.contains("example.com"), "{d}");
    let many: String = (1..=100).map(|i| format!("line {i}\n")).collect();
    let d = error_detail(&many);
    assert_eq!(d.lines().count(), 40);
    assert!(d.starts_with("line 61\n") && d.ends_with("line 100"), "{d}");
    let long = format!("{}\nfatal: the end", "x".repeat(9000));
    let d = error_detail(&long);
    assert!(d.chars().count() <= 4000 && d.ends_with("fatal: the end"), "{}", d.chars().count());
}

#[test]
fn a_trusted_repository_that_gained_a_command_since_is_skipped() {
    let (work, _bare) = with_origin("sync-stale-print");
    set_config(&work, &[("credential.helper", "store")]);
    let before = crate::trust::scan_at(&work).unwrap().print();
    set_config(&work, &[("core.sshCommand", "ssh-wrapper")]);
    let stale = SavedTrust::trusted(Some(before));
    assert_eq!(sync(&work, SyncOp::Fetch { prune: true }, Some(&stale)), Outcome::Untrusted);
    let now = SavedTrust::trusted(Some(crate::trust::scan_at(&work).unwrap().print()));
    assert_eq!(sync(&work, SyncOp::Fetch { prune: true }, Some(&now)), Outcome::UpToDate);
}
