//! The file history's background work: the walk of a file's or folder's commits
//! (`Effect::LoadFileHistory`) and a file's text as of a commit, with its syntax spans and, for
//! Markdown and images, what the File view's preview draws: a Markdown file's text (parsed by
//! the view's `MarkdownCache` when drawn) or a decoded image (`Effect::LoadFileText`).

use crate::effect::FileTextRequest;
use crate::msg::{HistoryMsg, Msg};
use crate::state::history::{FileTextData, Preview, TextError};
use std::path::Path;
use std::sync::Arc;
use trellis_core::highlight::{self, SyntaxHint};
use trellis_core::markdown::MARKDOWN_MAX_BYTES;
use trellis_core::preview::{self, PreviewKind};
use trellis_core::preview_image::{self, FILE_VIEW_SHOWN_MAX};
use trellis_core::repo::{self, FileAtOptions, HistoryStart, LfsState};

pub(super) fn walk(workdir: &Path, path: String, start: &HistoryStart, limit: usize, folder: bool, generation: u64) -> Msg {
    let result = if folder { repo::folder_history(workdir, &path, start, limit) } else { repo::file_history(workdir, &path, start, limit) };
    let result = result.map_err(|e| e.to_string());
    Msg::History(HistoryMsg::FileHistoryLoaded { path, generation, result })
}

/// Always highlighted: the view paints the spans only while syntax highlighting is on, as the
/// diff does. A Markdown file's text also comes joined for the preview (not a Git LFS pointer's,
/// nor one too long to render), and an image is decoded instead of read as text.
pub(super) fn file_text(r: FileTextRequest) -> Msg {
    let o = FileAtOptions { before: r.before, smudge: r.smudge };
    let kind = preview::kind(&r.path);
    let result = match kind {
        Some(PreviewKind::Image) => image_text(&r, o),
        _ => text(&r, o, kind == Some(PreviewKind::Markdown)),
    };
    Msg::History(HistoryMsg::FileTextLoaded { oid: r.oid, path: r.path, before: r.before, result: result.map(Arc::new) })
}

fn text(r: &FileTextRequest, o: FileAtOptions, markdown: bool) -> Result<FileTextData, TextError> {
    let f = repo::file_at_with(&r.workdir, r.oid, &r.path, o).map_err(text_error)?;
    let spans = highlight::highlight_file(SyntaxHint { path: &r.path, language: f.language.as_deref() }, &f.lines);
    let rendered = markdown && matches!(f.lfs, LfsState::None | LfsState::Content);
    let source = rendered.then(|| f.lines.join("\n"));
    let preview = source.filter(|s| s.len() <= MARKDOWN_MAX_BYTES).map(|source| Preview::Markdown { source });
    Ok(FileTextData { lines: f.lines, spans, lfs: f.lfs, preview })
}

/// An image's pixels, with no lines; a Git LFS pointer not swapped for its content is shown as
/// its text. An image that doesn't decode won't on a retry either.
fn image_text(r: &FileTextRequest, o: FileAtOptions) -> Result<FileTextData, TextError> {
    let f = repo::file_bytes_at(&r.workdir, r.oid, &r.path, o).map_err(text_error)?;
    if matches!(f.lfs, LfsState::Pointer | LfsState::Unavailable(_)) {
        let lines = String::from_utf8_lossy(&f.bytes).lines().map(String::from).collect();
        return Ok(FileTextData { lines, spans: None, lfs: f.lfs, preview: None });
    }
    if f.bytes.is_empty() {
        return Ok(FileTextData { lines: Vec::new(), spans: None, lfs: f.lfs, preview: None });
    }
    let image = preview_image::decode_sized(&f.bytes, FILE_VIEW_SHOWN_MAX)
        .map_err(|e| TextError { message: format!("{} could not be shown as an image: {e}", r.path), retry: false })?;
    let preview = Some(Preview::Image { image, size: f.bytes.len() as u64 });
    Ok(FileTextData { lines: Vec::new(), spans: None, lfs: f.lfs, preview })
}

/// A fixed reason (binary, too large, a submodule) won't change on a retry; a read error may.
fn text_error(e: trellis_core::error::Error) -> TextError {
    let retry = !matches!(e, trellis_core::error::Error::Unsupported(_));
    TextError { message: e.to_string(), retry }
}
