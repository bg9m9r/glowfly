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
    let dir = glowfly_core::paths::temp_dir().join(format!("trellis-log-worker-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let before = chrono::Utc::now().timestamp();

    let (w, rx) = worker(&dir);
    w.submit(LogsEffect::LoadActivity);
    assert_eq!(next(&rx), LogsMsg::ActivityLoaded(Ok(EarlierLog { dir: dir.clone(), entries: Vec::new() })));
    let a = dir.join("a");
    w.submit(LogsEffect::Append(vec![
        NewLine { ok: true, text: "Fetched origin".into(), repo: Some(a.clone()), run: None },
        NewLine { ok: false, text: "Push failed:\nrejected".into(), repo: None, run: None },
    ]));
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
    assert_eq!(earlier.entries[0].repo.as_deref(), Some(a.as_path()), "tagged with its tab's repository");
    assert_eq!(earlier.entries[1].repo, None);
    let session = earlier.entries[0].session;
    assert!((before..=after).contains(&session), "the first worker's session");
    assert!(earlier.entries.iter().all(|e| e.session == session && e.at >= session));

    w.submit(LogsEffect::ClearActivity { repo: Some(a.clone()) });
    w.submit(LogsEffect::ReadFile { kind: LogKind::Performance, generation: 8 });
    next(&rx);
    let (left, _) = logs::parse_activity(&std::fs::read_to_string(dir.join(logs::ACTIVITY_FILE)).unwrap());
    let texts: Vec<&str> = left.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(texts, ["Push failed:\nrejected"], "Clear removes only that tab's lines");
    w.submit(LogsEffect::ClearActivity { repo: None });
    w.submit(LogsEffect::ReadFile { kind: LogKind::Performance, generation: 9 });
    next(&rx);
    let (left, _) = logs::parse_activity(&std::fs::read_to_string(dir.join(logs::ACTIVITY_FILE)).unwrap());
    assert!(left.is_empty(), "{left:?}");
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn appended_runs_load_again() {
    let dir = glowfly_core::paths::temp_dir().join(format!("trellis-log-worker-runs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let run = logs::RunInfo { title: "Push".into(), summary: "Everything up-to-date".into(), took_ms: Some(1250), routine: false };
    let (w, rx) = worker(&dir);
    w.submit(LogsEffect::LoadActivity);
    next(&rx);
    w.submit(LogsEffect::Append(vec![NewLine { ok: true, text: "Push done\n$ git push".into(), repo: None, run: Some(run.clone()) }]));
    // Answered after the append is done.
    w.submit(LogsEffect::ReadFile { kind: LogKind::Errors, generation: 1 });
    next(&rx);
    drop(w);
    let (w, rx) = worker(&dir);
    w.submit(LogsEffect::LoadActivity);
    let LogsMsg::ActivityLoaded(Ok(earlier)) = next(&rx) else { panic!("not loaded") };
    assert_eq!(earlier.entries.len(), 1);
    assert_eq!(earlier.entries[0].run, Some(run));
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
