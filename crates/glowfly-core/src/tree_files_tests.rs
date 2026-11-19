//! `.gitattributes` and `.gitignore` in the work tree: a regular file is read as before, a link
//! reads as absent, a large one isn't read, and a link naming `testutil::net_place()` (never
//! touched) holds the reads of the working tree, and keeps libgit2 from a submodule that has
//! one. Link tests skip only on Windows where the account may not make links.

use super::*;
use crate::lfs::root_patterns;
use crate::repo::{
    DiffTarget, LoadOptions, compare_from_workdir, compare_workdir, load, load_diff, status_at, status_of_paths, wip_line_counts,
};
use crate::testutil::{git, link_to, net_place, scratch_repo, with_submodule, write};
use git2::{IndexEntry, IndexTime};

const LFS: &str = "*.psd filter=lfs diff=lfs merge=lfs -text\n";

/// A repository with `a.txt` committed and changed since.
fn repo(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "a"]);
    write(&dir, "a.txt", "changed\n");
    dir
}

/// Adds `path` to the index of `dir` as a link holding `target`, without git or the work tree.
fn track_link(dir: &Path, path: &str, target: &str) {
    let repo = Repository::open(dir).unwrap();
    let mut index = repo.index().unwrap();
    let zero = IndexTime::new(0, 0);
    let entry = IndexEntry {
        ctime: zero,
        mtime: zero,
        dev: 0,
        ino: 0,
        mode: LINK,
        uid: 0,
        gid: 0,
        file_size: 0,
        id: repo.blob(target.as_bytes()).unwrap(),
        flags: 0,
        flags_extended: 0,
        path: path.as_bytes().to_vec(),
    };
    index.add(&entry).unwrap();
    index.write().unwrap();
}

fn held(dir: &Path) -> Option<String> {
    linked_away(&Repository::open(dir).unwrap())
}

/// Whether `r` is the refusal naming `file`.
fn names<T>(r: crate::error::Result<T>, file: &str) -> bool {
    matches!(r, Err(Error::LinkedFile(f)) if f == file)
}

#[test]
fn a_regular_gitattributes_is_read_as_before() {
    let dir = repo("tree-files-regular");
    write(&dir, ".gitattributes", LFS);
    write(&dir, ".gitignore", "*.log\n");
    write(&dir, "x.log", "ignored\n");
    assert_eq!(read(&dir.join(".gitattributes")).as_deref(), Some(LFS.as_bytes()));
    assert_eq!(root_patterns(&dir).len(), 1);
    assert_eq!(held(&dir), None);
    let snap = load(&dir, &LoadOptions::default()).unwrap();
    assert_eq!(snap.lfs_patterns.len(), 1);
    let unstaged: Vec<&str> = snap.unstaged.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(unstaged, [".gitattributes", ".gitignore", "a.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_gitattributes_over_the_limit_is_not_read() {
    let dir = repo("tree-files-large");
    let mut large = LFS.as_bytes().to_vec();
    large.resize(MAX_BYTES as usize, b'\n');
    std::fs::write(dir.join(".gitattributes"), &large).unwrap();
    assert_eq!(root_patterns(&dir).len(), 1, "at the limit");
    large.push(b'\n');
    std::fs::write(dir.join(".gitattributes"), &large).unwrap();
    assert_eq!(read(&dir.join(".gitattributes")), None);
    assert!(root_patterns(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_or_a_missing_file_reads_as_absent() {
    let dir = repo("tree-files-folder");
    assert_eq!(read(&dir.join(".gitattributes")), None);
    std::fs::create_dir(dir.join(".gitattributes")).unwrap();
    assert_eq!(read(&dir.join(".gitattributes")), None);
    assert!(root_patterns(&dir).is_empty());
    assert_eq!(held(&dir), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_gitattributes_reads_as_absent() {
    let dir = repo("tree-files-linked");
    write(&dir, "real", LFS);
    if !link_to(Path::new("real"), &dir.join(".gitattributes"), false) {
        return;
    }
    assert_eq!(read(&dir.join(".gitattributes")), None);
    assert!(root_patterns(&dir).is_empty());
    // A link inside the work tree holds nothing back.
    assert_eq!(held(&dir), None);
    let snap = load(&dir, &LoadOptions::default()).unwrap();
    assert!(snap.lfs_patterns.is_empty());
    assert!(crate::lfs::status(&dir).unwrap().has_gitattributes);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_root_gitattributes_linked_to_a_network_place_holds_the_load() {
    let dir = repo("tree-files-root-away");
    // A staged change, made before the link so git never sees it.
    write(&dir, "b.txt", "b\n");
    git(&dir, &["add", "b.txt"]);
    let staged = DiffTarget::Staged("b.txt".into());
    assert!(load_diff(&dir, &staged).is_ok());
    let head = Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    let unstaged = DiffTarget::Unstaged("a.txt".into());
    assert!(compare_workdir(&dir, head).is_ok() && compare_from_workdir(&dir, head).is_ok() && load_diff(&dir, &unstaged).is_ok());
    // Not tracked: the root's two are looked at as they are on disk.
    if !link_to(&net_place().join("attributes"), &dir.join(".gitattributes"), false) {
        return;
    }
    // The diffs that reach the working tree: from a commit, to a commit, and the unstaged one.
    assert!(names(compare_workdir(&dir, head), ".gitattributes"));
    assert!(names(compare_from_workdir(&dir, head), ".gitattributes"));
    assert!(names(load_diff(&dir, &unstaged), ".gitattributes"));
    assert!(load_diff(&dir, &staged).is_ok(), "the index's diff reads no work tree");
    assert!(root_patterns(&dir).is_empty());
    assert_eq!(held(&dir).as_deref(), Some(".gitattributes"));
    assert!(names(load(&dir, &LoadOptions::default()), ".gitattributes"));
    for git_status in [false, true] {
        assert!(names(status_at(&dir, git_status), ".gitattributes"));
        assert!(names(status_of_paths(&dir, &["a.txt".into()], git_status), ".gitattributes"));
    }
    assert!(names(wip_line_counts(&dir), ".gitattributes"));
    // Once it is gone the repository reads again.
    std::fs::remove_file(dir.join(".gitattributes")).unwrap();
    assert!(load(&dir, &LoadOptions::default()).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_gitignore_in_a_subfolder_holds_the_status() {
    let dir = repo("tree-files-nested-away");
    std::fs::create_dir_all(dir.join("sub/deep")).unwrap();
    let away = net_place().join("ignore");
    track_link(&dir, "sub/deep/.gitignore", &away.to_string_lossy());
    // Tracked as a link, checked out as a plain file (`core.symlinks` off): nothing is held.
    write(&dir, "sub/deep/.gitignore", &away.to_string_lossy());
    assert_eq!(held(&dir), None);
    assert!(status_at(&dir, false).is_ok());
    std::fs::remove_file(dir.join("sub/deep/.gitignore")).unwrap();
    if !link_to(&away, &dir.join("sub/deep/.gitignore"), false) {
        return;
    }
    assert_eq!(held(&dir).as_deref(), Some("sub/deep/.gitignore"));
    assert!(names(status_at(&dir, false), "sub/deep/.gitignore"));
    assert!(names(load(&dir, &LoadOptions::default()), "sub/deep/.gitignore"));
    let mut ignores = crate::watch::Ignores::open(&dir);
    assert!(!ignores.is_ignored("sub/deep/x.log", crate::watch::Touch::Other));
    // The same link to a file inside the work tree is libgit2's to read.
    std::fs::remove_file(dir.join("sub/deep/.gitignore")).unwrap();
    write(&dir, "rules", "*.log\n");
    assert!(link_to(Path::new("../../rules"), &dir.join("sub/deep/.gitignore"), false));
    assert_eq!(held(&dir), None);
    assert!(status_at(&dir, false).is_ok());
    // A plain file again: the watcher asks once told the file changed.
    std::fs::remove_file(dir.join("sub/deep/.gitignore")).unwrap();
    write(&dir, "sub/deep/.gitignore", "*.log\n");
    assert!(!ignores.is_ignored("sub/deep/x.log", crate::watch::Touch::Other), "still as it was asked");
    ignores.clear();
    assert!(ignores.is_ignored("sub/deep/x.log", crate::watch::Touch::Other));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tracked_link_under_another_spelling_has_its_folder_looked_at() {
    for name in
        [".gitignore", ".GITIGNORE", ".gitattributes", ".GitAttributes", ".gitignore.", ".gitignore ", "GITIGN~1", ".git\u{200c}ignore"]
    {
        assert!(may_name(name.as_bytes()), "{name:?}");
    }
    assert!(may_name(b".gitignore::$DATA") && may_name(&[b'.', b'g', 0xFF]));
    for name in ["link", "gitignore", ".gitmodules", ".git", "ignore.txt"] {
        assert!(!may_name(name.as_bytes()), "{name:?}");
    }
    assert_eq!((named(b".GitAttributes"), named(b".gitignore."), named(b"GITIGN~1")), (Some(0), Some(1), None));
    let root = crate::paths::temp_dir();
    assert_eq!(folder_of(&root, b""), Some(root.clone()));
    assert_eq!(folder_of(&root, b"a/b"), Some(root.join("a").join("b")));
    assert_eq!(folder_of(&root, b"../a"), None);
}

#[test]
fn a_tracked_gitignore_in_a_linked_folder_holds_without_looking_inside() {
    let dir = repo("tree-files-folder-away");
    track_link(&dir, "A/.gitignore", "rules");
    // No folder there: nothing to read.
    assert_eq!(held(&dir), None);
    assert!(status_at(&dir, false).is_ok());
    // The folder itself leads away: the file inside is never asked for.
    if !link_to(&net_place().join("A"), &dir.join("A"), true) {
        return;
    }
    assert_eq!(held(&dir).as_deref(), Some("A/.gitignore"));
    assert!(names(status_at(&dir, false), "A/.gitignore"));
    assert!(names(load(&dir, &LoadOptions::default()), "A/.gitignore"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The paths of `dir`'s unstaged rows, read by libgit2 alone.
fn unstaged(dir: &Path) -> Vec<String> {
    status_at(dir, false).unwrap().0.into_iter().map(|f| f.path).collect()
}

#[test]
fn a_submodule_whose_gitignore_is_linked_away_is_left_out_of_the_status() {
    let (parent, sub) = with_submodule("tree-files-sub");
    write(&sub, "lib.txt", "changed\n");
    let unread = || crate::git_file::submodules_unread(&Repository::open(&parent).unwrap());
    // Tracked as a link there, checked out as a plain file: the submodule is read.
    std::fs::create_dir_all(sub.join("d")).unwrap();
    track_link(&sub, "d/.gitignore", "rules");
    write(&sub, "d/.gitignore", "rules");
    assert!(!unread());
    assert_eq!(unstaged(&parent), ["libs/lib"]);
    // Its root file, as it is on disk.
    if !link_to(&net_place().join("ignore"), &sub.join(".gitignore"), false) {
        return;
    }
    assert!(unread());
    assert!(unstaged(&parent).is_empty(), "the submodule's work tree isn't compared");
    assert_eq!(held(&parent), None, "the opened repository's own files are plain");
    std::fs::remove_file(sub.join(".gitignore")).unwrap();
    assert!(!unread());
    // One its index tracks as a link.
    std::fs::remove_file(sub.join("d/.gitignore")).unwrap();
    assert!(link_to(&net_place().join("ignore"), &sub.join("d/.gitignore"), false));
    assert!(unread());
    assert!(unstaged(&parent).is_empty());
    for dir in [&parent, &parent.with_file_name(format!("{}-lib", parent.file_name().unwrap().to_string_lossy()))] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn an_index_is_read_again_only_when_its_size_or_time_changed() {
    use std::time::Duration;
    let dir = repo("tree-files-memo");
    track_link(&dir, "sub/.gitignore", "rules");
    track_link(&dir, "sub/.GitAttributes", "rules");
    track_link(&dir, "other/link", "rules");
    let index = dir.join(".git").join("index");
    let dated = |secs: u64| {
        let at = SystemTime::now() - Duration::from_secs(secs);
        std::fs::File::options().write(true).open(&index).unwrap().set_modified(at).unwrap();
    };
    let want = Some(vec![(b"sub".to_vec(), Some(0)), (b"sub".to_vec(), Some(1))]);
    let real = std::fs::read(&index).unwrap();
    let other = vec![0u8; real.len()];

    // Just written: read every time, so bytes that are no index show at once.
    assert_eq!(link_folders(&index), want);
    std::fs::write(&index, &other).unwrap();
    assert_eq!(link_folders(&index), None);

    // Old enough to tell by: the same size and time are not read again.
    std::fs::write(&index, &real).unwrap();
    dated(60);
    let time = std::fs::metadata(&index).unwrap().modified().unwrap();
    assert_eq!(link_folders(&index), want);
    std::fs::write(&index, &other).unwrap();
    std::fs::File::options().write(true).open(&index).unwrap().set_modified(time).unwrap();
    assert_eq!(link_folders(&index), want, "remembered");

    // Another time, or another size, is read again.
    dated(30);
    assert_eq!(link_folders(&index), None, "another time");
    std::fs::write(&index, &real).unwrap();
    dated(30);
    assert_eq!(link_folders(&index), want);
    std::fs::write(&index, [&other[..], &[0u8][..]].concat()).unwrap();
    dated(30);
    assert_eq!(link_folders(&index), None, "another size");
    assert_eq!(link_folders(&dir.join("no-index")), Some(Vec::new()));
    let _ = std::fs::remove_dir_all(&dir);
}
