//! The history's File view previewing a Markdown file (rendered, and as text with the switch),
//! a Markdown Git LFS pointer with no preview, and images at their own size and scaled down,
//! their texture freed once no image is drawn.

use super::find_bar::FIND_HINT;
use super::history_file::{PREVIEW, TEXT, WRAP_LINES};
use super::snapshot_tests::{git, harness, harness_sized, remove_fixture, with_repo};
use super::snapshot_tests_history::{commit, head};
use crate::msg::{HistoryMsg, Msg};
use crate::state::AppState;
use crate::state::history::{FileHistoryView, FileText, FileTextData, HistoryPane, Preview, TextState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::highlight::{self, SyntaxHint};
use glowfly_core::lfs::pointer::size_text;
use glowfly_core::preview_image::{self, FILE_VIEW_SHOWN_MAX};
use glowfly_core::repo::{self, HistoryStart, LfsState, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const README: &str = "# Glowfly notes

## Getting started

Read the **guide** first, then *try* the `glowfly` command. See [the docs](https://example.com/docs) \
or [the changelog](CHANGELOG.md) for ~~older~~ newer notes.

- Install
  - Windows
    - Portable
  - macOS
- Configure

* [x] Write the parser
* [ ] Draw the preview

3. Third
4. Fourth

> A quote with *emphasis*.
>
> > And one inside it.

```rust
fn main() {
    println!(\"a long line that keeps going past the edge of the box to show its own sideways scroll bar\");
}
```

---

| Key | Value |
|-----|-------|
| a | `1` |
| b | **2** |
";

/// The history of `path`, opened on its only commit in the File view, with `data` as its text.
fn open(st: &mut AppState, dir: &Path, path: &str, at: Oid, data: FileTextData) {
    st.snap = Some(repo::load(dir, &LoadOptions::all(500)).unwrap());
    let h = repo::file_history(dir, path, &HistoryStart::Head, 500).unwrap();
    let mut view = FileHistoryView::loading(path.into(), None);
    view.append(h);
    view.batches = 1;
    view.selected = Some(0);
    view.pane = HistoryPane::File;
    view.text = Some(FileText { oid: at, path: path.into(), before: false, state: TextState::Ready(Arc::new(data)) });
    st.history = Some(view);
}

/// README.md committed and shown, its preview's text joined as the runner does.
fn with_readme(name: &str, theme: ThemeChoice, preview_on: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let at = commit(&dir, &[("README.md", README)], "Write the notes", "2026-01-11T10:00:00+00:00");
    let f = repo::file_at(&dir, at, "README.md").unwrap();
    let spans = highlight::highlight_file(SyntaxHint { path: "README.md", language: None }, &f.lines);
    let preview = Some(Preview::Markdown { source: f.lines.join("\n") });
    open(&mut st, &dir, "README.md", at, FileTextData { lines: f.lines, spans, lfs: f.lfs, preview });
    st.settings.history.preview = preview_on;
    (st, dir)
}

fn sent(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, want: impl Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

#[test]
fn file_history_markdown_preview() {
    let (st, dir) = with_readme("history-md-preview", ThemeChoice::Dark, true);
    let mut h = harness_sized(st, true, egui::vec2(1400.0, 1100.0));
    h.run();
    h.check_snapshot("file_history_markdown_preview");
    assert!(h.query_by_label(FIND_HINT).is_none(), "no find over the preview");
    assert!(h.query_by_label(WRAP_LINES).is_none());
    h.get_by_label("the docs").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::OpenUrl(u) if u == "https://example.com/docs")));
    h.state_mut().1.clear();
    h.get_by_label("the changelog").click();
    h.run();
    assert!(!sent(&h, |m| matches!(m, Msg::OpenUrl(_))), "relative links don't open");
    h.get_by_label(TEXT).click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::History(HistoryMsg::SetFilePreview(false)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_markdown_text_light() {
    let (st, dir) = with_readme("history-md-text", ThemeChoice::Light, false);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("file_history_markdown_text_light");
    assert!(h.query_by_label(FIND_HINT).is_some() && h.query_by_label(WRAP_LINES).is_some());
    h.get_by_label(PREVIEW).click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::History(HistoryMsg::SetFilePreview(true)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_markdown_lfs_pointer() {
    let (mut st, dir) = with_readme("history-md-lfs", ThemeChoice::Dark, true);
    let pointer = [
        "version https://git-lfs.github.com/spec/v1",
        "oid sha256:4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393",
        "size 18204",
    ];
    let text = st.history.as_mut().unwrap().text.as_mut().unwrap();
    let data = FileTextData { lines: pointer.map(String::from).to_vec(), spans: None, lfs: LfsState::Pointer, preview: None };
    text.state = TextState::Ready(Arc::new(data));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("file_history_markdown_lfs_pointer");
    assert!(h.get_by_label(PREVIEW).is_disabled(), "a pointer has no preview");
    assert!(h.query_by_label(FIND_HINT).is_some(), "the text is shown");
    drop(h);
    remove_fixture(&dir);
}

/// A gradient with a transparent top-left corner, as PNG bytes.
fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        let a = if x < w / 4 && y < h / 3 { 0 } else { 255 };
        image::Rgba([(x * 255 / w) as u8, (y * 255 / h) as u8, 160, a])
    });
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

/// logo.png committed and shown, decoded as the runner does.
fn with_image(name: &str, theme: ThemeChoice, w: u32, h: u32) -> (AppState, PathBuf, usize) {
    let (mut st, dir) = with_repo(theme, name);
    let bytes = png(w, h);
    std::fs::write(dir.join("logo.png"), &bytes).unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "logo.png"], d);
    git(&dir, &["commit", "-q", "-m", "Add the logo"], d);
    let at = head(&dir);
    let image = preview_image::decode_sized(&bytes, FILE_VIEW_SHOWN_MAX).unwrap();
    let preview = Some(Preview::Image { image, size: bytes.len() as u64 });
    open(&mut st, &dir, "logo.png", at, FileTextData { lines: Vec::new(), spans: None, lfs: LfsState::None, preview });
    (st, dir, bytes.len())
}

#[test]
fn file_history_image_preview() {
    let (st, dir, n) = with_image("history-image", ThemeChoice::Dark, 160, 100);
    let mut h = harness(st, true);
    h.run();
    h.run();
    h.check_snapshot("file_history_image_preview");
    let size = size_text(n as u64);
    assert!(h.query_by_label(&format!("PNG image · 160 × 100 · {size}")).is_some(), "{size}");
    assert!(h.query_by_label(WRAP_LINES).is_none() && h.query_by_label(FIND_HINT).is_none());
    assert!(h.query_by_label_contains("shown at").is_none(), "drawn at its own size");
    drop(h);
    remove_fixture(&dir);
}

/// How many textures the File view's image preview holds.
fn preview_textures(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>) -> usize {
    h.ctx.tex_manager().read().allocated().filter(|(_, m)| m.name == "history-preview").count()
}

#[test]
fn file_history_image_texture_freed_without_a_selection() {
    let (st, dir, _) = with_image("history-image-unselected", ThemeChoice::Dark, 16, 10);
    let mut h = harness(st, true);
    h.run();
    assert_eq!(preview_textures(&h), 1, "the image is uploaded");
    h.state_mut().0.history.as_mut().unwrap().selected = None;
    h.run();
    assert_eq!(preview_textures(&h), 0, "a history with no commit selected keeps no texture");
    h.state_mut().0.history.as_mut().unwrap().selected = Some(0);
    h.run();
    assert_eq!(preview_textures(&h), 1, "shown again");
    h.state_mut().0.history.as_mut().unwrap().pane = HistoryPane::Diff;
    // The diff isn't loaded, so its spinner keeps asking for frames.
    h.run_steps(2);
    assert_eq!(preview_textures(&h), 0, "the Diff pane keeps no texture");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_image_scaled() {
    let (st, dir, _) = with_image("history-image-scaled", ThemeChoice::Light, 1600, 400);
    let mut h = harness(st, false);
    h.run();
    h.run();
    h.check_snapshot("file_history_image_scaled");
    assert!(h.query_by_label_contains("shown at").is_some());
    drop(h);
    remove_fixture(&dir);
}
