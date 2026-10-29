//! Reading LFS patterns from nested `.gitattributes` files and `info/attributes`.

use super::*;
use crate::lfs::{PatternProblem, TrackTarget, check_target};
use crate::repo::{LoadOptions, load};
use crate::testutil::{git, scratch_repo, write};
use git2::{IndexEntry, IndexTime, Oid};

const LFS: &str = "filter=lfs diff=lfs merge=lfs -text";

fn shown(p: &[LfsPattern]) -> Vec<(String, String)> {
    p.iter().map(|p| (p.display.clone(), p.source.file())).collect()
}

fn commit_all(dir: &Path) {
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "files"]);
}

#[test]
fn nested_files_only() {
    let dir = scratch_repo("lfs-nested");
    std::fs::create_dir_all(dir.join("art/raw")).unwrap();
    std::fs::create_dir_all(dir.join("loose")).unwrap();
    // CRLF lines, as a Windows editor writes them.
    write(&dir, "art/.gitattributes", &format!("*.psd {LFS}\r\n*.txt text\r\n"));
    write(&dir, "art/raw/.gitattributes", &format!("my[[:space:]]take.wav {LFS} lockable\r\n"));
    write(&dir, "a.txt", "one\n");
    commit_all(&dir);
    // Not in the index: git would honour it, but only tracked files are read.
    write(&dir, "loose/.gitattributes", &format!("*.zip {LFS}\n"));

    let patterns = repo_patterns(&dir);
    assert_eq!(shown(&patterns), [("*.psd".into(), "art/.gitattributes".into()), ("my take.wav".into(), "art/raw/.gitattributes".into())]);
    assert_eq!(patterns[0].source, PatternSource::Nested("art".into()));
    assert!(patterns[1].lockable);
    assert_eq!(shown(&load(&dir, &LoadOptions::all(50)).unwrap().lfs_patterns), shown(&patterns), "a full load reads them too");
    assert!(root_patterns(&dir).is_empty());

    // Tracking writes to the root file, which a nested pattern doesn't cover.
    assert_eq!(check_target(&TrackTarget::Extension("*.psd".into()), &patterns), Ok(()));
    assert_eq!(crate::lfs::check_pattern("*.psd", &patterns), Ok("*.psd".into()));
    let mut root = patterns.clone();
    root.push(LfsPattern::root("*.psd", false));
    assert_eq!(check_target(&TrackTarget::Extension("*.psd".into()), &root), Err(PatternProblem::AlreadyTracked("*.psd".into())));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn info_attributes_only() {
    let dir = scratch_repo("lfs-info");
    write(&dir, "a.txt", "one\n");
    commit_all(&dir);
    std::fs::create_dir_all(dir.join(".git/info")).unwrap();
    write(&dir, ".git/info/attributes", &format!("# local only\r\n*.bin {LFS}\r\n"));
    assert_eq!(shown(&repo_patterns(&dir)), [("*.bin".to_owned(), ".git/info/attributes".to_owned())]);

    // A linked worktree reads the common git dir's file.
    let linked = dir.with_file_name(format!("{}-linked", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    let patterns = repo_patterns(&linked);
    assert_eq!(patterns.iter().map(|p| (&*p.raw, &p.source)).collect::<Vec<_>>(), [("*.bin", &PatternSource::Info)]);
    git(&dir, &["worktree", "remove", "--force", &linked.display().to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn root_nested_and_info_in_order() {
    let dir = scratch_repo("lfs-sources-order");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    write(&dir, ".gitattributes", &format!("*.psd {LFS}\n"));
    write(&dir, "sub/.gitattributes", &format!("*.psd {LFS}\n"));
    commit_all(&dir);
    write(&dir, ".git/info/attributes", &format!("*.iso {LFS}\n"));
    let files: Vec<String> = repo_patterns(&dir).iter().map(|p| p.source.file()).collect();
    assert_eq!(files, [".gitattributes", "sub/.gitattributes", ".git/info/attributes"]);
    let _ = std::fs::remove_dir_all(&dir);
}

fn entry(path: &str, mode: u32, stage: u16, id: Oid) -> IndexEntry {
    let zero = IndexTime::new(0, 0);
    IndexEntry {
        ctime: zero,
        mtime: zero,
        dev: 0,
        ino: 0,
        mode,
        uid: 0,
        gid: 0,
        file_size: 0,
        id,
        flags: stage << 12,
        flags_extended: 0,
        path: path.as_bytes().to_vec(),
    }
}

#[test]
fn skips_links_conflicts_and_stops_at_the_file_cap() {
    let dir = scratch_repo("lfs-sources-cap");
    let repo = Repository::open(&dir).unwrap();
    let blob = repo.blob(format!("*.psd {LFS}\n").as_bytes()).unwrap();
    let mut index = repo.index().unwrap();
    index.add(&entry("a-link/.gitattributes", 0o120000, 0, blob)).unwrap();
    index.add(&entry("conflict/.gitattributes", 0o100644, 2, blob)).unwrap();
    index.add(&entry("Upper/.GITATTRIBUTES", 0o100644, 0, blob)).unwrap();
    for i in 0..MAX_NESTED_FILES + 10 {
        index.add(&entry(&format!("d{i:04}/.gitattributes"), 0o100644, 0, blob)).unwrap();
    }
    index.write().unwrap();
    let patterns = patterns_in(&repo, &dir);
    assert_eq!(patterns.len(), MAX_NESTED_FILES);
    assert_eq!(patterns[0].source, PatternSource::Nested("d0000".into()));
    assert!(patterns.iter().all(|p| matches!(&p.source, PatternSource::Nested(f) if f.starts_with('d'))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nested_folders_by_bytes() {
    assert_eq!(nested_folder(b"a/b/.gitattributes"), Some(&b"a/b"[..]));
    assert_eq!(nested_folder(b".gitattributes"), None);
    assert_eq!(nested_folder(b"a/x.gitattributes"), None);
    assert_eq!(nested_folder(b"a/.gitattributes/x"), None);
    assert_eq!(PatternSource::Nested("a/b".into()).file(), "a/b/.gitattributes");
}
