//! Finding a deleted file in the trash and taking it back out: which item is picked (only one
//! from that path with the kept bytes, the newest first), how paths compare on Windows, reading
//! `.trashinfo` files, and putting a freedesktop item back without ever replacing anything. The
//! real Recycle Bin / Trash only in an ignored test run by hand.

use super::home::{bytes_of_info, info_path, list_home, take_out, trashinfo};
use super::untrash::{Candidate, newest_ours, same_place};
use std::path::{Path, PathBuf};

fn temp(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-untrash-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cand(original: &str, deleted: i64, bytes: &str) -> Candidate<u32> {
    Candidate { original: PathBuf::from(original), deleted, bytes: PathBuf::from(bytes), item: deleted as u32 }
}

#[test]
fn only_an_item_from_that_path_with_the_kept_bytes_is_picked_newest_first() {
    let items = vec![
        cand("/r/a.txt", 10, "old-same"),
        cand("/r/a.txt", 30, "new-other"),
        cand("/r/b.txt", 40, "b-same"),
        cand("/r/a.txt", 20, "mid-same"),
    ];
    let same = |p: &Path| p.to_string_lossy().ends_with("-same");
    let found = newest_ours(items.clone(), Path::new("/r/a.txt"), false, &same).expect("found");
    assert_eq!(found.item, 20, "the newest with the kept bytes, not a newer one with other bytes");
    assert!(newest_ours(items.clone(), Path::new("/r/c.txt"), false, &same).is_none(), "nothing from that path");
    assert!(newest_ours(items, Path::new("/r/a.txt"), false, &|_| false).is_none(), "nothing with those bytes");
}

#[test]
fn windows_paths_compare_without_case_separators_or_a_verbatim_prefix() {
    assert!(same_place(Path::new(r"C:\Repo\A.txt"), Path::new("c:/repo/a.txt"), true));
    assert!(same_place(Path::new(r"\\?\C:\repo\a.txt"), Path::new(r"C:\repo\a.txt"), true));
    assert!(same_place(Path::new(r"\\?\UNC\srv\share\a.txt"), Path::new(r"\\srv\share\a.txt"), true));
    assert!(!same_place(Path::new(r"C:\repo\a.txt"), Path::new(r"C:\repo\b.txt"), true));
    assert!(!same_place(Path::new("/r/A.txt"), Path::new("/r/a.txt"), false), "case matters elsewhere");
}

#[test]
fn a_trashinfo_path_decodes_and_its_bytes_are_next_to_the_info_folder() {
    let text = trashinfo(b"/home/u/r/a b%.txt", "2026-10-28T10:00:00");
    assert_eq!(info_path(&text), Some(PathBuf::from("/home/u/r/a b%.txt")));
    assert_eq!(info_path("[Trash Info]\nPath=rel/a.txt\n"), None, "a relative path is the drive's, not handled here");
    assert_eq!(info_path("[Trash Info]\n"), None);
    let info = Path::new("/t").join("info").join("a.txt.2.trashinfo");
    assert_eq!(bytes_of_info(&info), Path::new("/t").join("files").join("a.txt.2"));
}

#[test]
fn a_home_trash_folder_is_listed_from_its_info_files() {
    let t = temp("list");
    std::fs::create_dir_all(t.join("info")).unwrap();
    std::fs::create_dir_all(t.join("files")).unwrap();
    std::fs::write(t.join("info").join("a.txt.trashinfo"), trashinfo(b"/r/a.txt", "2026-10-28T10:00:00")).unwrap();
    std::fs::write(t.join("info").join("junk.trashinfo"), "[Trash Info]\nPath=relative\n").unwrap();
    std::fs::write(t.join("info").join("notes.txt"), "not an info file").unwrap();
    let items = list_home(&t);
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].original, PathBuf::from("/r/a.txt"));
    assert_eq!(items[0].bytes, t.join("files").join("a.txt"));
    assert!(list_home(&t.join("missing")).is_empty());
    let _ = std::fs::remove_dir_all(&t);
}

#[test]
fn taking_an_item_out_moves_it_back_and_drops_its_record_but_never_replaces_anything() {
    let t = temp("take-out");
    let (bytes, info, full) = (t.join("files-a.txt"), t.join("a.txt.trashinfo"), t.join("back").join("a.txt"));
    std::fs::create_dir_all(t.join("back")).unwrap();
    std::fs::write(&bytes, "kept\n").unwrap();
    std::fs::write(&info, "[Trash Info]\n").unwrap();

    std::fs::write(&full, "someone else's\n").unwrap();
    assert!(take_out(&bytes, &info, &full).is_err(), "something at the path");
    assert_eq!(std::fs::read_to_string(&full).unwrap(), "someone else's\n");
    assert!(bytes.exists() && info.exists(), "the trash is as it was");

    std::fs::remove_file(&full).unwrap();
    take_out(&bytes, &info, &full).unwrap();
    assert_eq!(std::fs::read_to_string(&full).unwrap(), "kept\n");
    assert!(!bytes.exists() && !info.exists(), "out of the trash");
    let _ = std::fs::remove_dir_all(&t);
}

/// The real Recycle Bin / Trash: moves a scratch file there and takes it back out. Ignored so it
/// never touches the user's trash unasked; run with
/// `GLOWFLY_REAL_TRASH=1 cargo test -p glowfly-core real_trash_round_trip -- --ignored`.
#[test]
#[ignore = "uses the real Recycle Bin / Trash"]
fn real_trash_round_trip() {
    let t = temp("real");
    let full = t.join(format!("glowfly-untrash-{}.txt", std::process::id()));
    std::fs::write(&full, "round trip\n").unwrap();
    super::os_trash(&full).expect("trashed");
    assert!(!full.exists());
    let got = super::os_untrash(&full, &|p: &Path| std::fs::read(p).is_ok_and(|b| b == b"round trip\n"));
    if cfg!(target_os = "macos") {
        assert_eq!(got, super::Untrashed::Unsupported);
    } else {
        assert_eq!(got, super::Untrashed::Restored);
        assert_eq!(std::fs::read_to_string(&full).unwrap(), "round trip\n");
    }
    let _ = std::fs::remove_dir_all(&t);
}
