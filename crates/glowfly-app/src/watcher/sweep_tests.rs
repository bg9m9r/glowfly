//! Sweeps asked of a live watcher (the Windows safety poll): the first reads everything, later
//! ones report only what changed, and only the watched repository's watcher sweeps.

use super::tests::{NoWake, init_repo, next_change, next_report, settle};
use super::*;

#[test]
fn a_live_watcher_sweeps_its_own_tree() {
    let dir: PathBuf = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-watch-sweep-{}", std::process::id()));
    init_repo(&dir);
    let (tx, rx) = std::sync::mpsc::channel();
    let target = WatchTarget { workdir: dir.clone(), git_dir: dir.join(".git") };
    let watcher = start(&target, tx, NoWake).expect("watcher starts");
    settle(&rx, &dir);
    let slot = WatchSlot(Arc::new(Mutex::new((1, Some(watcher)))));

    assert!(slot.sweep(&dir));
    assert_eq!(next_report(&rx, &dir, Duration::from_secs(10)), Some((false, None)), "the baseline reads the whole status");
    assert!(slot.sweep(&dir));
    assert_eq!(next_change(&rx, &dir, Duration::from_secs(1)), None, "nothing changed: nothing sent");
    let elsewhere = glowfly_core::paths::temp_dir().join("glowfly-app-test-watch-sweep-elsewhere");
    assert!(!slot.sweep(&elsewhere), "another repository's watcher isn't running");
    assert!(!WatchSlot::default().sweep(&dir), "no watcher at all");

    drop(slot);
    let _ = std::fs::remove_dir_all(&dir);
}
