//! The pull request panel's code comment threads: which are listed and open, their place in
//! the conversation and in a file's diff, the line markers, the texts that locate them, and
//! why replying, resolving or going to one is unavailable. Pure; the calls are made by
//! update/pull_request/threads.rs. When the files aren't of the service's head, the threads'
//! lines are mapped onto the local version (pull_request_line_map.rs).

use super::AppState;
use super::pull_request::{Files, PrView};
use super::pull_request_actions::{MAX_BODY, ready};
use super::pull_request_line_map::{LineMaps, Spot, current, mapped_note, spot, unmapped_why, wanted};
use glowfly_core::diff_rows::{LineSide, row_of_file_line};
use glowfly_core::forge::{Entry, PrPage, ReviewThread};
use glowfly_core::repo::{DiffTarget, PrLineMaps};
use glowfly_core::settings::DiffMode;
use std::collections::HashMap;

/// Which threads are shown, and which are open.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThreadView {
    pub show_resolved: bool,
    pub show_outdated: bool,
    /// Per-thread Show/Hide, overriding `is_open`'s default.
    pub open: HashMap<String, bool>,
    /// The thread a diff line marker or a go-to last chose.
    pub focus: Option<String>,
}

/// Whether `t` is in the lists: resolved and outdated threads only when asked for.
pub fn listed(tv: &ThreadView, t: &ReviewThread) -> bool {
    (!t.resolved || tv.show_resolved) && (!t.outdated || tv.show_outdated)
}

/// Whether `t`'s card shows its comments: as the user last chose, else unless it is resolved
/// or outdated.
pub fn is_open(tv: &ThreadView, t: &ReviewThread) -> bool {
    tv.open.get(&t.id).copied().unwrap_or(!(t.resolved || t.outdated))
}

fn on_path<'a>(page: &'a PrPage, path: Option<&'a str>) -> impl Iterator<Item = &'a ReviewThread> {
    page.threads.iter().filter(move |t| path.is_none_or(|p| t.path == p))
}

/// How many threads (on `path`, or all) are hidden as resolved and as outdated; one that is
/// both counts once, as resolved while resolved ones are hidden.
pub fn hidden(page: &PrPage, tv: &ThreadView, path: Option<&str>) -> (usize, usize) {
    on_path(page, path).fold((0, 0), |(r, o), t| {
        if t.resolved && !tv.show_resolved {
            (r + 1, o)
        } else if t.outdated && !tv.show_outdated {
            (r, o + 1)
        } else {
            (r, o)
        }
    })
}

/// "1 resolved thread hidden", "2 resolved and 1 outdated threads hidden"; None when nothing
/// is hidden.
pub fn hidden_text(resolved: usize, outdated: usize) -> Option<String> {
    let noun = if resolved + outdated == 1 { "thread" } else { "threads" };
    match (resolved, outdated) {
        (0, 0) => None,
        (r, 0) => Some(format!("{r} resolved {noun} hidden")),
        (0, o) => Some(format!("{o} outdated {noun} hidden")),
        (r, o) => Some(format!("{r} resolved and {o} outdated {noun} hidden")),
    }
}

/// How many threads (on `path`, or all) are resolved and how many outdated.
pub fn kind_counts(page: &PrPage, path: Option<&str>) -> (usize, usize) {
    on_path(page, path).fold((0, 0), |(r, o), t| (r + usize::from(t.resolved), o + usize::from(t.outdated)))
}

/// The resolved-threads toggle's label, given how many there are.
pub fn resolved_toggle(tv: &ThreadView, n: usize) -> String {
    if tv.show_resolved { "Hide resolved".to_owned() } else { format!("Show resolved ({n})") }
}

/// The outdated-threads toggle's label, given how many there are.
pub fn outdated_toggle(tv: &ThreadView, n: usize) -> String {
    if tv.show_outdated { "Hide outdated".to_owned() } else { format!("Show outdated ({n})") }
}

/// One item of the conversation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item<'a> {
    Entry(&'a Entry),
    Thread(&'a ReviewThread),
}

/// The conversation: comments and reviews with the listed threads among them, by time (an
/// entry before a thread at the same time).
pub fn timeline<'a>(page: &'a PrPage, tv: &ThreadView) -> Vec<Item<'a>> {
    let mut items: Vec<(i64, u8, Item<'a>)> = page.entries.iter().map(|e| (e.time, 0, Item::Entry(e))).collect();
    items.extend(page.threads.iter().filter(|t| listed(tv, t)).map(|t| (t.time(), 1, Item::Thread(t))));
    items.sort_by_key(|(time, kind, _)| (*time, *kind));
    items.into_iter().map(|(_, _, i)| i).collect()
}

/// The listed threads on `path`: comments on the whole file first, then by line, outdated
/// ones last.
pub fn file_threads<'a>(page: &'a PrPage, tv: &ThreadView, path: &str) -> Vec<&'a ReviewThread> {
    let mut list: Vec<&ReviewThread> = page.threads.iter().filter(|t| t.path == path && listed(tv, t)).collect();
    list.sort_by_key(|t| (t.outdated, t.line.is_some(), t.line));
    list
}

/// How many threads are on `path`, hidden ones included.
pub fn on_file(page: &PrPage, path: &str) -> usize {
    page.threads.iter().filter(|t| t.path == path).count()
}

/// Whether the threads can be placed on the lines of the local diff.
#[derive(Clone, Debug, PartialEq)]
pub enum Placement {
    OnLines,
    /// The local files are of another commit, and the threads' lines are mapped onto it; the
    /// text says so.
    Mapped(String),
    /// The local files are of another commit (or not loaded); the text says how to fix it.
    Elsewhere(String),
}

impl Placement {
    /// The note over the threads; None when they are on their lines.
    pub fn note(&self) -> Option<&str> {
        match self {
            Placement::OnLines => None,
            Placement::Mapped(s) | Placement::Elsewhere(s) => Some(s),
        }
    }
}

fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

/// OnLines when the files are loaded and their source tip (the remote-tracking branch, or the
/// listed head when it was used) is the head the service reported; Mapped once the threads'
/// lines are mapped onto another source tip; else Elsewhere, saying why.
pub fn placement(v: &PrView) -> Placement {
    let head = v.loaded().map(|p| p.detail.head_sha.as_str()).unwrap_or_default();
    let synced =
        matches!(v.files, Files::Loaded(_)) && !head.is_empty() && v.source_tip().is_some_and(|s| s.to_string().eq_ignore_ascii_case(head));
    if synced {
        return Placement::OnLines;
    }
    let fetch = || Placement::Elsewhere(format!("These comments are on {}; fetch {} to see them on their lines.", short(head), v.remote));
    match (wanted(v), current(v)) {
        (None, _) | (_, Some(LineMaps::Loaded { maps: PrLineMaps::HeadMissing, .. })) => fetch(),
        (Some(key), Some(LineMaps::Loaded { .. })) => Placement::Mapped(mapped_note(v, key.local)),
        (_, Some(LineMaps::Failed { .. })) => {
            Placement::Elsewhere(format!("These comments are on {}; their lines couldn't be found in the local version.", short(head)))
        }
        _ => Placement::Elsewhere("Placing these comments on the local version…".to_owned()),
    }
}

/// A diff line's comment marker.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    /// The thread a click opens (the first on the line).
    pub thread: String,
    /// Comments on the line, in all its threads.
    pub count: usize,
    /// Every thread on the line is resolved.
    pub resolved: bool,
}

pub type Marks = HashMap<(LineSide, u32), Mark>;

/// The markers for `path`'s diff lines: listed, current threads with a line, by (side, line).
/// While Mapped, on the local lines (and path) they map to. None unless the threads can be
/// placed on the local lines.
pub fn marks(v: &PrView, path: &str) -> Option<Marks> {
    let mapped = match placement(v) {
        Placement::OnLines => false,
        Placement::Mapped(_) => true,
        Placement::Elsewhere(_) => return None,
    };
    let mut out = Marks::new();
    for t in v.loaded()?.threads.iter().filter(|t| listed(&v.threads, t) && !t.outdated) {
        let at = if mapped {
            spot(v, t).filter(|s| s.path == path).and_then(|s| s.line.map(|n| (s.side, n)))
        } else {
            t.line.filter(|_| t.path == path).map(|n| (t.side, n))
        };
        let Some(at) = at else { continue };
        let m = out.entry(at).or_insert_with(|| Mark { thread: t.id.clone(), count: 0, resolved: true });
        m.count += t.comments.len();
        m.resolved &= t.resolved;
    }
    Some(out)
}

/// "line 42", "lines 40–42", "deleted line 12", or "the file" (no line).
fn lines_text(side: LineSide, start: Option<u32>, line: Option<u32>) -> String {
    let Some(line) = line else { return "the file".to_owned() };
    let deleted = if side == LineSide::Old { "deleted " } else { "" };
    match start.filter(|&s| s < line) {
        Some(s) => format!("{deleted}lines {s}\u{2013}{line}"),
        None => format!("{deleted}line {line}"),
    }
}

/// Where the thread is: "line 42", "lines 40–42", "deleted line 12", "the file", or "line 7 of
/// an earlier version"; while Mapped, its local lines then the head's ("line 40 (line 42 on
/// abc1234)"), or "line 42 on abc1234, not in the local version".
pub fn where_text(v: &PrView, t: &ReviewThread) -> String {
    if t.outdated {
        return match t.original_line {
            Some(n) => format!("line {n} of an earlier version"),
            None => "an earlier version".to_owned(),
        };
    }
    let head = lines_text(t.side, t.start_line, t.line);
    if !matches!(placement(v), Placement::Mapped(_)) {
        return head;
    }
    let short = short(v.loaded().map(|p| p.detail.head_sha.as_str()).unwrap_or_default());
    match spot(v, t).map(|s| lines_text(s.side, s.start_line, s.line)) {
        Some(local) if local == head => head,
        Some(local) => format!("{local} ({head} on {short})"),
        None => format!("{head} on {short}, not in the local version"),
    }
}

/// Where going to `t` opens: its file's diff in Files changed, and its side and line (0 for a
/// comment on the whole file, which no row has, so the diff opens where it usually does).
/// Paths are matched exactly against the loaded file list; nothing else builds a diff. While
/// Mapped, it opens the local file and line the thread maps to.
pub fn go_to(v: &PrView, t: &ReviewThread) -> Result<(DiffTarget, LineSide, u32), String> {
    if t.outdated {
        return Err("This comment is on an earlier version of the file".to_owned());
    }
    let s = match placement(v) {
        Placement::Elsewhere(why) => return Err(why),
        Placement::Mapped(_) => spot(v, t).ok_or_else(|| unmapped_why(t))?,
        Placement::OnLines => Spot { path: t.path.clone(), side: t.side, start_line: t.start_line, line: t.line },
    };
    if let Files::Loaded(c) = &v.files
        && !c.comparison.files.iter().any(|f| f.path == s.path)
    {
        return Err(format!("{} isn't among the files changed here", glowfly_core::text::printable(&s.path)));
    }
    let target = v.diff_target(&s.path).ok_or_else(|| format!("{} isn't among the files changed here", s.path))?;
    Ok((target, s.side, s.line.unwrap_or(0)))
}

/// `go_to`, also refused when that diff is open in the hunk view and its hunks don't show the
/// line.
pub fn go_to_open(st: &AppState, v: &PrView, t: &ReviewThread) -> Result<(DiffTarget, LineSide, u32), String> {
    let (target, side, n) = go_to(v, t)?;
    if let Some((open, d)) = &st.diff
        && *open == target
        && n > 0
        && st.settings.diff_mode == DiffMode::Hunks
        && row_of_file_line(d, DiffMode::Hunks, st.settings.diff_layout, &st.view.diff_expanded, side, n).is_none()
    {
        return Err(format!("Line {n} isn't in the changed lines; switch to Whole file to see it"));
    }
    Ok((target, side, n))
}

/// Why the Reply button is disabled; the first reason that applies.
pub fn reply_blocked(v: &PrView, t: &ReviewThread) -> Option<String> {
    let page = match ready(v) {
        Ok(p) => p,
        Err(why) => return Some(why),
    };
    let draft = v.forms.replies.get(&t.id).map(String::as_str).unwrap_or_default();
    Some(if !t.can_reply && page.detail.locked {
        "The conversation is locked on the service".to_owned()
    } else if !t.can_reply {
        "You can't reply to this thread".to_owned()
    } else if page.detail.pending_review.is_some() {
        "Finish your review in progress first (Your review > Finish review); replies can't be added while it's pending".to_owned()
    } else if t.reply_to().is_none() {
        "This thread's first comment can't be found, so there is nothing to reply to".to_owned()
    } else if draft.trim().is_empty() {
        "Write a reply first".to_owned()
    } else if draft.chars().count() > MAX_BODY {
        "Replies can have at most 65,536 characters".to_owned()
    } else {
        return None;
    })
}

/// Why the Resolve or Unresolve button is disabled.
pub fn resolve_blocked(v: &PrView, t: &ReviewThread) -> Option<String> {
    if glowfly_core::forge::new_thread_comment(&t.id).is_some() {
        return Some("This new thread can be resolved once the page has reloaded".to_owned());
    }
    if let Err(why) = ready(v) {
        return Some(why);
    }
    match (t.resolved, t.can_resolve, t.can_unresolve) {
        (true, _, false) => Some("You can't unresolve this thread".to_owned()),
        (false, false, _) => Some("Only the author and people with write access can resolve threads".to_owned()),
        _ => None,
    }
}

pub fn resolve_label(t: &ReviewThread) -> &'static str {
    if t.resolved { "Unresolve" } else { "Resolve" }
}

#[cfg(test)]
mod mapped_tests;
#[cfg(test)]
mod tests;
