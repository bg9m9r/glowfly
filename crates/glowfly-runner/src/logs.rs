//! The log worker: one thread that does every log file's work in order (`Effect::Logs`, slow
//! effects timed by `Runner::run`, and git commands reported by gitcli's run observer). It owns
//! the activity file and the performance log, stamps activity lines with this session's start
//! and the time, and writes the logs bundle after the appends queued before it. The bundle's
//! save dialog runs on an ordinary worker thread (`pick_export`), so it holds no log work up.

use super::Waker;
use super::execute::{Cx, Execute};
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::LOG_LIMIT;
use glowfly_core::gitcli::{self, GitRun};
use glowfly_core::logs::{self, ActivityEntry, ActivityFile, LogTail, RotatingFile};
use glowfly_core::native_path;
use glowfly_model::logs::{EarlierLog, LogKind, LogsEffect, LogsMsg, NewLine};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

/// Switches the error log between its normal and extended levels.
pub type FileDetail = Arc<dyn Fn(bool) + Send + Sync>;

/// What the app's tracing setup (`logging::init`) made, for the log worker.
pub struct LogSetup {
    /// The logs folder; None when this system has no place for it (logs stay in memory).
    pub dir: Option<PathBuf>,
    /// Sets the error log's level for extended logging (true) or not; None without a folder.
    pub file_detail: Option<FileDetail>,
}

/// Appends that fail this many times in a row turn the activity file off for the session.
const MAX_APPEND_FAILURES: u8 = 3;

/// No folder: the error for the file tabs and Open logs folder.
const NO_FOLDER: &str = "Glowfly has no folder for its logs on this system.";

pub(super) enum Job {
    Effect(LogsEffect),
    /// An effect that took `SLOW` or longer: its name and repository folder.
    Perf {
        took: Duration,
        what: String,
        detail: String,
    },
    Git(GitRun),
}

/// The handle `Runner` keeps; not attached (and doing nothing) in tests.
#[derive(Default)]
pub(super) struct LogWorker {
    jobs: Option<Sender<Job>>,
    /// Extended logging is on: every git command reaches the worker, and then the activity log.
    extended: Arc<AtomicBool>,
    file_detail: Option<FileDetail>,
}

impl LogWorker {
    /// Starts the worker thread on `setup`'s folder and has gitcli report every git command to
    /// it. Once, at startup.
    pub(super) fn attach(&mut self, setup: LogSetup, tx: Sender<Msg>, waker: Waker) {
        let Some(jobs) = self.start(setup, tx, waker) else { return };
        let extended = Arc::clone(&self.extended);
        gitcli::observe_runs(Box::new(move |run| {
            if run.took >= logs::SLOW || extended.load(Ordering::Relaxed) {
                let _ = jobs.send(Job::Git(run.clone()));
            }
        }));
    }

    /// Starts the worker thread, without the git observer (tests use this directly).
    pub(super) fn start(&mut self, setup: LogSetup, tx: Sender<Msg>, waker: Waker) -> Option<Sender<Job>> {
        self.file_detail = setup.file_detail;
        let (jobs, rx) = channel();
        let state = Worker::new(setup.dir, Arc::clone(&self.extended), tx, waker);
        if let Err(e) = std::thread::Builder::new().name("glowfly-logs".into()).spawn(move || state.run(rx)) {
            tracing::error!("could not start the log thread: {e}");
            return None;
        }
        self.jobs = Some(jobs.clone());
        Some(jobs)
    }

    pub(super) fn submit(&self, e: LogsEffect) {
        if let LogsEffect::SetExtended(on) = e {
            self.extended.store(on, Ordering::Relaxed);
            if let Some(detail) = &self.file_detail {
                detail(on);
            }
            return;
        }
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(Job::Effect(e));
        }
    }

    /// Where `Runner::run` sends a slow effect's timing; None when not attached.
    pub(super) fn perf_probe(&self) -> Option<Sender<Job>> {
        self.jobs.clone()
    }
}

/// The save dialog waits on the user, so it runs on a worker thread rather than on the log
/// worker; the rest go to the log worker in `Runner::run`.
impl Execute for LogsEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        match self {
            LogsEffect::PickExport { name } => Some(pick_export(&name)),
            LogsEffect::LoadActivity
            | LogsEffect::Append(_)
            | LogsEffect::ClearActivity { .. }
            | LogsEffect::ReadFile { .. }
            | LogsEffect::OpenFolder
            | LogsEffect::SetExtended(_)
            | LogsEffect::Export { .. } => {
                tracing::warn!("log effect reached a worker; it is handled by Runner::run");
                None
            }
        }
    }
}

/// The save dialog for the logs bundle, suggesting `name`. It waits on the user, so it runs on a
/// worker thread (runner/execute.rs), never on the log worker.
pub(super) fn pick_export(name: &str) -> Msg {
    let path =
        rfd::FileDialog::new().set_title("Export logs").set_file_name(name).add_filter("Text files", &[logs::BUNDLE_EXT]).save_file();
    Msg::Logs(LogsMsg::ExportTo(path))
}

/// The repository folder an effect works in, for its performance line ("" for none).
pub(super) fn folder_of(effect: &Effect) -> String {
    let path = match effect {
        Effect::LoadRepo { path, .. } => Some(path.as_path()),
        e => e.scope().map(PathBuf::as_path).or_else(|| e.queue()),
    };
    path.map(native_path::shown).unwrap_or_default()
}

struct Worker {
    dir: Option<PathBuf>,
    /// When this session started, in Unix seconds.
    session: i64,
    extended: Arc<AtomicBool>,
    activity: Option<ActivityFile>,
    /// Lines whose append failed, written with the next batch.
    unsaved: Vec<ActivityEntry>,
    failures: u8,
    perf: Option<RotatingFile>,
    tx: Sender<Msg>,
    waker: Waker,
}

impl Worker {
    fn new(dir: Option<PathBuf>, extended: Arc<AtomicBool>, tx: Sender<Msg>, waker: Waker) -> Self {
        let perf = dir.as_ref().map(|d| RotatingFile::new(d.join(logs::PERF_FILE), logs::PERF_LOG_BYTES, logs::KEEP_ROTATED));
        let session = chrono::Utc::now().timestamp();
        Self { dir, session, extended, activity: None, unsaved: Vec::new(), failures: 0, perf, tx, waker }
    }

    fn run(mut self, rx: Receiver<Job>) {
        for job in rx {
            match job {
                Job::Effect(e) => self.effect(e),
                Job::Perf { took, what, detail } => self.perf(took, &what, &detail),
                Job::Git(run) => {
                    if run.took >= logs::SLOW {
                        let args: Vec<_> = run.args.iter().map(|a| logs::redact_url_userinfo(a)).collect();
                        self.perf(run.took, &format!("git {}", args.join(" ")), &native_path::shown(&run.cwd));
                    }
                    if self.extended.load(Ordering::Relaxed) {
                        self.send(Msg::Logs(LogsMsg::GitRan(run)));
                    }
                }
            }
        }
    }

    fn send(&self, msg: Msg) {
        if self.tx.send(msg).is_ok() {
            self.waker.wake();
        }
    }

    fn effect(&mut self, e: LogsEffect) {
        match e {
            LogsEffect::LoadActivity => {
                let result = match &self.dir {
                    None => Err(NO_FOLDER.to_owned()),
                    Some(dir) => match ActivityFile::open(dir.join(logs::ACTIVITY_FILE), LOG_LIMIT) {
                        Ok((file, entries)) => {
                            self.activity = Some(file);
                            Ok(EarlierLog { dir: dir.clone(), entries })
                        }
                        Err(e) => Err(e.to_string()),
                    },
                };
                self.send(Msg::Logs(LogsMsg::ActivityLoaded(result)));
            }
            LogsEffect::Append(lines) => self.append(lines),
            LogsEffect::ClearActivity { repo } => {
                self.unsaved.retain(|e| !logs::same_repo(e.repo.as_deref(), repo.as_deref()));
                if let Some(Err(e)) = self.activity.as_mut().map(|f| f.remove_repo(repo.as_deref())) {
                    self.send(Msg::Logged { ok: false, text: format!("Couldn't clear the activity log: {e}") });
                }
            }
            LogsEffect::ReadFile { kind, generation } => {
                let result = match (&self.dir, kind) {
                    (_, LogKind::Activity) => Ok(LogTail::default()),
                    (None, _) => Err(NO_FOLDER.to_owned()),
                    (Some(dir), LogKind::Errors) => {
                        logs::read_tail(&dir.join(logs::ERROR_FILE), logs::TAIL_BYTES).map_err(|e| e.to_string())
                    }
                    (Some(dir), LogKind::Performance) => {
                        logs::read_tail(&dir.join(logs::PERF_FILE), logs::TAIL_BYTES).map_err(|e| e.to_string())
                    }
                };
                self.send(Msg::Logs(LogsMsg::FileRead { kind, generation, result }));
            }
            LogsEffect::OpenFolder => {
                let failed = match &self.dir {
                    None => Some(Msg::Logged { ok: false, text: NO_FOLDER.to_owned() }),
                    Some(dir) => match std::fs::create_dir_all(dir) {
                        Err(e) => Some(Msg::Logged { ok: false, text: format!("Couldn't create {}: {e}", native_path::shown(dir)) }),
                        Ok(()) => super::os::open_folder(dir),
                    },
                };
                if let Some(msg) = failed {
                    self.send(msg);
                }
            }
            LogsEffect::Export { dest, header } => {
                let result = match &self.dir {
                    None => Err(NO_FOLDER.to_owned()),
                    Some(dir) => logs::write_bundle(dir, &header, &dest).map_err(|e| e.to_string()),
                };
                self.send(Msg::Logs(LogsMsg::Exported { dest, result }));
            }
            // Handled by `LogWorker::submit` on the UI thread.
            LogsEffect::SetExtended(_) => {}
            // A worker thread runs it (runner/execute.rs): it waits on the user.
            LogsEffect::PickExport { .. } => {}
        }
    }

    /// Appends `lines` (after any that failed before); after `MAX_APPEND_FAILURES` failures in a
    /// row the file is dropped for the session.
    fn append(&mut self, lines: Vec<NewLine>) {
        let Some(file) = self.activity.as_mut() else { return };
        let at = chrono::Utc::now().timestamp();
        let session = self.session;
        self.unsaved.extend(lines.into_iter().map(|l| ActivityEntry { session, at, ok: l.ok, text: l.text, repo: l.repo, run: l.run }));
        let over = self.unsaved.len().saturating_sub(LOG_LIMIT);
        self.unsaved.drain(..over);
        match file.append(&self.unsaved) {
            Ok(()) => {
                self.unsaved.clear();
                self.failures = 0;
            }
            Err(e) => {
                self.failures += 1;
                if self.failures >= MAX_APPEND_FAILURES {
                    self.activity = None;
                    self.unsaved.clear();
                    self.send(Msg::Logs(LogsMsg::AppendFailed(e.to_string())));
                }
            }
        }
    }

    fn perf(&mut self, took: Duration, what: &str, detail: &str) {
        let Some(perf) = self.perf.as_mut() else { return };
        let line = logs::perf_line(chrono::Utc::now().timestamp_millis(), glowfly_core::dates::local_offset(), took, what, detail);
        if let Err(e) = std::io::Write::write_all(perf, line.as_bytes()) {
            tracing::debug!("could not write the performance log: {e}");
        }
    }
}

#[cfg(test)]
#[path = "logs_tests.rs"]
mod tests;
