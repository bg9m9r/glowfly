//! The opt-in state file for the real-input smoke tests (`scripts/smoke-linux.sh`): a small JSON
//! description of what is on screen and in state, which the script reads to find widgets by
//! their accessible names and to check what isn't on disk. Off unless `TRELLIS_SMOKE_PROBE`
//! names an absolute path; then the shell offers a snapshot (`ui/probe.rs`) after every frame,
//! and only a changed one is written, atomically, by this module's own thread.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread::JoinHandle;

/// The variable that turns the probe on: the file to write.
pub const ENV: &str = "TRELLIS_SMOKE_PROBE";

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
            .spawn(move || {
                let mut warned = false;
                while let Ok(mut text) = rx.recv() {
                    // Only the newest snapshot matters.
                    while let Ok(newer) = rx.try_recv() {
                        text = newer;
                    }
                    if let Err(e) = trellis_core::persist::atomic_write(&path, text.as_bytes())
                        && !warned
                    {
                        tracing::warn!("smoke probe: could not write {}: {e}", path.display());
                        warned = true;
                    }
                }
            })
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_an_absolute_path_turns_the_probe_on() {
        assert_eq!(probe_path(None), None);
        assert_eq!(probe_path(Some(OsString::new())), None);
        assert_eq!(probe_path(Some(OsString::from("probe.json"))), None);
        let abs = trellis_core::paths::temp_dir().join("trellis-probe-path");
        assert_eq!(probe_path(Some(abs.clone().into_os_string())), Some(abs));
    }

    #[test]
    fn only_changed_snapshots_are_written_with_a_rising_seq() {
        let dir = trellis_core::paths::temp_dir().join(format!("trellis-probe-writer-{}", std::process::id()));
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
}
