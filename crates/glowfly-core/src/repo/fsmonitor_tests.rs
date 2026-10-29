//! Git's built-in file system monitor: where it is offered, its commands, reading git's answers,
//! and starting it in a fixture repository.

use super::*;
use crate::gitcli::args;
use crate::repo::{StatusSpeedups, status_speedups, untracked_cache_cmds};
use crate::testutil::{git, scratch_repo};
use std::path::PathBuf;

#[test]
fn offers_follow_the_os_and_version() {
    for os in [Os::Windows, Os::MacOs] {
        for v in [Some((2, 36, 0)), Some((2, 54, 1)), None] {
            for daemon in [Some(true), None] {
                assert_eq!(monitor_offer_for(os, v, daemon), MonitorOffer::Daemon, "{os:?} {v:?}");
            }
            assert_eq!(monitor_offer_for(os, v, Some(false)), MonitorOffer::NotBuilt, "{os:?} {v:?}");
        }
        assert_eq!(monitor_offer_for(os, Some((2, 35, 9)), None), MonitorOffer::TooOld((2, 35, 9)));
        assert_eq!(monitor_offer_for(os, Some((2, 35, 9)), Some(false)), MonitorOffer::TooOld((2, 35, 9)));
    }
    for v in [Some((2, 35, 9)), Some((2, 36, 0)), Some((2, 54, 1)), None] {
        assert_eq!(monitor_offer_for(Os::Linux, v, Some(true)), MonitorOffer::Watchman, "{v:?}");
    }
    for v in [Some((2, 30, 0)), Some((2, 45, 0)), None] {
        assert_eq!(monitor_offer(v, None), monitor_offer_for(Os::current(), v, None));
    }
}

#[test]
fn the_commands_are_gits_own() {
    assert_eq!(monitor_status_cmd().join(" "), "fsmonitor--daemon status");
    assert_eq!(monitor_start_cmd().join(" "), "fsmonitor--daemon start");
    assert_eq!(monitor_enable_cmd().join(" "), "config core.fsmonitor true");
    assert_eq!(monitor_disable_cmd().join(" "), "config --unset-all core.fsmonitor");
    assert_eq!(monitor_stop_cmd().join(" "), "fsmonitor--daemon stop");
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
pub(crate) struct Stopped(pub(crate) PathBuf);

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
    if daemon_built(&dir) != Some(true) {
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

    // Off again: the setting is gone and no daemon watches; a second time changes nothing.
    let (ok, log) = turn_off_monitor(&dir);
    assert!(ok, "{log}");
    assert!(log.contains("$ git config --unset-all core.fsmonitor") && log.contains("$ git fsmonitor--daemon stop"), "{log}");
    assert!(!status_speedups(&dir).fsmonitor, "{log}");
    let status = gitcli::output(&dir, &monitor_status_cmd(), "", gitcli::NO_PROMPTS, None);
    assert_eq!(status_answer(&status), Ok(false), "{log}");
    let (ok, log) = turn_off_monitor(&dir);
    assert!(ok, "already off: {log}");
    assert_eq!(stop_monitor(&dir), "", "none running");
    drop(guard);
}

#[test]
fn build_options_name_the_daemon() {
    let built = "git version 2.54.0.windows.1\ncpu: x86_64\nfeature: fsmonitor--daemon\ngettext: enabled\n";
    assert!(daemon_in_build_options(built));
    assert!(daemon_in_build_options("feature: fsmonitor--daemon\r\n"));
    assert!(!daemon_in_build_options("git version 2.43.0\ncpu: x86_64\nsizeof-long: 8\n"));
    assert!(!daemon_in_build_options("feature: fsmonitor--daemon-ish\n"));
    let dir = scratch_repo("fsmonitor-probe");
    let probed = daemon_built(&dir).expect("git answers");
    let raw = gitcli::read(&dir, &args(&["version", "--build-options"])).stdout;
    assert_eq!(probed, raw.contains("feature: fsmonitor--daemon"), "{raw}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stopping_and_turning_off_without_a_daemon() {
    let dir = scratch_repo("fsmonitor-off");
    assert_eq!(stop_monitor(&dir), "", "no daemon runs here");
    git(&dir, &["config", "core.fsmonitor", "true"]);
    let (ok, log) = turn_off_monitor(&dir);
    assert!(ok, "{log}");
    assert!(!status_speedups(&dir).fsmonitor, "{log}");
    let (ok, log) = turn_off_monitor(&dir);
    assert!(ok, "never set: {log}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn turning_off_unsets_every_local_value() {
    let dir = scratch_repo("fsmonitor-off-twice");
    git(&dir, &["config", "--add", "core.fsmonitor", "true"]);
    git(&dir, &["config", "--add", "core.fsmonitor", "true"]);
    let (ok, log) = turn_off_monitor(&dir);
    assert!(ok, "{log}");
    let left = gitcli::read(&dir, &args(&["config", "--local", "--get-all", "core.fsmonitor"]));
    assert!(!left.ok && left.stdout.trim().is_empty(), "both values are gone: {}", left.stdout);
    assert!(!status_speedups(&dir).fsmonitor, "{log}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_daemon_serves_only_where_git_offers_it() {
    let dir = scratch_repo("fsmonitor-serves");
    let built = daemon_built(&dir) == Some(true);
    assert_eq!(daemon_serves(&dir), built && Os::current() != Os::Linux, "a local folder");
    let _ = std::fs::remove_dir_all(&dir);
}
