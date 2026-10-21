//! Each CI check on a pull request's head commit, for the pull request panel's Checks section:
//! its name, group, state, whether it is required, its times and its link. `checks.rs` keeps
//! the counts the lists show; this is the detail, read per pull request on demand.

use super::checks::CheckState;
use super::{Auth, Provider, Target, issue_details, links, service};
use crate::error::Result;

/// The most checks one list holds; a longer one is cut and marked incomplete.
pub const MAX_CHECKS: usize = 300;
/// The most characters of a check's description kept.
const MAX_DESCRIPTION: usize = 300;

/// One check's state, finer than the overall `CheckState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RunState {
    Passed,
    Failed,
    Running,
    Pending,
    ActionRequired,
    Cancelled,
    Skipped,
    Neutral,
}

impl RunState {
    /// Every state, in list order (most urgent first).
    pub const BY_RANK: [RunState; 8] = [
        RunState::Failed,
        RunState::ActionRequired,
        RunState::Running,
        RunState::Pending,
        RunState::Cancelled,
        RunState::Passed,
        RunState::Neutral,
        RunState::Skipped,
    ];

    /// Where checks in this state sort: failures first, skipped ones last.
    pub fn rank(self) -> u8 {
        match self {
            RunState::Failed => 0,
            RunState::ActionRequired => 1,
            RunState::Running => 2,
            RunState::Pending => 3,
            RunState::Cancelled => 4,
            RunState::Passed => 5,
            RunState::Neutral => 6,
            RunState::Skipped => 7,
        }
    }

    /// Still to finish: the list is read again while any check is.
    pub fn is_active(self) -> bool {
        matches!(self, RunState::Running | RunState::Pending)
    }

    /// What this check does to the overall result.
    pub fn outcome(self) -> CheckState {
        match self {
            RunState::Passed | RunState::Neutral | RunState::Skipped => CheckState::Passed,
            RunState::Failed | RunState::Cancelled | RunState::ActionRequired => CheckState::Failed,
            RunState::Running | RunState::Pending => CheckState::Pending,
        }
    }

    /// The state on its own, as a check's tooltip and accessible name say it.
    pub fn word(self) -> &'static str {
        match self {
            RunState::Passed => "Passed",
            RunState::Failed => "Failed",
            RunState::Running => "Running",
            RunState::Pending => "Queued",
            RunState::ActionRequired => "Needs action",
            RunState::Cancelled => "Cancelled",
            RunState::Skipped => "Skipped",
            RunState::Neutral => "Neutral",
        }
    }

    /// The state after a count, as the summary says it ("2 running").
    pub fn count_word(self) -> &'static str {
        match self {
            RunState::Passed => "passed",
            RunState::Failed => "failed",
            RunState::Running => "running",
            RunState::Pending => "queued",
            RunState::ActionRequired => "needs action",
            RunState::Cancelled => "cancelled",
            RunState::Skipped => "skipped",
            RunState::Neutral => "neutral",
        }
    }
}

/// One check: a CI job, a check run, a commit status or a build policy.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckRun {
    pub name: String,
    /// The workflow, app, stage or genre it belongs to, when the service names one.
    pub group: Option<String>,
    pub state: RunState,
    /// None when the service doesn't say whether merging waits for it.
    pub required: Option<bool>,
    /// Unix seconds.
    pub started: Option<i64>,
    pub finished: Option<i64>,
    /// The check's page; empty when the service gave no usable link.
    pub url: String,
    pub description: String,
}

/// The checks on one commit, most urgent first.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckList {
    /// The commit they ran on; empty when the service didn't say.
    pub sha: String,
    pub runs: Vec<CheckRun>,
    /// False when there were more than `MAX_CHECKS`.
    pub complete: bool,
}

impl Default for CheckList {
    fn default() -> Self {
        CheckList::empty("")
    }
}

impl CheckList {
    /// No checks on `sha`.
    pub fn empty(sha: impl Into<String>) -> CheckList {
        CheckList { sha: sha.into(), runs: Vec::new(), complete: true }
    }

    /// "1 failed, 2 running, 5 passed": each state that has checks, in list order; "No build
    /// status" without any.
    pub fn summary(&self) -> String {
        let parts: Vec<String> = RunState::BY_RANK
            .iter()
            .filter_map(|&s| {
                let n = self.runs.iter().filter(|r| r.state == s).count();
                (n > 0).then(|| format!("{n} {}", s.count_word()))
            })
            .collect();
        if parts.is_empty() { "No build status".to_owned() } else { parts.join(", ") }
    }

    /// The overall result from the checks: any failure fails it, else anything still to finish
    /// keeps it pending; None without checks.
    pub fn overall(&self) -> Option<CheckState> {
        let outcomes: Vec<CheckState> = self.runs.iter().map(|r| r.state.outcome()).collect();
        if outcomes.is_empty() {
            None
        } else if outcomes.contains(&CheckState::Failed) {
            Some(CheckState::Failed)
        } else if outcomes.contains(&CheckState::Pending) {
            Some(CheckState::Pending)
        } else {
            Some(CheckState::Passed)
        }
    }

    pub fn any_active(&self) -> bool {
        self.runs.iter().any(|r| r.state.is_active())
    }

    /// The first check's link, in list order.
    pub fn first_url(&self) -> Option<&str> {
        self.runs.iter().map(|r| r.url.as_str()).find(|u| !u.is_empty())
    }
}

/// Which pull request to read the checks of, and its head when the list knows it.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckQuery {
    pub id: u64,
    pub head: Option<String>,
}

/// Whether the pull request panel shows checks for this service (every code host).
pub fn checks_in_app(p: Provider) -> bool {
    !p.is_tracker_only()
}

/// How long `secs` took: "45s", "2m 14s", "1h 2m"; None for a negative span.
pub fn duration_text(secs: i64) -> Option<String> {
    if secs < 0 {
        return None;
    }
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    Some(match (h, m, s) {
        (0, 0, s) => format!("{s}s"),
        (0, m, 0) => format!("{m}m"),
        (0, m, s) => format!("{m}m {s}s"),
        (h, 0, _) => format!("{h}h"),
        (h, m, _) => format!("{h}h {m}m"),
    })
}

/// Every check on pull request `q.id`'s head commit. Links are checked here, once for every
/// service, and the list is cut to `MAX_CHECKS` and sorted.
pub fn check_list(t: &Target, auth: &Auth, q: &CheckQuery) -> Result<CheckList> {
    service(t.provider).check_list(t, auth, q).map(finish)
}

/// Safe links, trimmed descriptions, at most `MAX_CHECKS`, and most urgent first (required
/// checks first within a state, otherwise in the service's order).
fn finish(mut list: CheckList) -> CheckList {
    if list.runs.len() > MAX_CHECKS {
        list.runs.truncate(MAX_CHECKS);
        list.complete = false;
    }
    for r in &mut list.runs {
        r.url = links::build_url(std::mem::take(&mut r.url));
        let d = r.description.trim();
        r.description = d.chars().take(MAX_DESCRIPTION).collect();
    }
    list.runs.sort_by_key(|r| (r.state.rank(), r.required != Some(true)));
    list
}

/// A time from an API answer, in unix seconds; None when empty or not understood.
pub(super) fn time(s: &str) -> Option<i64> {
    Some(issue_details::api_time(s)).filter(|&t| t != 0)
}

/// `time` of an optional field.
pub(super) fn time_of(s: &Option<String>) -> Option<i64> {
    s.as_deref().and_then(time)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(name: &str, state: RunState, required: Option<bool>) -> CheckRun {
        CheckRun {
            name: name.into(),
            group: None,
            state,
            required,
            started: None,
            finished: None,
            url: String::new(),
            description: String::new(),
        }
    }

    #[test]
    fn durations() {
        let cases = [(0, "0s"), (45, "45s"), (120, "2m"), (134, "2m 14s"), (3600, "1h"), (3720, "1h 2m"), (3725, "1h 2m")];
        for (secs, want) in cases {
            assert_eq!(duration_text(secs).as_deref(), Some(want), "{secs}");
        }
        assert_eq!(duration_text(-5), None);
    }

    #[test]
    fn states_rank_count_and_finish() {
        use RunState::*;
        let table = [
            (Failed, 0, CheckState::Failed, false),
            (ActionRequired, 1, CheckState::Failed, false),
            (Running, 2, CheckState::Pending, true),
            (Pending, 3, CheckState::Pending, true),
            (Cancelled, 4, CheckState::Failed, false),
            (Passed, 5, CheckState::Passed, false),
            (Neutral, 6, CheckState::Passed, false),
            (Skipped, 7, CheckState::Passed, false),
        ];
        for (s, rank, outcome, active) in table {
            assert_eq!((s.rank(), s.outcome(), s.is_active()), (rank, outcome, active), "{s:?}");
        }
        assert!(RunState::BY_RANK.windows(2).all(|w| w[0].rank() < w[1].rank()));
    }

    #[test]
    fn summaries_name_each_state_in_list_order() {
        use RunState::*;
        let mut runs = vec![run("a", Passed, None); 5];
        for s in [Skipped, Skipped, Neutral, Cancelled, Pending, Running, Running, ActionRequired, Failed] {
            runs.push(run("x", s, None));
        }
        let list = CheckList { sha: String::new(), runs, complete: true };
        assert_eq!(list.summary(), "1 failed, 1 needs action, 2 running, 1 queued, 1 cancelled, 5 passed, 1 neutral, 2 skipped");
        assert_eq!(CheckList::empty("abc").summary(), "No build status");
        let only = CheckList { runs: vec![run("a", Passed, None), run("b", Skipped, None)], ..CheckList::default() };
        assert_eq!(only.summary(), "1 passed, 1 skipped");
        assert!(CheckList::default().complete);
    }

    #[test]
    fn overall_from_the_runs() {
        use RunState::*;
        let of = |states: &[RunState]| CheckList { runs: states.iter().map(|&s| run("a", s, None)).collect(), ..CheckList::default() };
        assert_eq!(of(&[]).overall(), None);
        assert_eq!(of(&[Passed, Skipped, Neutral]).overall(), Some(CheckState::Passed));
        assert_eq!(of(&[Passed, Running]).overall(), Some(CheckState::Pending));
        assert_eq!(of(&[Pending, Cancelled]).overall(), Some(CheckState::Failed));
        assert_eq!(of(&[ActionRequired]).overall(), Some(CheckState::Failed));
        assert!(of(&[Passed, Pending]).any_active() && !of(&[Failed]).any_active());
    }

    #[test]
    fn finish_cuts_checks_links_and_sorts() {
        use RunState::*;
        let mut runs: Vec<CheckRun> = (0..MAX_CHECKS + 5).map(|i| run(&format!("p{i}"), Passed, None)).collect();
        runs[0].url = "javascript:alert(1)".into();
        runs[1].url = "http://ci.example/1".into();
        runs[2].url = "https://ci.example/\u{202e}x".into();
        runs[3].url = "https://ci.example/3".into();
        runs[4].description = format!("  {}  ", "d".repeat(400));
        let list = finish(CheckList { sha: "s".into(), runs, complete: true });
        assert_eq!((list.runs.len(), list.complete), (MAX_CHECKS, false));
        assert_eq!(list.runs.iter().filter(|r| !r.url.is_empty()).count(), 1);
        assert_eq!(list.first_url(), Some("https://ci.example/3"));
        assert_eq!(list.runs[4].description.len(), 300);

        let runs = vec![
            run("ok", Passed, None),
            run("lint", Failed, Some(false)),
            run("build", Failed, Some(true)),
            run("docs", Skipped, None),
            run("test", Failed, None),
            run("deploy", Running, Some(true)),
        ];
        let names: Vec<String> = finish(CheckList { runs, ..CheckList::default() }).runs.into_iter().map(|r| r.name).collect();
        assert_eq!(names, ["build", "lint", "test", "deploy", "ok", "docs"]);
    }

    #[test]
    fn every_code_host_shows_checks() {
        for p in Provider::ALL {
            let tracker = matches!(p, Provider::JiraCloud | Provider::JiraDataCenter | Provider::Trello);
            assert_eq!(checks_in_app(p), !tracker, "{p:?}");
        }
        assert_eq!(Provider::ALL.iter().filter(|p| checks_in_app(**p)).count(), 7);
        assert_eq!(time("2026-01-02T10:00:00Z"), Some(1_767_348_000));
        assert_eq!((time(""), time_of(&None), time_of(&Some("x".into()))), (None, None, None));
    }
}
