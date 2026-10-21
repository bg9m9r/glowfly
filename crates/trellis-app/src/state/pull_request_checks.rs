//! The pull request panel's Checks section (update/pull_request/checks.rs,
//! ui/pull_request/checks.rs): every check on the pull request's head commit as its service
//! lists it, whether the section is open, when it was read, and the texts that describe it.
//! Pure: the list is loaded by `PrCall::Checks`, again every `TIP_PENDING_RECHECK` while a
//! check is still running.

use super::ForgeState;
use super::build_status::TIP_PENDING_RECHECK;
use super::pull_request::PrView;
use std::time::{Duration, Instant};
use trellis_core::dates;
use trellis_core::forge::{CheckList, CheckRun, CheckState, MAX_CHECKS, RunState, checks_in_app, duration_text};
use trellis_core::i18n::Lang;
use trellis_core::text::printable;

pub const LOADING: &str = "Loading checks…";
pub const FAILED_TITLE: &str = "Couldn't load the checks";
pub const NO_STATUS: &str = "No build status";
pub const HELD: &str = "Trust this repository to load its checks.";
pub const NO_LINK: &str = "The service gave no link for this check.";

/// What the section shows.
#[derive(Clone, PartialEq, Debug)]
pub enum ChecksPane {
    /// No code host (or an issue tracker only): the section isn't drawn.
    Unsupported,
    /// The repository isn't trusted, so nothing is asked.
    Held,
    Loading,
    Loaded(CheckList),
    Failed(String),
}

/// The open panel's checks.
#[derive(Clone, Debug)]
pub struct PrChecks {
    pub pane: ChecksPane,
    /// The section is expanded (it starts so each time the panel opens).
    pub open: bool,
    /// Loading again while the last list stays on screen.
    pub reloading: bool,
    /// A request is out.
    pub in_flight: bool,
    /// When the last answer came.
    pub fetched: Option<Instant>,
}

impl PrChecks {
    pub fn new(fs: &ForgeState) -> PrChecks {
        let pane = if fs.host_provider().is_some_and(checks_in_app) { ChecksPane::Loading } else { ChecksPane::Unsupported };
        PrChecks { pane, open: true, reloading: false, in_flight: false, fetched: None }
    }

    pub fn loaded(&self) -> Option<&CheckList> {
        match &self.pane {
            ChecksPane::Loaded(l) => Some(l),
            _ => None,
        }
    }

    /// When a list with a check still to finish should be read again; None otherwise.
    fn recheck_at(&self, now: Instant) -> Option<Instant> {
        if self.in_flight || !self.loaded().is_some_and(CheckList::any_active) {
            return None;
        }
        Some(self.fetched.map_or(now, |f| f + TIP_PENDING_RECHECK))
    }

    /// Whether the list should be read again now.
    pub fn due(&self, now: Instant) -> bool {
        self.recheck_at(now).is_some_and(|at| at <= now)
    }

    /// How long until the window should wake to read the list again (at least a second).
    pub fn next_wake(&self, now: Instant) -> Option<Duration> {
        self.recheck_at(now).map(|at| at.saturating_duration_since(now).max(Duration::from_secs(1)))
    }
}

/// The header's marker and summary: the service's own rollup when the list carries one (so it
/// matches the sidebar), else the checks'.
pub fn header_summary(v: &PrView) -> (Option<CheckState>, String) {
    let list = v.checks.loaded();
    let state = v.pr.checks.as_ref().map(|c| c.state).or_else(|| list.and_then(CheckList::overall));
    (state, list.map(CheckList::summary).unwrap_or_default())
}

/// "Open failed build" or "Open build page" and its link: the service's build page, else the
/// first check's; None without either.
pub fn build_link(v: &PrView) -> Option<(&'static str, String)> {
    let listed = v.pr.checks.as_ref().map(|c| c.url.clone()).filter(|u| !u.is_empty());
    let url = listed.or_else(|| v.checks.loaded().and_then(CheckList::first_url).map(str::to_owned))?;
    let label = if header_summary(v).0 == Some(CheckState::Failed) { "Open failed build" } else { "Open build page" };
    Some((label, url))
}

/// How long the check took ("2m 14s"), or how long it has been running ("running 3m", in
/// whole minutes since the window wakes once a minute); None when unknown.
pub fn time_text(r: &CheckRun, now: i64) -> Option<String> {
    match (r.started, r.finished) {
        (Some(s), Some(f)) => duration_text(f - s),
        (Some(s), None) if r.state == RunState::Running => {
            let secs = now - s;
            if (0..60).contains(&secs) {
                Some("running <1m".into())
            } else {
                duration_text(secs - secs % 60).map(|d| format!("running {d}"))
            }
        }
        _ => None,
    }
}

/// "CI · build": the check's name after its group, safe to show.
pub fn run_label(r: &CheckRun) -> String {
    match r.group.as_deref().map(printable).filter(|g| !g.is_empty()) {
        Some(g) => format!("{g} · {}", printable(&r.name)),
        None => printable(&r.name),
    }
}

/// The row's hover text, one item a line.
pub fn run_tooltip(r: &CheckRun, t: Lang, now: i64) -> String {
    let mut lines = vec![run_label(r)];
    lines.push(match r.required {
        Some(true) => format!("{} · Required", r.state.word()),
        Some(false) => format!("{} · Optional", r.state.word()),
        None => r.state.word().to_owned(),
    });
    let description = printable(&r.description);
    if !description.is_empty() {
        lines.push(description);
    }
    if let Some(s) = r.started {
        lines.push(format!("Started {}", dates::relative(t, now - s)));
    }
    if let Some(d) = r.started.zip(r.finished).and_then(|(s, f)| duration_text(f - s)) {
        lines.push(format!("Took {d}"));
    }
    if r.url.is_empty() {
        lines.push(NO_LINK.to_owned());
    }
    lines.join("\n")
}

/// The empty state's caption.
pub fn empty_text(noun: &str) -> String {
    format!("Nothing has reported a check or build on this {noun}'s latest commit yet.")
}

/// The note under a list that was cut.
pub fn truncated_text() -> String {
    format!("Showing the first {MAX_CHECKS} checks.")
}

#[cfg(test)]
mod tests;
