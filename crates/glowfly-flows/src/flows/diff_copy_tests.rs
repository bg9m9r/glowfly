//! Copy hunk as patch through the runner: the hunk read again from the repository keeps a CRLF
//! file's line endings; a Latin-1 file's hunk is refused with a toast, as the clipboard takes
//! only text; a request for a diff that changed since is dropped.

use super::helpers::finish_keeping;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use glowfly_core::repo::DiffTarget;
use glowfly_model::effect::{Effect, UiEffect};
use glowfly_model::msg::{DiffMsg, Msg, RepoMsg};
use glowfly_model::state::AppState;
use std::path::PathBuf;
use std::time::Instant;

/// `f.txt` committed as `base` byte for byte, changed to `now`, its unstaged diff open.
fn open(name: &str, base: &[u8], now: &[u8]) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    // Stored as written: Git for Windows' own config turns autocrlf on.
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join("f.txt"), base).unwrap();
    git(&dir, &["add", "f.txt"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    std::fs::write(dir.join("f.txt"), now).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("f.txt".into()))));
    assert!(st.session.diff.view.is_some(), "the diff is open");
    (st, dir)
}

/// Copy hunk as patch on hunk 0, run to the end; the text copied, if any.
fn copy_hunk(st: &mut AppState) -> Option<String> {
    let generation = st.hunk_ops_gen().expect("the diff on screen");
    let fx = update(st, Msg::Diff(DiffMsg::CopyHunkPatch { index: 0, generation }), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::CopyHunkPatch(_))), "read from the repository");
    finish_keeping(st, fx).into_iter().find_map(|e| if let Effect::Ui(UiEffect::CopyText(t)) = e { Some(t) } else { None })
}

#[test]
fn a_crlf_files_hunk_is_copied_with_its_line_endings() {
    let (mut st, dir) = open("diff-copy-crlf", b"a\r\nb\r\nc\r\n", b"a\r\nB\r\nc\r\n");
    let text = copy_hunk(&mut st).expect("copied");
    assert!(text.ends_with("@@ -1,3 +1,3 @@\n a\r\n-b\r\n+B\r\n c\r\n"), "{text:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_latin1_files_hunk_is_not_copied() {
    let (mut st, dir) = open("diff-copy-latin1", b"caf\xe9\nb\n", b"caf\xe9\nB\n");
    assert_eq!(copy_hunk(&mut st), None);
    assert!(st.toasts.iter().any(|t| t.text.contains("UTF-8")), "says why");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_copy_for_a_diff_that_changed_is_dropped() {
    let (mut st, dir) = open("diff-copy-stale", b"a\nb\n", b"a\nB\n");
    let generation = st.hunk_ops_gen().expect("the diff on screen");
    let fx = update(&mut st, Msg::Diff(DiffMsg::CopyHunkPatch { index: 0, generation: generation + 1 }), Instant::now());
    assert!(fx.is_empty(), "nothing read");
    let _ = std::fs::remove_dir_all(&dir);
}
