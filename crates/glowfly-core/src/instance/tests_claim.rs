//! The claim: one holder at a time, a lock file or instance file left behind by a window that is
//! gone never blocks start-up, and a waiting window takes over when the holder closes.

use super::file::{self, InstanceFile};
use super::tests::{listen, listen_with, scratch};
use super::*;
use std::time::{Duration, Instant};

/// The claim at `lock` once it is free, waiting up to `REQUEST_TIMEOUT`. A lock that was just
/// given up can look held for a moment on Linux: a process another test thread is starting holds
/// a copy of every open file until it execs.
fn claim_soon(lock: &std::path::Path) -> Option<Claim> {
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    loop {
        if let Some(c) = claim_at(lock).unwrap() {
            return Some(c);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn only_one_window_holds_the_claim() {
    let dir = scratch("claim-one");
    let lock = dir.join(LOCK_NAME);
    let first = claim_at(&lock).unwrap().expect("free");
    assert!(claim_at(&lock).unwrap().is_none(), "held: a second claim gets nothing");
    assert!(claim_at(&lock).unwrap().is_none(), "still held");
    drop(first);
    let second = claim_soon(&lock).expect("given up with its holder");
    drop(second);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn files_a_crashed_window_left_never_block_start_up() {
    let dir = scratch("claim-stale");
    let path = dir.join(FILE_NAME);
    // A crash leaves both files: the lock file (whose lock went with the process) and an
    // instance file naming a closed port.
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(LOCK_NAME), b"").unwrap();
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = closed.local_addr().unwrap().to_string();
    drop(closed);
    file::write(&path, &InstanceFile { addr, token: "1".repeat(32), pid: 1 }).unwrap();

    let started = Instant::now();
    let claim = claim_at(&dir.join(LOCK_NAME)).unwrap().expect("a stale lock file is free");
    let (listener, rx) = listen_with(&path, Some(claim));
    assert!(started.elapsed() < Duration::from_secs(1), "took {:?}", started.elapsed());
    assert!(listener.is_listening());
    assert_ne!(file::read(&path).unwrap().token, "1".repeat(32), "the stale file is replaced");
    hand_off_via(&path, &Handed::Link("glowfly://repo?root=abc".into())).unwrap();
    assert_eq!(rx.recv_timeout(REQUEST_TIMEOUT).unwrap(), Handed::Link("glowfly://repo?root=abc".into()));
    drop(listener);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_second_window_waits_and_takes_over() {
    let dir = scratch("claim-takeover");
    let path = dir.join(FILE_NAME);
    let (first, first_rx) = listen(&path);
    let (second, second_rx) = listen_with(&path, None);
    std::thread::sleep(Duration::from_millis(100));
    assert!(!second.is_listening(), "the first window still holds the claim");
    assert_eq!(second.addr(), None);
    let a = Handed::Folder(crate::paths::temp_dir().join("a"));
    hand_off_via(&path, &a).unwrap();
    assert_eq!(first_rx.recv_timeout(REQUEST_TIMEOUT).unwrap(), a, "only the holder hears it");

    drop(first);
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    while !second.is_listening() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(second.is_listening(), "the waiting window took over");
    let b = Handed::Folder(crate::paths::temp_dir().join("b"));
    hand_off_via(&path, &b).unwrap();
    assert_eq!(second_rx.recv_timeout(REQUEST_TIMEOUT).unwrap(), b);
    assert!(first_rx.try_recv().is_err());
    drop(second);
    assert!(!path.exists());
    assert!(claim_soon(&dir.join(LOCK_NAME)).is_some(), "and gave the claim up when it closed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_waiting_window_that_closed_gives_a_later_claim_up() {
    let dir = scratch("claim-closed");
    let path = dir.join(FILE_NAME);
    let (first, _rx) = listen(&path);
    let (second, _rx2) = listen_with(&path, None);
    drop(second);
    drop(first);
    // The closed window's thread takes the lock and lets it go again at once.
    let claim = claim_soon(&dir.join(LOCK_NAME));
    assert!(claim.is_some(), "nobody keeps the claim");
    assert!(!path.exists(), "and nobody listens");
    drop(claim);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_window_still_starting_is_waited_for() {
    let dir = scratch("claim-starting");
    let path = dir.join(FILE_NAME);
    // The window holds the claim, but listens only a moment later.
    let claim = claim_at(&dir.join(LOCK_NAME)).unwrap().unwrap();
    let starter = {
        let path = path.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            listen_with(&path, Some(claim))
        })
    };
    let f = Handed::Folder(crate::paths::temp_dir().join("starting"));
    hand_off_waiting(&path, &f, START_WAIT).unwrap();
    let (listener, rx) = starter.join().unwrap();
    assert_eq!(rx.recv_timeout(REQUEST_TIMEOUT).unwrap(), f);
    drop(listener);

    // Nobody listening at all: it gives up once the wait is over.
    let started = Instant::now();
    let wait = Duration::from_millis(400);
    assert!(matches!(hand_off_waiting(&path, &f, wait), Err(HandOffError::NotRunning)));
    assert!(started.elapsed() >= Duration::from_millis(250), "it kept trying: {:?}", started.elapsed());
    assert!(started.elapsed() < wait + CONNECT_TIMEOUT * 2, "took {:?}", started.elapsed());
    let _ = std::fs::remove_dir_all(&dir);
}
