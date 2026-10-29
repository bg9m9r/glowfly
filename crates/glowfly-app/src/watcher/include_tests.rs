//! A live watcher and the config files the repository's config includes from outside it: an
//! edit to one reloads everything, and an include added later is followed.

use super::tests::{NoWake, init_repo, next_change, settle};
use super::*;
use glowfly_core::testkit::git_cli as git;

#[test]
fn edits_to_included_config_files_outside_the_repository_reload_everything() {
    let dir: PathBuf = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-watch-include-{}", std::process::id()));
    let shared = dir.with_file_name(format!("{}-shared", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&shared);
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(shared.join("team.cfg"), "[core]\n").unwrap();
    init_repo(&dir);
    let named = |p: &Path| p.display().to_string().replace('\\', "/");
    git(&dir, &["config", "include.path", &named(&shared.join("team.cfg"))]);

    let (tx, rx) = std::sync::mpsc::channel();
    let target = WatchTarget { workdir: dir.clone(), git_dir: dir.join(".git") };
    let watcher = start(&target, tx, NoWake).expect("watcher starts");
    settle(&rx, &dir);

    std::fs::write(shared.join("other.txt"), "x\n").unwrap();
    assert_eq!(next_change(&rx, &dir, Duration::from_secs(1)), None, "only the included file counts");
    std::fs::write(shared.join("team.cfg"), "[core]\n\tautocrlf = false\n").unwrap();
    assert_eq!(next_change(&rx, &dir, Duration::from_secs(10)), Some(true), "the included file reloads in full");
    while next_change(&rx, &dir, Duration::from_secs(1)).is_some() {}

    // An include added later: the config reloads, and the new file is watched from then on.
    let later = shared.join("later");
    std::fs::create_dir_all(&later).unwrap();
    git(&dir, &["config", "--add", "include.path", &named(&later.join("more.cfg"))]);
    assert_eq!(next_change(&rx, &dir, Duration::from_secs(10)), Some(true));
    while next_change(&rx, &dir, Duration::from_secs(1)).is_some() {}
    std::fs::write(later.join("more.cfg"), "[core]\n").unwrap();
    assert_eq!(next_change(&rx, &dir, Duration::from_secs(10)), Some(true), "a new include is followed");

    drop(watcher);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&shared);
}
