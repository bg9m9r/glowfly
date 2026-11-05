//! Undo, redo and the journal for symbolic links against real repositories: a discarded,
//! deleted or replaced link is kept as git stores one (its target as a mode-120000 blob) and
//! comes back as a link, never over anything, or as a plain file holding its target when
//! `core.symlinks` is false, which then counts as the link. Every test that makes a link skips
//! where links can't be made (Windows without Developer Mode or admin rights).

use super::kept::{self, KeptItem, KeptWhat, Restored};
use super::test_bin::never;
use super::*;
use crate::repo::{self, Deleted, Untrashed};
use crate::testutil::*;
use std::path::{Path, PathBuf};

const SKIPPED: &str = "skipped: symbolic links can't be made here";

/// A link is removed itself, so a test whose trash is used fails.
fn no_untrash(_: &Path, _: &dyn Fn(&Path) -> bool) -> Untrashed {
    panic!("a link never goes to the trash")
}

const NO_TRASH: Trashers = Trashers { system: never, home: never, restore: no_untrash };

fn base(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    set_config(&dir, &[("core.symlinks", "true"), ("core.autocrlf", "false")]);
    write(&dir, "a.txt", "a\n");
    write(&dir, "b.txt", "b\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    dir
}

/// `base` with the link `l` to a.txt committed; `None` where links can't be made.
fn with_link(name: &str) -> Option<PathBuf> {
    let dir = base(name);
    if !file_link("a.txt", &dir.join("l")) {
        let _ = std::fs::remove_dir_all(&dir);
        return None;
    }
    git(&dir, &["add", "l"]);
    git(&dir, &["commit", "-q", "-m", "link"]);
    Some(dir)
}

fn hash(s: &str) -> git2::Oid {
    git2::Oid::hash_object(git2::ObjectType::Blob, s.as_bytes()).unwrap()
}

/// Whether `path` is a symbolic link to `target`.
fn links_to(dir: &Path, path: &str, target: &str) -> bool {
    let full = dir.join(path);
    full.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) && std::fs::read_link(&full).is_ok_and(|t| t == Path::new(target))
}

/// Plans and carries out an undo (or redo) of `a`, which must plan a file write.
fn apply(dir: &Path, a: &Action, redo: bool) {
    match plan(dir, a, redo).unwrap() {
        Plan::Write { path, content } => write_saved(dir, &path, content).unwrap(),
        other => panic!("expected a write, got {other:?}"),
    }
}

/// Plans and carries out the undo of a delete, which must put the link back without the trash.
fn put_link_back(dir: &Path, a: &Action) {
    match plan(dir, a, false).unwrap() {
        Plan::PutBack { path, content, staged, via } => {
            assert_eq!(put_back(dir, &path, content, staged.as_ref(), via, NO_TRASH).unwrap(), PutBackHow::Written { trash: None });
        }
        other => panic!("expected a put-back, got {other:?}"),
    }
}

/// The tracked link `l` re-pointed to b.txt, then discarded (back to a.txt).
fn discarded_link(name: &str) -> Option<(PathBuf, Action)> {
    let dir = with_link(name)?;
    std::fs::remove_file(dir.join("l")).unwrap();
    assert!(file_link("b.txt", &dir.join("l")));
    let a = discard_saving(&dir, "l", Discarded::File, || repo::discard_file(&dir, "l")).unwrap().expect("undoable");
    Some((dir, a))
}

#[test]
fn discarding_a_changed_link_undoes_and_redoes() {
    let Some((dir, a)) = discarded_link("lnk-discard") else { return eprintln!("{SKIPPED}") };
    let Action::Discard { before: Some(before), after: Some(after), .. } = &a else { panic!("{a:?}") };
    assert!(before.link && after.link, "{a:?}");
    assert_eq!((before.blob, after.blob), (hash("b.txt"), hash("a.txt")));
    assert!(links_to(&dir, "l", "a.txt"), "the discard pointed it back");
    apply(&dir, &a, false);
    assert!(links_to(&dir, "l", "b.txt"), "undo points it at the new target again");
    apply(&dir, &a, true);
    assert!(links_to(&dir, "l", "a.txt"), "redo points it back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_discarded_link_is_kept_and_listed_until_undone() {
    let Some((dir, a)) = discarded_link("lnk-kept") else { return eprintln!("{SKIPPED}") };
    let list = kept::list(&dir).unwrap();
    let [k] = &list[..] else { panic!("one record: {list:?}") };
    assert_eq!(k.what, KeptWhat::File);
    assert_eq!(k.item, KeptItem::File { path: "l".into(), blob: hash("b.txt"), executable: false, link: true });
    let repo = git2::Repository::open(&dir).unwrap();
    let tree = repo.find_commit(k.id).unwrap().tree().unwrap();
    assert_eq!(tree.get_name("file").unwrap().filemode(), 0o120000, "kept as git stores a link");
    drop(tree);
    drop(repo);
    apply(&dir, &a, false);
    assert!(kept::list(&dir).unwrap().is_empty(), "the link points there again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discarding_an_untracked_link_undoes() {
    let dir = base("lnk-untracked");
    if !file_link("a.txt", &dir.join("new-link")) {
        let _ = std::fs::remove_dir_all(&dir);
        return eprintln!("{SKIPPED}");
    }
    let d = discard_untracked_saving(&dir, "new-link", DeleteVia::Trash, false, NO_TRASH).unwrap();
    assert_eq!(d.how, Deleted::LinkRemoved);
    let a = d.undo.expect("undoable");
    assert!(matches!(&a, Action::DeleteFile { before, .. } if before.link && before.blob == hash("a.txt")), "{a:?}");
    assert!(dir.join("new-link").symlink_metadata().is_err());
    let list = kept::list(&dir).unwrap();
    assert!(
        list.iter()
            .any(|k| k.what == KeptWhat::Untracked && matches!(&k.item, KeptItem::File { path, link: true, .. } if path == "new-link")),
        "{list:?}"
    );

    put_link_back(&dir, &a);
    assert!(links_to(&dir, "new-link", "a.txt"));
    assert_eq!(std::fs::read(dir.join("a.txt")).unwrap(), b"a\n", "the target is untouched");
    match plan(&dir, &a, true).unwrap() {
        Plan::DeleteAgain { path, via, staged } => {
            assert_eq!(delete_again(&dir, &path, via, staged, NO_TRASH).unwrap(), Deleted::LinkRemoved)
        }
        other => panic!("expected a delete, got {other:?}"),
    }
    assert!(dir.join("new-link").symlink_metadata().is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_link_replaced_by_a_file_is_put_back() {
    let Some(dir) = with_link("lnk-typechange") else { return eprintln!("{SKIPPED}") };
    std::fs::remove_file(dir.join("l")).unwrap();
    write(&dir, "l", "text");
    let a = discard_saving(&dir, "l", Discarded::File, || repo::discard_file(&dir, "l")).unwrap().expect("undoable");
    assert!(matches!(&a, Action::Discard { before: Some(b), after: Some(f), .. } if !b.link && f.link), "{a:?}");
    assert!(links_to(&dir, "l", "a.txt"));
    apply(&dir, &a, false);
    let full = dir.join("l");
    assert!(full.symlink_metadata().unwrap().is_file(), "the file replaced the link");
    assert_eq!(std::fs::read(&full).unwrap(), b"text");
    assert_eq!(std::fs::read(dir.join("a.txt")).unwrap(), b"a\n", "never written through the link");
    apply(&dir, &a, true);
    assert!(links_to(&dir, "l", "a.txt"), "redo makes the link again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_and_delete_of_a_staged_link_restages_it() {
    let dir = base("lnk-staged");
    if !file_link("a.txt", &dir.join("new-link")) {
        let _ = std::fs::remove_dir_all(&dir);
        return eprintln!("{SKIPPED}");
    }
    git(&dir, &["add", "new-link"]);
    let staged = repo::staged_new(&dir, "new-link").unwrap();
    assert_eq!(staged.mode, 0o120000);
    let d = delete_staged_saving(&dir, "new-link", DeleteVia::Trash, false, NO_TRASH).unwrap();
    assert_eq!(d.how, Deleted::LinkRemoved);
    assert!(!repo::staged_new_matches(&dir, "new-link", &staged).unwrap(), "unstaged");
    put_link_back(&dir, &d.undo.expect("undoable"));
    assert!(links_to(&dir, "new-link", "a.txt"));
    assert!(repo::staged_new_matches(&dir, "new-link", &staged).unwrap(), "staged again as a link");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_journal_restores_a_kept_link_as_a_link() {
    let dir = base("lnk-journal");
    if !file_link("a.txt", &dir.join("new-link")) {
        let _ = std::fs::remove_dir_all(&dir);
        return eprintln!("{SKIPPED}");
    }
    discard_untracked_saving(&dir, "new-link", DeleteVia::Trash, false, NO_TRASH).unwrap();
    let id = kept::list(&dir).unwrap()[0].id;
    assert_eq!(kept::restore(&dir, id, false).unwrap(), Restored::Wrote { path: "new-link".into() });
    assert!(links_to(&dir, "new-link", "a.txt"));
    assert_eq!(kept::restore(&dir, id, false).unwrap(), Restored::AlreadyThere { path: "new-link".into() });
    assert_eq!(kept::restore(&dir, id, true).unwrap(), Restored::Wrote { path: "new-link (restored)".into() });
    assert!(links_to(&dir, "new-link (restored)", "a.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn with_core_symlinks_off_a_link_comes_back_as_a_plain_file() {
    let dir = base("lnk-nosymlinks");
    if !file_link("a.txt", &dir.join("new-link")) {
        let _ = std::fs::remove_dir_all(&dir);
        return eprintln!("{SKIPPED}");
    }
    let a = discard_untracked_saving(&dir, "new-link", DeleteVia::Trash, false, NO_TRASH).unwrap().undo.expect("undoable");
    git(&dir, &["config", "core.symlinks", "false"]);
    put_link_back(&dir, &a);
    let full = dir.join("new-link");
    assert!(full.symlink_metadata().unwrap().is_file(), "a plain file, as git checks a link out then");
    assert_eq!(std::fs::read(&full).unwrap(), b"a.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The default on Windows: `core.symlinks` false, with a real link made outside git.
#[test]
fn with_core_symlinks_off_an_untracked_link_put_back_as_a_file_redoes_and_leaves_the_lost_tab() {
    let dir = base("lnk-nosymlinks-redo");
    git(&dir, &["config", "core.symlinks", "false"]);
    if !file_link("a.txt", &dir.join("new-link")) {
        let _ = std::fs::remove_dir_all(&dir);
        return eprintln!("{SKIPPED}");
    }
    let a = discard_untracked_saving(&dir, "new-link", DeleteVia::Trash, false, NO_TRASH).unwrap().undo.expect("undoable");
    let id = kept::list(&dir).unwrap()[0].id;
    put_link_back(&dir, &a);
    assert!(dir.join("new-link").symlink_metadata().unwrap().is_file(), "a plain file holding the target");
    assert!(kept::list(&dir).unwrap().is_empty(), "the file holding the target counts as the link");
    assert_eq!(kept::restore(&dir, id, false).unwrap(), Restored::AlreadyThere { path: "new-link".into() });
    match plan(&dir, &a, true).unwrap() {
        // The plain file put back goes to the trash like any file.
        Plan::DeleteAgain { path, via, staged } => assert!(delete_again(&dir, &path, via, staged, super::test_bin::T).is_ok()),
        other => panic!("redo isn't refused as stale: {other:?}"),
    }
    assert!(dir.join("new-link").symlink_metadata().is_err());
    put_link_back(&dir, &a);
    assert_eq!(std::fs::read(dir.join("new-link")).unwrap(), b"a.txt", "undo works again");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The tracked link re-pointed and discarded with `core.symlinks` false all along.
#[test]
fn with_core_symlinks_off_a_discarded_link_undoes_and_redoes() {
    let Some(dir) = with_link("lnk-nosymlinks-discard") else { return eprintln!("{SKIPPED}") };
    git(&dir, &["config", "core.symlinks", "false"]);
    std::fs::remove_file(dir.join("l")).unwrap();
    assert!(file_link("b.txt", &dir.join("l")));
    let a = discard_saving(&dir, "l", Discarded::File, || repo::discard_file(&dir, "l")).unwrap().expect("undoable");
    let Action::Discard { before: Some(before), .. } = &a else { panic!("{a:?}") };
    assert!(before.link && before.blob == hash("b.txt"), "{a:?}");
    apply(&dir, &a, false);
    assert_eq!(std::fs::read(dir.join("l")).unwrap(), b"b.txt", "undo puts the target back (a plain file here)");
    assert!(kept::list(&dir).unwrap().is_empty(), "the discarded target is back");
    apply(&dir, &a, true);
    assert_eq!(std::fs::read(dir.join("l")).unwrap(), b"a.txt", "redo isn't refused as stale");
    apply(&dir, &a, false);
    assert_eq!(std::fs::read(dir.join("l")).unwrap(), b"b.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The undo history and journal as a discard of a real link left them, with `core.symlinks`
/// false: needs no link, so it runs everywhere. The link `new-link` to a.txt was discarded
/// (kept, and undoable as a delete).
fn discarded_link_record(name: &str) -> (PathBuf, Action, git2::Oid) {
    let dir = base(name);
    git(&dir, &["config", "core.symlinks", "false"]);
    let blob = git2::Repository::open(&dir).unwrap().blob(b"a.txt").unwrap();
    let link = Saved { blob, executable: false, link: true };
    let id = kept::keep_file(&dir, "new-link", link, KeptWhat::Untracked).unwrap();
    let a = Action::DeleteFile { path: "new-link".into(), before: link, via: DeleteVia::Permanent, staged: None };
    (dir, a, id)
}

#[test]
fn without_core_symlinks_a_link_put_back_as_a_file_redoes_and_leaves_the_lost_tab() {
    let (dir, a, id) = discarded_link_record("lnk-nosymlinks-record");
    assert_eq!(kept::list(&dir).unwrap().len(), 1);
    put_link_back(&dir, &a);
    assert!(dir.join("new-link").symlink_metadata().unwrap().is_file());
    assert!(kept::list(&dir).unwrap().is_empty(), "the file holding the target counts as the link");
    assert_eq!(kept::restore(&dir, id, false).unwrap(), Restored::AlreadyThere { path: "new-link".into() });
    match plan(&dir, &a, true).unwrap() {
        Plan::DeleteAgain { path, via, staged } => assert!(delete_again(&dir, &path, via, staged, NO_TRASH).is_ok()),
        other => panic!("redo isn't refused as stale: {other:?}"),
    }
    assert!(dir.join("new-link").symlink_metadata().is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_core_symlinks_a_discarded_link_target_written_back_redoes() {
    let dir = base("lnk-nosymlinks-write");
    git(&dir, &["config", "core.symlinks", "false"]);
    let repo = git2::Repository::open(&dir).unwrap();
    let before = Saved { blob: repo.blob(b"b.txt").unwrap(), executable: false, link: true };
    let after = Saved::file(repo.blob(b"a.txt").unwrap(), false);
    drop(repo);
    write(&dir, "l", "a.txt");
    let a = Action::Discard { path: "l".into(), what: Discarded::File, before: Some(before), after: Some(after) };
    apply(&dir, &a, false);
    assert_eq!(std::fs::read(dir.join("l")).unwrap(), b"b.txt");
    apply(&dir, &a, true);
    assert_eq!(std::fs::read(dir.join("l")).unwrap(), b"a.txt", "redo isn't refused as stale");
    // Where links are made as links, a plain file isn't the link, so redo is refused.
    apply(&dir, &a, false);
    git(&dir, &["config", "core.symlinks", "true"]);
    assert!(matches!(plan(&dir, &a, true).unwrap(), Plan::Refuse(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Runs everywhere, also where links can't be made.
#[test]
fn a_plain_file_holding_the_target_counts_as_the_link_only_without_core_symlinks() {
    let link = Saved { blob: hash("a.txt"), executable: false, link: true };
    let file = Saved::file(hash("a.txt"), false);
    assert!(link.held_by(Some(link), true) && link.held_by(Some(link), false));
    assert!(link.held_by(Some(file), false), "how a link is put back there");
    assert!(!link.held_by(Some(file), true));
    assert!(!file.held_by(Some(link), false), "a link never counts as a plain file");
    assert!(!link.held_by(Some(Saved::file(hash("b.txt"), false)), false));
    assert!(!link.held_by(None, false));
}

#[cfg(windows)]
#[test]
fn a_folder_link_is_not_saved() {
    let dir = base("lnk-folder");
    std::fs::create_dir(dir.join("d")).unwrap();
    if std::os::windows::fs::symlink_dir("d", dir.join("dl")).is_err() {
        let _ = std::fs::remove_dir_all(&dir);
        return eprintln!("{SKIPPED}");
    }
    assert!(matches!(super::saved::save(&dir, "dl"), Err(crate::error::Error::Unsupported(_))));
    let _ = std::fs::remove_dir_all(&dir);
}
