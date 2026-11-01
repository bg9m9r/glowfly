//! Each way loading `settings.json` can end: read cleanly, read after a failed try (a copy is
//! kept first), never read (saving waits), and damaged but moved aside or not.

use super::{LoadProblem, Settings};
use crate::settings::tests::scratch_settings;
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::time::Duration;

const NO_WAITS: [Duration; 3] = [Duration::ZERO; 3];

fn json(s: &Settings) -> String {
    serde_json::to_string(s).unwrap()
}

/// A read that fails like a Windows sharing violation `fails` times, then reads the file.
fn busy_for(fails: usize) -> impl FnMut(&Path) -> std::io::Result<Vec<u8>> {
    let mut left = fails;
    move |p| {
        if left > 0 {
            left -= 1;
            return Err(Error::new(ErrorKind::PermissionDenied, "the file is in use by another process"));
        }
        std::fs::read(p)
    }
}

fn backups(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".bak"))
        .collect();
    names.sort();
    names
}

#[test]
fn a_clean_read_needs_no_copy() {
    let text = br#"{"auto_fetch_minutes": 9}"#;
    let (dir, file) = scratch_settings("clean-read", text);
    let (s, problem) = Settings::load_with(&file, &NO_WAITS, busy_for(0), false);
    assert_eq!((s.auto_fetch_minutes, problem), (9, None));
    assert!(backups(&dir).is_empty(), "{:?}", backups(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_read_that_works_on_a_later_try_keeps_a_copy_before_any_save() {
    let text = br#"{"auto_fetch_minutes": 9}"#;
    let (dir, file) = scratch_settings("later-try", text);
    let (s, problem) = Settings::load_with(&file, &NO_WAITS, busy_for(2), false);
    assert_eq!((s.auto_fetch_minutes, problem), (9, None), "read on the third try");
    // The copy is there before anything overwrites the file.
    assert_eq!(std::fs::read(dir.join("settings.json.bak")).unwrap(), text);
    Settings::default().save_to(&file).unwrap();
    assert_eq!(std::fs::read(dir.join("settings.json.bak")).unwrap(), text, "the save left the copy alone");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_that_never_reads_is_left_alone_and_locks_saving() {
    let text = br#"{"auto_fetch_minutes": 9}"#;
    let (dir, file) = scratch_settings("never-reads", text);
    let mut tries = 0;
    let read = |_: &Path| {
        tries += 1;
        Err(Error::new(ErrorKind::PermissionDenied, "access is denied"))
    };
    let (s, problem) = Settings::load_with(&file, &NO_WAITS, read, false);
    assert_eq!(tries, 4, "the first try and one after each wait");
    assert_eq!(json(&s), json(&Settings { welcome_done: true, ..Settings::default() }), "an existing user's defaults");
    let problem = problem.expect("a problem to report");
    assert_eq!(problem.locked.as_deref(), Some(file.as_path()));
    assert_eq!(problem.backup, None);
    assert!(problem.detail.contains("access is denied"), "{}", problem.detail);
    assert!(problem.message().contains("nothing is saved"), "{}", problem.message());
    assert_eq!(std::fs::read(&file).unwrap(), text, "the file is untouched");
    assert!(backups(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_reload_after_a_locked_load_keeps_a_copy_first() {
    let text = br#"{"auto_fetch_minutes": 9}"#;
    let (dir, file) = scratch_settings("reload-copy", text);
    let (s, problem) = Settings::load_with(&file, &NO_WAITS, busy_for(0), true);
    assert_eq!((s.auto_fetch_minutes, problem), (9, None));
    assert_eq!(std::fs::read(dir.join("settings.json.bak")).unwrap(), text);
    // A second failure later keeps another copy rather than replacing the first.
    let (_, problem) = Settings::load_with(&file, &NO_WAITS, busy_for(1), false);
    assert_eq!(problem, None);
    assert_eq!(backups(&dir), ["settings.json.bak", "settings.json.bak-2"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_read_that_works_but_cant_be_copied_locks_saving() {
    let text = br#"{"auto_fetch_minutes": 9}"#;
    let (dir, _) = scratch_settings("no-copy", text);
    // A file stands where the backup's folder would be, so the copy can't be written.
    std::fs::write(dir.join("not-a-folder"), b"").unwrap();
    let file = dir.join("not-a-folder").join("settings.json");
    let (s, problem) = Settings::load_with(&file, &NO_WAITS, |_| Ok(text.to_vec()), true);
    assert_eq!(json(&s), json(&Settings { welcome_done: true, ..Settings::default() }), "not the settings read");
    let problem = problem.expect("a problem to report");
    assert_eq!(problem.locked.as_deref(), Some(file.as_path()));
    assert!(problem.detail.contains("could not back it up"), "{}", problem.detail);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_damaged_file_is_moved_aside_and_saving_may_go_on() {
    let torn = br#"{"auto_fetch_minutes": 5, "conn"#;
    let (dir, file) = scratch_settings("damaged-moved", torn);
    let (_, problem) = Settings::load_with(&file, &NO_WAITS, busy_for(0), false);
    let problem = problem.expect("a problem to report");
    assert_eq!(problem.locked, None, "the original is safe in the backup");
    assert_eq!(problem.backup.as_deref(), Some(dir.join("settings.json.bak").as_path()));
    assert!(!file.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_damaged_file_that_cant_be_moved_locks_saving() {
    let torn = br#"{"auto_fetch_minutes": 5, "conn"#;
    let (dir, _) = scratch_settings("damaged-stuck", torn);
    // Read from a path whose folder doesn't exist: the bytes come from the read, but the
    // rename that would move the file aside fails.
    let gone = dir.join("missing-folder").join("settings.json");
    let (s, problem) = Settings::load_with(&gone, &NO_WAITS, |_| Ok(torn.to_vec()), false);
    assert_eq!(json(&s), json(&Settings { welcome_done: true, ..Settings::default() }));
    let problem = problem.expect("a problem to report");
    assert_eq!(problem.locked.as_deref(), Some(gone.as_path()));
    assert_eq!(problem.backup, None);
    assert!(problem.detail.contains("could not back it up"), "{}", problem.detail);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_locked_problem_says_saving_waits() {
    let problem = LoadProblem { backup: None, reset: None, detail: "busy".into(), locked: Some("settings.json".into()) };
    assert_eq!(problem.message(), "Settings could not be read; defaults are used and nothing is saved until the file can be read (busy).");
}

#[cfg(unix)]
#[test]
fn the_settings_file_and_its_copy_are_private_on_unix() {
    use std::os::unix::fs::PermissionsExt;
    let text = br#"{"auto_fetch_minutes": 9}"#;
    let (dir, file) = scratch_settings("private", text);
    // Written readable by everyone, as earlier versions did.
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    let _ = Settings::load_with(&file, &NO_WAITS, busy_for(1), false);
    assert_eq!(mode(&dir.join("settings.json.bak")), 0o600, "the copy kept after a failed try");
    Settings::default().save_to(&file).unwrap();
    assert_eq!(mode(&file), 0o600);
    // A folder the save creates is this user's only too.
    let nested = dir.join("config").join("settings.json");
    Settings::default().save_to(&nested).unwrap();
    assert_eq!((mode(&nested), mode(nested.parent().unwrap())), (0o600, 0o700));
    let _ = std::fs::remove_dir_all(&dir);
}
