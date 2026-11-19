//! `.gitattributes` and `.gitignore` in the work tree: a regular file is read as before, a link
//! reads as absent, a large one isn't read, and a link naming `testutil::net_place()` (never
//! touched) holds the reads of the working tree. Link tests skip where links can't be made.

use super::*;
use crate::lfs::root_patterns;
use crate::repo::{LoadOptions, load, status_at, status_of_paths, wip_line_counts};
use crate::testutil::{git, link_to, net_place, scratch_repo, write};
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
    // Not tracked: the root's two are looked at as they are on disk.
    if !link_to(&net_place().join("attributes"), &dir.join(".gitattributes"), false) {
        return;
    }
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
    let root = crate::paths::temp_dir();
    assert_eq!(folder_of(&root, b""), Some(root.clone()));
    assert_eq!(folder_of(&root, b"a/b"), Some(root.join("a").join("b")));
    assert_eq!(folder_of(&root, b"../a"), None);
}
