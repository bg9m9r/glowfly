//! One exploration session: the scene built from a seed, the app opened on its two repositories,
//! and a list of steps, chosen at random or replayed, each checked by the rules. A panic (the
//! harness's settle timeout among them) is the no-crash rule breaking at that step. Also the
//! case text a failing session is saved as, and the report it prints.
//!
//! A dialog left open is settled before choosing; when it offers no usable button even then,
//! Escape closes it. That Escape is not a step and is not recorded: a replay meets the same
//! dialog and presses it again (`Outcome::state` says before which steps it was pressed). A
//! dialog that Escape doesn't close either ends the walk there, and `state` says so.
//!
//! An action that opens a dialog (delete a branch, check out with changes, some undos) is judged
//! by rules 5 and 6 when the dialog's button finishes it, against the reading from before it.

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
use git2::RepositoryState;
use std::collections::BTreeMap;
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
    /// Per tab, the reading from before the step that left its repository in the middle of an
    /// operation (a merge or rebase stopped on conflicts). Undo after the step that finishes the
    /// operation undoes all of it, back to this. None once another undoable action (a stash, a
    /// branch deleted) was recorded during the operation: one Undo doesn't undo that too, so
    /// where it returns isn't known from the readings.
    started: BTreeMap<String, Option<Reading>>,
    /// The action whose dialog is still open, to judge when its button finishes it.
    pending: Option<Judged>,
    /// The steps before which `unstick` pressed Escape.
    escapes: Vec<usize>,
    /// The step before which the walk stopped on a dialog nothing closed.
    stalled: Option<usize>,
}

/// An action as rules 5 and 6 judge it: its tab, the reading and log mark from before it, and
/// what an Undo then had to return to.
#[derive(Clone, Debug)]
struct Judged {
    tab: String,
    action: Action,
    before: Reading,
    mark: u64,
    undoable: Option<(String, Reading)>,
}

/// Which action, if any, this step finishes: `now` itself when it was performed and left no
/// dialog open, or the pending action a dialog's button (not Cancel) finished. An action that
/// opened a dialog waits in `pending`; Cancel drops it.
fn judged(pending: &mut Option<Judged>, now: Judged, performed: bool, dialog_open: bool) -> Option<Judged> {
    match &now.action {
        Action::Confirm(label) if label == "Cancel" && !dialog_open => {
            *pending = None;
            None
        }
        Action::Confirm(_) if dialog_open => None,
        Action::Confirm(_) => pending.take().filter(|p| performed && p.tab == now.tab),
        _ if !performed => None,
        _ if dialog_open => {
            *pending = Some(now);
            None
        }
        _ => {
            *pending = None;
            Some(now)
        }
    }
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
        started: BTreeMap::new(),
        pending: None,
        escapes: Vec::new(),
        stalled: None,
    };
    // The journey lives inside: a panic drops it (the app and its threads) before the scene.
    let broke = match catch_unwind(AssertUnwindSafe(|| walk.session(tag, seed, plan))) {
        Ok(broke) => broke,
        Err(payload) => {
            walk.steps.extend(walk.current.take().map(|action| Step { action, faults: Vec::new() }));
            Some(Broken { rule: Rule::NoCrash, step: walk.steps.len(), detail: panic_text(payload.as_ref()) })
        }
    };
    let mut state = String::new();
    if let Some(at) = walk.stalled {
        state.push_str(&format!("the walk ended early, before step {at}: a dialog that neither its buttons nor Escape closed\n"));
    }
    if !walk.escapes.is_empty() {
        state.push_str(&format!("Escape closed a dialog with no usable button before steps {:?}\n", walk.escapes));
    }
    state.push_str(&state_of(walk.dir(&walk.active)));
    Outcome { seed, steps: walk.steps, broke, state }
}

/// An auto-stash's put-back: the pop that brings the changes back after the operation.
fn is_put_back(a: &glowfly_core::undo::Action) -> bool {
    matches!(a, glowfly_core::undo::Action::StashApply { pop: true, message, .. } if glowfly_core::stash_ops::is_auto_stash(message))
}

/// The actions of a session, in order.
pub(super) fn actions(o: &Outcome) -> Vec<Action> {
    o.steps.iter().map(|s| s.action.clone()).collect()
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
/// next step, a pop, a checkout whose stash came back conflicted) did something and also didn't finish: the app says so with a failure toast
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
        let fx = Fixture::new(&format!("explore-session-{tag}-{seed}"));
        let mut j = Journey::start_many(fx, s.app.clone(), vec![s.lib.clone()], vec![s.app.clone(), s.lib.clone()]);
        let (count, replay) = match plan {
            Plan::Random { steps } => (steps, None),
            Plan::Replay(list) => (list.len(), Some(list)),
        };
        for at in 0..count {
            if !self.unstick(&mut j) {
                self.stalled = Some(self.steps.len() + 1);
                break;
            }
            self.active = active_tab(&j);
            let before = probe::read(self.dir(&self.active));
            let mark = j.log_mark();
            let action = match &replay {
                Some(list) => list[at].clone(),
                None => {
                    let options = applicable(&j, s, &self.active, &before, &mut self.rng);
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

    /// Settles an open dialog; when it still offers no usable button, presses Escape. False when
    /// the dialog is open even then.
    fn unstick(&mut self, j: &mut Journey) -> bool {
        for _ in 0..2 {
            if !j.modal_open() {
                return true;
            }
            j.settle();
            let r = probe::read(self.dir(&active_tab(j)));
            // With a dialog open, `applicable` lists only its buttons and draws nothing.
            if !j.modal_open() || !applicable(j, self.scene, &self.active, &r, &mut self.rng).is_empty() {
                return true;
            }
        }
        self.escapes.push(self.steps.len() + 1);
        self.pending = None;
        j.press(Key::Escape);
        j.settle();
        !j.modal_open()
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
        let before_state = before.state;
        let started = self.started.get(&tab).cloned().flatten();
        if after.state == RepositoryState::Clean {
            self.started.remove(&tab);
        } else if before.state == RepositoryState::Clean {
            self.started.insert(tab.clone(), Some(before.clone()));
        }

        if let Err(detail) = rules::no_crash(j) {
            return broken(Rule::NoCrash, detail);
        }
        let shown = if self.active == tab { after.clone() } else { probe::read(self.dir(&self.active)) };
        if let Err(detail) = rules::screen_agrees(j, &shown) {
            return broken(Rule::ScreenAgrees, detail);
        }
        let performed = matches!(done, Done::Performed);
        let now = Judged { tab: tab.clone(), action: action.clone(), before, mark, undoable: self.undoable.clone() };
        let subject = judged(&mut self.pending, now, performed, j.modal_open());
        if let Some(s) = &subject {
            let toasts = j.toasts_since_action();
            let errors = j.errors_logged_since(s.mark);
            let refused = toasts.iter().any(|(ok, _)| !ok) || !errors.is_empty();
            if s.action == Action::Undo
                && !refused
                && let Some((undone_tab, earlier)) = &s.undoable
                && *undone_tab == s.tab
                && let Err(detail) = rules::undo_returns(earlier, &after)
            {
                return broken(Rule::UndoReturns, detail);
            }
            if let Err(detail) = rules::errors_reported(took(&s.action, &s.before, &after), &toasts, &errors) {
                let what = if s.action == action { String::new() } else { format!(" ({} finished by this dialog)", s.action) };
                return broken(Rule::ErrorsReported, format!("{detail}{what} (toasts {toasts:?}, logged errors {errors:?})"));
            }
        }
        if performed {
            let in_app = !matches!(action, Action::Outside(_)) && self.active == tab;
            // One Undo reverses one entry. A checkout or other operation that stashed the changes
            // and put them back records the put-back too, and one Undo reverses only that. Any
            // other step that recorded more is still held to rule 5.
            let entries = if in_app { j.state().session.undo.recorded.saturating_sub(recorded) } else { 0 };
            let put_back = entries == 2 && j.state().session.undo.undo.last().is_some_and(is_put_back);
            let undoable = in_app && entries > 0 && !put_back;
            // The operation's own steps are part of it; anything else recorded during it is not.
            let what = subject.as_ref().map_or(&action, |s| &s.action);
            let own = matches!(what, Action::ResolveOurs(_) | Action::ResolveTheirs(_) | Action::Discard(_));
            let during = before_state != RepositoryState::Clean && after.state != RepositoryState::Clean;
            if in_app && entries > 0 && !own && during {
                self.started.insert(tab.clone(), None);
            }
            // A step that finishes an operation (Continue, or the commit of a merge) records the
            // whole operation, so Undo returns to before it began.
            let finished = |s: &Judged| s.before.state != RepositoryState::Clean && after.state == RepositoryState::Clean;
            self.undoable =
                subject.filter(|_| undoable).and_then(|s| if finished(&s) { started.map(|r| (tab, r)) } else { Some((tab, s.before)) });
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
        "exploration broke rule {} at step {} (seed {}):\n{}\n\nthe session, shrunk from {} to {} actions; save it as src/journey/explore/cases/<name>.txt:\n{}\nrepository state at the end of the session:\n{}\n",
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
