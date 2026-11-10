//! Names a repository gives that aren't plain relative paths inside the work tree (absolute,
//! drive-relative, UNC and device spellings, `..` parts): never joined to it (`work_path`), and a
//! commit's file named so is listed as absent without being looked up.

use super::*;
use crate::testutil::*;
use git2::{ObjectType, Oid, Repository};
use std::path::{Component, Path, PathBuf};

/// Refused on every system.
const NEVER: &[&str] =
    &["/etc/passwd", "/", "//h/s/a", "//?/C:/x", "..", "../x", "a/../../x", "a/..", "a/./b", "./a", "", "a//b", ".git/config", "a/.git/x"];

/// Refused on Windows, where they name another place or another file; one plain name (or a
/// plain path, for the one holding `/`) elsewhere.
const WINDOWS: &[&str] = &[
    r"C:\x",
    "C:x",
    "C:/x",
    "C:",
    r"\x",
    r"\\h\s\a",
    r"\\?\C:\x",
    r"\\?\UNC\h\s\a",
    r"\\.\pipe\x",
    r"\??\C:\x",
    r"a\..\..\x",
    r"..\x",
    "a:b",
    "a.txt::$DATA",
    "a./b",
    "a /b",
];

fn root() -> PathBuf {
    if cfg!(windows) { PathBuf::from(r"C:\work\repo") } else { PathBuf::from("/work/repo") }
}

/// Every part `p` adds to `root` is a plain name.
fn stays_inside(p: &Path, root: &Path) -> bool {
    p.strip_prefix(root).is_ok_and(|rest| rest.components().all(|c| matches!(c, Component::Normal(_))))
}

#[test]
fn only_plain_relative_names_are_joined_to_the_work_tree() {
    let root = root();
    for name in NEVER {
        assert!(work_path(&root, name).is_err(), "{name:?}");
    }
    for name in WINDOWS {
        let joined = work_path(&root, name).ok();
        assert_eq!(joined.is_none(), cfg!(windows), "{name:?} is a plain name only outside Windows");
        if let Some(p) = joined {
            assert!(stays_inside(&p, &root), "{name:?} -> {p:?}");
        }
    }
    for (name, parts) in
        [("a.txt", vec!["a.txt"]), ("dir/sub/a.txt", vec!["dir", "sub", "a.txt"]), ("dir/", vec!["dir"]), (" a.txt", vec![" a.txt"])]
    {
        let p = work_path(&root, name).unwrap();
        assert_eq!(p, parts.iter().fold(root.clone(), |at, part| at.join(part)), "{name:?}");
        assert!(stays_inside(&p, &root));
    }
}

/// A plain join of the same names would leave the work tree, which is why they are refused.
#[test]
fn a_plain_join_of_the_refused_names_leaves_the_work_tree() {
    let root = root();
    let mut leaving: Vec<&str> = vec!["/etc/passwd", "//h/s/a", "../x", "a/../../x"];
    if cfg!(windows) {
        leaving.extend([r"C:\x", "C:x", r"\\h\s\a", r"\\?\C:\x", r"\x"]);
    }
    for name in leaving {
        assert!(!stays_inside(&root.join(name), &root), "{name:?}");
        assert!(work_path(&root, name).is_err(), "{name:?}");
    }
}

/// Writes a tree object as given: libgit2's tree builder and git's index refuse these names.
fn raw_tree(repo: &Repository, entries: &[(&str, &str, Oid)]) -> Oid {
    let mut sorted: Vec<_> = entries.to_vec();
    sorted.sort_by_key(|(mode, name, _)| if *mode == "40000" { format!("{name}/") } else { (*name).to_owned() });
    let mut bytes = Vec::new();
    for (mode, name, id) in sorted {
        bytes.extend_from_slice(format!("{mode} {name}\0").as_bytes());
        bytes.extend_from_slice(id.as_bytes());
    }
    repo.odb().unwrap().write(ObjectType::Tree, &bytes).unwrap()
}

#[test]
fn a_commit_file_named_outside_the_work_tree_is_absent_and_not_looked_up() {
    let dir = scratch_repo("work-path-outside");
    write(&dir, "kept.txt", "kept\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "one"]);
    // A file next to the repository, which the names below would reach if they were joined.
    let outside = dir.with_file_name(format!("glowfly-test-work-path-marker-{}.txt", std::process::id()));
    std::fs::write(&outside, "outside\n").unwrap();
    let marker = outside.file_name().unwrap().to_string_lossy().into_owned();
    assert!(dir.join("..").join(&marker).exists());

    let repo = Repository::open(&dir).unwrap();
    let blob = repo.blob(b"x\n").unwrap();
    let kept = repo.head().unwrap().peel_to_tree().unwrap().get_name("kept.txt").unwrap().id();
    let up = raw_tree(&repo, &[("100644", &marker, blob)]);
    let absolute = outside.to_string_lossy().into_owned();
    let mut entries = vec![("100644", "kept.txt", kept), ("40000", "..", up)];
    let mut named = vec![format!("../{marker}")];
    if cfg!(windows) {
        // Backslashes are name bytes to git: one entry whose name is the file's full path.
        entries.push(("100644", &absolute, blob));
        named.push(absolute.clone());
    }
    let tree = repo.find_tree(raw_tree(&repo, &entries)).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let oid = repo.commit(None, &sig, &sig, "names", &tree, &[&head]).unwrap();

    let detail = commit_detail(&dir, oid).unwrap();
    let listed: Vec<&str> = detail.files.iter().map(|f| f.path.as_str()).collect();
    for name in &named {
        assert!(listed.contains(&name.as_str()), "{name:?} in {listed:?}");
        assert!(detail.absent.contains(name), "{name:?} is listed as absent: {:?}", detail.absent);
        assert!(!on_disk_for_test(&repo, name), "{name:?}");
        assert_eq!(conflict_markers(&dir, std::slice::from_ref(name))[0].1, None, "{name:?}");
        let stamp = open_file_stamp(&dir, name);
        std::fs::write(
            &outside,
            "changed outside
",
        )
        .unwrap();
        assert_eq!(open_file_stamp(&dir, name), stamp, "{name:?}");
        std::fs::write(
            &outside, "outside
",
        )
        .unwrap();
    }
    assert!(on_disk_for_test(&repo, "kept.txt"));
    let _ = std::fs::remove_file(&outside);
    let _ = std::fs::remove_dir_all(&dir);
}

fn on_disk_for_test(repo: &Repository, path: &str) -> bool {
    super::diff::on_disk(repo, path)
}
