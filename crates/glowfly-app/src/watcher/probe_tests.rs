//! The watcher's probe: answered by its own file's event, timing out to silent, unknown without
//! a writable git dir, and a live watcher on a fixture repository seeing its probe.

use super::*;
use crate::effect::WatchTarget;
use crate::msg::{Msg, WatchMsg};
use crate::watcher::tests::{NoWake, init_repo};
use notify::EventKind;
use notify::event::{CreateKind, RemoveKind};

fn names_of(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()
}

fn probe_files(git_dir: &Path) -> Vec<String> {
    names_of(git_dir).into_iter().filter(|n| n.starts_with("glowfly-watch-")).collect()
}

/// A started probe and when its wait began (its deadline less the timeout).
fn started(git_dir: &Path) -> (Prober, Instant) {
    let (p, unknown) = Prober::start(git_dir);
    assert_eq!(unknown, None);
    let Prober::Waiting { deadline, .. } = &p else { panic!("the probe is out: {p:?}") };
    let t0 = *deadline - PROBE_TIMEOUT;
    (p, t0)
}

#[test]
fn the_probe_s_own_event_makes_it_live_and_removes_it() {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-probe-seen-{}", std::process::id()));
    init_repo(&dir);
    let git_dir = dir.join(".git");
    let before = Instant::now();
    let (mut p, t0) = started(&git_dir);
    assert!(t0 >= before, "the wait starts once the file is out");
    assert_eq!(p.wait(t0), Some(PROBE_TIMEOUT));
    let name = probe_files(&git_dir).pop().expect("the probe is out");
    assert!(name.ends_with(".lock"));
    let other = Event::new(EventKind::Create(CreateKind::File)).add_path(dir.join("a.txt"));
    assert_eq!(p.event(&other), None, "another event doesn't answer it");
    assert_eq!(p.due(t0 + PROBE_TIMEOUT / 2), None);
    // As Windows spells the git dir under a recursive watch of the work tree.
    let seen = Event::new(EventKind::Create(CreateKind::File)).add_path(dir.join(".git").join(&name));
    assert_eq!(p.event(&seen), Some(Probe::Live));
    assert!(probe_files(&git_dir).is_empty(), "removed once seen");
    assert_eq!(p.wait(t0), None);
    assert_eq!(p.event(&other), None, "answered once");
    assert_eq!(p.due(t0 + PROBE_TIMEOUT * 2), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_event_in_time_is_silent_until_any_event_comes() {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-probe-silent-{}", std::process::id()));
    init_repo(&dir);
    let git_dir = dir.join(".git");
    let (mut p, t0) = started(&git_dir);
    assert_eq!(p.due(t0 + PROBE_TIMEOUT), Some(Probe::Silent));
    assert!(probe_files(&git_dir).is_empty(), "removed at the timeout");
    assert_eq!(p.due(t0 + PROBE_TIMEOUT * 2), None, "said once");
    assert_eq!(p.wait(t0), None, "nothing to wait for");
    let late = Event::new(EventKind::Remove(RemoveKind::File)).add_path(dir.join("a.txt"));
    assert_eq!(p.event(&late), Some(Probe::Live), "events do arrive after all");
    assert_eq!(p.event(&late), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_git_dir_that_cannot_be_written_is_unknown() {
    let missing = glowfly_core::paths::temp_dir().join("glowfly-app-test-probe-missing").join(".git");
    let (p, unknown) = Prober::start(&missing);
    assert_eq!(unknown, Some(Probe::Unknown));
    assert_eq!(p.wait(Instant::now()), None);
}

#[test]
fn waits_take_the_sooner() {
    let s = Duration::from_secs;
    assert_eq!(sooner(Some(s(1)), Some(s(2))), Some(s(1)));
    assert_eq!(sooner(None, Some(s(2))), Some(s(2)));
    assert_eq!(sooner(Some(s(3)), None), Some(s(3)));
    assert_eq!(sooner(None, None), None);
}

#[test]
fn a_live_watcher_sees_its_probe() {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-probe-live-{}", std::process::id()));
    init_repo(&dir);
    let (tx, rx) = std::sync::mpsc::channel();
    let target = WatchTarget { workdir: dir.clone(), git_dir: dir.join(".git") };
    let watcher = crate::watcher::start(&target, tx, NoWake).expect("watcher starts");
    let deadline = Instant::now() + Duration::from_secs(10);
    let probe = loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Msg::Watch(WatchMsg::Probed { workdir, probe })) => {
                assert_eq!(workdir, dir);
                break probe;
            }
            Ok(Msg::Watch(WatchMsg::WatchFailed { error, .. })) => panic!("watcher failed: {error}"),
            Ok(_) => {} // what the fixture's own writes reported
            Err(e) => panic!("no probe answer: {e}"),
        }
    };
    assert_eq!(probe, Probe::Live);
    // The file is removed as soon as it is seen; its removal reloads nothing.
    let gone = Instant::now() + Duration::from_secs(5);
    while !probe_files(&dir.join(".git")).is_empty() && Instant::now() < gone {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(probe_files(&dir.join(".git")).is_empty());
    drop(watcher);
    let _ = std::fs::remove_dir_all(&dir);
}
