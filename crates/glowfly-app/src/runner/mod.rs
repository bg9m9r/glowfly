//! Executes `Effect`s on worker threads and reports results as `Msg`s. This is the only place
//! the app starts background work.
//!
//! `Runner::run` starts one named thread per effect. A few effects get special handling: the
//! file watcher (`Effect::Watch`), credential-prompt answers, the terminal's shell sessions and
//! the search index's builds and queries (`index`, with its saved copies in `index_disk`) are run
//! by the runner itself, the log files have one ordered worker (`logs`), effects are timed for
//! the performance log, and saves of Glowfly's own files are kept in order (`saves`).
//! `execute.rs` maps each effect to its work: an effect area's `Execute` impl lives in this
//! folder's file for that area (`worktrees`, `rebase`, `sparse`, …), as does work longer than a
//! line for the flat variants (`files`, `highlight`, …); the git, file and network code itself
//! lives in `glowfly-core`.

use crate::effect::Effect;
use crate::msg::Msg;
use glowfly_core::repo;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

mod askpass;
mod background;
mod blame;
mod branches;
mod clone;
mod config;
mod conflict_check;
mod conflict_files;
mod credentials;
mod custom_commands;
mod dashboard;
mod deep_link;
mod diff_gaps;
mod execute;
mod expect;
pub(crate) mod files;
mod forge_calls;
mod forge_filter_calls;
mod forks;
mod git_env;
mod git_ops;
mod guard;
mod highlight;
mod history;
mod index;
mod index_disk;
mod index_query;
mod init;
mod instance;
mod issues;
mod job_steps;
mod journal;
mod keys;
mod lfs;
mod line_counts;
mod load;
mod lock;
mod logs;
mod monitor;
mod notify;
mod onboarding;
mod op_preview;
mod op_stops;
mod os;
mod partial;
mod patches;
mod path_search;
mod pick;
mod profiles;
mod pull_request;
mod queue;
mod range_diff;
mod rebase;
mod repos;
mod revert;
#[cfg(test)]
mod sandbox;
mod saves;
mod selection;
mod signatures;
mod sparse;
mod ssh;
mod stack;
mod stashes;
mod stats;
mod status;
mod stops;
mod submodules;
mod terminal;
mod terminal_hold;
mod terminal_out;
mod themes;
mod tools;
mod trust;
mod trust_gate;
mod ui;
mod undo_run;
mod undo_stash;
mod update_restart;
mod updates;
mod worktrees;

#[cfg(test)]
pub use execute::execute;
use execute::{Cx, err, execute_with};
#[cfg(test)]
pub(crate) use highlight::{decorate, decorate_in_context};
#[cfg(test)]
pub(crate) use sandbox::rule as sandbox_rule;
#[cfg(test)]
pub use trust::execute_checked;

/// Wakes the UI when a result arrives (so it can process the message promptly).
pub trait Waker: Send + Sync + 'static {
    fn wake(&self);
}

impl Waker for eframe::egui::Context {
    fn wake(&self) {
        self.request_repaint();
    }
}

pub struct Runner<W: Waker + Clone> {
    tx: Sender<Msg>,
    waker: W,
    watch: crate::watcher::WatchSlot,
    /// The credential-prompt listener, once started (never in tests).
    askpass: Option<Arc<glowfly_core::askpass::Server>>,
    /// The listener for links a second Glowfly process hands over, once started (never in
    /// tests); dropping it removes the instance file.
    instance: Option<glowfly_core::instance::Listener>,
    /// Counts HighlightDiff jobs started: a job whose number is no longer the latest stops.
    highlight_epoch: Arc<AtomicU64>,
    /// The built-in terminal's shell sessions.
    terminals: terminal::Terminals,
    /// Keeps saves of each settings file in order.
    saves: saves::Saves,
    /// Effects that change a repository run one at a time per repository.
    queues: Arc<queue::RepoQueues>,
    /// Works out the graph's line counts, newest request first.
    stats: stats::StatsWorker,
    /// Finds the commits touching the search's path terms, newest request first.
    path_search: path_search::PathSearchWorker,
    /// Builds the tabs' whole-history search indexes, one at a time, and asks them for matches.
    index: index::IndexWorker,
    /// Ahead/behind and containment answers, kept between repository loads.
    graph_cache: Arc<repo::GraphCache>,
    /// The log files' worker (attached at startup, never in tests).
    logs: logs::LogWorker,
    /// The stops of the running git commands that can be cancelled (`Effect::StopGit`).
    stops: Arc<stops::Stops>,
    /// Whole-app tests: where saves go and which effects are dropped (`sandbox.rs`).
    #[cfg(test)]
    sandbox: Option<sandbox::Sandbox>,
    #[cfg(test)]
    tally: Arc<sandbox::Tally>,
}

impl<W: Waker + Clone> Runner<W> {
    pub fn new(tx: Sender<Msg>, waker: W) -> Self {
        Self {
            tx,
            waker,
            watch: Default::default(),
            askpass: None,
            instance: None,
            highlight_epoch: Default::default(),
            terminals: Default::default(),
            saves: Default::default(),
            queues: Default::default(),
            graph_cache: Default::default(),
            stats: Default::default(),
            path_search: Default::default(),
            index: Default::default(),
            logs: Default::default(),
            stops: Default::default(),
            #[cfg(test)]
            sandbox: None,
            #[cfg(test)]
            tally: Default::default(),
        }
    }

    /// Starts the log worker on the folder `logging::init` chose. Once, at startup.
    pub fn attach_logs(&mut self, setup: crate::logging::LogSetup) {
        self.logs.attach(setup, self.tx.clone(), self.waker.clone());
    }

    /// The search index store, at startup (never in tests).
    pub fn attach_index_store(&mut self, dir: Option<std::path::PathBuf>) {
        if let Some(d) = dir {
            self.index.attach_store(d);
        }
    }

    /// Adds a line to the performance log (with the log worker running): `what` took `took`.
    /// The shell's slow frames come this way.
    pub fn log_perf(&self, took: std::time::Duration, what: &str, detail: String) {
        if let Some(probe) = self.logs.perf_probe() {
            let _ = probe.send(logs::Job::Perf { took, what: what.to_owned(), detail });
        }
    }

    /// Starts `effect` on a worker thread, or queues it behind the earlier effects that change the
    /// same repository. UI effects must be handled by the caller instead. A panic in the effect
    /// is reported as its failure, so a job it was running ends. An effect with a trust re-check
    /// (`Effect::Checked`) runs only if that passes, checked on its worker just before it
    /// starts (a shell's in its session thread; runner/trust.rs).
    pub fn run(&self, effect: Effect) {
        match effect {
            Effect::Checked(c) => {
                let crate::effect::Checked { effect, check } = *c;
                self.run_checked(effect, Some(check))
            }
            e => self.run_checked(e, None),
        }
    }

    fn run_checked(&self, effect: Effect, check: Option<crate::effect::TrustCheck>) {
        #[cfg(test)]
        let Some(effect) = self.admit(effect) else { return };
        let effect = match effect {
            Effect::Watch(target) => return self.watch.set(target, self.tx.clone(), self.waker.clone()),
            Effect::SweepWatch { workdir } => return self.sweep_watch(workdir),
            Effect::AnswerPrompt { id, reply } => return self.answer_prompt(id, reply),
            Effect::Terminal(t) => return self.terminal(t, check),
            Effect::Stats { workdir, oids } => return self.stats.submit(workdir, oids, self.tx.clone(), self.waker.clone()),
            Effect::Logs(e) if e.on_log_worker() => return self.logs.submit(e),
            Effect::StopGit(key) => {
                let asked = self.stops.stop(key);
                tracing::debug!(?key, asked, "stop git");
                return;
            }
            Effect::SearchPaths { workdir, request, terms, oids } => {
                let r = path_search::Request { workdir, request, terms, oids };
                return self.path_search.submit(r, self.tx.clone(), self.waker.clone());
            }
            Effect::Index(e) => return self.index.submit(e, self.tx.clone(), self.waker.clone()),
            other => match self.save(other) {
                Some(other) => other,
                None => return,
            },
        };
        let tx = self.tx.clone();
        let waker = self.waker.clone();
        let name = effect.name();
        let epoch = &self.highlight_epoch;
        let ticket = matches!(effect, Effect::HighlightDiff { .. }).then(|| (epoch.clone(), epoch.fetch_add(1, Ordering::SeqCst) + 1));
        let repo = effect.queue().map(Path::to_owned);
        let hold = effect.holds().map(Path::to_owned);
        let recovery = guard::Recovery::of(&effect, name);
        // What a check that fails sends back: the failure its sender waits for.
        let failed = guard::Recovery::of(&effect, name);
        let graph_cache = Arc::clone(&self.graph_cache);
        // Registered now, so a stop asked while it waits in its repository's queue is kept.
        let stop = effect.stop_key().map(|key| (key, self.stops.register(key)));
        let stops = Arc::clone(&self.stops);
        // Effects that don't wait on the user are timed for the performance log.
        let perf = if effect.timed() { self.logs.perf_probe().map(|p| (p, logs::folder_of(&effect))) } else { None };
        #[cfg(test)]
        let busy = self.tally.begin(name);
        let job: queue::Job = Box::new(move || {
            let _span = tracing::debug_span!("effect", name).entered();
            let _hold = hold.as_deref().map(glowfly_core::repo_pool::hold);
            let cancelled = || ticket.as_ref().is_some_and(|(e, mine)| e.load(Ordering::Relaxed) != *mine);
            let started = std::time::Instant::now();
            let send = |m: Msg| {
                if tx.send(m).is_ok() {
                    waker.wake();
                }
            };
            let stop_ref = stop.as_ref().map(|(_, s)| s);
            let cx = Cx { cancelled: &cancelled, graph_cache: &graph_cache, stop: stop_ref, gate: None, send: &send };
            let msg = guard::guarded(recovery, || match check {
                Some(c) => trust::checked(c, effect, |text| failed.failed(text), &cx),
                None => execute_with(effect, &cx),
            });
            if let Some((key, s)) = &stop {
                stops.release(*key, s);
            }
            let took = started.elapsed();
            if let Some((probe, detail)) = perf
                && took >= glowfly_core::logs::SLOW
            {
                let _ = probe.send(logs::Job::Perf { took, what: name.to_owned(), detail });
            }
            if let Some(msg) = msg
                && tx.send(msg).is_ok()
            {
                waker.wake();
            }
            #[cfg(test)]
            drop(busy);
        });
        let job = match repo {
            Some(repo) => match self.queues.submit(&repo, job) {
                Ok(()) => return,
                Err(job) => job,
            },
            None => job,
        };
        if let Err(e) = std::thread::Builder::new().name(format!("glowfly-{name}")).spawn(job) {
            tracing::error!("could not start a worker thread for {name}: {e}");
        }
    }

    /// Has the watcher of `workdir` sweep its tree; with none running there, reads the whole
    /// status instead (as the poll did before the sweep).
    fn sweep_watch(&self, workdir: std::path::PathBuf) {
        if self.watch.sweep(&workdir) {
            return;
        }
        let msg = crate::msg::WatchMsg::RepoChanged { full: false, paths: None, last_event: std::time::Instant::now() };
        if self.tx.send(Msg::Scoped { workdir, msg: Box::new(Msg::Watch(msg)) }).is_ok() {
            self.waker.wake();
        }
    }
}
