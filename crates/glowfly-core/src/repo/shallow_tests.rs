use super::{LoadOptions, load, refs_key};
use crate::clone_ops::{CloneOptions, HistoryLimit, Shallow, clone_cmd};
use crate::gitcli::run;
use crate::shallow_ops::{Deepen, deepen_cmd};
use crate::testutil::*;
use git2::Oid;
use std::path::{Path, PathBuf};

/// A source repository with `commits` commits on main, and a depth-`depth` clone of it next to
/// it. Returns (source, clone) and the source's commits, oldest first.
fn shallow_clone(name: &str, commits: usize, depth: u32) -> (PathBuf, PathBuf, Vec<Oid>) {
    let src = scratch_repo(name);
    let mut oids = Vec::new();
    for i in 0..commits {
        write(&src, "a.txt", &format!("{i}\n"));
        git(&src, &["add", "."]);
        git(&src, &["commit", "-q", "-m", &format!("c{i}")]);
        oids.push(git2::Repository::open(&src).unwrap().head().unwrap().target().unwrap());
    }
    let dest = src.with_file_name(format!("{}-sh", src.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&dest);
    let opts = CloneOptions { shallow: Some(Shallow { limit: HistoryLimit::Depth(depth), branch: None }), sparse: None, submodules: false };
    let cmd = clone_cmd(&src.display().to_string(), &dest.file_name().unwrap().to_string_lossy(), &opts);
    let (ok, out) = run(src.parent().unwrap(), &cmd);
    assert!(ok, "clone failed: {out}");
    (src, dest, oids)
}

fn deepen(dest: &Path, how: Deepen) {
    let (ok, out) = run(dest, &deepen_cmd("origin", how));
    assert!(ok, "{how:?} failed: {out}");
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn deepen_then_unshallow_a_local_shallow_clone() {
    let (src, dest, oids) = shallow_clone("shallow-deepen", 4, 1);
    let opts = LoadOptions::all(100);
    let snap = load(&dest, &opts).unwrap();
    assert!(snap.is_shallow());
    assert_eq!(snap.shallow_roots, [oids[3]]);
    assert_eq!(snap.commits.len(), 1);
    assert!(snap.commits[0].parents.is_empty(), "the boundary commit's parents weren't downloaded");
    assert!(git2::Repository::open(&dest).unwrap().is_shallow());
    let k1 = refs_key(&dest).unwrap();

    deepen(&dest, Deepen::By(1));
    let snap = load(&dest, &opts).unwrap();
    assert!(snap.is_shallow());
    assert_eq!(snap.shallow_roots, [oids[2]]);
    assert_eq!(snap.commits.len(), 2);
    let k2 = refs_key(&dest).unwrap();
    assert_ne!(k2, k1, "the shallow file changed while the refs stayed put");

    deepen(&dest, Deepen::Full);
    let snap = load(&dest, &opts).unwrap();
    assert!(!snap.is_shallow());
    assert!(snap.shallow_roots.is_empty());
    assert_eq!(snap.commits.len(), 4);
    assert_ne!(refs_key(&dest).unwrap(), k2, "unshallow removed the shallow file");
    cleanup(&[&src, &dest]);
}

#[test]
fn full_clone_has_no_shallow_roots() {
    let dir = scratch_repo("shallow-full");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let snap = load(&dir, &LoadOptions::all(100)).unwrap();
    assert!(snap.shallow_roots.is_empty());
    assert!(!snap.is_shallow());
    cleanup(&[&dir]);
}

#[test]
fn deepening_to_the_root_makes_the_clone_full() {
    let (src, dest, _) = shallow_clone("shallow-root", 2, 1);
    deepen(&dest, Deepen::By(5));
    let snap = load(&dest, &LoadOptions::all(100)).unwrap();
    assert!(!snap.is_shallow());
    assert_eq!(snap.commits.len(), 2);
    cleanup(&[&src, &dest]);
}
