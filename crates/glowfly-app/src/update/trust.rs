//! Repository trust: asking once whether to trust a repository whose own config or hooks make
//! git run programs (and again when a trusted one gains more), recording the answer with the
//! print of what the user saw, bringing the prompt back on request (the toolbar's Untrusted chip,
//! the palette), and trusting repositories Trellis cloned or created. What is held until then is
//! checked where it starts (`AppState::runs_git_unasked`); a job the user starts is gated on the
//! repository it runs in (`gate`): one where git is held waits here for the decision (`hold`),
//! runs on Trust and run, and is dropped on Cancel. Passing trust on to worktrees and submodules
//! Trellis makes from a trusted repository is in trust_inherit.rs.

use super::jobs::Started;
use crate::effect::{Effect, LoadKind};
use crate::msg::TrustMsg;
use crate::state::tabs::is_repo_at;
use crate::state::trust::{Created, HeldJob, Standing, TrustPrompt, prompt_for, record, record_trusted, standing};
use crate::state::{AppState, Modal, RunningJob};
use glowfly_core::paths::same_spelling;
use glowfly_core::settings::RepoTrust;
use std::path::Path;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: TrustMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        TrustMsg::Decide { real, trusted } => decide(st, &real, trusted, now, fx),
        TrustMsg::Ask { real } => ask(st, &real, false),
        TrustMsg::AskForTerminal { real } => ask(st, &real, true),
        TrustMsg::Inherited { kind, from, repos, open } => super::trust_inherit::inherited(st, kind, &from, repos, open, fx),
    }
}

/// Whether git is held in a repository that stands `s`.
fn holds(s: Standing) -> bool {
    matches!(s, Standing::Undecided | Standing::NotTrusted | Standing::Changed)
}

/// Shows the prompt again for the active repository at `real`, if git is held there (Undecided,
/// NotTrusted or Changed) and no other dialog is open; `terminal`: on behalf of its held
/// terminal shells. Closes the palette, which never covers a dialog.
fn ask(st: &mut AppState, real: &Path, terminal: bool) {
    let Some(snap) = st.snap.as_ref().filter(|s| same_spelling(&s.real_workdir, real)) else { return };
    if !holds(standing(&st.settings, snap)) || st.modal.is_some() {
        return;
    }
    let mut prompt = prompt_for(&st.settings, snap);
    prompt.terminal = terminal;
    if !st.trust.asked.iter().any(|p| same_spelling(p, &prompt.real)) {
        st.trust.asked.push(prompt.real.clone());
    }
    st.palette = None;
    st.modal = Some(Modal::Trust(prompt));
}

fn decide(st: &mut AppState, real: &Path, trusted: bool, now: Instant, fx: &mut Vec<Effect>) {
    // Only the prompt that names the held job can release it; any other held job is stale.
    let held = st.trust.held.take();
    let named = matches!(&st.modal, Some(Modal::Trust(p)) if p.held.is_some());
    if trusted {
        // What the user was shown: the open prompt's scan, else the active repository's.
        let print = match (&st.modal, &st.snap) {
            (Some(Modal::Trust(p)), _) if same_spelling(&p.real, real) => Some(p.scan.print()),
            (_, Some(s)) if same_spelling(&s.real_workdir, real) => Some(s.trust_print.clone()),
            _ => None,
        };
        match print {
            Some(print) => record_trusted(&mut st.settings, real, print),
            None => record(&mut st.settings, real, RepoTrust::Trusted),
        }
        // A later change in this session asks again.
        st.trust.asked.retain(|p| !same_spelling(p, real));
    } else {
        record(&mut st.settings, real, RepoTrust::NotTrusted);
    }
    fx.push(super::prefs::save_settings(st));
    if matches!(st.modal, Some(Modal::Trust(_))) {
        st.modal = None;
    }
    let Some(snap) = st.snap.as_ref().filter(|s| same_spelling(&s.real_workdir, real)) else {
        match held {
            // A job for another open repository runs there whichever tab is active.
            Some(h) if trusted && h.elsewhere && same_spelling(&h.real, real) => {
                st.log(true, format!("{} is trusted.", h.name));
                release(st, h, real, named, fx);
            }
            Some(h) => forget(st, h, trusted),
            None => {}
        }
        return;
    };
    let name = snap.name.clone();
    if trusted {
        st.toast(true, format!("{name} is trusted. Background fetch is on."), now);
        // The load options changed (trusted), so this is a full load whose status git reads.
        super::repo::refresh(st, fx);
        super::forge::refresh(st, true, false, now, fx);
        released(st, fx);
        if let Some(h) = held {
            release(st, h, real, named, fx);
        }
    } else {
        st.log(true, format!("{name} isn't trusted. Trellis won't fetch or contact its hosting service for it on its own."));
        if let Some(h) = held {
            forget(st, h, false);
        }
    }
}

/// Starts the held job `h` exactly as it was requested, now that the repository at `real` is
/// trusted, if it was held for that repository, its prompt was the one decided (`named`), it is
/// still the active one (or the job runs elsewhere anyway) and no other job is running;
/// otherwise drops it.
fn release(st: &mut AppState, h: HeldJob, real: &Path, named: bool, fx: &mut Vec<Effect>) {
    let active = st.snap.as_ref().is_some_and(|s| same_spelling(&s.real_workdir, &h.real));
    let here = named && same_spelling(&h.real, real) && (h.elsewhere || active);
    if !here || super::jobs::busy(st, false) {
        return forget(st, h, true);
    }
    tracing::debug!("running {} now that {} is trusted", h.job.label, h.name);
    st.job_stop = super::job_stop::for_effect(&h.effect);
    st.job = Some(h.job);
    fx.push(h.effect);
}

/// What the trust gate says about a job.
pub(super) enum Gate {
    Run,
    /// Wait for this prompt; true when the job runs in a repository other than the active one.
    Hold(Box<TrustPrompt>, bool),
    /// Not run, for this reason (already worded for the log).
    Refuse(String),
}

/// Whether `job`, about to run in `cwd`, may run, by the decision of the repository at `cwd`:
/// the active one (held while it is Undecided, NotTrusted or Changed), another one open in a
/// tab (its parked snapshot's standing), or one open nowhere (its saved decision: only Trusted
/// runs, as nothing says what its settings and hooks run now). Clones and `git init`s Trellis
/// trusts when they open always run.
pub(super) fn gate(st: &AppState, job: &RunningJob, cwd: &Path) -> Gate {
    if matches!(job.then_open, Some((_, Created::Clone | Created::Init))) {
        return Gate::Run;
    }
    if let Some(snap) = st.snap.as_ref().filter(|s| is_repo_at(s, cwd)) {
        return if st.held() { Gate::Hold(Box::new(prompt_for(&st.settings, snap)), false) } else { Gate::Run };
    }
    let parked = st.tabs.iter().find_map(|t| t.parked.as_ref()?.snap.as_ref().filter(|s| t.is(cwd) || is_repo_at(s, cwd)));
    if let Some(snap) = parked {
        return if holds(standing(&st.settings, snap)) { Gate::Hold(Box::new(prompt_for(&st.settings, snap)), true) } else { Gate::Run };
    }
    let name = cwd.file_name().map_or_else(|| cwd.display().to_string(), |n| n.to_string_lossy().into_owned());
    match st.saved_trust_for(cwd).map(|t| t.trust) {
        Some(RepoTrust::Trusted) => Gate::Run,
        Some(RepoTrust::NotTrusted) => Gate::Refuse(format!("{} wasn't run: you chose not to trust {name}.", job.label)),
        None => Gate::Refuse(format!(
            "{} wasn't run: {name} isn't open, so Trellis can't check what its settings and hooks run. Open it and start it again.",
            job.label
        )),
    }
}

/// Keeps `job` and its `effect` as the held job and opens `prompt` naming it, closing the
/// palette and replacing any other dialog; `elsewhere`: the job runs in a repository other than
/// the active one. Pushes no effect. A quiet job is dropped instead, and so is a second job
/// while a held prompt is open.
pub(super) fn hold(st: &mut AppState, job: RunningJob, effect: Effect, mut prompt: TrustPrompt, elsewhere: bool) -> Started {
    let name = prompt.name.clone();
    if job.quiet {
        st.log_quiet(false, format!("{} skipped: {name} isn't trusted.", job.label));
        return Started::No;
    }
    if st.trust.held.is_some() {
        if matches!(&st.modal, Some(Modal::Trust(p)) if p.held.is_some()) {
            st.log(false, format!("{} wasn't run: decide whether to trust {name} first.", job.label));
            return Started::No;
        }
        // Its prompt was closed some other way.
        if let Some(old) = st.trust.held.take() {
            forget(st, old, false);
        }
    }
    prompt.held = Some(job.label.clone());
    if !st.trust.asked.iter().any(|p| same_spelling(p, &prompt.real)) {
        st.trust.asked.push(prompt.real.clone());
    }
    tracing::debug!("holding {} until {name} is trusted", job.label);
    let real = prompt.real.clone();
    st.palette = None;
    st.modal = Some(Modal::Trust(prompt));
    st.trust.held = Some(HeldJob { real, name, job, effect, elsewhere });
    Started::Held
}

/// The trust prompt was closed without a decision: the held job, if any, is dropped.
pub(super) fn dismissed(st: &mut AppState) {
    if let Some(h) = st.trust.held.take() {
        forget(st, h, false);
    }
}

/// Drops the held job `h`: clears what its caller set up to follow it, in its own tab whether
/// or not that is still the active one, and says it didn't run, and why (`trusted`: the
/// repository is trusted now, but the job couldn't run as asked).
fn forget(st: &mut AppState, h: HeldJob, trusted: bool) {
    let label = h.job.label.as_str();
    st.gitflow_finish.take_if(|j| j.label == label);
    st.tag_job.take_if(|j| j.label == label);
    st.patch_apply.take_if(|j| j.label == label);
    st.auto_stash.making.take_if(|m| m.label == label);
    st.auto_stash.unstashed.take_if(|u| u.label == label);
    if label == super::rebase::JOB_LABEL {
        let active = st.snap.as_ref().is_some_and(|s| is_repo_at(s, &h.real));
        let panel = if active {
            st.rebase.as_mut()
        } else {
            st.tabs
                .iter_mut()
                .filter_map(|t| t.parked.as_mut())
                .find(|p| p.snap.as_ref().is_some_and(|s| is_repo_at(s, &h.real)))
                .and_then(|p| p.rebase.as_mut())
        };
        if let Some(p) = panel {
            p.starting = false;
        }
    }
    let text = if trusted {
        format!("{label} wasn't run: {} is trusted now, so start it again.", h.name)
    } else {
        format!("{label} wasn't run. Trellis runs git commands in {} only after you trust it.", h.name)
    };
    st.log(true, text);
}

/// A job that made the repository at `path` succeeded and Trellis is about to open it: a clone,
/// or a `git init` whose `output` says it made a new repository, is trusted when it loads. (A
/// worktree made from a trusted repository is trusted by trust_inherit.rs.)
pub(super) fn created(st: &mut AppState, path: &Path, how: Created, output: &str) {
    let new = match how {
        Created::Clone => true,
        Created::Init => glowfly_core::trust::init_created_new(output),
        Created::Worktree => false,
    };
    if new {
        st.trust.created.push(path.to_path_buf());
    }
}

/// The active tab's snapshot was just replaced by a full load of `kind`: a repository Trellis
/// made is trusted on what this load found, a decision saved before prints were kept takes this
/// load's print, and one a `git lfs install` from Trellis changed adopts it; then it may ask.
pub(super) fn loaded(st: &mut AppState, kind: LoadKind, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let (real, workdir, print) = (snap.real_workdir.clone(), snap.workdir.clone(), snap.trust_print.clone());
    let made = st.trust.created.iter().position(|p| st.names_repo(snap, p));
    let changed = standing(&st.settings, snap) == Standing::Changed;
    let adopt = st.trust.adopt_next.as_ref().is_some_and(|p| same_spelling(p, &real));
    if adopt {
        st.trust.adopt_next = None;
    }
    let key = [&real, &workdir].into_iter().find(|p| st.settings.saved_trust(p).is_some()).cloned();
    let legacy = key.as_ref().and_then(|k| st.settings.saved_trust(k)).is_some_and(|t| t.is_trusted() && t.print.is_none());
    let save = if let Some(i) = made {
        st.trust.created.remove(i);
        record_trusted(&mut st.settings, &real, print);
        true
    } else if let (true, Some(key)) = (legacy, key) {
        record_trusted(&mut st.settings, &key, print);
        true
    } else if changed && adopt {
        tracing::debug!("adopting what git lfs install added to {}", real.display());
        record_trusted(&mut st.settings, &real, print);
        true
    } else {
        false
    };
    if save {
        fx.push(super::prefs::save_settings(st));
    }
    maybe_ask(st, kind == LoadKind::Open);
    released(st, fx);
}

/// What waits outside the job gate for the active repository to run git unasked: held terminal
/// shells and the LFS and Sparse Checkout pages' reads.
pub(super) fn released(st: &mut AppState, fx: &mut Vec<Effect>) {
    if !st.runs_git_unasked() {
        return;
    }
    super::terminal::release_held(st, fx);
    super::lfs::trust_released(st, fx);
    super::sparse::trust_released(st, fx);
    super::range_diff::trust_released(st, fx);
    super::partial_content::trust_released(st, fx);
}

/// Shows the trust prompt for the active repository if it is undecided (or gained settings or
/// hooks since it was trusted), no other dialog is open, and it wasn't asked about this session
/// (a `fresh_open` asks again).
pub(super) fn maybe_ask(st: &mut AppState, fresh_open: bool) {
    let Some(snap) = &st.snap else { return };
    if !matches!(standing(&st.settings, snap), Standing::Undecided | Standing::Changed) || st.modal.is_some() {
        return;
    }
    let asked = st.trust.asked.iter().any(|p| same_spelling(p, &snap.real_workdir));
    if asked && !fresh_open {
        return;
    }
    tracing::debug!("asking whether to trust {}", snap.real_workdir.display());
    let prompt = prompt_for(&st.settings, snap);
    if !asked {
        st.trust.asked.push(prompt.real.clone());
    }
    st.modal = Some(Modal::Trust(prompt));
}
