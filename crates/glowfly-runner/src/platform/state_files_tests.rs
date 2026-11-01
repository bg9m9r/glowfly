//! Tests of the recent-repositories and open-tabs files: their format, lines that aren't valid
//! UTF-8, reads that fail, and (on Unix) who may read them.

use super::*;

/// A fresh folder for one test, named for it so tests running at once don't share files.
fn scratch(name: &str) -> PathBuf {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-state-files-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn tabs_and_recent_files_round_trip_and_read_the_old_format() {
    let paths = vec![PathBuf::from("/w/a"), PathBuf::from("/w/two\nlines"), PathBuf::from(r"C:\w\b")];
    let none = vec![None; 3];
    assert_eq!(parse_tabs(&format_tabs(&paths, &none, 1)), (paths.clone(), none.clone(), 1));
    assert_eq!(parse_recent(&format_recent(&paths)), paths);
    // Written by earlier versions: plain lines, Windows line endings, an index past the end.
    let old = parse_tabs("5\r\nC:/old/a/\r\n\r\nC:/old/b\r\n");
    assert_eq!(old, (vec![PathBuf::from("C:/old/a/"), PathBuf::from("C:/old/b")], vec![None, None], 1));
    assert_eq!(parse_recent("/old/a\n/old/b\n"), [PathBuf::from("/old/a"), PathBuf::from("/old/b")]);
    assert_eq!(parse_tabs(""), (Vec::new(), Vec::new(), 0));
}

#[test]
fn tabs_file_keeps_labels() {
    let paths = vec![PathBuf::from("/w/web"), PathBuf::from("/w/api")];
    let labels = vec![Some("Web app".to_owned()), None];
    assert_eq!(parse_tabs(&format_tabs(&paths, &labels, 0)), (paths.clone(), labels.clone(), 0));
    // A path holding a tab or a newline is encoded, so the label still splits off right.
    let odd = vec![PathBuf::from("/w/a\tb"), PathBuf::from("/w/two\nlines")];
    let labels = vec![Some("Tabbed".to_owned()), Some("Two".to_owned())];
    assert_eq!(parse_tabs(&format_tabs(&odd, &labels, 1)), (odd, labels, 1));
    // Labels with control characters (written by hand) are cleaned; an empty one is none.
    let (_, labels, _) = parse_tabs("0\n/w/a\t  Front\u{7}end \n/w/b\t \n/w/c\t\t");
    assert_eq!(labels, [Some("Frontend".to_owned()), None, None]);
    // Fewer labels than paths: the rest have none.
    assert_eq!(parse_tabs(&format_tabs(&paths, &[], 0)).1, [None, None]);
}

#[test]
fn saves_to_a_named_file_round_trip() {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-platform-saves-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let paths = vec![dir.join("web"), dir.join("api")];
    save_recent_to(&dir.join("recent.txt"), &paths).unwrap();
    assert_eq!(parse_recent(&std::fs::read_to_string(dir.join("recent.txt")).unwrap()), paths);
    let labels = vec![None, Some("API".to_owned())];
    save_tabs_to(&dir.join("tabs.txt"), &paths, &labels, 1).unwrap();
    assert_eq!(parse_tabs(&std::fs::read_to_string(dir.join("tabs.txt")).unwrap()), (paths, labels, 1));
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn names_that_are_not_utf8_survive_the_files() {
    use std::os::unix::ffi::OsStrExt;
    let p = PathBuf::from(std::ffi::OsStr::from_bytes(b"/w/caf\xe9"));
    assert_eq!(parse_tabs(&format_tabs(std::slice::from_ref(&p), &[], 0)).0, std::slice::from_ref(&p));
    assert_eq!(parse_recent(&format_recent(std::slice::from_ref(&p))), [p]);
}

#[test]
fn a_line_that_is_not_utf8_costs_only_that_line() {
    let recent = parse_recent_bytes(b"/w/a\n/w/caf\xe9\r\n/w/b\n");
    assert_eq!(recent, [PathBuf::from("/w/a"), PathBuf::from("/w/b")]);
    // A bad path drops its tab; a bad byte in a label only loses that character.
    let (paths, labels, active) = parse_tabs_bytes(b"2\n/w/a\tFront\xffend\n/w/\xc3(\tGone\n/w/c\n");
    assert_eq!(paths, [PathBuf::from("/w/a"), PathBuf::from("/w/c")]);
    assert_eq!(labels, [Some("Front\u{fffd}end".to_owned()), None]);
    assert_eq!(active, 1, "clamped to the tabs that were kept");
    // Through the file: the good lines come back.
    let dir = scratch("bad-utf8");
    std::fs::write(dir.join("recent.txt"), b"/w/a\n\xfe\xfe\n/w/b").unwrap();
    std::fs::write(dir.join("tabs.txt"), b"0\n/w/a\n\x80\n").unwrap();
    let read = |p: &Path| std::fs::read(p);
    assert_eq!(load_recent_with(&dir.join("recent.txt"), &[], read), [PathBuf::from("/w/a"), PathBuf::from("/w/b")]);
    assert_eq!(load_tabs_with(&dir.join("tabs.txt"), &[], read).0, [PathBuf::from("/w/a")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_read_keeps_the_list_from_the_next_save() {
    let dir = scratch("failed-read");
    let (recent, tabs) = (dir.join("recent.txt"), dir.join("tabs.txt"));
    let kept = vec![dir.join("one"), dir.join("two"), dir.join("three")];
    save_recent_to(&recent, &kept).unwrap();
    save_tabs_to(&tabs, &kept, &[], 2).unwrap();
    let denied = |_: &Path| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    assert!(load_recent_with(&recent, &[], denied).is_empty());
    assert!(load_tabs_with(&tabs, &[], denied).0.is_empty());
    // This session's saves leave the files as they were, for the next start to restore.
    save_recent_to(&recent, &[dir.join("new")]).unwrap();
    save_tabs_to(&tabs, &[dir.join("new")], &[], 0).unwrap();
    assert_eq!(parse_recent_bytes(&std::fs::read(&recent).unwrap()), kept);
    assert_eq!(parse_tabs_bytes(&std::fs::read(&tabs).unwrap()), (kept, vec![None; 3], 2));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_read_that_fails_then_works_is_used_and_saves_go_on() {
    let dir = scratch("retried-read");
    let recent = dir.join("recent.txt");
    save_recent_to(&recent, &[dir.join("one")]).unwrap();
    let mut tries = 0;
    let busy_once = |p: &Path| {
        tries += 1;
        if tries == 1 { Err(std::io::Error::other("held by another program")) } else { std::fs::read(p) }
    };
    assert_eq!(load_recent_with(&recent, &[Duration::from_millis(1)], busy_once), [dir.join("one")]);
    save_recent_to(&recent, &[dir.join("two"), dir.join("one")]).unwrap();
    assert_eq!(parse_recent_bytes(&std::fs::read(&recent).unwrap()), [dir.join("two"), dir.join("one")]);
    // No file yet is no failure: the first save writes it.
    let missing = dir.join("tabs.txt");
    assert_eq!(load_tabs_with(&missing, &[], |p: &Path| std::fs::read(p)), (Vec::new(), Vec::new(), 0));
    save_tabs_to(&missing, &[dir.join("one")], &[], 0).unwrap();
    assert!(missing.is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn the_files_are_private_on_unix() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("private");
    let folder = dir.join("config");
    let (recent, tabs) = (folder.join("recent.txt"), folder.join("tabs.txt"));
    save_recent_to(&recent, &[dir.join("one")]).unwrap();
    // A file an earlier version left readable by everyone becomes private on the next save.
    std::fs::write(&tabs, b"0\n").unwrap();
    std::fs::set_permissions(&tabs, std::fs::Permissions::from_mode(0o644)).unwrap();
    save_tabs_to(&tabs, &[dir.join("one")], &[], 0).unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!((mode(&recent), mode(&tabs), mode(&folder)), (0o600, 0o600, 0o700));
    let _ = std::fs::remove_dir_all(&dir);
}
