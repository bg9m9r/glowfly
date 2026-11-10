//! Removing what stopped downloads left: only Glowfly's own temporary files, only regular files,
//! only once they are an hour old; at the next download too.

use super::super::download_tests::{feed_with, large};
use super::super::feed_tests::{MSI, offer_for, scratch};
use super::super::{InstallKind, download_saved};
use super::{is_download_temp, remove_stopped_downloads};
use std::path::Path;
use std::time::{Duration, SystemTime};

const HOURS_2: Duration = Duration::from_secs(2 * 60 * 60);

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn temps_of_stopped_downloads_are_removed_and_nothing_else() {
    let dir = scratch("leftovers");
    let removed = [
        ".glowfly-v0.1.0-x86_64-pc-windows-msvc.msi.999999.0.tmp",
        ".glowfly-v0.1.0-x86_64-pc-windows-msvc (2).msi.12.3.tmp",
        ".glowfly-v0.1.0-x86_64-unknown-linux-gnu.tar.gz.7.0.tmp",
    ];
    let kept = [
        ".notes.patch.5.0.tmp",
        ".glowfly-v0.1.0-x.msi.12.tmp",
        "glowfly-v0.1.0-x86_64-pc-windows-msvc.msi",
        ".glowfly-v0.1.0-x.msi.1.0.tmp.part",
    ];
    for name in removed.iter().chain(&kept) {
        std::fs::write(dir.join(name), "partial").unwrap();
    }
    let folder = ".glowfly-v0.1.0-y.msi.4.0.tmp";
    std::fs::create_dir(dir.join(folder)).unwrap();
    let all = names(&dir);
    assert_eq!(remove_stopped_downloads(&dir, SystemTime::now()), 0, "fresh ones may still be written");
    assert_eq!(names(&dir), all);
    assert_eq!(remove_stopped_downloads(&dir, SystemTime::now() + HOURS_2), 3);
    let mut left: Vec<String> = kept.iter().chain([&folder]).map(|s| s.to_string()).collect();
    left.sort();
    assert_eq!(names(&dir), left);
    assert_eq!(remove_stopped_downloads(&dir.join("missing"), SystemTime::now() + HOURS_2), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn a_link_with_a_temp_name_is_left_alone() {
    let dir = scratch("leftover-link");
    let (dl, outside) = (dir.join("dl"), dir.join("outside.msi"));
    std::fs::create_dir_all(&dl).unwrap();
    std::fs::write(&outside, "kept").unwrap();
    let link = dl.join(".glowfly-v0.1.0-x.msi.1.0.tmp");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    assert_eq!(remove_stopped_downloads(&dl, SystemTime::now() + HOURS_2), 0);
    assert!(link.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()), "the link is kept");
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "kept");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_next_download_clears_what_a_stopped_one_left() {
    let dir = scratch("leftover-next");
    let bytes = large();
    let (feed, dl) = feed_with(&dir, &bytes, &bytes);
    std::fs::create_dir_all(&dl).unwrap();
    // A download of another version that was killed, and a saved patch's write that stopped.
    let killed = ".glowfly-v0.1.0-x86_64-pc-windows-msvc.msi.424242.0.tmp";
    let patch = ".notes.patch.1.0.tmp";
    let two_hours_ago = SystemTime::now() - HOURS_2;
    for name in [killed, patch] {
        std::fs::write(dl.join(name), "partial").unwrap();
        std::fs::OpenOptions::new().write(true).open(dl.join(name)).unwrap().set_modified(two_hours_ago).unwrap();
    }
    // One still being written by another Glowfly.
    let running = ".glowfly-v0.1.0-x86_64-pc-windows-msvc.msi.424243.0.tmp";
    std::fs::write(dl.join(running), "partial").unwrap();
    let saved = download_saved(&feed, &offer_for(&feed, InstallKind::Installer), &dl).unwrap();
    assert_eq!(saved.path, dl.join(MSI));
    let mut want = vec![patch, running, MSI];
    want.sort();
    assert_eq!(names(&dl), want);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn update_file_names() {
    let temp = |name: &str| format!(".{name}.1.0.tmp");
    for name in [
        "glowfly-v0.1.0-x86_64-pc-windows-msvc.msi",
        "glowfly-v0.1.0-x86_64-pc-windows-msvc (12).msi",
        "glowfly-v0.1.0-x86_64-unknown-linux-gnu.tar.gz",
        "glowfly-v0.1.0-x86_64-unknown-linux-gnu (12).tar.gz",
    ] {
        assert!(is_download_temp(&temp(name)), "{name}");
    }
    for name in ["notes.patch", "other-v0.1.0.msi", "glowfly-a b.msi", "glowfly-x ().msi", "glowfly-x (a).msi", ".glowfly-x.msi"] {
        assert!(!is_download_temp(&temp(name)), "{name}");
    }
    assert!(!is_download_temp("glowfly-v0.1.0-x86_64-pc-windows-msvc.msi"), "a finished download");
}
