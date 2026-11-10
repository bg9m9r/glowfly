//! Tests of backup names: which names count, how a new one is numbered, and pruning that keeps
//! the newest and leaves every other file, folder and link alone.

use super::super::tests::{leftovers, scratch, symlink_file};
use super::*;

#[test]
fn backup_names_are_recognised() {
    let file = "settings.json";
    for (name, n) in [("settings.json.bak", 1), ("settings.json.bak-2", 2), ("settings.json.bak-10", 10)] {
        assert_eq!(backup_number(file, name), Some(n), "{name}");
    }
    for name in [
        "settings.json.bak-1",
        "settings.json.bak-02",
        "settings.json.bak-",
        "settings.json.bak-2x",
        "settings.json.bak.txt",
        "settings.json",
        "other.json.bak",
        "settings.json.bak-2.record",
        "Settings.json.bak",
        ".settings.json.1.0.tmp",
    ] {
        assert_eq!(backup_number(file, name), None, "{name}");
    }
}

#[test]
fn backups_number_past_the_highest() {
    let dir = scratch("past-highest");
    let file = dir.join("settings.json");
    std::fs::write(dir.join("settings.json.bak-3"), "old").unwrap();
    std::fs::write(&file, "one").unwrap();
    assert_eq!(move_aside(&file).unwrap(), dir.join("settings.json.bak-4"));
    assert_eq!(keep_copy(&file, b"two").unwrap(), dir.join("settings.json.bak-5"));
    assert_eq!(std::fs::read(dir.join("settings.json.bak-5")).unwrap(), b"two");
    // An empty folder starts at `.bak`, then `.bak-2`.
    let fresh = scratch("past-highest-empty");
    let file = fresh.join("settings.json");
    assert_eq!(keep_copy(&file, b"a").unwrap(), fresh.join("settings.json.bak"));
    assert_eq!(keep_copy(&file, b"b").unwrap(), fresh.join("settings.json.bak-2"));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&fresh);
}

#[test]
fn prune_keeps_the_newest_three_and_leaves_other_files() {
    let dir = scratch("prune");
    let file = dir.join("settings.json");
    std::fs::write(&file, "{}").unwrap();
    let names = ["settings.json.bak", "settings.json.bak-2", "settings.json.bak-3", "settings.json.bak-4", "settings.json.bak-5"];
    for name in names {
        std::fs::write(dir.join(name), name).unwrap();
    }
    let others = ["settings.json.bak-02", "settings.json.bak.txt", "other.json.bak", ".settings.json.1.0.tmp", "notes.txt"];
    for name in others {
        std::fs::write(dir.join(name), "keep").unwrap();
    }
    std::fs::create_dir(dir.join("settings.json.bak-9")).unwrap();
    let mut removed = prune_backups(&file, KEPT_BACKUPS);
    removed.sort();
    assert_eq!(removed, [dir.join("settings.json.bak"), dir.join("settings.json.bak-2")]);
    for name in &names[2..] {
        assert_eq!(std::fs::read_to_string(dir.join(name)).unwrap(), *name);
    }
    for name in others {
        assert_eq!(std::fs::read_to_string(dir.join(name)).unwrap(), "keep", "{name}");
    }
    assert!(dir.join("settings.json.bak-9").is_dir(), "a folder is never pruned");
    assert!(prune_backups(&file, KEPT_BACKUPS).is_empty(), "nothing more to prune");
    // The folder named `.bak-9` takes its number, so the next copy is past it.
    assert_eq!(keep_copy(&file, b"new").unwrap(), dir.join("settings.json.bak-10"));
    assert_eq!(prune_backups(&file, KEPT_BACKUPS), [dir.join("settings.json.bak-3")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_copy_after_a_prune_is_the_newest_and_kept() {
    let dir = scratch("prune-plain");
    let file = dir.join("settings.json");
    for name in ["settings.json.bak", "settings.json.bak-2", "settings.json.bak-3", "settings.json.bak-4", "settings.json.bak-5"] {
        std::fs::write(dir.join(name), name).unwrap();
    }
    prune_backups(&file, KEPT_BACKUPS);
    assert_eq!(keep_copy(&file, b"new").unwrap(), dir.join("settings.json.bak-6"));
    assert_eq!(prune_backups(&file, KEPT_BACKUPS), [dir.join("settings.json.bak-3")]);
    assert_eq!(leftovers(&dir), ["settings.json.bak-4", "settings.json.bak-5", "settings.json.bak-6"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_link_named_as_a_backup_is_neither_removed_nor_counted_by_pruning() {
    let dir = scratch("prune-link");
    let file = dir.join("settings.json");
    let target = dir.join("target.json");
    std::fs::write(&target, "linked").unwrap();
    if !symlink_file(&target, &dir.join("settings.json.bak-8")) {
        eprintln!("skipped: this system refused to create a symbolic link");
        return;
    }
    for name in ["settings.json.bak", "settings.json.bak-2", "settings.json.bak-3"] {
        std::fs::write(dir.join(name), name).unwrap();
    }
    // Three regular backups: the link doesn't count as a fourth, so nothing goes.
    assert!(prune_backups(&file, KEPT_BACKUPS).is_empty());
    assert!(std::fs::symlink_metadata(dir.join("settings.json.bak-8")).unwrap().file_type().is_symlink());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "linked");
    let _ = std::fs::remove_dir_all(&dir);
}
