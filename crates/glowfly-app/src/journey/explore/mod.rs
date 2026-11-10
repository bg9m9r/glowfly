//! Exploration tests: seeded random sessions on the whole-app journey harness, with the six rules
//! checked after every step.
//!
//! - `explore_pr` runs a few fixed seeds on every build (`SEEDS` x `STEPS`, measured to stay near
//!   a minute on the Windows CI runner); `explore_cases` replays every saved case in `cases/`;
//!   `explore_soak` (ignored) runs fresh seeds for as long as the environment asks.
//! - A broken rule is shrunk to the shortest list of actions that still breaks it, written to
//!   `glowfly-explore-<seed>.txt` in the temporary folder and printed as a case to save.
//!
//! The pieces:
//!
//! - `rng`: the seeded generator (splitmix64).
//! - `action`: the action vocabulary and its one-line case text.
//! - `scene`: the fixture repositories built from a seed.
//! - `shadow`: the markers of live edits the session expects to find again.
//! - `reach`: whether markers and commit summaries are reachable in a repository.
//! - `choose`: the actions the screen offers a step, with their weights.
//! - `probe`: whether a step's action took effect, from readings of the repository around it.
//! - `rules`: the six invariants every step must keep.
//! - `perform`: doing one action, through the app's widgets or on disk (`perform_outside`).
//! - `session`: a whole session (random or replayed) with the rules after each step, its case
//!   text and report.
//! - `shrink`: shrinking a failing session to the shortest action list that still fails.

mod action;
mod choose;
mod perform;
mod perform_outside;
mod probe;
mod reach;
mod rng;
mod rules;
mod scene;
mod session;
mod shadow;
mod shrink;

use action::Action;
use session::{Checks, Outcome, Plan, case_text, parse_case, report, run};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// The seeds every build explores, and how many steps each takes: about 40 s here (a fifth of a
/// second a step), near a minute on the Windows CI runner. Between them these seeds commit, stage
/// hunks, stash and drop stashes, make branches, and resolve, continue and abort conflicts.
const SEEDS: [u64; 3] = [2, 7, 9];
const STEPS: usize = 60;

/// How long shrinking a failing session may take, and how many replays it runs at once at most
/// (half the processors, up to this).
const SHRINK_FOR: Duration = Duration::from_secs(60);
const SHRINK_WIDTH: usize = 4;

fn actions(o: &Outcome) -> Vec<Action> {
    o.steps.iter().map(|s| s.action.clone()).collect()
}

/// The report of a session that broke a rule: its actions shrunk (each replay asking for the same
/// rule), the case written to the temporary folder.
fn shrunk_report(tag: &str, o: &Outcome) -> String {
    let Some(broke) = &o.broke else { return report(o, &actions(o)) };
    let rule = broke.rule;
    // A replay runs the slow rules once, at its end: a repository broken or work lost on the way
    // stays so, and replays after every step of them would leave no time to shrink a long
    // session. The other rules are checked after every step anyway.
    let slow_every = usize::MAX;
    // Each replay gets folders of its own: replays run side by side, and on Windows a finished
    // one's may not be gone yet.
    let tries = AtomicUsize::new(0);
    let began = Instant::now();
    let replay = |list: &[Action]| {
        let n = tries.fetch_add(1, Ordering::SeqCst) + 1;
        let again = run(&format!("{tag}-shrink{n}"), o.seed, Plan::Replay(list.to_vec()), Checks { slow_every });
        again.broke.is_some_and(|b| b.rule == rule)
    };
    let width = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).clamp(1, SHRINK_WIDTH));
    let shrunk = shrink::shrink(actions(o), replay, began + SHRINK_FOR, width);
    let took = format!("shrunk in {} replays, {width} at a time, {:.0?}", tries.load(Ordering::SeqCst), began.elapsed());
    let path = glowfly_core::paths::temp_dir().join(format!("glowfly-explore-{}.txt", o.seed));
    let saved = match std::fs::write(&path, case_text(o.seed, rule, &shrunk)) {
        Ok(()) => format!("the case is also in {}", path.display()),
        Err(e) => format!("the case could not be written to {}: {e}", path.display()),
    };
    format!(
        "{}{saved} ({took})
",
        report(o, &shrunk)
    )
}

#[test]
fn explore_pr() {
    for seed in SEEDS {
        let o = run("pr", seed, Plan::Random { steps: STEPS }, Checks { slow_every: 10 });
        if o.broke.is_some() {
            panic!("{}", shrunk_report("pr", &o));
        }
    }
}

/// The saved cases, sorted by file name: `(name, text)`.
fn saved_cases(dir: &Path) -> Vec<(String, String)> {
    let mut cases: Vec<(String, String)> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("reading {name}: {e}"));
            (name, text)
        })
        .collect();
    cases.sort();
    cases
}

#[test]
fn explore_cases() {
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/journey/explore/cases"));
    for (name, text) in saved_cases(dir) {
        let (seed, list) = parse_case(&text).unwrap_or_else(|e| panic!("cases/{name}: {e}"));
        let o = run("case", seed, Plan::Replay(list), Checks { slow_every: 1 });
        assert!(o.broke.is_none(), "cases/{name} breaks again:\n{}", report(&o, &actions(&o)));
    }
}

/// The number in the environment variable `name`, or `default` when it is unset.
fn env_number(name: &str, default: u64) -> u64 {
    match std::env::var(name) {
        Ok(v) => v.trim().parse().unwrap_or_else(|e| panic!("{name}={v:?}: {e}")),
        Err(_) => default,
    }
}

/// A long run on fresh seeds: `GLOWFLY_EXPLORE_SEEDS` seeds (20) of `GLOWFLY_EXPLORE_STEPS`
/// steps (500) from `GLOWFLY_EXPLORE_START` (1), the slow rules after every step. Every failure
/// is shrunk and reported before the test fails.
#[test]
#[ignore = "a long run; see CLAUDE.md's Commands"]
fn explore_soak() {
    let seeds = env_number("GLOWFLY_EXPLORE_SEEDS", 20);
    let steps = env_number("GLOWFLY_EXPLORE_STEPS", 500) as usize;
    let start = env_number("GLOWFLY_EXPLORE_START", 1);
    let mut failures = Vec::new();
    for seed in start..start + seeds {
        let began = Instant::now();
        let o = run("soak", seed, Plan::Random { steps }, Checks { slow_every: 1 });
        println!(
            "seed {seed}: {} steps in {:.1?}, {}",
            o.steps.len(),
            began.elapsed(),
            o.broke.as_ref().map_or("passed", |b| b.rule.name())
        );
        if o.broke.is_some() {
            let text = shrunk_report("soak", &o);
            println!("{text}");
            failures.push(text);
        }
    }
    assert!(failures.is_empty(), "{} of {seeds} seeds broke a rule:\n\n{}", failures.len(), failures.join("\n\n"));
}
