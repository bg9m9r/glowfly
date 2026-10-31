//! The Range-diff view's state (`RepoSession::range_diff`, parked with its tab) and the pure words
//! and rules it shows: the subtitle naming the two ends, how the ranges were cut, the counts,
//! the refusals as sentences, why the journal's and the multi-commit menu's entry points are
//! off, and the mainline a comparison is cut at (which names to leave out for a branch or a
//! remote-tracking ref).

use super::git_env::GitState;
use super::journal::{against, event_label, short, subject_name};
use super::selection::MultiSelection;
use git2::Oid;
use glowfly_core::git_version::Feature;
use glowfly_core::i18n::Lang;
use glowfly_core::journal::Row;
use glowfly_core::range_diff::{Compared, Ends, MAX_COMMITS, Outcome, Pair, Status};
use glowfly_core::repo::{BranchInfo, Snapshot};
use glowfly_core::settings::Settings;

/// What was asked to be compared.
#[derive(Clone, Debug, PartialEq)]
pub struct Origin {
    pub ends: Ends,
    /// The branch (or HEAD) the comparison is about: a journal state's subject, the short name
    /// of a `LastRewrite` ref; None for two commits.
    pub label: Option<String>,
    /// A journal state's time.
    pub at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Loading,
    /// The repository isn't trusted yet: nothing runs until it is.
    Held,
    Failed(String),
    Ready(Box<Outcome>),
}

/// The open view.
#[derive(Clone, Debug, PartialEq)]
pub struct RangeDiffView {
    pub origin: Origin,
    /// The comparison in flight or last asked for (`RepoSession::range_diff`).
    pub generation: u64,
    pub phase: Phase,
    /// The selected pair, by its place in `Compared::pairs`.
    pub selected: Option<usize>,
    /// Each pair's widest interdiff line in columns (tabs expanded), measured at load.
    pub widest: Vec<usize>,
}

impl RangeDiffView {
    pub fn loading(origin: Origin, generation: u64) -> Self {
        Self { origin, generation, phase: Phase::Loading, selected: None, widest: Vec::new() }
    }

    pub fn compared(&self) -> Option<&Compared> {
        match &self.phase {
            Phase::Ready(o) => match o.as_ref() {
                Outcome::Compared(c) => Some(c),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn pair(&self, i: usize) -> Option<&Pair> {
        self.compared()?.pairs.get(i)
    }
}

/// `refs/heads/x` is "x", `refs/remotes/r/x` is "r/x".
pub fn short_ref(refname: &str) -> &str {
    refname.strip_prefix("refs/heads/").or_else(|| refname.strip_prefix("refs/remotes/")).unwrap_or(refname)
}

/// The first changed pair, which the view selects on load.
pub fn first_changed(c: &Compared) -> Option<usize> {
    c.pairs.iter().position(|p| p.status == Status::Changed)
}

/// Each pair's widest interdiff line, in columns with tabs expanded to `tab`.
pub fn widest(c: &Compared, tab: usize) -> Vec<usize> {
    let width = |p: &Pair| {
        let hunks = p.interdiff.iter().flat_map(|d| &d.hunks);
        let lines = hunks.flat_map(|h| std::iter::once(h.header.as_str()).chain(h.lines.iter().map(|l| l.text.as_str())));
        lines.map(|l| glowfly_core::tabs::columns(l, tab)).max().unwrap_or(0)
    };
    c.pairs.iter().map(width).collect()
}

pub fn status_label(t: Lang, s: Status) -> &'static str {
    match s {
        Status::Unchanged => t.tr("range_diff.status.same"),
        Status::Changed => t.tr("range_diff.status.changed"),
        Status::Removed => t.tr("range_diff.status.removed"),
        Status::Added => t.tr("range_diff.status.added"),
    }
}

/// "Changed 1 · Dropped 1 · New 1 · Same 1".
pub fn counts_line(t: Lang, c: &Compared) -> String {
    let n = |s: Status| c.pairs.iter().filter(|p| p.status == s).count();
    let (changed, removed, added, same) = (n(Status::Changed), n(Status::Removed), n(Status::Added), n(Status::Unchanged));
    t.trf("range_diff.counts", &[("changed", &changed), ("removed", &removed), ("added", &added), ("same", &same)])
}

/// The header's subtitle, naming the two ends.
pub fn subtitle(t: Lang, origin: &Origin, c: &Compared, settings: &Settings, now: i64, utc_offset: i32) -> String {
    let (old, new) = (short(c.old.tip), short(c.new.tip));
    let branch = origin.label.as_deref().unwrap_or("HEAD");
    let when = |at: i64| settings.format_time(at, now, utc_offset);
    let ends: [(&str, &dyn std::fmt::Display); 2] = [("old", &old), ("new", &new)];
    match (&origin.ends, &c.rewrite, origin.at) {
        (Ends::LastRewrite { .. }, Some(r), _) => {
            let w = when(r.at);
            let text = t.trf("range_diff.subtitle.rewrite", &[("branch", &branch), ends[0], ("when", &w), ends[1]]);
            let event = event_label(t, &glowfly_core::journal::events::parse(&r.message))
                .unwrap_or_else(|| glowfly_core::text::printable(&r.message));
            if event.is_empty() { text } else { format!("{text} · {event}") }
        }
        (Ends::Tips { .. }, _, Some(at)) if origin.label.is_some() => {
            let w = when(at);
            t.trf("range_diff.subtitle.state", &[("branch", &branch), ends[0], ("when", &w), ends[1]])
        }
        _ => t.trf("range_diff.subtitle.tips", &ends),
    }
}

/// How the ranges were cut: at the mainline, or where the two versions split.
pub fn base_line(t: Lang, c: &Compared) -> String {
    match &c.mainline {
        Some(m) => t.trf("range_diff.base.mainline", &[("branch", m)]),
        None => t.trf("range_diff.base.split", &[("sha", &short(c.old.base))]),
    }
}

/// A refusal as a sentence; None for a comparison.
pub fn outcome_failure(t: Lang, o: &Outcome) -> Option<String> {
    let capped = |n: usize| if n > MAX_COMMITS { format!("{MAX_COMMITS}+") } else { n.to_string() };
    Some(match o {
        Outcome::Compared(_) => return None,
        Outcome::NoRewrite { refname } => t.trf("range_diff.no_rewrite", &[("branch", &short_ref(refname))]),
        Outcome::NoSharedHistory => t.tr("range_diff.no_shared_history").to_owned(),
        Outcome::TooMany { old, new } => {
            t.trf("range_diff.too_many", &[("old", &capped(*old)), ("new", &capped(*new)), ("max", &MAX_COMMITS)])
        }
    })
}

/// Why the journal's Range-diff with now is off for `row`: its commit is gone, its branch is
/// gone, it is where its subject is now, or git is too old.
pub fn journal_blocked(t: Lang, snap: &Snapshot, row: &Row, git: &GitState) -> Option<String> {
    let name = subject_name(&row.subject);
    if row.summary.is_none() {
        return Some(t.tr("journal.blocked.gone").to_owned());
    }
    match against(snap, &row.subject) {
        None => return Some(t.trf("range_diff.blocked.no_tip", &[("branch", &name)])),
        Some(now) if now == row.oid => return Some(t.trf("range_diff.blocked.same", &[("branch", &name)])),
        Some(_) => {}
    }
    git.blocked(Feature::RangeDiff)
}

/// Why the multi-commit menu's Range-diff the two commits is off.
pub fn multi_blocked(t: Lang, m: &MultiSelection) -> Option<&'static str> {
    (m.oids.len() != 2).then(|| t.tr("range_diff.blocked.two"))
}

/// For local branch `b`: its tracking remote, and the names the mainline must not be (the
/// branch and its upstream).
fn exclude_for_branch(b: &BranchInfo) -> (Option<String>, Vec<String>) {
    let remote = b.tracking.as_ref().filter(|t| t.is_remote()).map(|t| t.remote.clone());
    let mut exclude = vec![b.name.clone()];
    exclude.extend(b.tracking.as_ref().map(|t| t.display()));
    exclude.extend(b.upstream.clone());
    (remote, exclude)
}

/// For remote-tracking branch `remote`/`name`: that remote, and the names the mainline must
/// not be (the ref itself and the local branches tracking it).
pub fn exclude_for_remote(snap: &Snapshot, remote: &str, name: &str) -> (Option<String>, Vec<String>) {
    let mut exclude = vec![format!("{remote}/{name}")];
    let tracking = snap.locals.iter().filter(|b| b.tracking.as_ref().is_some_and(|t| t.remote == remote && t.merge == name));
    exclude.extend(tracking.map(|b| b.name.clone()));
    (Some(remote.to_owned()), exclude)
}

/// `refs/remotes/<remote>/<name>` split at a remote the snapshot knows (remote names may hold
/// slashes; the longest match wins).
fn split_remote<'a>(snap: &Snapshot, short: &'a str) -> Option<(&'a str, &'a str)> {
    let mut known: Vec<&str> = snap.remotes.iter().map(|(r, _)| r.as_str()).collect();
    known.sort_by_key(|r| std::cmp::Reverse(r.len()));
    known.into_iter().find_map(|r| {
        let rest = short.strip_prefix(r)?.strip_prefix('/')?;
        Some((&short[..r.len()], rest))
    })
}

/// The branch both ranges are cut at for `origin` (`glowfly_core::range_diff::bases::mainline`),
/// leaving out the branch being compared and the refs that follow it.
pub fn mainline_for(snap: &Snapshot, origin: &Origin) -> Option<(String, Oid)> {
    let branch = |name: &str| match snap.locals.iter().find(|b| b.name == name) {
        Some(b) => exclude_for_branch(b),
        None => (None, vec![name.to_owned()]),
    };
    let (remote, exclude) = match &origin.ends {
        Ends::LastRewrite { refname } if refname.starts_with("refs/remotes/") => match split_remote(snap, short_ref(refname)) {
            Some((r, name)) => exclude_for_remote(snap, r, name),
            None => (None, vec![short_ref(refname).to_owned()]),
        },
        Ends::LastRewrite { refname } => branch(short_ref(refname)),
        Ends::Tips { .. } => match origin.label.as_deref() {
            Some(name) if name != "HEAD" => branch(name),
            _ => (None, Vec::new()),
        },
    };
    let exclude: Vec<&str> = exclude.iter().map(String::as_str).collect();
    glowfly_core::range_diff::bases::mainline(snap, remote.as_deref(), &exclude)
}

#[cfg(test)]
#[path = "range_diff_tests.rs"]
mod tests;
