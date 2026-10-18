//! Blaming a file for the blame view, with its syntax spans (`Effect::LoadBlame`).

use crate::effect::BlameRequest;
use crate::msg::{BlameMsg, Msg};
use crate::state::blame::BlameData;
use trellis_core::highlight::{self, SyntaxHint};
use trellis_core::repo;

pub(super) fn load(r: BlameRequest) -> Msg {
    let result = repo::blame_file(&r.workdir, &r.path, r.at, &r.opts).map_err(|e| e.to_string()).map(|blame| {
        let spans = if r.highlight {
            highlight::highlight_file(SyntaxHint { path: &r.path, language: blame.language.as_deref() }, &blame.lines)
        } else {
            None
        };
        let (lines, commits, syntax) = (blame.lines.len(), blame.commits.len(), spans.is_some());
        tracing::debug!(path = r.path, lines, commits, syntax, "blame loaded");
        Box::new(BlameData::new(blame, spans))
    });
    Msg::Blame(BlameMsg::Loaded { path: r.path, generation: r.generation, result })
}
