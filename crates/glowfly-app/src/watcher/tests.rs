//! The watcher's bursts, its classification of raw events, and live watchers on fixture
//! repositories.

use super::*;
use glowfly_core::gitcli::{args, run};
use std::path::PathBuf;

const MS: Duration = Duration::from_millis(1);

#[test]
fn bursts_wait_for_quiet() {
    let t0 = Instant::now();
    let mut b = Burst::default();
    b.add(Change::Status, Vec::new(), t0);
    b.add(Change::Status, Vec::new(), t0 + 100 * MS);
    assert!(b.take_due(t0 + 350 * MS).is_none());
    assert_eq!(b.wait(t0 + 350 * MS), Some(50 * MS));
    assert_eq!(b.take_due(t0 + 400 * MS), Some((Change::Status, Vec::new(), t0 + 100 * MS)));
    assert_eq!(b.wait(t0 + 400 * MS), None, "reset after firing");
    assert!(b.take_due(t0 + 5000 * MS).is_none());
}

#[test]
fn a_continuous_burst_fires_within_max_wait() {
    let t0 = Instant::now();
    let mut b = Burst::default();
    let mut fired = None;
    for i in 0..30u32 {
        let t = t0 + i * 100 * MS;
        if let Some(f) = b.take_due(t) {
            fired = Some((t, f));
            break;
        }
        b.add(Change::Status, Vec::new(), t);
    }
    let (at, _) = fired.expect("fired during the burst");
    assert!(at <= t0 + MAX_WAIT, "fired after {:?}", at - t0);
}

#[test]
fn bursts_merge_and_ignore_nothing() {
    let t0 = Instant::now();
    let mut b = Burst::default();
    b.add(Change::Status, Vec::new(), t0);
    b.add(Change::Full, Vec::new(), t0 + 10 * MS);
    assert_eq!(b.take_due(t0 + 400 * MS).map(|(c, _, _)| c), Some(Change::Full));

    b.add(Change::None, Vec::new(), t0);
    assert_eq!(b.wait(t0), None, "None doesn't start a burst");
    b.add(Change::Status, Vec::new(), t0);
    b.add(Change::None, Vec::new(), t0 + 200 * MS);
    assert_eq!(b.take_due(t0 + 300 * MS), Some((Change::Status, Vec::new(), t0)), "None doesn't extend it");
}

#[test]
fn linked_worktree_folder_modifies_are_quiet() {
    use notify::event::{CreateKind, DataChange, MetadataKind, RemoveKind, RenameMode};
    let wd = glowfly_core::paths::temp_dir().join("glowfly-watch-kinds-missing");
    let gd = wd.join(".git");
    let paths = WatchPaths::new(&wd, &gd);
    let mut ignores = Ignores::open(&wd);
    let mut c = |kind, rel: &str| classify(&Event::new(kind).add_path(gd.join(rel)), &paths, &mut ignores).0;
    // What Windows reports for `.git/worktrees/wt` on every index write in that worktree.
    assert_eq!(c(EventKind::Modify(ModifyKind::Any), "worktrees/wt"), Change::None);
    assert_eq!(c(EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)), "worktrees/wt"), Change::None);
    assert_eq!(c(EventKind::Modify(ModifyKind::Data(DataChange::Any)), "worktrees/wt/index"), Change::None);
    // A worktree added, removed or renamed, and the files that describe it.
    assert_eq!(c(EventKind::Create(CreateKind::Folder), "worktrees/wt"), Change::Full);
    assert_eq!(c(EventKind::Remove(RemoveKind::Any), "worktrees/wt"), Change::Full);
    assert_eq!(c(EventKind::Create(CreateKind::Any), "worktrees"), Change::Full);
    assert_eq!(c(EventKind::Modify(ModifyKind::Name(RenameMode::Any)), "worktrees/wt"), Change::Full);
    assert_eq!(c(EventKind::Any, "worktrees/wt"), Change::Full);
    assert_eq!(c(EventKind::Modify(ModifyKind::Any), "worktrees/wt/locked"), Change::Full);
    assert_eq!(c(EventKind::Modify(ModifyKind::Any), "index"), Change::Index);
}

fn owned(p: &[&str]) -> Vec<String> {
    p.iter().map(|s| s.to_string()).collect()
}

#[test]
fn bursts_collect_paths_until_something_else_changes() {
    let t0 = Instant::now();
    let mut b = Burst::default();
    b.add(Change::Paths, owned(&["b.txt", "a.txt"]), t0);
    b.add(Change::Paths, owned(&["a.txt", "src/c.rs"]), t0 + 10 * MS);
    assert_eq!(b.take_due(t0 + 400 * MS), Some((Change::Paths, owned(&["a.txt", "b.txt", "src/c.rs"]), t0 + 10 * MS)), "sorted, once each");

    for other in [Change::Status, Change::Full] {
        b.add(Change::Paths, owned(&["a.txt"]), t0);
        b.add(other, Vec::new(), t0);
        b.add(Change::Paths, owned(&["b.txt"]), t0);
        assert_eq!(b.take_due(t0 + 400 * MS), Some((other, Vec::new(), t0)), "{other:?} drops the paths");
    }

    let many: Vec<String> = (0..=MAX_WATCH_PATHS).map(|i| format!("f{i}.txt")).collect();
    b.add(Change::Paths, many[..MAX_WATCH_PATHS].to_vec(), t0);
    b.add(Change::Paths, many[..1].to_vec(), t0);
    assert_eq!(b.paths.len(), MAX_WATCH_PATHS, "a repeat doesn't count");
    b.add(Change::Paths, many[MAX_WATCH_PATHS..].to_vec(), t0);
    assert_eq!(b.take_due(t0 + 400 * MS), Some((Change::Status, Vec::new(), t0)), "past the cap: the whole status");
}

#[test]
fn an_index_change_keeps_the_burst_s_paths() {
    let t0 = Instant::now();
    let mut b = Burst::default();
    b.add(Change::Paths, owned(&["a.txt"]), t0);
    b.add(Change::Index, Vec::new(), t0);
    b.add(Change::Paths, owned(&["b.txt"]), t0);
    assert_eq!(b.take_due(t0 + 400 * MS), Some((Change::Index, owned(&["a.txt", "b.txt"]), t0)), "Paths + Index keeps them");

    b.add(Change::Index, owned(&["a.txt"]), t0);
    b.add(Change::Status, Vec::new(), t0);
    assert_eq!(b.take_due(t0 + 400 * MS), Some((Change::Status, Vec::new(), t0)), "Index + Status drops them");

    let many: Vec<String> = (0..=MAX_WATCH_PATHS).map(|i| format!("f{i}.txt")).collect();
    b.add(Change::Index, Vec::new(), t0);
    b.add(Change::Paths, many, t0);
    assert_eq!(b.take_due(t0 + 400 * MS), Some((Change::Status, Vec::new(), t0)), "past the cap: the whole status");
}

#[test]
fn events_name_their_work_tree_paths() {
    use notify::event::{CreateKind, ModifyKind, RenameMode};
    let wd = glowfly_core::paths::temp_dir().join("glowfly-watch-names-missing");
    let paths = WatchPaths::new(&wd, &wd.join(".git"));
    let mut ignores = Ignores::open(&wd);
    let rename =
        Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both))).add_path(wd.join("a.txt")).add_path(wd.join("dir").join("b.txt"));
    assert_eq!(classify(&rename, &paths, &mut ignores), (Change::Paths, owned(&["a.txt", "dir/b.txt"])));
    let with_index = Event::new(EventKind::Create(CreateKind::Any)).add_path(wd.join("a.txt")).add_path(wd.join(".git").join("index"));
    assert_eq!(classify(&with_index, &paths, &mut ignores), (Change::Index, owned(&["a.txt"])), "the index keeps the named paths");
    let ignore = Event::new(EventKind::Create(CreateKind::Any)).add_path(wd.join(".gitignore"));
    assert_eq!(classify(&ignore, &paths, &mut ignores), (Change::Status, Vec::new()));
    let rescan = Event::new(EventKind::Any).add_path(wd.join("a.txt")).set_flag(notify::event::Flag::Rescan);
    assert_eq!(classify(&rescan, &paths, &mut ignores), (Change::Full, Vec::new()));
}

#[derive(Clone)]
pub(super) struct NoWake;
impl Waker for NoWake {
    fn wake(&self) {}
}

fn git(dir: &Path, a: &[&str]) {
    let (ok, out) = run(dir, &args(a));
    assert!(ok, "git {a:?} failed: {out}");
}

/// A report the watcher sent.
#[derive(Debug, PartialEq)]
enum Report {
    /// `RepoChanged`, as (`full`, `paths`).
    Changed(bool, Option<Vec<String>>),
    /// `IndexChanged`, with its paths.
    Index(Vec<String>),
}

/// The next report for `workdir` within `timeout`.
fn next_any(rx: &Receiver<Msg>, workdir: &Path, timeout: Duration) -> Option<Report> {
    match rx.recv_timeout(timeout).ok()? {
        Msg::Scoped { workdir: w, msg } if w == workdir => match *msg {
            Msg::Watch(WatchMsg::RepoChanged { full, paths, .. }) => Some(Report::Changed(full, paths)),
            Msg::Watch(WatchMsg::IndexChanged { paths, .. }) => Some(Report::Index(paths)),
            _ => panic!("unexpected scoped message"),
        },
        Msg::Watch(WatchMsg::WatchFailed { error, .. }) => panic!("watcher failed: {error}"),
        _ => panic!("unexpected message"),
    }
}

/// The next report for `workdir` within `timeout`, as (`full`, `paths`); an index change reads
/// as `(false, None)`.
pub(super) fn next_report(rx: &Receiver<Msg>, workdir: &Path, timeout: Duration) -> Option<(bool, Option<Vec<String>>)> {
    next_any(rx, workdir, timeout).map(|r| match r {
        Report::Changed(full, paths) => (full, paths),
        Report::Index(_) => (false, None),
    })
}

/// The next report for `workdir` within `timeout`, as `full`.
pub(super) fn next_change(rx: &Receiver<Msg>, workdir: &Path, timeout: Duration) -> Option<bool> {
    next_report(rx, workdir, timeout).map(|(full, _)| full)
}

/// A fresh repository at `dir` with `a.txt` and a `.gitignore` for `target/` committed.
pub(super) fn init_repo(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@example.com"]);
    git(dir, &["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    std::fs::write(dir.join(".gitignore"), "target/\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "first"]);
}

#[test]
fn watcher_reports_an_outside_edit() {
    let dir: PathBuf = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-watch-{}", std::process::id()));
    init_repo(&dir);

    let (tx, rx) = std::sync::mpsc::channel();
    let target = WatchTarget { workdir: dir.clone(), git_dir: dir.join(".git") };
    let watcher = start(&target, tx, NoWake).expect("watcher starts");
    std::thread::sleep(Duration::from_millis(300));

    std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
    assert_eq!(next_report(&rx, &dir, Duration::from_secs(10)), Some((false, Some(owned(&["a.txt"])))), "an edit re-reads its path");
    std::fs::write(dir.join(".gitignore"), "target/\n*.log\n").unwrap();
    assert_eq!(next_report(&rx, &dir, Duration::from_secs(10)), Some((false, None)), "an ignore file re-reads everything");

    git(&dir, &["checkout", "-q", "-b", "x"]);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !next_change(&rx, &dir, deadline.saturating_duration_since(Instant::now())).expect("a full refresh") {}
    while next_change(&rx, &dir, Duration::from_secs(1)).is_some() {}

    // A remote added outside Glowfly: the config reloads everything.
    git(&dir, &["remote", "add", "origin", "https://example.com/r.git"]);
    assert_eq!(next_change(&rx, &dir, Duration::from_secs(10)), Some(true), "the config reloads in full");
    // Let any trailing burst finish before checking that ignored files stay quiet.
    while next_change(&rx, &dir, Duration::from_secs(1)).is_some() {}

    std::fs::create_dir_all(dir.join("target/debug")).unwrap();
    std::fs::write(dir.join("target/debug/x.o"), "obj").unwrap();
    assert_eq!(next_change(&rx, &dir, Duration::from_secs(1)), None, "ignored files don't refresh");

    // Dropping the watcher ends its thread, which drops the message sender.
    drop(watcher);
    assert!(matches!(rx.recv_timeout(Duration::from_secs(10)), Err(RecvTimeoutError::Disconnected)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edits_to_a_force_added_ignored_file_are_reported() {
    let dir: PathBuf = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-watch-forced-{}", std::process::id()));
    init_repo(&dir);
    std::fs::create_dir_all(dir.join("target")).unwrap();
    std::fs::write(
        dir.join("target/keep.txt"),
        "keep
",
    )
    .unwrap();
    git(&dir, &["add", "-f", "target/keep.txt"]);
    git(&dir, &["commit", "-q", "-m", "forced"]);

    let (tx, rx) = std::sync::mpsc::channel();
    let target = WatchTarget { workdir: dir.clone(), git_dir: dir.join(".git") };
    let watcher = start(&target, tx, NoWake).expect("watcher starts");
    std::thread::sleep(Duration::from_millis(300));
    while next_change(&rx, &dir, Duration::from_millis(500)).is_some() {}

    std::fs::write(dir.join("target/out.o"), "obj").unwrap();
    assert_eq!(next_change(&rx, &dir, Duration::from_secs(1)), None, "untracked ignored files stay quiet");
    std::fs::write(
        dir.join("target/keep.txt"),
        "keep
more
",
    )
    .unwrap();
    assert_eq!(next_report(&rx, &dir, Duration::from_secs(10)), Some((false, Some(owned(&["target/keep.txt"])))), "the tracked file's row");

    drop(watcher);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_terminal_add_reports_an_index_change() {
    let dir: PathBuf = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-watch-add-{}", std::process::id()));
    init_repo(&dir);
    std::fs::write(
        dir.join("a.txt"),
        "edited
",
    )
    .unwrap();

    let (tx, rx) = std::sync::mpsc::channel();
    let target = WatchTarget { workdir: dir.clone(), git_dir: dir.join(".git") };
    let watcher = start(&target, tx, NoWake).expect("watcher starts");
    std::thread::sleep(Duration::from_millis(300));
    while next_change(&rx, &dir, Duration::from_millis(500)).is_some() {}

    git(&dir, &["add", "a.txt"]);
    let mut seen = Vec::new();
    while let Some(r) = next_any(&rx, &dir, Duration::from_secs(if seen.is_empty() { 10 } else { 1 })) {
        seen.push(r);
    }
    assert!(matches!(seen.first(), Some(Report::Index(_))), "{seen:?}");
    assert!(seen.iter().all(|r| matches!(r, Report::Index(_))), "no whole read: {seen:?}");

    drop(watcher);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn index_writes_in_a_linked_worktree_stay_quiet() {
    let dir: PathBuf = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-watch-wt-{}", std::process::id()));
    let linked = dir.with_file_name(format!("glowfly-app-test-watch-wt-{}-linked", std::process::id()));
    let _ = std::fs::remove_dir_all(&linked);
    init_repo(&dir);
    git(&dir, &["worktree", "add", "-q", "-b", "linked", &linked.display().to_string()]);

    let (tx, rx) = std::sync::mpsc::channel();
    let target = WatchTarget { workdir: dir.clone(), git_dir: dir.join(".git") };
    let watcher = start(&target, tx, NoWake).expect("watcher starts");
    std::thread::sleep(Duration::from_millis(300));
    while next_change(&rx, &dir, Duration::from_millis(500)).is_some() {}

    // Staging and status in the linked worktree rewrite `.git/worktrees/<name>/index`.
    for i in 0..3 {
        std::fs::write(linked.join("a.txt"), format!("edit {i}\n")).unwrap();
        git(&linked, &["add", "a.txt"]);
        git(&linked, &["status", "--porcelain"]);
    }
    assert_eq!(next_change(&rx, &dir, Duration::from_secs(1)), None, "the main repository's tab doesn't reload");

    git(&dir, &["worktree", "lock", &linked.display().to_string()]);
    assert_eq!(next_change(&rx, &dir, Duration::from_secs(10)), Some(true), "locking it reloads in full");

    drop(watcher);
    let _ = std::fs::remove_dir_all(&linked);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_worktree_sees_branches_change_in_the_common_git_dir() {
    let dir: PathBuf = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-watch-common-{}", std::process::id()));
    let linked = dir.with_file_name(format!("glowfly-app-test-watch-common-{}-linked", std::process::id()));
    let _ = std::fs::remove_dir_all(&linked);
    init_repo(&dir);
    git(&dir, &["worktree", "add", "-q", "-b", "linked", &linked.display().to_string()]);
    let git_dir = git2::Repository::open(&linked).unwrap().path().to_path_buf();

    let (tx, rx) = std::sync::mpsc::channel();
    let target = WatchTarget { workdir: linked.clone(), git_dir };
    let watcher = start(&target, tx, NoWake).expect("watcher starts");
    std::thread::sleep(Duration::from_millis(300));
    while next_change(&rx, &linked, Duration::from_millis(500)).is_some() {}

    // A branch made in the main worktree lands in the shared refs.
    git(&dir, &["branch", "from-main"]);
    assert_eq!(next_change(&rx, &linked, Duration::from_secs(10)), Some(true), "the shared refs reload in full");
    while next_change(&rx, &linked, Duration::from_secs(1)).is_some() {}

    git(&dir, &["commit", "--allow-empty", "-q", "-m", "moved"]);
    assert_eq!(next_change(&rx, &linked, Duration::from_secs(10)), Some(true), "a ref moved in the main worktree");
    while next_change(&rx, &linked, Duration::from_secs(1)).is_some() {}

    git(&dir, &["pack-refs", "--all"]);
    assert_eq!(next_change(&rx, &linked, Duration::from_secs(10)), Some(true), "packed refs");
    while next_change(&rx, &linked, Duration::from_secs(1)).is_some() {}

    git(&dir, &["remote", "add", "origin", "https://example.com/r.git"]);
    assert_eq!(next_change(&rx, &linked, Duration::from_secs(10)), Some(true), "the shared config");

    drop(watcher);
    let _ = std::fs::remove_dir_all(&linked);
    let _ = std::fs::remove_dir_all(&dir);
}

/// inotify would otherwise add watches for everything a link points to (a link to the home
/// folder can use up `max_user_watches`), and report changes there as the repository's.
#[cfg(target_os = "linux")]
#[test]
fn links_out_of_the_repository_are_not_followed() {
    let dir: PathBuf = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-watch-link-{}", std::process::id()));
    let outside = dir.with_file_name(format!("glowfly-app-test-watch-link-{}-outside", std::process::id()));
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir_all(&outside).unwrap();
    init_repo(&dir);
    std::os::unix::fs::symlink(&outside, dir.join("link")).unwrap();
    std::fs::write(dir.join(".gitignore"), "target/\n").unwrap();

    let (tx, rx) = std::sync::mpsc::channel();
    let target = WatchTarget { workdir: dir.clone(), git_dir: dir.join(".git") };
    let watcher = start(&target, tx, NoWake).expect("watcher starts");
    std::thread::sleep(Duration::from_millis(300));
    while next_change(&rx, &dir, Duration::from_millis(500)).is_some() {}

    std::fs::write(outside.join("x.txt"), "outside").unwrap();
    assert_eq!(next_change(&rx, &dir, Duration::from_secs(1)), None, "the link's target isn't watched");
    std::fs::write(dir.join("a.txt"), "inside\n").unwrap();
    assert_eq!(next_change(&rx, &dir, Duration::from_secs(10)), Some(false), "the repository still is");

    drop(watcher);
    let _ = std::fs::remove_dir_all(&outside);
    let _ = std::fs::remove_dir_all(&dir);
}
