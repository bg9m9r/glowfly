//! The file history's background work: the walk of a file's or folder's commits
//! (`Effect::LoadFileHistory`) and a file's text as of a commit, with its syntax spans and, for
//! Markdown and images, what the File view's preview draws: a Markdown file's text (parsed by
//! the view's `MarkdownCache` when drawn) or a decoded image (`Effect::LoadFileText`).

use super::execute::{Cx, Execute};
use crate::effect::{Effect, FileTextRequest};
use crate::msg::{HistoryMsg, Msg, PartialMsg};
use crate::state::history::{FileTextData, Preview, TextError};
use glowfly_core::error::Error;
use glowfly_core::highlight::{self, SyntaxHint};
use glowfly_core::markdown::MARKDOWN_MAX_BYTES;
use glowfly_core::partial::MissingContent;
use glowfly_core::preview::{self, PreviewKind};
use glowfly_core::preview_image::{self, FILE_VIEW_SHOWN_MAX};
use glowfly_core::repo::{self, FileAtOptions, HistoryStart, LfsState};
use std::path::Path;
use std::sync::Arc;

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
    let result = match result {
        // A partial clone hasn't downloaded this version: read again once it has.
        Err(Failed::Missing(missing)) => {
            return Msg::Partial(PartialMsg::ContentMissing { missing, retry: Box::new(Effect::LoadFileText(r)) });
        }
        Err(Failed::Text(e)) => Err(e),
        Ok(d) => Ok(d),
    };
    Msg::History(HistoryMsg::FileTextLoaded { oid: r.oid, path: r.path, before: r.before, result: result.map(Arc::new) })
}

impl Execute for FileTextRequest {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(file_text(self))
    }
}

/// Why a file's text wasn't read.
enum Failed {
    Text(TextError),
    Missing(MissingContent),
}

impl From<TextError> for Failed {
    fn from(e: TextError) -> Self {
        Failed::Text(e)
    }
}

fn text(r: &FileTextRequest, o: FileAtOptions, markdown: bool) -> Result<FileTextData, Failed> {
    let f = repo::file_at_with(&r.workdir, r.oid, &r.path, o).map_err(failed)?;
    let spans = highlight::highlight_file(SyntaxHint { path: &r.path, language: f.language.as_deref() }, &f.lines);
    let rendered = markdown && matches!(f.lfs, LfsState::None | LfsState::Content);
    let source = rendered.then(|| f.lines.join("\n"));
    let preview = source.filter(|s| s.len() <= MARKDOWN_MAX_BYTES).map(|source| Preview::Markdown { source });
    Ok(FileTextData { lines: f.lines, spans, lfs: f.lfs, preview })
}

/// An image's pixels, with no lines; a Git LFS pointer not swapped for its content is shown as
/// its text. An image that doesn't decode won't on a retry either.
fn image_text(r: &FileTextRequest, o: FileAtOptions) -> Result<FileTextData, Failed> {
    let f = repo::file_bytes_at(&r.workdir, r.oid, &r.path, o).map_err(failed)?;
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
fn failed(e: Error) -> Failed {
    if let Error::MissingContent(m) = e {
        return Failed::Missing(m);
    }
    let retry = !matches!(e, Error::Unsupported(_));
    Failed::Text(TextError { message: e.to_string(), retry })
}
