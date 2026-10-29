//! Sweeps of fixture repositories: the baseline, folders and files coming and going, edits found
//! by rotation or a racy listing, what is never walked, the git dir's files and the size limit.
//! Each change a test expects a sweep to find through a stamp moves that stamp itself (`moved`).

use super::*;
use crate::testutil::{git, scratch_repo, write};
use std::time::Duration;

/// Well after every file the test wrote: no listing is racy.
fn later() -> SystemTime {
    SystemTime::now() + Duration::from_secs(60)
}

/// Gives `p` a time it has never had. A create or remove inside a folder changes the folder's
/// time, but Linux takes it from a coarse clock tick, so changes within one tick can leave it
/// equal. The app lists again any folder whose time was younger than `RACY_WINDOW`, so it still
/// sees them, but these tests' `later()` makes every time old: a change they expect a sweep to
/// find through a stamp moves that stamp itself, the same on every OS and file system. The
/// times are years in the past (never racy, never an OS-given time) and each call's is new.
fn moved(p: &Path) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1_000_000_000);
    let t = SystemTime::UNIX_EPOCH + Duration::from_secs(NEXT.fetch_add(1, Ordering::Relaxed));
    filetime::set_file_mtime(p, filetime::FileTime::from_system_time(t)).unwrap();
}

fn start(dir: &Path) -> (Sweep, Ignores) {
    let mut s = Sweep::new(dir, &dir.join(".git"), None);
    let mut ig = Ignores::open(dir);
    assert_eq!(s.sweep(&mut ig, later()), None, "the first sweep records and reads everything");
    (s, ig)
}

/// A repository with a.txt and `n` folders d00… each holding x.txt, all committed.
fn repo(name: &str, n: usize) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "a\n");
    for i in 0..n {
        std::fs::create_dir_all(dir.join(format!("d{i:02}"))).unwrap();
        write(&dir, &format!("d{i:02}/x.txt"), "x\n");
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "files"]);
    dir
}

#[test]
fn the_baseline_then_nothing() {
    let dir = repo("sweep-baseline", 2);
    let (mut s, mut ig) = start(&dir);
    assert_eq!(s.sweep(&mut ig, later()), Some(Vec::new()));
    assert_eq!(s.sweep(&mut ig, later()), Some(Vec::new()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn files_and_folders_coming_and_going_are_reported() {
    let dir = repo("sweep-come-go", 1);
    let (mut s, mut ig) = start(&dir);
    write(&dir, "new.txt", "new\n");
    moved(&dir);
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![dir.join("new.txt")]));

    std::fs::create_dir_all(dir.join("n/deep")).unwrap();
    write(&dir, "n/deep/x.txt", "x\n");
    moved(&dir);
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![dir.join("n")]), "only the new folder: its path covers the rest");
    assert_eq!(s.sweep(&mut ig, later()), Some(Vec::new()), "walked into, not reported again");
    write(&dir, "n/deep/y.txt", "y\n");
    moved(&dir.join("n/deep"));
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![dir.join("n/deep/y.txt")]), "the new folder is watched too");

    std::fs::remove_file(dir.join("a.txt")).unwrap();
    moved(&dir);
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![dir.join("a.txt")]));
    std::fs::remove_dir_all(dir.join("n")).unwrap();
    moved(&dir);
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![dir.join("n")]));
    for _ in 0..SWEEP_ROUNDS {
        assert_eq!(s.sweep(&mut ig, later()), Some(Vec::new()), "its folders are forgotten");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Makes the sweep's record of the root match its stamp now, as when the removal lands in the
/// same clock tick as the root's last change (Linux stamps folders from a coarse tick), and
/// keeps the root (the first folder) out of the next sweep's rotating share.
fn root_time_unchanged(s: &mut Sweep, dir: &Path) {
    let seen = s.seen.as_mut().unwrap();
    seen.folders.get_mut(Path::new("")).unwrap().stamp = Stamp::of(dir);
    seen.cursor = 1;
}

#[test]
fn a_folder_gone_while_its_parent_s_time_stays_is_reported_once() {
    let dir = repo("sweep-gone-once", 3);
    let (mut s, mut ig) = start(&dir);
    std::fs::remove_dir_all(dir.join("d00")).unwrap();
    root_time_unchanged(&mut s, &dir);
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![dir.join("d00")]), "found by its own stamp");
    for _ in 0..SWEEP_ROUNDS {
        assert_eq!(s.sweep(&mut ig, later()), Some(Vec::new()), "the root's next listing knows it is gone");
    }
    std::fs::remove_dir_all(dir.join("d01")).unwrap();
    write(&dir, "d01", "now a file\n");
    root_time_unchanged(&mut s, &dir);
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![dir.join("d01")]), "a folder turned into a file");
    for _ in 0..SWEEP_ROUNDS {
        assert_eq!(s.sweep(&mut ig, later()), Some(Vec::new()), "the root's next listing knows it is a file");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_content_edit_is_found_by_rotation() {
    let dir = repo("sweep-rotation", 20);
    let (mut s, mut ig) = start(&dir);
    write(&dir, "d15/x.txt", "longer\n");
    let mut found = Vec::new();
    for _ in 0..SWEEP_ROUNDS {
        found.extend(s.sweep(&mut ig, later()).expect("a sweep"));
    }
    assert_eq!(found, [dir.join("d15/x.txt")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_racy_folder_is_listed_again_next_time() {
    let dir = repo("sweep-racy", 20);
    let file = dir.join("d19/x.txt");
    let stamp = SystemTime::now() + Duration::from_secs(60);
    let set = |t| std::fs::File::options().write(true).open(&file).unwrap().set_modified(t).unwrap();
    set(stamp);
    let mut s = Sweep::new(&dir, &dir.join(".git"), None);
    let mut ig = Ignores::open(&dir);
    assert_eq!(s.sweep(&mut ig, stamp), None, "taken at the file's own time");
    // Rewritten with another length but the same time; the folder's own stamp doesn't change.
    write(&dir, "d19/x.txt", "longer\n");
    set(stamp);
    assert_eq!(s.sweep(&mut ig, stamp + Duration::from_secs(60)), Some(vec![file.clone()]), "d19 isn't in the first share");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ignored_and_nested_git_folders_are_not_walked() {
    let dir = scratch_repo("sweep-ignored");
    write(&dir, ".gitignore", "target/\nbuild/\n");
    for d in ["target", "build", "nested/.git"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    write(&dir, "target/x.o", "obj");
    write(&dir, "build/keep.txt", "keep\n");
    write(&dir, "nested/.git/HEAD", "ref: refs/heads/main\n");
    git(&dir, &["add", ".gitignore"]);
    git(&dir, &["add", "-f", "build/keep.txt"]);
    git(&dir, &["commit", "-q", "-m", "files"]);
    let (mut s, mut ig) = start(&dir);

    write(&dir, "target/y.o", "obj");
    write(&dir, "nested/.git/HEAD", "ref: refs/heads/other\n");
    write(&dir, "nested/.git/ORIG_HEAD", "x\n");
    write(&dir, ".git/description", "changed\n");
    for d in ["target", "nested/.git", ".git"] {
        moved(&dir.join(d)); // a stamp that changed doesn't make them walked
    }
    let mut found = Vec::new();
    for _ in 0..SWEEP_ROUNDS {
        found.extend(s.sweep(&mut ig, later()).expect("a sweep"));
    }
    assert!(found.is_empty(), "{found:?}");

    write(&dir, "build/keep.txt", "keep\nmore\n");
    for _ in 0..SWEEP_ROUNDS {
        found.extend(s.sweep(&mut ig, later()).expect("a sweep"));
    }
    assert_eq!(found, [dir.join("build/keep.txt")], "a tracked file under an ignored folder");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_git_dir_s_files_are_compared() {
    let dir = repo("sweep-git", 1);
    let gd = dir.join(".git");
    let (mut s, mut ig) = start(&dir);
    write(&dir, "a.txt", "edited\n");
    git(&dir, &["add", "a.txt"]);
    moved(&gd.join("index"));
    let found = s.sweep(&mut ig, later()).unwrap();
    assert!(found.contains(&gd.join("index")), "{found:?}");
    git(&dir, &["branch", "x"]);
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![gd.join("HEAD")]), "a ref moved");
    std::fs::create_dir_all(gd.join("info")).unwrap();
    write(&gd, "info/exclude", "*.tmp\n");
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![gd.join("info/exclude")]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tree_past_the_limit_reads_everything_every_time() {
    let dir = repo("sweep-limit", 3);
    let mut s = Sweep::with_limit(&dir, &dir.join(".git"), None, 3);
    let mut ig = Ignores::open(&dir);
    for _ in 0..3 {
        assert_eq!(s.sweep(&mut ig, later()), None);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_folder_that_crosses_the_limit_reads_everything_from_then_on() {
    // The root (a.txt, d00) and d00 (x.txt) hold 3 entries, under the limit of 5.
    let dir = repo("sweep-limit-later", 1);
    let mut s = Sweep::with_limit(&dir, &dir.join(".git"), None, 5);
    let mut ig = Ignores::open(&dir);
    assert_eq!(s.sweep(&mut ig, later()), None, "the baseline");
    assert_eq!(s.sweep(&mut ig, later()), Some(Vec::new()), "the baseline fits");
    std::fs::create_dir_all(dir.join("n")).unwrap();
    for f in ["1", "2", "3"] {
        write(&dir, &format!("n/{f}.txt"), "x\n");
    }
    moved(&dir);
    for _ in 0..3 {
        assert_eq!(s.sweep(&mut ig, later()), None, "n's 3 entries take it past the limit");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Sweeps until a round finds nothing new, gathering what each found.
fn rounds(s: &mut Sweep, ig: &mut Ignores) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for _ in 0..SWEEP_ROUNDS {
        found.extend(s.sweep(ig, later()).expect("a sweep"));
    }
    found
}

#[test]
fn a_nested_repository_is_walked_by_its_own_rules_and_its_commits_are_found() {
    let dir = repo("sweep-nested", 1);
    let inner = dir.join("vendor/inner");
    std::fs::create_dir_all(inner.join("build")).unwrap();
    git(&inner, &["init", "-q", "-b", "main"]);
    crate::testutil::set_config(&inner, &[("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false")]);
    write(&inner, ".gitignore", "build/\n");
    write(&inner, "src.txt", "s\n");
    write(&inner, "build/x.o", "obj");
    git(&inner, &["add", "."]);
    git(&inner, &["commit", "-q", "-m", "inner"]);
    let (mut s, mut ig) = start(&dir);

    write(&inner, "build/y.o", "obj");
    moved(&inner.join("build"));
    assert_eq!(rounds(&mut s, &mut ig), Vec::<PathBuf>::new(), "ignored by the nested repository's own rules");

    write(&inner, "src.txt", "edited\n");
    assert_eq!(rounds(&mut s, &mut ig), [inner.join("src.txt")], "its work tree is walked");

    git(&inner, &["commit", "-q", "-am", "edit"]);
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![inner.join(".git/HEAD")]), "a commit inside moves its HEAD");
    assert_eq!(rounds(&mut s, &mut ig), Vec::<PathBuf>::new());

    std::fs::remove_dir_all(inner.join(".git")).unwrap();
    moved(&inner);
    let found = s.sweep(&mut ig, later()).unwrap();
    assert!(found.contains(&inner), "no longer a repository: {found:?}");
    assert!(s.seen.as_ref().unwrap().nested.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_submodule_s_commits_and_edits_are_found() {
    let (parent, sub) = crate::testutil::with_submodule("sweep-submodule");
    let (mut s, mut ig) = start(&parent);
    write(&sub, "lib.txt", "lib\nmore\n");
    assert_eq!(rounds(&mut s, &mut ig), [sub.join("lib.txt")]);
    git(&sub, &["commit", "-q", "-am", "more"]);
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![sub.join(".git/HEAD")]), "its git dir sits in the parent's modules");
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn included_config_files_are_compared() {
    let dir = repo("sweep-include", 1);
    let outside = dir.with_file_name(format!("{}-shared", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir_all(&outside).unwrap();
    write(&outside, "team.cfg", "[core]\n");
    let named = |p: &Path| p.display().to_string().replace('\\', "/");
    crate::testutil::set_config(&dir, &[("include.path", &named(&outside.join("team.cfg")))]);
    let (mut s, mut ig) = start(&dir);
    write(&outside, "team.cfg", "[core]\n\tautocrlf = false\n");
    moved(&outside.join("team.cfg"));
    let found = s.sweep(&mut ig, later()).unwrap();
    assert!(found.contains(&crate::paths::resolve(&outside.join("team.cfg"))), "{found:?}");

    // A new include: the config changes, and from then on the new file is compared too.
    crate::testutil::set_config(&dir, &[("include.path", &named(&outside.join("more.cfg")))]);
    moved(&dir.join(".git/config"));
    let found = s.sweep(&mut ig, later()).unwrap();
    assert!(found.contains(&dir.join(".git/config")), "{found:?}");
    write(&outside, "more.cfg", "[core]\n");
    let found = s.sweep(&mut ig, later()).unwrap();
    assert!(found.contains(&crate::paths::resolve(&outside.join("more.cfg"))), "created: {found:?}");
    let _ = std::fs::remove_dir_all(&outside);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn at_most_max_nested_repositories_are_tracked_and_the_rest_still_walked() {
    let dir = repo("sweep-nested-cap", 0);
    let n = nested::MAX_NESTED + 4;
    for i in 0..n {
        git2::Repository::init(dir.join(format!("n{i:03}"))).unwrap();
    }
    let (mut s, mut ig) = start(&dir);
    let seen = s.seen.as_ref().unwrap();
    assert_eq!(seen.nested.len(), nested::MAX_NESTED);
    assert_eq!((0..n).filter(|i| seen.folders.contains_key(Path::new(&format!("n{i:03}")))).count(), n, "every one walked");
    assert_eq!(s.sweep(&mut ig, later()), Some(Vec::new()));
    let _ = std::fs::remove_dir_all(&dir);
}
