//! Git's built-in file system monitor: where it is offered, its commands, reading git's answers,
//! and starting it in a fixture repository.

use super::*;
use crate::gitcli::args;
use crate::repo::{StatusSpeedups, status_speedups, untracked_cache_cmds};
use crate::testutil::scratch_repo;
use std::path::PathBuf;

#[test]
fn offers_follow_the_os_and_version() {
    for os in [Os::Windows, Os::MacOs] {
        for v in [Some((2, 36, 0)), Some((2, 54, 1)), None] {
            assert_eq!(monitor_offer_for(os, v), MonitorOffer::Daemon, "{os:?} {v:?}");
        }
        assert_eq!(monitor_offer_for(os, Some((2, 35, 9))), MonitorOffer::TooOld((2, 35, 9)));
    }
    for v in [Some((2, 35, 9)), Some((2, 36, 0)), Some((2, 54, 1)), None] {
        assert_eq!(monitor_offer_for(Os::Linux, v), MonitorOffer::Watchman, "{v:?}");
    }
    for v in [Some((2, 30, 0)), Some((2, 45, 0)), None] {
        assert_eq!(monitor_offer(v), monitor_offer_for(Os::current(), v));
    }
}

#[test]
fn the_commands_are_gits_own() {
    assert_eq!(monitor_status_cmd().join(" "), "fsmonitor--daemon status");
    assert_eq!(monitor_start_cmd().join(" "), "fsmonitor--daemon start");
    assert_eq!(monitor_enable_cmd().join(" "), "config core.fsmonitor true");
    assert_eq!(monitor_record_cmd().join(" "), "status --porcelain --untracked-files=all");
    assert_eq!(untracked_cache_cmds(StatusSpeedups::default()).last(), Some(&monitor_record_cmd()));
}

#[test]
fn gits_refusals_read_plainly() {
    use MonitorTrouble::*;
    assert_eq!(trouble_in("fatal: fsmonitor--daemon not supported on this platform\n"), Some(Unsupported));
    assert_eq!(trouble_in("git: 'fsmonitor--daemon' is not a git command. See 'git --help'.\n"), Some(Unsupported));
    assert_eq!(trouble_in("error: remote repository '//srv/share/r' is incompatible with fsmonitor\n"), Some(Remote));
    assert_eq!(
        trouble_in("error: bare repository '/x' is incompatible with fsmonitor\n"),
        Some(Incompatible("bare repository '/x' is incompatible with fsmonitor".into()))
    );
    assert_eq!(
        trouble_in("socket directory '/x' is incompatible with fsmonitor"),
        Some(Incompatible("socket directory '/x' is incompatible with fsmonitor".into()))
    );
    assert_eq!(trouble_in("fatal: not a git repository (or any of the parent directories): .git"), None);

    let troubles = [
        Unsupported,
        Remote,
        Incompatible("bare repository '/x' is incompatible with fsmonitor".into()),
        NotWatching,
        Failed("boom".into()),
    ];
    for t in troubles {
        let text = t.text();
        let sentences = text.matches(". ").count() + usize::from(text.ends_with('.'));
        assert!((1..=2).contains(&sentences), "{text}");
        assert!(!text.contains("error: ") && !text.contains("fatal: "), "{text}");
    }
    assert!(Remote.text().contains("network drives"), "{}", Remote.text());
    assert_eq!(Failed("it timed out.".into()).text(), "Git's file system monitor didn't start: it timed out. It was left off.");
}

#[test]
fn status_answers() {
    let out = |code, stdout: &str, stderr: &str| GitOutput {
        ok: code == 0,
        code: Some(code),
        stdout: stdout.into(),
        stderr: stderr.into(),
        ..Default::default()
    };
    assert_eq!(status_answer(&out(0, "fsmonitor-daemon is watching '/r'\n", "")), Ok(true));
    assert_eq!(status_answer(&out(1, "fsmonitor-daemon is not watching '/r'\n", "")), Ok(false));
    assert_eq!(
        status_answer(&out(1, "", "error: remote repository '//srv/r' is incompatible with fsmonitor\n")),
        Err(MonitorTrouble::Remote)
    );
    assert_eq!(status_answer(&out(128, "", "fatal: fsmonitor--daemon not supported on this platform\n")), Err(MonitorTrouble::Unsupported));
}

#[test]
fn turning_it_on_keeps_trust() {
    let dir = scratch_repo("fsmonitor-trust");
    let (ok, out) = gitcli::run(&dir, &monitor_enable_cmd());
    assert!(ok, "{out}");
    assert!(status_speedups(&dir).fsmonitor);
    let scan = crate::trust::scan(&git2::Repository::open(&dir).unwrap());
    assert!(scan.config.is_empty(), "a boolean core.fsmonitor runs no program: {:?}", scan.config);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Stops the daemon watching `0` and removes the folder, even when an assertion failed: on
/// Windows the daemon holds the folder open.
struct Stopped(PathBuf);

impl Drop for Stopped {
    fn drop(&mut self) {
        let _ = gitcli::run(&self.0, &args(&["fsmonitor--daemon", "stop"]));
        for _ in 0..20 {
            if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// `turn_on_monitor` on another thread, failing rather than hanging if a pipe stays open.
fn turned_on(dir: &Path) -> (MonitorOutcome, String) {
    let (tx, rx) = std::sync::mpsc::channel();
    let dir = dir.to_path_buf();
    std::thread::spawn(move || {
        let _ = tx.send(turn_on_monitor(&dir));
    });
    rx.recv_timeout(Duration::from_secs(120)).expect("turn_on_monitor returned")
}

#[test]
fn the_daemon_starts_and_is_confirmed() {
    let dir = scratch_repo("fsmonitor-live");
    let guard = Stopped(dir.clone());
    let build = gitcli::read(&dir, &args(&["version", "--build-options"])).stdout;
    if !build.contains("feature: fsmonitor--daemon") {
        eprintln!("skipped: this git has no built-in file system monitor");
        return;
    }
    let (outcome, log) = turned_on(&dir);
    assert_eq!(outcome, MonitorOutcome::Watching { started: true }, "{log}");
    assert!(status_speedups(&dir).fsmonitor, "{log}");
    assert!(log.contains("$ git fsmonitor--daemon start"), "{log}");
    assert!(log.contains("$ git config core.fsmonitor true"), "{log}");

    let (outcome, log) = turned_on(&dir);
    assert_eq!(outcome, MonitorOutcome::Watching { started: false }, "{log}");
    assert!(!log.contains("fsmonitor--daemon start"), "{log}");
    drop(guard);
}
