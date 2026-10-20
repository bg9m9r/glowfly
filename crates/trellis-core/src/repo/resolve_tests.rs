//! Whole-side resolutions of conflicts involving symbolic links: the chosen side's entry is
//! staged as is and a link git left in the working tree is never written through.

use super::*;
use crate::gitcli::{args, run};
use crate::testutil::*;
use git2::{IndexEntry, IndexTime, Oid, Repository};
use std::path::Path;

const LINK: u32 = 0o120000;
const FILE: u32 = 0o100644;

/// Stages `path` as a symbolic link to `target`, without needing filesystem link support.
fn stage_link(dir: &Path, path: &str, target: &str) {
    let id = Repository::open(dir).unwrap().blob(target.as_bytes()).unwrap();
    git(dir, &["update-index", "--add", "--cacheinfo", &format!("120000,{id},{path}")]);
}

/// Replaces `link` with a real symbolic link to `target`, if this system allows it.
fn make_link(dir: &Path, link: &str, target: &str) -> bool {
    let full = dir.join(link);
    let _ = std::fs::remove_file(&full);
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, &full);
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(target.replace('/', "\\"), &full);
    let ok = made.is_ok();
    if !ok {
        std::fs::write(&full, target).unwrap();
    }
    ok
}

/// The working path is a link to `target`, or a plain file holding it where links are off.
fn assert_link(dir: &Path, path: &str, target: &str) {
    let full = dir.join(path);
    if std::fs::symlink_metadata(&full).unwrap().file_type().is_symlink() {
        let got = std::fs::read_link(&full).unwrap().to_string_lossy().replace('\\', "/");
        assert_eq!(got, target);
    } else {
        assert_eq!(std::fs::read_to_string(&full).unwrap(), target);
    }
}

/// The resolved (stage 0) mode and id of `path`; fails if anything is still conflicted.
fn staged(dir: &Path, path: &str) -> (u32, Oid) {
    let index = Repository::open(dir).unwrap().index().unwrap();
    assert!(!index.has_conflicts(), "conflict left in the index");
    let e = index.get_path(Path::new(path), 0).expect("path not staged");
    (e.mode, e.id)
}

fn read(dir: &Path, f: &str) -> String {
    std::fs::read_to_string(dir.join(f)).unwrap()
}

#[test]
fn use_theirs_on_a_link_conflict_stages_their_link_without_writing_through_ours() {
    let dir = scratch_repo("resolve-link");
    std::fs::create_dir_all(dir.join("shared")).unwrap();
    write(&dir, "shared/a.json", "A\n");
    write(&dir, "shared/b.json", "B\n");
    git(&dir, &["add", "."]);
    stage_link(&dir, "cfg", "shared/base.json");
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-f", "-b", "other"]);
    stage_link(&dir, "cfg", "shared/b.json");
    git(&dir, &["commit", "-q", "-m", "theirs"]);
    git(&dir, &["checkout", "-q", "-f", "main"]);
    stage_link(&dir, "cfg", "shared/a.json");
    git(&dir, &["commit", "-q", "-m", "ours"]);
    git(&dir, &["checkout", "-q", "-f", "main"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");
    // Git leaves our version in the working tree; make it a real link where possible.
    make_link(&dir, "cfg", "shared/a.json");

    let f = load_conflict(&dir, "cfg").unwrap();
    assert_eq!(f.content, ConflictContent::Unsupported(Unmergeable::SymbolicLink));
    resolve_with_side(&dir, "cfg", Side::Theirs, &f.ids).unwrap();

    assert_eq!(read(&dir, "shared/a.json"), "A\n", "wrote through the link");
    assert_eq!(staged(&dir, "cfg"), (LINK, f.ids.theirs.unwrap()));
    assert_link(&dir, "cfg", "shared/b.json");
    let (clean, out) = run(&dir, &args(&["diff", "--quiet", "--", "cfg"]));
    assert!(clean, "working tree differs from the resolution: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A conflict at `path` built straight into the index from (mode, content) per stage
/// (ancestor, ours, theirs; `None` for a side that deleted it), with ours in the working tree.
fn conflict(dir: &Path, path: &str, stages: [Option<(u32, &str)>; 3]) -> ConflictIds {
    let repo = Repository::open(dir).unwrap();
    let mut index = repo.index().unwrap();
    let mut ids = [None; 3];
    for (i, side) in stages.iter().enumerate() {
        let Some((mode, text)) = *side else { continue };
        let id = repo.blob(text.as_bytes()).unwrap();
        let zero = IndexTime::new(0, 0);
        let flags = ((i as u16 + 1) << 12) | path.len() as u16;
        let e = IndexEntry {
            ctime: zero,
            mtime: zero,
            dev: 0,
            ino: 0,
            mode,
            uid: 0,
            gid: 0,
            file_size: 0,
            id,
            flags,
            flags_extended: 0,
            path: path.as_bytes().to_vec(),
        };
        index.add(&e).unwrap();
        ids[i] = Some(id);
    }
    index.write().unwrap();
    match stages[1] {
        Some((LINK, target)) => _ = make_link(dir, path, target),
        Some((_, text)) => write(dir, path, text),
        None => {}
    }
    ConflictIds { ancestor: ids[0], ours: ids[1], theirs: ids[2] }
}

#[test]
fn use_a_side_across_a_file_and_link_type_change_keeps_that_sides_type() {
    let dir = scratch_repo("resolve-typechange");
    write(&dir, "victim.txt", "keep\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    // Ours a regular file, theirs a link.
    let to_link = conflict(&dir, "to_link", [Some((FILE, "base\n")), Some((FILE, "plain\n")), Some((LINK, "victim.txt"))]);
    // Ours a link, theirs a regular file.
    let to_file = conflict(&dir, "to_file", [Some((FILE, "base\n")), Some((LINK, "victim.txt")), Some((FILE, "plain\n"))]);
    // Ours a link, theirs deleted it.
    let kept = conflict(&dir, "kept", [Some((LINK, "base")), Some((LINK, "victim.txt")), None]);

    // Taking theirs stages a link, not a file holding its target, and leaves the other conflicts.
    let f = load_conflict(&dir, "to_link").unwrap();
    assert_eq!((&f.ids, &f.content), (&to_link, &ConflictContent::Unsupported(Unmergeable::SymbolicLink)));
    resolve_with_side(&dir, "to_link", Side::Theirs, &to_link).unwrap();
    assert_link(&dir, "to_file", "victim.txt");
    let index = Repository::open(&dir).unwrap().index().unwrap();
    assert!(index.conflict_get(Path::new("to_file")).is_ok() && index.conflict_get(Path::new("kept")).is_ok());

    // Taking theirs replaces our link instead of following it.
    resolve_with_side(&dir, "to_file", Side::Theirs, &to_file).unwrap();
    assert!(!std::fs::symlink_metadata(dir.join("to_file")).unwrap().file_type().is_symlink());
    assert_eq!(read(&dir, "to_file"), "plain\n");

    assert_eq!(load_conflict(&dir, "kept").unwrap().content, ConflictContent::DeletedBy(Side::Theirs));
    resolve_with_side(&dir, "kept", Side::Ours, &kept).unwrap();

    assert_eq!(staged(&dir, "to_link"), (LINK, to_link.theirs.unwrap()));
    assert_eq!(staged(&dir, "to_file"), (FILE, to_file.theirs.unwrap()));
    assert_eq!(staged(&dir, "kept"), (LINK, kept.ours.unwrap()));
    assert_link(&dir, "to_link", "victim.txt");
    assert_link(&dir, "kept", "victim.txt");
    assert_eq!(read(&dir, "victim.txt"), "keep\n", "wrote through a link");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn submodules_and_text_that_isnt_utf8_say_why_they_cant_be_merged() {
    let dir = scratch_repo("resolve-unmergeable");
    write(&dir, "keep.txt", "keep\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    // A submodule on every side: the merge never reads the (made up) commit ids as blobs.
    conflict(&dir, "sub", [Some((0o160000, "base")), Some((0o160000, "ours")), Some((0o160000, "theirs"))]);
    assert_eq!(load_conflict(&dir, "sub").unwrap().content, ConflictContent::Unsupported(Unmergeable::Submodule));

    // Latin-1 text on both sides of a real merge.
    std::fs::write(dir.join("latin.txt"), b"caf\xe9\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "latin"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    std::fs::write(dir.join("latin.txt"), b"th\xe9\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("latin.txt"), b"cr\xe8me\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");
    assert_eq!(load_conflict(&dir, "latin.txt").unwrap().content, ConflictContent::Unsupported(Unmergeable::NotUtf8));
    let _ = std::fs::remove_dir_all(&dir);
}
