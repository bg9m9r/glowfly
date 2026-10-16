use super::find::{Found, TargetState, check, rank, root_commit};
use super::{Link, Target};
use crate::testutil::{git, scratch_repo, with_origin, write};
use std::path::{Path, PathBuf};

fn commit(dir: &Path, file: &str, msg: &str) {
    write(dir, file, msg);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", msg]);
}

fn head(dir: &Path) -> git2::Oid {
    git2::Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn first_commit(dir: &Path) -> String {
    let repo = git2::Repository::open(dir).unwrap();
    let mut walk = repo.revwalk().unwrap();
    walk.push_head().unwrap();
    walk.simplify_first_parent().unwrap();
    walk.last().unwrap().unwrap().to_string()
}

/// A folder next to `of` for a clone, emptied first.
fn sibling(of: &Path, tag: &str) -> PathBuf {
    let p = of.with_file_name(format!("{}-{tag}", of.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

/// `file:///srv/x` on Unix, `file:///C:/x` on Windows.
fn file_url(p: &Path) -> String {
    let s = p.display().to_string().replace('\\', "/");
    if s.starts_with('/') { format!("file://{s}") } else { format!("file:///{s}") }
}

fn link(url: Option<String>, root: Option<String>, target: Target) -> Link {
    Link { url, root, target }
}

fn checked(path: &Path, l: &Link) -> Found {
    check(path, l).unwrap().unwrap_or_else(|| panic!("{} should match", path.display()))
}

#[test]
fn the_root_is_the_end_of_the_first_parent_chain() {
    let dir = scratch_repo("link-root");
    assert_eq!(root_commit(&dir).unwrap(), None, "an unborn HEAD has no root");
    commit(&dir, "a.txt", "one");
    let first = head(&dir).to_string();
    commit(&dir, "a.txt", "two");
    assert_eq!(root_commit(&dir).unwrap(), Some(first.clone()));

    // An unrelated history merged in doesn't move the root.
    let other = scratch_repo("link-root-other");
    commit(&other, "b.txt", "other");
    git(&dir, &["fetch", "-q", &other.display().to_string(), "main:other"]);
    git(&dir, &["merge", "-q", "--allow-unrelated-histories", "--no-edit", "other"]);
    assert_eq!(root_commit(&dir).unwrap(), Some(first));
}

#[test]
fn a_shallow_clone_has_no_root() {
    let dir = scratch_repo("link-shallow-src");
    commit(&dir, "a.txt", "one");
    commit(&dir, "a.txt", "two");
    let shallow = sibling(&dir, "shallow");
    git(dir.parent().unwrap(), &["clone", "-q", "--depth", "1", &file_url(&dir), &shallow.display().to_string()]);
    assert_eq!(root_commit(&shallow).unwrap(), None);

    // Still found by its remote's URL, though not by the root it lacks.
    let l = link(Some(file_url(&dir)), Some(first_commit(&dir)), Target::Repository);
    let f = checked(&shallow, &l);
    assert!(!f.by_root && f.by_url);
    assert_eq!(rank(&f), 1);
    assert_eq!(f.remote.as_deref(), Some("origin"));
}

#[test]
fn a_clone_under_another_name_matches_by_root_and_url() {
    let (work, bare) = with_origin("link-clone");
    let copy = sibling(&work, "copy");
    git(work.parent().unwrap(), &["clone", "-q", &bare.display().to_string(), &copy.display().to_string()]);
    let root = root_commit(&work).unwrap();
    // The link's url is spelled another way than the clone's origin.
    let url = format!("{}/", bare.display());
    let f = checked(&copy, &link(Some(url), root.clone(), Target::Repository));
    assert!(f.by_root && f.by_url && f.main_worktree);
    assert_eq!(rank(&f), 3);
    assert_eq!(f.target, TargetState::Present { oid: head(&work), label: "HEAD".into() });

    // By root alone when the URL names another repository.
    let f = checked(&copy, &link(Some("https://example.com/other/repo".into()), root.clone(), Target::Repository));
    assert!(f.by_root && !f.by_url && f.remote.is_none());
    assert_eq!(rank(&f), 2);

    // An unrelated repository, and a folder that isn't one.
    let unrelated = scratch_repo("link-unrelated");
    commit(&unrelated, "z.txt", "z");
    assert_eq!(check(&unrelated, &link(None, root.clone(), Target::Repository)).unwrap(), None);
    let plain = sibling(&work, "plain");
    std::fs::create_dir_all(&plain).unwrap();
    assert_eq!(check(&plain, &link(None, root, Target::Repository)).unwrap(), None);
}

#[test]
fn targets_are_found_or_reported_missing() {
    let (work, bare) = with_origin("link-targets");
    let url = Some(bare.display().to_string());
    let root = root_commit(&work).unwrap();
    let main = head(&work);
    git(&work, &["branch", "local-only"]);
    // A remote branch that has a local branch under another name tracking it.
    git(&work, &["push", "-q", "origin", "main:feature-x"]);
    git(&work, &["branch", "--track", "my-x", "origin/feature-x"]);
    // A remote branch with no local branch at all.
    commit(&work, "b.txt", "two");
    let second = head(&work);
    git(&work, &["push", "-q", "origin", "main:remote-only"]);
    git(&work, &["reset", "-q", "--hard", &main.to_string()]);
    git(&work, &["tag", "-a", "v1.0", "-m", "release", &second.to_string()]);

    let state = |t: Target| checked(&work, &link(url.clone(), root.clone(), t)).target;
    let present = |oid, label: &str| TargetState::Present { oid, label: label.into() };
    assert_eq!(state(Target::Branch("local-only".into())), present(main, "local-only"));
    assert_eq!(state(Target::Branch("feature-x".into())), present(main, "my-x"));
    assert_eq!(state(Target::Branch("remote-only".into())), present(second, "origin/remote-only"));
    assert_eq!(state(Target::Branch("gone".into())), TargetState::Missing);
    assert_eq!(state(Target::Tag("v1.0".into())), present(second, "v1.0"), "an annotated tag is peeled");
    assert_eq!(state(Target::Tag("v9".into())), TargetState::Missing);
    let sha = second.to_string();
    assert_eq!(state(Target::Commit(sha.clone())), present(second, &sha[..7]));
    assert_eq!(state(Target::Commit(sha[..9].to_owned())), present(second, &sha[..7]));
    assert_eq!(state(Target::Commit("f".repeat(40))), TargetState::Missing);

    // Without a matching remote, a branch is still found on any remote.
    let by_root = checked(&work, &link(None, root, Target::Branch("remote-only".into())));
    assert_eq!(by_root.target, present(second, "origin/remote-only"));
}

#[test]
fn worktrees_share_their_common_dir() {
    let (work, _bare) = with_origin("link-worktrees");
    let linked = sibling(&work, "wt");
    git(&work, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    let l = link(None, root_commit(&work).unwrap(), Target::Repository);
    let (main, other) = (checked(&work, &l), checked(&linked, &l));
    assert_eq!(main.common_dir, other.common_dir);
    assert!(main.main_worktree && !other.main_worktree);
    assert!(other.by_root);
}
