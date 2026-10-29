//! Old programs left by a swap: which names count, that only regular files beside the program
//! are found, that only a recorded backup that still matches its record is removed (a
//! hand-made one is kept), and that one a process runs (or when the process list can't be
//! read) is kept.

use super::*;
use crate::persist;

/// A backup the swap made: written, then recorded.
fn swapped(path: &Path, bytes: &str) {
    std::fs::write(path, bytes).unwrap();
    record(path).unwrap();
}

fn scratch(tag: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-backups-{tag}-{}", std::process::id()));
    let _ = persist::remove_tree(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn backup_names() {
    for name in ["glowfly.exe.bak", "glowfly.exe.bak-2", "glowfly.exe.bak-10"] {
        assert!(is_backup_name("glowfly.exe", name), "{name}");
    }
    for name in [
        "glowfly.exe",
        "glowfly.exe.bak-1",
        "glowfly.exe.bak-02",
        "glowfly.exe.bak-",
        "glowfly.exe.bak-x",
        "glowfly.exe.bak.bak",
        "glowfly.exe.backup",
        "other.exe.bak",
        "xglowfly.exe.bak",
        "glowfly.bak",
    ] {
        assert!(!is_backup_name("glowfly.exe", name), "{name}");
    }
}

#[test]
fn only_regular_files_beside_the_program_are_backups() {
    let dir = scratch("found");
    let exe = dir.join("glowfly");
    std::fs::write(&exe, "new program").unwrap();
    swapped(&dir.join("glowfly.bak"), "old");
    swapped(&dir.join("glowfly.bak-2"), "older");
    std::fs::write(dir.join("notes.bak"), "someone else's").unwrap();
    std::fs::create_dir(dir.join("glowfly.bak-3")).unwrap();
    let elsewhere = scratch("found-target");
    std::fs::write(elsewhere.join("target"), "keep me").unwrap();
    // A link with a backup's name isn't one (and its target is never touched).
    let linked = make_link(&elsewhere.join("target"), &dir.join("glowfly.bak-4"));
    assert_eq!(backups_of(&exe), vec![dir.join("glowfly.bak"), dir.join("glowfly.bak-2")]);
    let cleared = remove_unused(std::slice::from_ref(&exe), || Some(Vec::new()));
    assert_eq!(cleared.removed, vec![dir.join("glowfly.bak"), dir.join("glowfly.bak-2")]);
    assert!(!dir.join("glowfly.bak.record").exists() && !dir.join("glowfly.bak-2.record").exists(), "the records go too");
    assert!(exe.is_file() && dir.join("notes.bak").is_file() && dir.join("glowfly.bak-3").is_dir());
    assert_eq!(std::fs::read(elsewhere.join("target")).unwrap(), b"keep me");
    if linked {
        assert!(dir.join("glowfly.bak-4").symlink_metadata().is_ok(), "the link stays");
    }
    let _ = persist::remove_tree(&dir);
    let _ = persist::remove_tree(&elsewhere);
}

fn make_link(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        // Needs developer mode or a privilege; without it there is no link to test.
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
}

#[test]
fn a_backup_still_running_or_an_unreadable_list_keeps_it() {
    let dir = scratch("kept");
    let exe = dir.join("glowfly.exe");
    let bak = dir.join("glowfly.exe.bak");
    std::fs::write(&exe, "new").unwrap();
    swapped(&bak, "old");
    let programs = std::slice::from_ref(&exe);
    let cleared = remove_unused(programs, || None);
    assert_eq!((cleared.running, cleared.removed), (vec![bak.clone()], Vec::new()));
    let cleared = remove_unused(programs, || Some(vec![exe.clone(), bak.clone()]));
    assert_eq!((cleared.running, cleared.removed), (vec![bak.clone()], Vec::new()));
    assert!(bak.is_file());
    // Nothing runs it now.
    let cleared = remove_unused(programs, || Some(vec![exe.clone()]));
    assert_eq!(cleared.removed, vec![bak.clone()]);
    assert!(!bak.exists() && exe.is_file());
    // Nothing to remove: the process list isn't even asked for.
    let cleared = remove_unused(programs, || panic!("not needed"));
    assert_eq!(cleared, Cleared::default());
    let _ = persist::remove_tree(&dir);
}

/// A real process started from a copy of a system program, renamed to a backup's name while it
/// runs (as the swap does), is seen running from the backup.
#[test]
fn the_process_list_sees_a_renamed_running_program() {
    #[cfg(windows)]
    let (source, args, name) = {
        let root = std::env::var_os("SystemRoot").map(PathBuf::from).expect("SystemRoot");
        (root.join("System32").join("PING.EXE"), ["-n", "30", "127.0.0.1"], "glowfly.exe")
    };
    #[cfg(unix)]
    let (source, args, name) =
        (PathBuf::from(if Path::new("/bin/sleep").exists() { "/bin/sleep" } else { "/usr/bin/sleep" }), ["30"], "glowfly");
    let dir = scratch("live");
    let exe = dir.join(name);
    std::fs::copy(&source, &exe).unwrap();
    let mut child = std::process::Command::new(&exe)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let bak = persist::move_aside(&exe).unwrap();
    record(&bak).unwrap();
    std::fs::write(&exe, "new program").unwrap();
    let list = crate::terminal::program_paths();
    let cleared = remove_unused(std::slice::from_ref(&exe), || list.clone());
    let _ = child.kill();
    let _ = child.wait();
    if list.is_some() {
        assert_eq!(
            cleared.running,
            vec![bak.clone()],
            "{:?}",
            list.map(|l| l.into_iter().filter(|p| p.starts_with(&dir)).collect::<Vec<_>>())
        );
        assert!(bak.is_file());
    }
    // Once it has exited, the backup goes.
    let cleared = remove_unused(std::slice::from_ref(&exe), crate::terminal::program_paths);
    if crate::terminal::program_paths().is_some() {
        assert_eq!(cleared.removed, vec![bak.clone()]);
    }
    let _ = persist::remove_tree(&dir);
}

#[test]
fn a_hand_made_glowfly_bak_is_kept() {
    let dir = scratch("handmade");
    let exe = dir.join("glowfly.exe");
    std::fs::write(&exe, "new").unwrap();
    // The user's own copy: Glowfly's backup name, but no record.
    let bak = dir.join("glowfly.exe.bak");
    std::fs::write(&bak, "my copy").unwrap();
    let cleared = remove_unused(std::slice::from_ref(&exe), || Some(Vec::new()));
    assert_eq!((cleared.unrecorded, cleared.removed), (vec![bak.clone()], Vec::new()));
    assert_eq!(std::fs::read(&bak).unwrap(), b"my copy");
    let _ = persist::remove_tree(&dir);
}

#[test]
fn a_backup_changed_since_it_was_recorded_is_kept() {
    let dir = scratch("changed");
    let exe = dir.join("glowfly");
    std::fs::write(&exe, "new").unwrap();
    let (same_size, other_size, bad) = (dir.join("glowfly.bak"), dir.join("glowfly.bak-2"), dir.join("glowfly.bak-3"));
    for b in [&same_size, &other_size, &bad] {
        swapped(b, "old program");
    }
    // The user put another file there since: same size, other bytes; another size.
    std::fs::write(&same_size, "OLD PROGRAM").unwrap();
    std::fs::write(&other_size, "a longer file of the user's").unwrap();
    // A record that isn't one.
    std::fs::write(record_path(&bad), "not json").unwrap();
    let cleared = remove_unused(std::slice::from_ref(&exe), || Some(Vec::new()));
    assert_eq!((cleared.unrecorded, cleared.removed), (vec![same_size.clone(), other_size.clone(), bad.clone()], Vec::new()));
    assert!(same_size.is_file() && other_size.is_file() && bad.is_file());
    let _ = persist::remove_tree(&dir);
}

#[test]
fn a_record_names_the_backup_beside_it() {
    let bak = crate::paths::temp_dir().join("app").join("glowfly.exe.bak-2");
    assert_eq!(record_path(&bak), bak.with_file_name("glowfly.exe.bak-2.record"));
    assert!(!is_backup_name("glowfly.exe", "glowfly.exe.bak.record"), "a record is no backup");
}
