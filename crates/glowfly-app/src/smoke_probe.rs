//! The opt-in state file the Linux installer checks (`scripts/installer-lib-linux.sh`) read to
//! see the installed app start and draw: a small JSON description of what is on screen and in
//! state. Off unless `GLOWFLY_SMOKE_PROBE` names an absolute path; then the shell offers a
//! snapshot (`ui/probe.rs`) after every frame, and only a changed one is written, atomically,
//! by this module's own thread. A write that fails (a reader holding the file on Windows, say)
//! is retried until it lands or a newer snapshot replaces it, since an unchanged state is never
//! offered again.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

/// How long the thread waits before trying a failed write again.
const RETRY: Duration = Duration::from_millis(50);
/// How many more times a failed write is tried once the writer is gone.
const LAST_TRIES: u32 = 3;

/// The variable that turns the probe on: the file to write.
pub const ENV: &str = "GLOWFLY_SMOKE_PROBE";

/// The probe file named by the variable's `value`: None when it is unset, empty or relative (a
/// relative path is ignored with a warning, since it would depend on where the app started).
pub fn probe_path(value: Option<OsString>) -> Option<PathBuf> {
    let value = value.filter(|v| !v.is_empty())?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        tracing::warn!("{ENV} must name an absolute path; ignoring {}", path.display());
        return None;
    }
    Some(path)
}

/// The probe, when the environment asks for one.
pub fn from_env() -> Option<ProbeWriter> {
    let path = probe_path(std::env::var_os(ENV))?;
    tracing::info!("smoke probe on: writing {}", path.display());
    Some(ProbeWriter::start(path))
}

/// Hands changed snapshots to the writing thread, each with the next `seq`.
pub struct ProbeWriter {
    tx: mpsc::Sender<String>,
    /// The last snapshot offered, without its `seq`.
    last: Option<serde_json::Value>,
    seq: u64,
}

impl ProbeWriter {
    /// Starts the thread that writes the file at `path`.
    pub fn start(path: PathBuf) -> Self {
        Self::spawn(path).0
    }

    /// The writer and its thread, which ends once the writer is dropped and the last snapshot
    /// is written.
    fn spawn(path: PathBuf) -> (Self, JoinHandle<()>) {
        let (tx, rx) = mpsc::channel::<String>();
        let thread = std::thread::Builder::new()
            .name("smoke-probe".into())
            .spawn(move || write_loop(&path, &rx))
            .expect("the smoke probe thread starts");
        (Self { tx, last: None, seq: 0 }, thread)
    }

    /// Sends `body` to be written with the next `seq`, unless it equals the last one offered.
    /// Returns whether it was sent.
    pub fn offer(&mut self, body: serde_json::Value) -> bool {
        if self.last.as_ref() == Some(&body) {
            return false;
        }
        self.seq += 1;
        let mut out = body.clone();
        if let Some(map) = out.as_object_mut() {
            map.insert("seq".into(), self.seq.into());
        }
        self.last = Some(body);
        // The thread only stops when this writer is dropped.
        let _ = self.tx.send(out.to_string());
        true
    }
}

/// Writes each snapshot `rx` delivers, keeping the newest one pending until a write lands.
fn write_loop(path: &Path, rx: &mpsc::Receiver<String>) {
    let mut out = Outcome::default();
    let mut pending: Option<String> = None;
    loop {
        let next = match pending {
            None => match rx.recv() {
                Ok(text) => Some(text),
                Err(_) => return,
            },
            Some(_) => match rx.recv_timeout(RETRY) {
                Ok(text) => Some(text),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => {
                    for _ in 0..LAST_TRIES {
                        let Some(text) = pending.take() else { return };
                        pending = out.write(path, text);
                        if pending.is_some() {
                            std::thread::sleep(RETRY);
                        }
                    }
                    return;
                }
            },
        };
        if let Some(mut text) = next {
            // Only the newest snapshot matters.
            while let Ok(newer) = rx.try_recv() {
                text = newer;
            }
            pending = Some(text);
        }
        if let Some(text) = pending.take() {
            pending = out.write(path, text);
        }
    }
}

/// What the writing thread remembers between writes.
#[derive(Default)]
struct Outcome {
    /// A failure was logged already, so later ones stay quiet.
    warned: bool,
    /// The last write failed, so the next one that lands is logged.
    failing: bool,
}

impl Outcome {
    /// Writes `text`; gives it back when the write failed, to be retried.
    fn write(&mut self, path: &Path, text: String) -> Option<String> {
        match glowfly_core::persist::atomic_write(path, text.as_bytes()) {
            Ok(()) => {
                if std::mem::take(&mut self.failing) {
                    tracing::debug!("smoke probe: {} written after a retry", path.display());
                }
                None
            }
            Err(e) => {
                if !self.warned {
                    tracing::warn!("smoke probe: could not write {} (retrying): {e}", path.display());
                    self.warned = true;
                }
                self.failing = true;
                Some(text)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_an_absolute_path_turns_the_probe_on() {
        assert_eq!(probe_path(None), None);
        assert_eq!(probe_path(Some(OsString::new())), None);
        assert_eq!(probe_path(Some(OsString::from("probe.json"))), None);
        let abs = glowfly_core::paths::temp_dir().join("glowfly-probe-path");
        assert_eq!(probe_path(Some(abs.clone().into_os_string())), Some(abs));
    }

    #[test]
    fn only_changed_snapshots_are_written_with_a_rising_seq() {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-probe-writer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("probe.json");
        let (mut w, thread) = ProbeWriter::spawn(path.clone());
        assert!(w.offer(json!({ "a": 1 })));
        assert!(!w.offer(json!({ "a": 1 })), "an unchanged snapshot is not sent again");
        assert!(w.offer(json!({ "a": 2 })));
        drop(w);
        thread.join().unwrap();
        let got: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(got, json!({ "a": 2, "seq": 2 }));
        let names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, ["probe.json"], "no temporary file is left beside it");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Polls `path` for up to 5 s until it holds `want`.
    fn wait_for(path: &Path, want: &serde_json::Value) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let got = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
            if got.as_ref() == Some(want) {
                return;
            }
            assert!(std::time::Instant::now() < deadline, "{} holds {got:?}, not {want}", path.display());
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_failed_write_is_retried_until_it_lands() {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-probe-retry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A file where the probe's folder should be: every write fails until it goes.
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, "").unwrap();
        let path = blocker.join("probe.json");
        let (mut w, thread) = ProbeWriter::spawn(path.clone());
        assert!(w.offer(json!({ "a": 1 })));
        std::thread::sleep(Duration::from_millis(150));
        assert!(!path.exists());
        // The thread makes the folder itself on its next try.
        std::fs::remove_file(&blocker).unwrap();
        wait_for(&path, &json!({ "a": 1, "seq": 1 }));
        drop(w);
        thread.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(windows)]
    #[test]
    fn a_reader_holding_the_file_delays_but_never_loses_the_snapshot() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 1;
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-probe-held-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("probe.json");
        let (mut w, thread) = ProbeWriter::spawn(path.clone());
        assert!(w.offer(json!({ "a": 1 })));
        wait_for(&path, &json!({ "a": 1, "seq": 1 }));
        // Read-only sharing: replacing the file fails while this handle is open.
        let held = std::fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ).open(&path).unwrap();
        assert!(w.offer(json!({ "a": 2 })));
        std::thread::sleep(Duration::from_millis(300));
        drop(held);
        wait_for(&path, &json!({ "a": 2, "seq": 2 }));
        drop(w);
        thread.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
