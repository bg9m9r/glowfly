//! Streamed downloads from a local feed folder: progress, the temporary file beside the final
//! name, Cancel and a checksum mismatch removing it, and checking a saved file again.

use super::feed_tests::{MSI, is_empty, offer_for, scratch, sum_line, write_feed};
use super::*;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

/// A file of several chunks, so a download reports progress more than once.
fn large() -> Vec<u8> {
    (0..300_000u32).map(|i| (i % 251) as u8).collect()
}

/// A feed folder in `dir` publishing `bytes` as the MSI, with `sum_of`'s checksum; and the
/// download folder beside it.
fn feed_with(dir: &Path, bytes: &[u8], sum_of: &[u8]) -> (Feed, PathBuf) {
    let feed_dir = dir.join("feed");
    std::fs::create_dir_all(&feed_dir).unwrap();
    write_feed(&feed_dir, "v0.1.1", &[(MSI, bytes)], Some(sum_line(MSI, sum_of)));
    (Feed::Folder(feed_dir), dir.join("dl"))
}

fn names(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default()
}

#[test]
fn a_streamed_download_reports_progress_and_is_verified() {
    let dir = scratch("stream");
    let bytes = large();
    let (feed, dl) = feed_with(&dir, &bytes, &bytes);
    let o = offer_for(&feed, InstallKind::Installer);
    let seen = RefCell::new(Vec::new());
    let mut progress = |p: Progress| {
        // While it streams, only a temporary file is in the folder, never one under the final name.
        let now = names(&dl);
        assert!(now.len() == 1 && now[0].starts_with('.') && now[0] != MSI, "{now:?}");
        seen.borrow_mut().push(p);
    };
    let saved = download_streamed(&feed, &o, &dl, &mut progress, &|| false).unwrap().expect("not cancelled");
    assert_eq!(saved, Saved { path: dl.join(MSI), sha256: sha256(&bytes) });
    assert_eq!(std::fs::read(&saved.path).unwrap(), bytes);
    assert_eq!(names(&dl), [MSI], "the temporary file was renamed into place");
    let seen = seen.into_inner();
    let total = Some(bytes.len() as u64);
    assert!(seen.len() >= 3, "{} reports", seen.len());
    assert!(seen.windows(2).all(|w| w[0].done < w[1].done) && seen.iter().all(|p| p.total == total));
    assert_eq!(seen.last(), Some(&Progress { done: bytes.len() as u64, total }));
    assert_eq!(seen.last().and_then(Progress::fraction), Some(1.0));
    assert_eq!(Progress { done: 5, total: None }.fraction(), None);
    // Asked again, the matching file is used where it is, with nothing downloaded.
    let mut again = 0;
    let found = download_streamed(&feed, &o, &dl, &mut |_| again += 1, &|| false).unwrap();
    assert_eq!((found, again), (Some(saved), 0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancel_removes_the_partial_file() {
    let dir = scratch("cancel");
    let bytes = large();
    let (feed, dl) = feed_with(&dir, &bytes, &bytes);
    let o = offer_for(&feed, InstallKind::Installer);
    let stop = Cell::new(false);
    let mut progress = |p: Progress| {
        assert!(p.done < bytes.len() as u64, "stopped before the end");
        stop.set(true);
    };
    assert_eq!(download_streamed(&feed, &o, &dl, &mut progress, &|| stop.get()), Ok(None));
    assert!(is_empty(&dl), "no file and no temporary file: {:?}", names(&dl));
    // Cancelled before it starts: nothing is fetched or written.
    assert_eq!(download_streamed(&feed, &o, &dl, &mut |_| panic!("no progress"), &|| true), Ok(None));
    assert!(is_empty(&dl));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_mismatch_removes_the_partial_file_and_says_so() {
    let dir = scratch("mismatch");
    let bytes = large();
    let (feed, dl) = feed_with(&dir, &bytes, b"what the release published");
    let o = offer_for(&feed, InstallKind::Installer);
    let mut reports = 0;
    let err = download_streamed(&feed, &o, &dl, &mut |_| reports += 1, &|| false).unwrap_err();
    assert_eq!(err, "The download didn't match its published checksum and was discarded.");
    assert!(reports >= 3, "it streamed first");
    assert!(is_empty(&dl), "no file and no temporary file: {:?}", names(&dl));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_saved_file_is_checked_again() {
    let dir = scratch("saved");
    let bytes = large();
    let (feed, dl) = feed_with(&dir, &bytes, &bytes);
    let saved = download_saved(&feed, &offer_for(&feed, InstallKind::Installer), &dl).unwrap();
    assert_eq!(check_saved(&saved.path, &saved.sha256), None);
    std::fs::write(&saved.path, b"changed").unwrap();
    assert_eq!(check_saved(&saved.path, &saved.sha256), Some(SavedProblem::Changed));
    std::fs::remove_file(&saved.path).unwrap();
    assert_eq!(check_saved(&saved.path, &saved.sha256), Some(SavedProblem::Missing));
    std::fs::create_dir(&saved.path).unwrap();
    assert_eq!(check_saved(&saved.path, &saved.sha256), Some(SavedProblem::Missing), "a folder in its place");
    let _ = std::fs::remove_dir_all(&dir);
}
