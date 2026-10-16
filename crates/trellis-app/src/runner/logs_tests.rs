use super::*;
use std::sync::mpsc::Receiver;

#[derive(Clone)]
struct NoWake;

impl Waker for NoWake {
    fn wake(&self) {}
}

fn worker(dir: &std::path::Path) -> (LogWorker, Receiver<Msg>) {
    let (tx, rx) = channel();
    let mut w = LogWorker::default();
    let setup = LogSetup { dir: Some(dir.to_owned()), file_detail: None };
    assert!(w.start(setup, tx, NoWake).is_some());
    (w, rx)
}

fn next(rx: &Receiver<Msg>) -> LogsMsg {
    match rx.recv_timeout(Duration::from_secs(10)).expect("the log worker answers") {
        Msg::Logs(m) => m,
        _ => panic!("not a logs message"),
    }
}

#[test]
fn worker_keeps_the_activity_log_across_sessions() {
    let dir = trellis_core::paths::temp_dir().join(format!("trellis-log-worker-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let before = chrono::Utc::now().timestamp();

    let (w, rx) = worker(&dir);
    w.submit(LogsEffect::LoadActivity);
    assert_eq!(next(&rx), LogsMsg::ActivityLoaded(Ok(EarlierLog { dir: dir.clone(), entries: Vec::new() })));
    w.submit(LogsEffect::Append(vec![(true, "Fetched origin".into()), (false, "Push failed:\nrejected".into())]));
    // A missing log reads as empty; the answer also shows the append before it was done.
    w.submit(LogsEffect::ReadFile { kind: LogKind::Errors, generation: 7 });
    assert_eq!(next(&rx), LogsMsg::FileRead { kind: LogKind::Errors, generation: 7, result: Ok(LogTail::default()) });
    drop(w);
    let after = chrono::Utc::now().timestamp();

    let (w, rx) = worker(&dir);
    w.submit(LogsEffect::LoadActivity);
    let LogsMsg::ActivityLoaded(Ok(earlier)) = next(&rx) else { panic!("not loaded") };
    let texts: Vec<(bool, &str)> = earlier.entries.iter().map(|e| (e.ok, e.text.as_str())).collect();
    assert_eq!(texts, [(true, "Fetched origin"), (false, "Push failed:\nrejected")]);
    let session = earlier.entries[0].session;
    assert!((before..=after).contains(&session), "the first worker's session");
    assert!(earlier.entries.iter().all(|e| e.session == session && e.at >= session));

    w.submit(LogsEffect::ClearActivity);
    w.submit(LogsEffect::ReadFile { kind: LogKind::Performance, generation: 8 });
    next(&rx);
    assert!(!dir.join(logs::ACTIVITY_FILE).exists(), "Clear deletes the file");
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unattached_worker_does_nothing() {
    let w = LogWorker::default();
    w.submit(LogsEffect::LoadActivity);
    w.submit(LogsEffect::SetExtended(true));
    assert!(w.perf_probe().is_none());
}
