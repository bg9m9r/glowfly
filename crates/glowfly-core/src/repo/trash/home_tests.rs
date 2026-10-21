//! The home Trash's pure parts and moving a file into it, on every system: where it is, the
//! `.trashinfo` text, reading the mount table, which trash folder on a drive can be used, and
//! `move_into` with a real rename or a stand-in that reports another drive.

use super::home::*;
use std::ffi::OsString;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

fn env(vars: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
    move |k: &str| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| (*v).into())
}

#[test]
fn the_home_trash_is_under_xdg_data_home_or_home() {
    assert_eq!(home_trash_dir(env(&[("XDG_DATA_HOME", "/data"), ("HOME", "/home/ada")])), Some(Path::new("/data").join("Trash")));
    let under_home = Path::new("/home/ada").join(".local").join("share").join("Trash");
    assert_eq!(home_trash_dir(env(&[("HOME", "/home/ada")])), Some(under_home.clone()));
    assert_eq!(home_trash_dir(env(&[("XDG_DATA_HOME", ""), ("HOME", "/home/ada")])), Some(under_home), "empty counts as unset");
    assert_eq!(home_trash_dir(env(&[("XDG_DATA_HOME", ""), ("HOME", "")])), None);
    assert_eq!(home_trash_dir(env(&[])), None);
}

#[test]
fn trashinfo_encodes_the_path() {
    let text = trashinfo("/home/ada/my notes/100%#ü~a-b_c.txt".as_bytes(), "2026-10-18T09:05:01");
    assert_eq!(text, "[Trash Info]\nPath=/home/ada/my%20notes/100%25%23%C3%BC~a-b_c.txt\nDeletionDate=2026-10-18T09:05:01\n");
}

#[test]
fn mount_points_and_the_mount_a_path_is_on() {
    let table = "sysfs /sys sysfs rw 0 0\n/dev/sda1 / ext4 rw 0 0\n/dev/sdb1 /mnt/data ext4 rw 0 0\n/dev/sdc1 /mnt/my\\040disk vfat rw 0 0\n\
                 /dev/sdd1 /mnt/back\\134slash ext4 rw 0 0\n/dev/sde1 /mnt/database ext4 rw 0 0\n";
    let mounts = parse_mounts(table);
    let want: Vec<PathBuf> = ["/sys", "/", "/mnt/data", "/mnt/my disk", "/mnt/back\\slash", "/mnt/database"].map(PathBuf::from).into();
    assert_eq!(mounts, want);
    let top = |p: &str| topdir_of(Path::new(p), &mounts).map(Path::to_path_buf);
    assert_eq!(top("/mnt/data/repo/a.txt"), Some(PathBuf::from("/mnt/data")));
    assert_eq!(top("/mnt/database/a.txt"), Some(PathBuf::from("/mnt/database")), "not /mnt/data");
    assert_eq!(top("/mnt/datas/a.txt"), Some(PathBuf::from("/")), "component by component");
    assert_eq!(top("/mnt/my disk/a.txt"), Some(PathBuf::from("/mnt/my disk")));
    assert_eq!(top("/home/ada/a.txt"), Some(PathBuf::from("/")));
    assert_eq!(topdir_of(Path::new("/a"), &[]), None);
}

#[test]
fn which_drive_trash_folders_can_be_used() {
    use AdminTrash as A;
    use UserTrash as U;
    let usable = |admin, user| volume_trash_usable(&VolumeTrashFacts { admin, user });
    let good_user = U::Folder { writable: true };
    assert!(usable(A::UserFolder { writable: true }, U::NotAFolder));
    assert!(!usable(A::UserFolder { writable: false }, good_user), ".Trash/uid is the only choice once it is there");
    for admin in [A::Invalid, A::Missing, A::NoUserFolder] {
        assert!(usable(admin, good_user), "{admin:?}");
        assert!(usable(admin, U::Missing { topdir_writable: true }), "{admin:?}");
        assert!(!usable(admin, U::Missing { topdir_writable: false }), "{admin:?}");
        assert!(!usable(admin, U::Folder { writable: false }), "{admin:?}");
        assert!(!usable(admin, U::NotAFolder), "{admin:?}");
    }
}

/// A new, empty folder for one test.
fn fresh(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("trellis-home-trash-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn info_text(trash: &Path, item: &str) -> String {
    std::fs::read_to_string(trash.join("info").join(format!("{item}.trashinfo"))).unwrap()
}

#[test]
fn moving_into_the_trash_names_items_uniquely() {
    let dir = fresh("move");
    let trash = dir.join("Trash");
    let src = dir.join("a b.txt");
    std::fs::write(&src, "first").unwrap();
    let went = move_into(&src, &trash, "2026-10-18T09:05:01", |a, b| std::fs::rename(a, b)).unwrap();
    assert_eq!(went, trash.join("files").join("a b.txt"));
    assert!(!src.exists());
    assert_eq!(std::fs::read_to_string(&went).unwrap(), "first");
    let info = info_text(&trash, "a b.txt");
    assert!(info.starts_with("[Trash Info]\nPath="), "{info}");
    assert!(info.contains("a%20b.txt\n") && info.ends_with("DeletionDate=2026-10-18T09:05:01\n"), "{info}");

    std::fs::write(&src, "second").unwrap();
    let went = move_into(&src, &trash, "2026-10-18T09:05:02", |a, b| std::fs::rename(a, b)).unwrap();
    assert_eq!(went, trash.join("files").join("a b.txt.2"));
    assert_eq!(std::fs::read_to_string(&went).unwrap(), "second");
    assert!(info_text(&trash, "a b.txt.2").ends_with("DeletionDate=2026-10-18T09:05:02\n"));

    std::fs::create_dir(dir.join("folder")).unwrap();
    assert_eq!(move_into(&dir.join("folder"), &trash, "d", |a, b| std::fs::rename(a, b)).unwrap_err().kind(), ErrorKind::InvalidInput);
    let _ = std::fs::remove_dir_all(&dir);
}

fn other_drive(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::from(ErrorKind::CrossesDevices))
}

fn denied(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::from(ErrorKind::PermissionDenied))
}

#[test]
fn another_drive_is_copied_and_a_failure_leaves_everything() {
    let dir = fresh("copy");
    let trash = dir.join("Trash");
    let src = dir.join("big.bin");
    let bytes: Vec<u8> = (0..=255u8).cycle().take(70_000).collect();
    std::fs::write(&src, &bytes).unwrap();
    // Windows won't remove a read-only file, and Linux, where this runs for real, doesn't mind.
    if cfg!(unix) {
        let mut p = std::fs::metadata(&src).unwrap().permissions();
        p.set_readonly(true);
        std::fs::set_permissions(&src, p).unwrap();
    }
    let went = move_into(&src, &trash, "2026-10-18T09:05:01", other_drive).unwrap();
    assert!(!src.exists(), "the original is removed after the copy");
    assert_eq!(std::fs::read(&went).unwrap(), bytes);
    assert_eq!(std::fs::metadata(&went).unwrap().permissions().readonly(), cfg!(unix), "permissions are kept");

    std::fs::write(&src, "stays").unwrap();
    assert_eq!(move_into(&src, &trash, "d", denied).unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(std::fs::read_to_string(&src).unwrap(), "stays");
    let infos: Vec<_> = std::fs::read_dir(trash.join("info")).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(infos, [OsString::from("big.bin.trashinfo")], "no info file is left for the failed move");
    assert!(!trash.join("files").join("big.bin.2").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
