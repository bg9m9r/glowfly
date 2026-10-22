//! The pull request panel's code comment threads placed on the local version of their files
//! when the files were loaded from a source tip that isn't the service's head: which lines are
//! asked to be mapped (the key), whether the maps on hand are for that key, and where each
//! thread lands. Pure; the map is made by `PrEffect::LineMaps` (update/pull_request/line_map.rs).

use super::pull_request::{Files, PrView};
use git2::Oid;
use glowfly_core::diff_rows::LineSide;
use glowfly_core::forge::ReviewThread;
use glowfly_core::repo::{LineMap, PrLineMaps, is_commit_prefix};
use std::collections::HashMap;

/// What a line map is made from; a map made for another key is never used.
#[derive(Clone, PartialEq, Debug)]
pub struct MapKey {
    /// The service's head, lowercase.
    pub head: String,
    /// The source tip the files were loaded from.
    pub local: Oid,
    /// The local diff's base.
    pub base: Oid,
    /// The target's tip.
    pub target: Oid,
    /// The current threads' paths (as the head names them), sorted, each once.
    pub paths: Vec<String>,
}

/// The threads' lines mapped onto the local source tip.
#[derive(Clone, PartialEq, Debug, Default)]
pub enum LineMaps {
    /// Not asked for: the files are of the head, or not loaded.
    #[default]
    None,
    Loading(MapKey),
    Loaded {
        key: MapKey,
        maps: PrLineMaps,
    },
    Failed {
        key: MapKey,
        error: String,
    },
}

impl LineMaps {
    pub fn key(&self) -> Option<&MapKey> {
        match self {
            LineMaps::None => None,
            LineMaps::Loading(key) | LineMaps::Loaded { key, .. } | LineMaps::Failed { key, .. } => Some(key),
        }
    }
}

/// The key the threads' lines should be mapped for now: Some once the files and the page are
/// loaded, the service names its head, and the files' source tip isn't that head (compared
/// ignoring case, as `placement` does).
pub fn wanted(v: &PrView) -> Option<MapKey> {
    let Files::Loaded(c) = &v.files else { return None };
    let page = v.loaded()?;
    let head = page.detail.head_sha.as_str();
    if !is_commit_prefix(head) {
        return None;
    }
    let local = v.source_tip().filter(|s| !s.to_string().eq_ignore_ascii_case(head))?;
    let target = v.tips.target?;
    let mut paths: Vec<String> = page.threads.iter().filter(|t| !t.outdated).map(|t| t.path.clone()).collect();
    paths.sort();
    paths.dedup();
    Some(MapKey { head: head.to_ascii_lowercase(), local, base: c.base, target, paths })
}

/// The maps on hand, when they are for the key wanted now.
pub fn current(v: &PrView) -> Option<&LineMaps> {
    let key = wanted(v)?;
    (v.lines.key() == Some(&key)).then_some(&v.lines)
}

/// Whether deleted-line numbers hold, and the maps by the head's paths, once they are made.
fn mapped(v: &PrView) -> Option<(bool, &HashMap<String, LineMap>)> {
    match current(v)? {
        LineMaps::Loaded { maps: PrLineMaps::Mapped { old_kept, files }, .. } => Some((*old_kept, files)),
        _ => None,
    }
}

/// Where a thread is on the local version.
#[derive(Clone, PartialEq, Debug)]
pub struct Spot {
    pub path: String,
    pub side: LineSide,
    pub start_line: Option<u32>,
    /// None for a comment on the whole file.
    pub line: Option<u32>,
}

/// Where `t` is on the local version, once the maps are made; None while they aren't, for an
/// outdated thread, and when its line (or its file) isn't there. A range whose first line
/// changed is its last line alone.
pub fn spot(v: &PrView, t: &ReviewThread) -> Option<Spot> {
    let (old_kept, files) = mapped(v)?;
    if t.outdated {
        return None;
    }
    let m = files.get(&t.path)?;
    let path = m.path.clone()?;
    let Some(n) = t.line else { return Some(Spot { path, side: t.side, start_line: None, line: None }) };
    match t.side {
        LineSide::New => {
            let line = m.line(n)?;
            let start_line = t.start_line.and_then(|s| m.line(s)).filter(|s| *s < line);
            Some(Spot { path, side: LineSide::New, start_line, line: Some(line) })
        }
        // The old side is the merge base's file: its numbers hold while the base is the same.
        LineSide::Old => old_kept.then_some(Spot { path, side: LineSide::Old, start_line: t.start_line, line: Some(n) }),
    }
}

/// Why going to `t` is refused when it has no `spot` on the local version.
pub fn unmapped_why(t: &ReviewThread) -> String {
    match t.line {
        None => format!("{} isn't in the local version", glowfly_core::text::printable(&t.path)),
        Some(_) => "The commented line was changed or removed in the local version".to_owned(),
    }
}

/// The head's name for the local file `local`, whose threads its pane lists: `local` itself
/// when its map keeps that name, else the first thread's path that maps onto it (a rename),
/// else `local`.
pub fn head_path<'a>(v: &'a PrView, local: &'a str) -> &'a str {
    let Some((_, files)) = mapped(v) else { return local };
    let lands = |p: &str| files.get(p).is_some_and(|m| m.path.as_deref() == Some(local));
    if lands(local) {
        return local;
    }
    v.loaded().and_then(|p| p.threads.iter().map(|t| t.path.as_str()).find(|p| lands(p))).unwrap_or(local)
}

fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

/// The note over the threads once they are placed on the local version (`local`).
pub fn mapped_note(v: &PrView, local: Oid) -> String {
    let head = v.loaded().map(|p| p.detail.head_sha.as_str()).unwrap_or_default();
    format!("These comments were made on {} and are shown on the local version ({}).", short(head), short(&local.to_string()))
}

#[cfg(test)]
mod tests;
