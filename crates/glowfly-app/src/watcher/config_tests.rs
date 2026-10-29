//! A live watcher and the repository's config: an outside edit to it sends
//! `WatchMsg::ConfigChanged` right after the full reload it causes, and other edits don't.

use super::tests::{NoWake, init_repo, settle};
use super::*;
use glowfly_core::testkit::git_cli as git;

/// The watcher's scoped messages for `workdir` until `until` says stop or `timeout` passes, as
/// `Some(full)` for `RepoChanged` and None for `ConfigChanged` (index changes are left out).
fn reports(
    rx: &std::sync::mpsc::Receiver<Msg>,
    workdir: &Path,
    timeout: Duration,
    until: impl Fn(&[Option<bool>]) -> bool,
) -> Vec<Option<bool>> {
    let deadline = Instant::now() + timeout;
    let mut seen = Vec::new();
    while !until(&seen) {
        let Ok(msg) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) else { break };
        let Msg::Scoped { workdir: w, msg } = msg else { panic!("unexpected message") };
        assert_eq!(w, workdir);
        match *msg {
            Msg::Watch(WatchMsg::RepoChanged { full, .. }) => seen.push(Some(full)),
            Msg::Watch(WatchMsg::ConfigChanged) => seen.push(None),
            _ => {}
        }
    }
    seen
}

#[test]
fn an_outside_config_edit_says_the_config_changed() {
    let dir: PathBuf = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-watch-config-{}", std::process::id()));
    init_repo(&dir);
    let (tx, rx) = std::sync::mpsc::channel();
    let target = WatchTarget { workdir: dir.clone(), git_dir: dir.join(".git") };
    let watcher = start(&target, tx, NoWake).expect("watcher starts");
    settle(&rx, &dir);

    git(&dir, &["config", "user.name", "Grace Hopper"]);
    let seen = reports(&rx, &dir, Duration::from_secs(10), |s| s.contains(&None));
    let at = seen.iter().position(Option::is_none).unwrap_or_else(|| panic!("no ConfigChanged: {seen:?}"));
    assert!(at > 0 && seen[at - 1] == Some(true), "it follows the full reload: {seen:?}");
    while !reports(&rx, &dir, Duration::from_secs(1), |s| !s.is_empty()).is_empty() {}

    std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
    let seen = reports(&rx, &dir, Duration::from_secs(3), |_| false);
    assert!(seen.contains(&Some(false)) && !seen.contains(&None), "a work-tree edit leaves the config alone: {seen:?}");

    drop(watcher);
    let _ = std::fs::remove_dir_all(&dir);
}
