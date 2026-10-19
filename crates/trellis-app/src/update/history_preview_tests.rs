//! Headless tests for the File view's previews: the remembered Text / Preview choice, the
//! Search key doing nothing over a preview, when a view counts as previewing, and the runner
//! reading Markdown, images, empty and broken images and Git LFS pointers end to end.

use super::history_tests::{oid, with_view};
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{HistoryMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::history::{FileText, FileTextData, HistoryPane, Preview, TextState};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use trellis_core::markdown::MARKDOWN_MAX_BYTES;
use trellis_core::repo::LfsState;

fn send(st: &mut AppState, m: HistoryMsg) -> Vec<Effect> {
    update(st, Msg::History(m), Instant::now())
}

fn saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SaveSettings(_))).count()
}

#[test]
fn the_choice_is_remembered_and_saved_once() {
    let (mut st, dir) = with_view("history-preview-choice");
    assert!(!st.settings.history.preview);
    let fx = send(&mut st, HistoryMsg::SetFilePreview(true));
    assert_eq!((saves(&fx), fx.len()), (1, 1), "{fx:?}");
    assert!(st.settings.history.preview);
    assert!(st.history.as_ref().unwrap().nav.to_top, "the preview starts at the top");
    assert_eq!(saves(&send(&mut st, HistoryMsg::SetFilePreview(true))), 0, "unchanged: nothing to save");
    assert_eq!(saves(&send(&mut st, HistoryMsg::SetFilePreview(false))), 1);
    assert!(!st.settings.history.preview);
    st.history = None;
    assert!(send(&mut st, HistoryMsg::SetFilePreview(true)).is_empty());
    assert!(!st.settings.history.preview, "no history open: nothing changes");
    let _ = std::fs::remove_dir_all(&dir);
}

fn markdown_data() -> FileTextData {
    let preview = Some(Preview::Markdown { source: "# Hi".into() });
    FileTextData { lines: vec!["# Hi".into()], spans: None, lfs: LfsState::None, preview }
}

fn image_data() -> FileTextData {
    let mut png = Vec::new();
    image::RgbaImage::new(2, 2).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let image = trellis_core::preview_image::decode(&png).unwrap();
    let preview = Some(Preview::Image { image, size: png.len() as u64 });
    FileTextData { lines: Vec::new(), spans: None, lfs: LfsState::None, preview }
}

/// `with_view` in the File pane, entry 0's text arrived as `data`.
fn shown(name: &str, data: FileTextData) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_view(name);
    send(&mut st, HistoryMsg::SelectHistoryEntry(0));
    send(&mut st, HistoryMsg::SetHistoryPane(HistoryPane::File));
    send(&mut st, HistoryMsg::FileTextLoaded { oid: oid(1), path: "a.txt".into(), before: false, result: Ok(Arc::new(data)) });
    (st, dir)
}

fn focuses(st: &mut AppState) -> bool {
    st.history.as_mut().unwrap().nav.focus_find = false;
    send(st, HistoryMsg::FocusFileFind);
    st.history.as_ref().unwrap().nav.focus_find
}

#[test]
fn the_search_key_does_nothing_over_a_preview() {
    let (mut st, dir) = shown("history-preview-find-md", markdown_data());
    st.settings.history.preview = true;
    assert!(!focuses(&mut st), "the field is hidden under the preview");
    st.settings.history.preview = false;
    assert!(focuses(&mut st), "the text view has it");
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = shown("history-preview-find-img", image_data());
    assert!(!focuses(&mut st), "an image is always previewed");
    let _ = std::fs::remove_dir_all(&dir);

    let plain = FileTextData { lines: vec!["x".into()], spans: None, lfs: LfsState::None, preview: None };
    let (mut st, dir) = shown("history-preview-find-txt", plain);
    st.settings.history.preview = true;
    assert!(focuses(&mut st), "plain text has no preview");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn previewing_needs_the_file_pane_and_the_shown_text() {
    let (mut st, dir) = shown("history-preview-truth", markdown_data());
    let view = st.history.as_mut().unwrap();
    assert!(view.previewing(true) && !view.previewing(false));
    view.pane = HistoryPane::Diff;
    assert!(!view.previewing(true), "the Diff pane");
    view.pane = HistoryPane::File;
    let ready = view.text.clone();
    view.text.as_mut().unwrap().state = TextState::Loading;
    assert!(!view.previewing(true), "still loading");
    view.text = Some(FileText { oid: oid(2), ..ready.clone().unwrap() });
    assert!(!view.previewing(true), "text of another commit");
    view.text = ready;
    assert!(view.previewing(true));
    let _ = std::fs::remove_dir_all(&dir);
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    image::RgbaImage::new(w, h).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

fn commit(dir: &Path, files: &[(&str, &[u8])]) {
    for (name, bytes) in files {
        std::fs::write(dir.join(name), bytes).unwrap();
        git(dir, &["add", name]);
    }
    git(dir, &["commit", "-q", "-m", "add"]);
}

/// The File view's text of `path` at HEAD, read through the runner.
fn read(st: &mut AppState, path: &str) -> TextState {
    drive(st, Msg::History(HistoryMsg::OpenFileHistory { path: path.into(), at: None }));
    drive(st, Msg::History(HistoryMsg::SetHistoryPane(HistoryPane::File)));
    st.history.as_ref().and_then(|v| v.text.as_ref()).map(|t| t.state.clone()).expect("text asked for")
}

fn ready(state: TextState) -> Arc<FileTextData> {
    match state {
        TextState::Ready(d) => d,
        other => panic!("not ready: {other:?}"),
    }
}

#[test]
fn markdown_and_images_end_to_end() {
    let dir = scratch_repo("history-preview-e2e");
    let logo_png = png(40, 30);
    commit(&dir, &[("README.md", b"# Title\n\nSome *text*\n"), ("logo.png", &logo_png), ("empty.png", b""), ("bad.png", b"not an image")]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));

    let md = ready(read(&mut st, "README.md"));
    assert_eq!(md.lines, ["# Title", "", "Some *text*"]);
    let Some(Preview::Markdown { source }) = &md.preview else { panic!("no Markdown preview: {:?}", md.preview) };
    assert_eq!(source, "# Title\n\nSome *text*", "the lines joined, parsed only when drawn");

    let logo = ready(read(&mut st, "logo.png"));
    assert!(logo.lines.is_empty());
    let Some(Preview::Image { image, size }) = &logo.preview else { panic!("no image preview: {:?}", logo.preview) };
    assert_eq!((image.width, image.height, image.format), (40, 30, "PNG"));
    assert_eq!(*size, logo_png.len() as u64);

    let empty = ready(read(&mut st, "empty.png"));
    assert!(empty.lines.is_empty() && empty.preview.is_none());

    match read(&mut st, "bad.png") {
        TextState::Failed(e) => {
            assert!(e.message.starts_with("bad.png could not be shown as an image: "), "{e:?}");
            assert!(!e.retry, "a broken image stays broken");
        }
        other => panic!("expected a failure, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_long_markdown_file_has_no_preview() {
    let dir = scratch_repo("history-preview-long");
    let line = format!("{}\n", "a".repeat(99));
    let text = line.repeat(MARKDOWN_MAX_BYTES / line.len() + 1);
    assert!(text.len() - 1 > MARKDOWN_MAX_BYTES, "longer than the limit once its lines are joined");
    commit(&dir, &[("README.md", text.as_bytes())]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let d = ready(read(&mut st, "README.md"));
    assert_eq!(d.lines.len(), text.lines().count());
    assert!(d.preview.is_none(), "too long to render: the text view only");
    let _ = std::fs::remove_dir_all(&dir);
}

const POINTER: &str =
    "version https://git-lfs.github.com/spec/v1\noid sha256:4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393\nsize 18204\n";

/// `name` committed as a Git LFS pointer in a repository whose git is held (untrusted), read
/// through the runner.
fn untrusted_pointer(slug: &str, name: &str) -> Arc<FileTextData> {
    let dir = scratch_repo(slug);
    commit(&dir, &[(name, POINTER.as_bytes())]);
    // A config key that runs a program: git is held until the user decides.
    git(&dir, &["config", "gpg.program", "evil"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.held());
    let d = ready(read(&mut st, name));
    let _ = std::fs::remove_dir_all(&dir);
    d
}

#[test]
fn an_untrusted_image_pointer_shows_its_text() {
    let d = untrusted_pointer("history-preview-lfs", "pic.png");
    assert_eq!(d.lfs, LfsState::Pointer);
    assert_eq!(d.lines.len(), 3);
    assert!(d.lines[0].starts_with("version https://git-lfs"));
    assert!(d.preview.is_none());
}

#[test]
fn an_untrusted_markdown_pointer_has_no_preview() {
    let d = untrusted_pointer("history-preview-lfs-md", "README.md");
    assert_eq!(d.lfs, LfsState::Pointer);
    assert_eq!(d.lines.len(), 3);
    assert!(d.preview.is_none(), "a pointer's text is not the document");
}

#[test]
fn the_switch_needs_nothing_for_an_inactive_tab() {
    assert!(HistoryMsg::SetFilePreview(true).on_inactive().is_none());
}
