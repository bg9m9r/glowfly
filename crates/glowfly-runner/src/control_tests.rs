//! Tests for the effects `Runner::run` carries out itself (runner/control.rs): a stop and a
//! credential answer take effect before `run` returns, while the repository queue they would
//! otherwise wait in is still held by a long job, and neither starts a worker.

use super::*;
use crate::Waker;
use glowfly_core::askpass::client::{self, Outcome};
use glowfly_core::askpass::{Reply, Secret, server};
use glowfly_model::effect::Effect;
use glowfly_model::effect::partial::StopKey;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(30);

/// Holds `repo`'s queue with a job that runs until the returned sender sends or is dropped.
fn hold_queue(runner: &Runner, repo: &Path) -> Sender<()> {
    let (gate_tx, gate_rx) = mpsc::channel::<()>();
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let job: crate::queue::Job = Box::new(move || {
        started_tx.send(()).unwrap();
        let _ = gate_rx.recv();
    });
    assert!(runner.queues.submit(repo, job).is_ok(), "queued");
    started_rx.recv_timeout(WAIT).expect("the long job started");
    gate_tx
}

#[test]
fn a_stop_reaches_its_job_before_run_returns_while_the_queue_is_held() {
    let (tx, _rx) = mpsc::channel();
    let runner = Runner::new(tx, Waker::none());
    let gate = hold_queue(&runner, Path::new("C:/glowfly-control-stop"));
    // The job it is for, registered as `Runner::run` registers a stoppable job.
    let stop = runner.stops.register(StopKey::Job);
    runner.run(Effect::Control(ControlEffect::StopGit(StopKey::Job)));
    assert!(stop.requested(), "asked on the calling thread, not behind the held queue");
    assert_eq!(runner.tally.started.load(Ordering::SeqCst), 0, "no worker was started");
    gate.send(()).unwrap();
}

#[test]
fn an_answer_reaches_the_helper_while_the_queue_is_held() {
    let (events_tx, events_rx) = mpsc::channel();
    let events_tx = std::sync::Mutex::new(events_tx);
    let listener = server::start_with(WAIT, move |e| {
        let _ = events_tx.lock().unwrap().send(e);
    })
    .unwrap();
    let (tx, _rx) = mpsc::channel();
    let mut runner = Runner::new(tx, Waker::none());
    runner.askpass = Some(listener.clone());
    let gate = hold_queue(&runner, Path::new("C:/glowfly-control-answer"));
    let (answer_tx, answer_rx) = mpsc::channel();
    let (addr, token) = (listener.addr(), listener.token().to_owned());
    std::thread::spawn(move || {
        let _ = answer_tx.send(client::ask(&addr, &token, "Password for 'https://ada@example.invalid': ", None, WAIT));
    });
    let id = match events_rx.recv_timeout(WAIT).expect("the prompt") {
        server::Event::Asked { id, .. } => id,
        other => panic!("expected a prompt, got {other:?}"),
    };
    runner.run(Effect::Control(ControlEffect::AnswerPrompt { id, reply: Reply::Answer(Secret::new("hunter2")) }));
    match answer_rx.recv_timeout(WAIT).expect("the helper's result, with the queue still held") {
        Ok(Outcome::Answer(a)) => assert_eq!(a.expose(), "hunter2"),
        other => panic!("expected the answer, got {other:?}"),
    }
    assert_eq!(runner.tally.started.load(Ordering::SeqCst), 0, "no worker was started");
    gate.send(()).unwrap();
}
