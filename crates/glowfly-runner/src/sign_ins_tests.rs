use super::*;
use glowfly_core::forge::Provider;
use std::sync::Barrier;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

fn key(account: &str, cwd: &str, interactive: bool, connected: bool) -> FlightKey {
    FlightKey { account: account.into(), cwd: PathBuf::from(cwd), interactive, connected }
}

fn token(source: &str) -> Answer {
    Ok(Auth::from_token(Provider::GitHub, None, "t", source))
}

/// Waits (bounded) until `n` requests wait on the flight for `key`.
fn wait_for_waiters(flights: &SignInFlights, key: &FlightKey, n: usize) {
    let end = Instant::now() + Duration::from_secs(10);
    while flights.waiting(key) < n && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Runs `answers.len()` joins on one key at once; the leader's lookup returns `answer` once every
/// other request waits. Returns each participant's answer and how many lookups ran.
fn crowd(flights: &SignInFlights, k: &FlightKey, threads: usize, answer: impl Fn() -> Answer + Sync) -> (Vec<Answer>, usize) {
    let (calls, start) = (AtomicUsize::new(0), Barrier::new(threads));
    let got = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    start.wait();
                    flights.join(k.clone(), || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        wait_for_waiters(flights, k, threads - 1);
                        answer()
                    })
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect::<Vec<_>>()
    });
    (got, calls.load(Ordering::SeqCst))
}

#[test]
fn five_concurrent_requests_for_one_account_start_one_lookup() {
    let flights = SignInFlights::new();
    let k = key("github.com", "repo", false, true);
    let (got, calls) = crowd(&flights, &k, 5, || token("Git Credential Manager"));
    assert_eq!(calls, 1);
    assert_eq!(got.len(), 5);
    for a in got {
        assert_eq!(a.unwrap().source, "Git Credential Manager");
    }
}

#[test]
fn waiters_share_the_failure() {
    let flights = SignInFlights::new();
    let k = key("github.com", "repo", true, false);
    let (got, calls) = crowd(&flights, &k, 5, || Err("not signed in to GitHub".into()));
    assert_eq!(calls, 1);
    for a in got {
        assert_eq!(a.unwrap_err(), "not signed in to GitHub");
    }
}

#[test]
fn other_folders_and_sign_in_windows_look_up_on_their_own() {
    let flights = SignInFlights::new();
    let keys = [
        key("github.com", "a", false, false),
        key("github.com", "b", false, false),
        key("github.com", "a", true, false),
        key("github.com", "a", false, true),
        key("gitlab.com", "a", false, false),
    ];
    let total = keys.len();
    let (calls, start) = (AtomicUsize::new(0), Barrier::new(total));
    std::thread::scope(|s| {
        for k in &keys {
            let (flights, calls, start) = (&flights, &calls, &start);
            s.spawn(move || {
                start.wait();
                flights.join(k.clone(), || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    // Every lookup runs at the same time: none waits for another's end.
                    let end = Instant::now() + Duration::from_secs(10);
                    while calls.load(Ordering::SeqCst) < total && Instant::now() < end {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    token("test")
                })
            });
        }
    });
    assert_eq!(calls.load(Ordering::SeqCst), total);
}

#[test]
fn a_panicking_lookup_answers_its_waiters() {
    let flights = SignInFlights::new();
    let k = key("github.com", "repo", false, true);
    let waiter_answer = std::thread::scope(|s| {
        let leader = s.spawn(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                flights.join(k.clone(), || {
                    wait_for_waiters(&flights, &k, 1);
                    panic!("the helper blew up")
                })
            }))
        });
        // Joins only once the leader's flight exists.
        let end = Instant::now() + Duration::from_secs(10);
        while lock(&flights.flights).is_empty() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(2));
        }
        let answer = flights.join(k.clone(), || token("never"));
        assert!(leader.join().unwrap().is_err(), "the leader's panic is its own");
        answer
    });
    assert!(waiter_answer.is_err(), "the waiter was answered");
    assert!(lock(&flights.flights).is_empty(), "the key is free");
    assert!(flights.join(k, || token("again")).is_ok(), "a new join runs a new lookup");
}

#[test]
fn a_finished_lookup_is_not_kept() {
    let flights = SignInFlights::new();
    let (k, calls) = (key("github.com", "repo", false, true), AtomicUsize::new(0));
    for _ in 0..2 {
        let a = flights.join(k.clone(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            token("test")
        });
        assert!(a.is_ok());
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(lock(&flights.flights).is_empty());
}

#[test]
fn an_environment_only_ask_never_joins_a_flight() {
    let flights = SignInFlights::new();
    let account = Account { provider: Provider::GitHubEnterprise, host: "ghe.example".into(), org: String::new() };
    let ask = Ask { account, cwd: Path::new("repo"), interactive: false, connected: false, env_only: true, asked: false };
    let err = flights.lookup(&ask).unwrap_err();
    assert!(forge::says_not_connected(&err.to_string()), "{err}");
    assert!(lock(&flights.flights).is_empty());
}

#[test]
fn keys_compare_folders_by_spelling() {
    let dir = glowfly_core::paths::temp_dir().join("flight-key-repo");
    let at = |p: PathBuf| FlightKey { cwd: p, ..key("github.com", "", false, false) };
    assert_eq!(at(dir.clone()), at(dir.join("")), "a trailing separator is the same folder");
    assert_ne!(at(dir.clone()), at(dir.with_file_name("flight-key-other")));
}
