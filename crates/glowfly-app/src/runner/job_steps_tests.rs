//! Stopping a job's network commands for real: git and what it started end, what they left is
//! removed (a fetch's locks, a clone's folder, never a folder that was there), a local command
//! is never killed, and progress is sent no more than a few times a second.

use super::*;
use glowfly_core::gitcli::args;
use std::cell::RefCell;
use std::path::PathBuf;

fn temp(name: &str) -> PathBuf {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-job-steps-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn git_in(dir: &Path, a: &[&str]) {
    let (ok, out) = gitcli::run(dir, &args(a));
    assert!(ok, "git {a:?}: {out}");
}

/// A repository with one commit at `dir`.
fn repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git_in(dir, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("a.txt"), "a\n").unwrap();
    git_in(dir, &["add", "."]);
    git_in(dir, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-q", "-m", "one"]);
}

/// A path the shell git runs remote programs with can name.
fn sh_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// A remote program that hangs: it writes `marker` after 3 s unless it was stopped first.
fn hanging(marker: &Path) -> String {
    format!("sleep 3; echo late > '{}'; sleep 20; :", sh_path(marker))
}

/// Stops `stop` once `ready` holds (or after 20 s).
fn stop_when(stop: &Stop, ready: impl Fn() -> bool + Send + 'static) {
    let asker = stop.clone();
    std::thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(20);
        while !ready() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        asker.stop();
    });
}

/// Runs `cmds` in `cwd` until stopped, returning the result and the messages sent.
fn run_stopped(cwd: &Path, cmds: &[Vec<String>], ready: impl Fn() -> bool + Send + 'static) -> (Ran, Vec<Msg>) {
    let stop = Stop::new();
    stop_when(&stop, ready);
    let sent = RefCell::new(Vec::new());
    let started = Instant::now();
    let ran = run(cwd, cmds, false, &stop, &|m| sent.borrow_mut().push(m), false);
    assert!(started.elapsed() < Duration::from_secs(15), "took {:?}: {}", started.elapsed(), ran.output);
    (ran, sent.into_inner())
}

fn steps(sent: &[Msg]) -> Vec<Step> {
    sent.iter().filter_map(|m| if let Msg::Job(JobMsg::Step(s)) = m { Some(*s) } else { None }).collect()
}

#[test]
fn a_stopped_fetch_ends_what_it_started_and_removes_what_it_left_behind() {
    let dir = temp("fetch");
    let (work, origin, marker) = (dir.join("work"), dir.join("origin"), dir.join("marker"));
    repo(&origin);
    repo(&work);
    let lock = work.join(".git/refs/heads/topic.lock");
    let url = format!("file://{}", sh_path(&origin));
    let upload = format!("--upload-pack=echo x > '{}'; {}", sh_path(&lock), hanging(&marker));
    let cmds = [args(&["fetch", &upload, &url]), args(&["status"])];
    let watched = lock.clone();
    let (ran, sent) = run_stopped(&work, &cmds, move || watched.exists());
    assert!(ran.stopped && !ran.ok, "{}", ran.output);
    assert!(ran.output.contains("Cancelled.") && ran.output.contains("Removed 1 file the stopped command left behind."), "{}", ran.output);
    assert!(!ran.output.contains("$ git status"), "nothing after the stopped command runs");
    assert!(!lock.exists());
    assert_eq!(steps(&sent), [Step::Fetch]);
    // The remote program git started was stopped with it.
    std::thread::sleep(Duration::from_secs(4));
    assert!(!marker.exists(), "the upload-pack git started outlived the stop");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stopped_push_is_reported_stopped() {
    let dir = temp("push");
    let (work, origin, marker) = (dir.join("work"), dir.join("origin"), dir.join("marker"));
    repo(&origin);
    repo(&work);
    let url = format!("file://{}", sh_path(&origin));
    let receive = format!("--receive-pack={}", hanging(&marker));
    let cmds = [args(&["push", &receive, &url, "main:refs/heads/new"])];
    let (ran, sent) = run_stopped(&work, &cmds, || waited(Duration::from_millis(1500)));
    assert!(ran.stopped && !ran.ok, "{}", ran.output);
    assert_eq!(steps(&sent), [Step::Push]);
    let (_, refs) = gitcli::run(&origin, &args(&["branch", "--list", "new"]));
    assert!(refs.trim().is_empty(), "the remote is as it was: {refs}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A condition that holds once `d` has passed since the first time it is asked.
fn waited(d: Duration) -> bool {
    std::thread::sleep(d);
    true
}

#[test]
fn a_stopped_clone_removes_the_folder_it_made() {
    let dir = temp("clone");
    let (origin, marker) = (dir.join("origin"), dir.join("marker"));
    repo(&origin);
    let url = format!("file://{}", sh_path(&origin));
    let upload = format!("--upload-pack={}", hanging(&marker));
    let dest = dir.join("copy");
    let cmds = [args(&["clone", &upload, "--", &url, "copy"])];
    let made = dest.join(".git");
    let (ran, sent) = run_stopped(&dir, &cmds, move || made.exists() || waited(Duration::from_millis(1500)));
    assert!(ran.stopped, "{}", ran.output);
    assert_eq!(steps(&sent), [Step::Clone]);
    assert!(!dest.exists(), "{}", ran.output);
    assert!(origin.join(".git").is_dir(), "nothing else is touched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stopped_clone_keeps_an_empty_folder_that_was_there() {
    let dir = temp("clone-empty");
    let (origin, marker) = (dir.join("origin"), dir.join("marker"));
    repo(&origin);
    let dest = dir.join("picked");
    std::fs::create_dir(&dest).unwrap();
    let url = format!("file://{}", sh_path(&origin));
    let upload = format!("--upload-pack={}", hanging(&marker));
    let cmds = [args(&["clone", &upload, "--", &url, "picked"])];
    let made = dest.join(".git");
    let (ran, _) = run_stopped(&dir, &cmds, move || made.exists() || waited(Duration::from_millis(1500)));
    assert!(ran.stopped, "{}", ran.output);
    assert!(dest.is_dir(), "the folder the user picked is kept: {}", ran.output);
    assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 0, "{}", ran.output);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stopped_pull_stops_in_its_fetch_and_never_merges() {
    let dir = temp("pull");
    let (work, origin, marker) = (dir.join("work"), dir.join("origin"), dir.join("marker"));
    repo(&origin);
    git_in(&dir, &["clone", "-q", &origin.display().to_string(), "work"]);
    std::fs::write(origin.join("b.txt"), "b\n").unwrap();
    git_in(&origin, &["add", "."]);
    git_in(&origin, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-q", "-m", "two"]);
    let head = || gitcli::run(&work, &args(&["rev-parse", "HEAD"])).1;
    let before = head();
    let upload = format!("--upload-pack={}", hanging(&marker));
    let cmds = [args(&["pull", "--ff-only", &upload])];
    let (ran, sent) = run_stopped(&work, &cmds, || waited(Duration::from_millis(1500)));
    assert!(ran.stopped, "{}", ran.output);
    assert_eq!(steps(&sent), [Step::Fetch], "{}", ran.output);
    assert_eq!(head(), before, "nothing was merged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_local_command_runs_to_its_end_and_the_stop_comes_before_the_next() {
    let dir = temp("local");
    repo(&dir);
    let done = dir.join("done");
    let alias = format!("alias.slow=!sleep 2 && echo x > '{}'", sh_path(&done));
    let cmds = [args(&["-c", &alias, "slow"]), args(&["fetch", "--all"])];
    let (ran, sent) = run_stopped(&dir, &cmds, || waited(Duration::from_millis(300)));
    assert!(ran.stopped && !ran.ok, "{}", ran.output);
    assert!(done.exists(), "the local command wasn't killed: {}", ran.output);
    assert_eq!(steps(&sent), [Step::Local], "the fetch never started");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn progress_is_asked_for_and_sent_a_few_times_a_second() {
    assert_eq!(with_progress(&args(&["fetch", "origin"])), args(&["fetch", "--progress", "origin"]));
    assert_eq!(with_progress(&args(&["-c", "a=b", "push", "-u", "o"])), args(&["-c", "a=b", "push", "--progress", "-u", "o"]));
    assert_eq!(with_progress(&args(&["fetch", "-q"])), args(&["fetch", "-q"]));
    let mut w = Watch::default();
    let t = Instant::now();
    assert!(w.chunk(b"Receiving objects:  10% (1/10)\r", t).is_some(), "the first is sent");
    assert!(w.chunk(b"Receiving objects:  20% (2/10)\r", t + Duration::from_millis(50)).is_none(), "too soon");
    assert!(w.chunk(b"Resolving deltas:  10% (1/10)\r", t + Duration::from_millis(60)).is_some(), "a new phase is sent");
    assert!(w.chunk(b"Resolving deltas:  50% (5/10)\r", t + Duration::from_millis(300)).is_some());
    assert!(w.chunk(b"From x\n", t + Duration::from_secs(1)).is_none());
}

#[test]
fn a_stop_before_a_rebasing_pull_s_checks_says_only_that_it_was_cancelled() {
    let dir = temp("pull-checks");
    repo(&dir);
    let stop = Stop::new();
    stop.stop();
    let mut live = Live::new(&dir, false, &stop, &|_| {});
    let (ok, text) = live.command(&args(&["pull", "--rebase"]));
    assert!(!ok && live.stopped);
    assert_eq!(text, "$ git pull --rebase", "no check it cut short reads as failed");
    assert_eq!(live.finish(format!("{text}\n")), "$ git pull --rebase\nCancelled.");
    let _ = std::fs::remove_dir_all(&dir);
}
