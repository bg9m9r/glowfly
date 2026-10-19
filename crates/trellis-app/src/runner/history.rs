//! The file history's background work: the walk of a file's or folder's commits
//! (`Effect::LoadFileHistory`) and a file's text as of a commit, with its syntax spans
//! (`Effect::LoadFileText`).

use crate::effect::FileTextRequest;
use crate::msg::{HistoryMsg, Msg};
use crate::state::history::{FileTextData, TextError};
use std::path::Path;
use std::sync::Arc;
use trellis_core::highlight::{self, SyntaxHint};
use trellis_core::repo::{self, FileAtOptions, HistoryStart};

pub(super) fn walk(workdir: &Path, path: String, start: &HistoryStart, limit: usize, folder: bool, generation: u64) -> Msg {
    let result = if folder { repo::folder_history(workdir, &path, start, limit) } else { repo::file_history(workdir, &path, start, limit) };
    let result = result.map_err(|e| e.to_string());
    Msg::History(HistoryMsg::FileHistoryLoaded { path, generation, result })
}

/// Always highlighted: the view paints the spans only while syntax highlighting is on, as the
/// diff does.
pub(super) fn file_text(r: FileTextRequest) -> Msg {
    let o = FileAtOptions { before: r.before, smudge: r.smudge };
    let result = repo::file_at_with(&r.workdir, r.oid, &r.path, o).map_err(text_error).map(|f| {
        let spans = highlight::highlight_file(SyntaxHint { path: &r.path, language: f.language.as_deref() }, &f.lines);
        Arc::new(FileTextData { lines: f.lines, spans, lfs: f.lfs })
    });
    Msg::History(HistoryMsg::FileTextLoaded { oid: r.oid, path: r.path, before: r.before, result })
}

/// A fixed reason (binary, too large, a submodule) won't change on a retry; a read error may.
fn text_error(e: trellis_core::error::Error) -> TextError {
    let retry = !matches!(e, trellis_core::error::Error::Unsupported(_));
    TextError { message: e.to_string(), retry }
}
