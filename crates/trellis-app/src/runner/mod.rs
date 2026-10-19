//! Executes `Effect`s on worker threads and reports results as `Msg`s. This is the only place
//! the app starts background work.
//!
//! `Runner::run` starts one named thread per effect. A few effects get special handling: the
//! file watcher (`Effect::Watch`), credential-prompt answers and the terminal's shell sessions
//! are run by the runner itself, the log files have one ordered worker (`logs`), effects are
//! timed for the performance log, and saves of Trellis's own files are kept in order (`saves`).
//! `execute.rs` maps each effect to its work; work longer than a line lives in this folder's
//! per-area files (`files`, `worktrees`, `rebase`, `highlight`, …); the git, file and
//! network code itself lives in `trellis-core`.

use crate::effect::Effect;
use crate::msg::Msg;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use trellis_core::repo;

mod askpass;
mod blame;
mod clone;
mod config;
mod conflict_check;
mod conflict_files;
mod credentials;
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
mod init;
mod instance;
mod issues;
mod keys;
mod line_counts;
mod logs;
mod notify;
mod onboarding;
mod os;
mod patches;
mod path_search;
mod profiles;
mod pull_request;
mod queue;
mod rebase;
mod repos;
mod revert;
#[cfg(test)]
mod sandbox;
mod saves;
mod signatures;
mod sparse;
mod ssh;
mod stashes;
mod stats;
mod status;
mod submodules;
mod terminal;
mod terminal_out;
mod themes;
mod tools;
mod updates;
mod worktrees;

#[cfg(test)]
pub use execute::execute;
use execute::{err, execute_with};
#[cfg(test)]
pub(crate) use highlight::{decorate, decorate_in_context};

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
    askpass: Option<Arc<trellis_core::askpass::Server>>,
    /// The listener for links a second Trellis process hands over, once started (never in
    /// tests); dropping it removes the instance file.
    instance: Option<trellis_core::instance::Listener>,
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
    /// Ahead/behind and containment answers, kept between repository loads.
    graph_cache: Arc<repo::GraphCache>,
    /// The log files' worker (attached at startup, never in tests).
    logs: logs::LogWorker,
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
            logs: Default::default(),
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

    /// Starts `effect` on a worker thread, or queues it behind the earlier effects that change the
    /// same repository. UI effects must be handled by the caller instead. A panic in the effect
    /// is reported as its failure, so a job it was running ends.
    pub fn run(&self, effect: Effect) {
        #[cfg(test)]
        let Some(effect) = self.admit(effect) else { return };
        let effect = match effect {
            Effect::Watch(target) => return self.watch.set(target, self.tx.clone(), self.waker.clone()),
            Effect::AnswerPrompt { id, reply } => return self.answer_prompt(id, reply),
            Effect::Terminal(t) => return self.terminal(t),
            Effect::Stats { workdir, oids } => return self.stats.submit(workdir, oids, self.tx.clone(), self.waker.clone()),
            Effect::Logs(e) => return self.logs.submit(e),
            Effect::SearchPaths { workdir, request, terms, oids } => {
                let r = path_search::Request { workdir, request, terms, oids };
                return self.path_search.submit(r, self.tx.clone(), self.waker.clone());
            }
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
        let repo = queue::repo_of(&effect).map(Path::to_owned);
        let recovery = guard::Recovery::of(&effect, name);
        let graph_cache = Arc::clone(&self.graph_cache);
        // Effects that don't wait on the user are timed for the performance log.
        let perf = if effect.timed() { self.logs.perf_probe().map(|p| (p, logs::folder_of(&effect))) } else { None };
        #[cfg(test)]
        let busy = self.tally.begin(name);
        let job: queue::Job = Box::new(move || {
            let _span = tracing::debug_span!("effect", name).entered();
            let cancelled = || ticket.as_ref().is_some_and(|(e, mine)| e.load(Ordering::Relaxed) != *mine);
            let started = std::time::Instant::now();
            let msg = guard::guarded(recovery, || execute_with(effect, &cancelled, &graph_cache));
            let took = started.elapsed();
            if let Some((probe, detail)) = perf
                && took >= trellis_core::logs::SLOW
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
        if let Err(e) = std::thread::Builder::new().name(format!("trellis-{name}")).spawn(job) {
            tracing::error!("could not start a worker thread for {name}: {e}");
        }
    }
}
