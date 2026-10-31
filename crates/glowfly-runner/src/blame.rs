//! Blaming a file for the blame view, with its syntax spans (`Effect::LoadBlame`).

use super::execute::{Cx, Execute};
use crate::effect::BlameRequest;
use crate::msg::{BlameMsg, Msg};
use crate::state::blame::BlameData;
use glowfly_core::highlight::{self, SyntaxHint};
use glowfly_core::repo;

pub(super) fn load(r: BlameRequest) -> Msg {
    // A HEAD blame re-run after HEAD moved follows the file through renames since then; a name
    // that leads nowhere blames the old path, which reports it is not in the commit.
    let path = match (r.at, r.follow) {
        (None, Some(since)) => repo::name_at_head(&r.workdir, &r.path, since).ok().flatten().unwrap_or_else(|| r.path.clone()),
        _ => r.path.clone(),
    };
    let read = match repo::blame_file(&r.workdir, &path, r.at, &r.opts) {
        // A partial clone hasn't downloaded every version: blame again once it has.
        Err(glowfly_core::error::Error::MissingContent(missing)) => {
            return Msg::Partial(crate::msg::PartialMsg::ContentMissing { missing, retry: Box::new(crate::effect::Effect::LoadBlame(r)) });
        }
        read => read,
    };
    let result = read.map_err(|e| e.to_string()).map(|blame| {
        let spans = if r.highlight {
            highlight::highlight_file(SyntaxHint { path: &path, language: blame.language.as_deref() }, &blame.lines)
        } else {
            None
        };
        let (lines, commits, syntax) = (blame.lines.len(), blame.commits.len(), spans.is_some());
        tracing::debug!(path, requested = r.path, lines, commits, syntax, "blame loaded");
        Box::new(BlameData::new(blame, spans))
    });
    // The requested path, so update matches it to the open view.
    Msg::Blame(BlameMsg::Loaded { path: r.path, generation: r.generation, result })
}

impl Execute for BlameRequest {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(load(self))
    }
}
