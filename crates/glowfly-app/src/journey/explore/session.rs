//! One exploration session: the scene built from a seed, the app opened on its two repositories,
//! and a list of steps, chosen at random or replayed, each checked by the rules. A panic (the
//! harness's settle timeout among them) is the no-crash rule breaking at that step. Also the
//! case text a failing session is saved as, and the report it prints.
//!
//! A dialog left open is settled before choosing; when it offers no usable button even then,
//! Escape closes it. That Escape is not a step and is not recorded: a replay meets the same
//! dialog and presses it again.

use super::super::fixture::{Fixture, git_output};
use super::super::harness::Journey;
use super::action::{self, Action};
use super::choose::applicable;
use super::perform::{Done, perform};
use super::probe::{self, Reading, took_effect};
use super::rng::Rng;
use super::rules::{self, Broken, Rule};
use super::scene::{self, Scene};
use super::shadow::Shadow;
use eframe::egui::Key;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

/// A fault planned for a step (an effect made to fail or stall); layer 2 fills this in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Fault {}

#[derive(Clone, Debug)]
pub(super) struct Step {
    pub action: Action,
    #[expect(dead_code, reason = "layer 2 plans faults per step; layer 1 records none")]
    pub faults: Vec<Fault>,
}

pub(super) enum Plan {
    Random { steps: usize },
    Replay(Vec<Action>),
}

/// How often the slow rules run.
#[derive(Clone, Copy, Debug)]
pub(super) struct Checks {
    /// Rules 2 and 3 (`git fsck` and the reachability scan) run after every this many steps,
    /// and at the end; 1 runs them after every step.
    pub slow_every: usize,
}

#[derive(Debug)]
pub(super) struct Outcome {
    pub seed: u64,
    pub steps: Vec<Step>,
    pub broke: Option<Broken>,
    /// The active repository at the end: its branch, `git status --short` and the last commits.
    pub state: String,
}

/// What a session carries from step to step. It lives outside the part that may panic, so the
/// steps taken so far survive one.
struct Walk<'a> {
    scene: &'a Scene,
    checks: Checks,
    rng: Rng,
    shadow: Shadow,
    steps: Vec<Step>,
    /// The action being performed, recorded with the panic if it panics.
    current: Option<Action>,
    /// The active tab: `app` or `lib`.
    active: String,
    /// The tab and the reading from before the last performed step, when that step recorded an
    /// action in the app's undo history: what an Undo next must return to.
    undoable: Option<(String, Reading)>,
}

/// Runs a session on the scene of `seed`; `tag` keeps the folders of sessions running at once apart.
pub(super) fn run(tag: &str, seed: u64, plan: Plan, checks: Checks) -> Outcome {
    let scene = scene::build(tag, seed);
    let mut walk = Walk {
        scene: &scene,
        checks: Checks { slow_every: checks.slow_every.max(1) },
        rng: Rng::new(seed),
        shadow: Shadow::new(seed),
        steps: Vec::new(),
        current: None,
        active: "app".to_owned(),
        undoable: None,
    };
    // The journey lives inside: a panic drops it (the app and its threads) before the scene.
    let broke = match catch_unwind(AssertUnwindSafe(|| walk.session(tag, seed, plan))) {
        Ok(broke) => broke,
        Err(payload) => {
            walk.steps.extend(walk.current.take().map(|action| Step { action, faults: Vec::new() }));
            Some(Broken { rule: Rule::NoCrash, step: walk.steps.len(), detail: panic_text(payload.as_ref()) })
        }
    };
    let state = state_of(walk.dir(&walk.active));
    Outcome { seed, steps: walk.steps, broke, state }
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    let text = payload.downcast_ref::<&str>().map(|s| (*s).to_owned()).or_else(|| payload.downcast_ref::<String>().cloned());
    format!("panicked: {}", text.unwrap_or_else(|| "(no message)".to_owned()))
}

/// The branch, `git status --short` and `git log --oneline -5 --all` of `dir`.
fn state_of(dir: &Path) -> String {
    let (_, branch) = git_output(dir, &["branch", "--show-current"]);
    let (_, status) = git_output(dir, &["status", "--short"]);
    let (_, log) = git_output(dir, &["log", "--oneline", "-5", "--all"]);
    let branch = if branch.trim().is_empty() { "(none: detached or unborn)" } else { branch.trim() };
    format!("branch: {branch}\ngit status --short:\n{}\ngit log --oneline -5 --all:\n{}", status.trim_end(), log.trim_end())
}

/// The active tab's name, `lib` or `app`.
fn active_tab(j: &Journey) -> String {
    let st = j.state();
    let lib = st.tabs.get(st.active_tab).is_some_and(|t| t.name == "lib");
    (if lib { "lib" } else { "app" }).to_owned()
}

/// What rule 6 holds `action` to. One that stopped on new conflicts (a merge, a rebase or its
/// next step, a pop) did something and also didn't finish: the app says so with a failure toast
/// ("Stopped on conflicts…"), and the conflicts on screen are rule 4's, so either is right here.
fn took(action: &Action, before: &Reading, after: &Reading) -> Option<bool> {
    let stopped = !after.conflicted.is_empty() && !after.conflicted.is_subset(&before.conflicted);
    if stopped { None } else { took_effect(action, before, after) }
}

impl Walk<'_> {
    fn dir(&self, tab: &str) -> &Path {
        if tab == "lib" { &self.scene.lib } else { &self.scene.app }
    }

    fn session(&mut self, tag: &str, seed: u64, plan: Plan) -> Option<Broken> {
        let s = self.scene;
        let fx = Fixture::new(&format!("explore-ui-{tag}-{seed}"));
        let mut j = Journey::start_many(fx, s.app.clone(), vec![s.lib.clone()], vec![s.app.clone(), s.lib.clone()]);
        let (count, replay) = match plan {
            Plan::Random { steps } => (steps, None),
            Plan::Replay(list) => (list.len(), Some(list)),
        };
        for at in 0..count {
            self.unstick(&mut j);
            self.active = active_tab(&j);
            let before = probe::read(self.dir(&self.active));
            let mark = j.log_mark();
            let action = match &replay {
                Some(list) => list[at].clone(),
                None => {
                    let options = applicable(&j, s, &self.active, &before, &mut self.rng, &mut self.shadow);
                    match self.rng.weighted(&options) {
                        Some(action) => action.clone(),
                        None => continue,
                    }
                }
            };
            if let Some(broke) = self.step(&mut j, action, before, mark) {
                return Some(broke);
            }
        }
        let n = self.steps.len();
        if n == 0 || !n.is_multiple_of(self.checks.slow_every) { self.slow(n).err() } else { None }
    }

    /// Settles an open dialog; when it still offers no usable button, presses Escape.
    fn unstick(&mut self, j: &mut Journey) {
        for _ in 0..2 {
            if !j.modal_open() {
                return;
            }
            j.settle();
            let r = probe::read(self.dir(&active_tab(j)));
            // With a dialog open, `applicable` lists only its buttons and draws nothing.
            if !j.modal_open() || !applicable(j, self.scene, &self.active, &r, &mut self.rng, &mut self.shadow).is_empty() {
                return;
            }
        }
        j.press(Key::Escape);
    }

    /// Performs `action`, records it and checks the rules after it.
    fn step(&mut self, j: &mut Journey, action: Action, before: Reading, mark: u64) -> Option<Broken> {
        let n = self.steps.len() + 1;
        let tab = self.active.clone();
        let recorded = j.state().session.undo.recorded;
        self.current = Some(action.clone());
        let done = perform(j, self.scene, &tab, &mut self.shadow, &action);
        j.settle();
        let after = probe::read(self.dir(&tab));
        self.active = active_tab(j);
        self.steps.extend(self.current.take().map(|action| Step { action, faults: Vec::new() }));
        let broken = |rule: Rule, detail: String| Some(Broken { rule, step: n, detail });

        if let Err(detail) = rules::no_crash(j) {
            return broken(Rule::NoCrash, detail);
        }
        let shown = if self.active == tab { after.clone() } else { probe::read(self.dir(&self.active)) };
        if let Err(detail) = rules::screen_agrees(j, &shown) {
            return broken(Rule::ScreenAgrees, detail);
        }
        // An action that opened a dialog isn't finished: the dialog's button is the next step.
        let finished = matches!(done, Done::Performed) && !j.modal_open();
        let toasts = j.toasts_since_action();
        let errors = j.errors_logged_since(mark);
        let refused = toasts.iter().any(|(ok, _)| !ok) || !errors.is_empty();
        if action == Action::Undo
            && finished
            && !refused
            && let Some((undone_tab, earlier)) = &self.undoable
            && *undone_tab == tab
            && let Err(detail) = rules::undo_returns(earlier, &after)
        {
            return broken(Rule::UndoReturns, detail);
        }
        if finished && let Err(detail) = rules::errors_reported(took(&action, &before, &after), &toasts, &errors) {
            return broken(Rule::ErrorsReported, format!("{detail} (toasts {toasts:?}, logged errors {errors:?})"));
        }
        if matches!(done, Done::Performed) {
            let in_app = !matches!(action, Action::Outside(_)) && self.active == tab;
            let undoable = in_app && j.state().session.undo.recorded > recorded;
            self.undoable = undoable.then_some((tab, before));
        }
        if n.is_multiple_of(self.checks.slow_every) { self.slow(n).err() } else { None }
    }

    /// Rules 2 and 3 on both repositories.
    fn slow(&self, n: usize) -> Result<(), Broken> {
        for name in ["app", "lib"] {
            let dir = self.dir(name);
            rules::sound(dir).map_err(|detail| Broken { rule: Rule::Sound, step: n, detail })?;
            rules::no_lost_work(dir, name, &self.shadow).map_err(|detail| Broken { rule: Rule::NoLostWork, step: n, detail })?;
        }
        Ok(())
    }
}

/// What a person reads when a session broke a rule: the rule, the step and what was wrong, the
/// seed, the (shrunk) actions as a case to save, and the state of the active repository.
pub(super) fn report(o: &Outcome, shrunk: &[Action]) -> String {
    let Some(b) = &o.broke else {
        return format!("seed {}: no rule broke in {} steps\n{}", o.seed, o.steps.len(), o.state);
    };
    format!(
        "exploration broke rule {} at step {} (seed {}):\n{}\n\nthe session, shrunk from {} to {} actions; save it as \
         src/journey/explore/cases/<name>.txt:\n{}\nrepository state at the end of the session:\n{}\n",
        b.rule.name(),
        b.step,
        o.seed,
        b.detail,
        o.steps.len(),
        shrunk.len(),
        case_text(o.seed, b.rule, shrunk),
        o.state,
    )
}

/// A saved case: a comment naming the rule, `seed <n>`, then one action per line.
pub(super) fn case_text(seed: u64, rule: Rule, actions: &[Action]) -> String {
    let mut text = format!("# An exploration session that broke rule {}.\nseed {seed}\n", rule.name());
    for a in actions {
        text.push_str(&format!("{a}\n"));
    }
    text
}

/// The seed and actions of a case; blank lines and `#` comments are skipped.
pub(super) fn parse_case(text: &str) -> Result<(u64, Vec<Action>), String> {
    let mut seed = None;
    let mut actions = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let at = |e: String| format!("line {}: {e}", i + 1);
        match seed {
            None => {
                let n = line.strip_prefix("seed ").ok_or_else(|| at(format!("expected `seed <n>`, found {line:?}")))?;
                seed = Some(n.trim().parse::<u64>().map_err(|e| at(format!("bad seed {n:?}: {e}")))?);
            }
            Some(_) => actions.push(action::parse(line).map_err(at)?),
        }
    }
    seed.map(|s| (s, actions)).ok_or_else(|| "no `seed <n>` line".to_owned())
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
