//! Conflict prediction: when the automatic check runs (and when it doesn't), one at a time per
//! tab, deduped, kept across tab switches, asked again by F5, errors logged once, the
//! preference, and the checks the branch menus ask for.

use super::tests::{git, same_place, scratch_repo, skipped, state};
use super::update;
use crate::conflict_check::{CheckJob, ConflictCheckMsg};
use crate::effect::{Effect, LoadKind};
use crate::msg::{Msg, PrefsMsg, RepoMsg, TabsMsg, TrustMsg};
use crate::runner;
use crate::state::conflict_check::current_request;
use crate::state::{AppState, Modal};
use git2::Oid;
use glowfly_core::conflict_check::{Prediction, TargetKind};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A repository checked out on `feature`, which (like `main`) changed the second line of a.txt.
fn diverged(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["checkout", "-q", "--", "a.txt"]);
    git(&dir, &["branch", "feature"]);
    commit(&dir, "one\nmain\n");
    git(&dir, &["checkout", "-q", "feature"]);
    commit(&dir, "one\nfeature\n");
    dir
}

fn commit(dir: &Path, text: &str) {
    std::fs::write(dir.join("a.txt"), text).unwrap();
    git(dir, &["commit", "-q", "-am", text.trim()]);
}

fn oid(dir: &Path, rev: &str) -> Oid {
    git2::Repository::open(dir).unwrap().revparse_single(rev).unwrap().id()
}

/// Runs `msg` and what it leads to, as `drive` does, but holds conflict checks back.
fn drive_holding(st: &mut AppState, msg: Msg) -> Vec<CheckJob> {
    let mut held = Vec::new();
    let mut queue = vec![msg];
    while let Some(m) = queue.pop() {
        for effect in update(st, m, Instant::now()) {
            match effect {
                Effect::CheckConflicts(job) => held.push(job),
                e if skipped(&e) => {}
                e => queue.extend(runner::execute(e)),
            }
        }
    }
    held
}

fn open(st: &mut AppState, dir: &Path) -> Vec<CheckJob> {
    drive_holding(st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())))
}

/// Runs a held check and applies its result; returns the checks that leads to.
fn deliver(st: &mut AppState, job: CheckJob) -> Vec<CheckJob> {
    let msg = runner::execute(Effect::CheckConflicts(job)).unwrap();
    drive_holding(st, msg)
}

/// A full reload of the active tab, as after a commit or fetch (without F5's invalidation).
fn reload(st: &mut AppState, dir: &Path) -> Vec<CheckJob> {
    let snap = glowfly_core::repo::load(dir, &st.load_options(dir)).unwrap();
    let msg = RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Refresh, result: Ok(Box::new(snap)) };
    drive_holding(st, Msg::Repo(msg))
}

fn shown(st: &AppState) -> Option<Result<Prediction, String>> {
    let current = current_request(st.snap.as_ref()?);
    st.conflict_checks.shown(current.as_ref()).map(|r| r.result.clone())
}

fn conflicts_in_a() -> Option<Result<Prediction, String>> {
    Some(Ok(Prediction::Conflicts(vec!["a.txt".into()])))
}

fn logged(st: &AppState, text: &str) -> usize {
    st.log.iter().filter(|l| l.text.contains(text)).count()
}

#[test]
fn opening_a_branch_checks_it_once() {
    let dir = diverged("cc-open");
    let mut st = state();
    let mut jobs = open(&mut st, &dir);
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    let job = jobs.remove(0);
    assert!(!job.manual);
    assert!(same_place(&job.workdir, &dir));
    assert_eq!((job.request.branch.as_str(), job.request.target.name.as_str()), ("feature", "main"));
    assert_eq!(job.request.target.kind, TargetKind::DefaultBranch);
    assert!(deliver(&mut st, job).is_empty());
    assert_eq!(shown(&st), conflicts_in_a());
}

#[test]
fn no_check_without_a_branch_target_or_permission() {
    let detached = diverged("cc-detached");
    git(&detached, &["checkout", "-q", "--detach"]);
    assert!(open(&mut state(), &detached).is_empty(), "detached HEAD");

    let on_main = diverged("cc-on-main");
    git(&on_main, &["checkout", "-q", "main"]);
    assert!(open(&mut state(), &on_main).is_empty(), "nothing to compare main with");

    let mut st = state();
    st.settings.conflict_check = false;
    assert!(open(&mut st, &diverged("cc-pref-off")).is_empty(), "preference off");

    let untrusted = diverged("cc-untrusted");
    git(&untrusted, &["config", "core.sshCommand", "x"]);
    let mut st = state();
    assert!(open(&mut st, &untrusted).is_empty(), "undecided trust");
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    let jobs = drive_holding(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }));
    assert_eq!(jobs.len(), 1, "trusting reloads and checks");
}

#[test]
fn checks_are_deduped_and_one_at_a_time() {
    let dir = diverged("cc-throttle");
    let mut st = state();
    let first = open(&mut st, &dir).pop().unwrap();
    // A reload with the same inputs while the first runs: queued behind it, nothing sent.
    assert!(reload(&mut st, &dir).is_empty());
    commit(&dir, "one\nfeature 2\n");
    assert!(reload(&mut st, &dir).is_empty(), "one check at a time");
    let mut next = deliver(&mut st, first);
    assert_eq!(next.len(), 1, "the queued request goes out");
    let next = next.remove(0);
    assert_eq!(next.request.ours, oid(&dir, "feature"));
    assert!(deliver(&mut st, next).is_empty());
    assert_eq!(shown(&st), conflicts_in_a());
    assert!(reload(&mut st, &dir).is_empty(), "known inputs aren't checked again");
}

#[test]
fn late_replies_are_ignored() {
    let dir = diverged("cc-late");
    let mut st = state();
    let job = open(&mut st, &dir).pop().unwrap();
    st.settings.conflict_check = false;
    assert!(drive_holding(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited)).is_empty());
    assert!(deliver(&mut st, job).is_empty());
    assert_eq!(st.conflict_checks.shown(current_request(st.snap.as_ref().unwrap()).as_ref()), None, "cleared");
    st.settings.conflict_check = true;
    let jobs = drive_holding(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited));
    assert_eq!(jobs.len(), 1, "turning it on checks at once");
    assert!(drive_holding(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited)).is_empty(), "every frame is not a new check");
}

#[test]
fn tab_switches_keep_the_result_and_retry_what_was_in_flight() {
    let a = diverged("cc-tab-a");
    let b = scratch_repo("cc-tab-b");
    let mut st = state();
    let in_flight = open(&mut st, &a).pop().unwrap();
    assert!(open(&mut st, &b).is_empty());
    assert!(deliver(&mut st, in_flight).is_empty(), "the reply for the parked tab is dropped");
    let mut again = drive_holding(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(again.len(), 1, "asked for again on return");
    assert!(deliver(&mut st, again.remove(0)).is_empty());
    assert_eq!(shown(&st), conflicts_in_a());

    assert!(drive_holding(&mut st, Msg::Tabs(TabsMsg::SelectTab(1))).is_empty());
    assert!(drive_holding(&mut st, Msg::Tabs(TabsMsg::SelectTab(0))).is_empty(), "unchanged inputs aren't checked again");
    assert_eq!(shown(&st), conflicts_in_a(), "kept while parked");
}

#[test]
fn f5_checks_again_without_flicker() {
    let dir = diverged("cc-f5");
    let mut st = state();
    let job = open(&mut st, &dir).pop().unwrap();
    deliver(&mut st, job);
    let mut jobs = drive_holding(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    assert_eq!(shown(&st), conflicts_in_a(), "the old result stays until the new one arrives");
    assert!(deliver(&mut st, jobs.remove(0)).is_empty());
    assert_eq!(shown(&st), conflicts_in_a());
}

#[test]
fn f5_with_preferences_open_checks_once() {
    let dir = diverged("cc-f5-prefs");
    let mut st = state();
    let job = open(&mut st, &dir).pop().unwrap();
    deliver(&mut st, job);
    // F5 while Preferences sends SettingsEdited every frame.
    let mut jobs = drive_holding(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    for _ in 0..3 {
        assert!(drive_holding(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited)).is_empty());
    }
    assert!(deliver(&mut st, jobs.remove(0)).is_empty(), "no second check");
    assert!(drive_holding(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited)).is_empty());

    // An F5 where no check can start isn't owed later, either.
    git(&dir, &["checkout", "-q", "--detach"]);
    assert!(drive_holding(&mut st, Msg::Repo(RepoMsg::Refresh)).is_empty(), "detached HEAD");
    git(&dir, &["checkout", "-q", "feature"]);
    let mut jobs = reload(&mut st, &dir);
    assert_eq!(jobs.len(), 1, "the F5 is answered once back on the branch");
    for _ in 0..3 {
        assert!(drive_holding(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited)).is_empty());
    }
    assert!(deliver(&mut st, jobs.remove(0)).is_empty(), "and only once");
}

#[test]
fn a_repeated_error_is_logged_once() {
    let dir = diverged("cc-errors");
    let mut st = state();
    let old = "Predicting conflicts needs git 2.38 or newer; this is git 2.30.0.";
    let fail = |st: &mut AppState, job: CheckJob, text: &str| {
        let msg = ConflictCheckMsg::Checked { request: job.request, manual: false, generation: job.generation, result: Err(text.into()) };
        drive_holding(st, Msg::ConflictCheck(msg))
    };
    let job = open(&mut st, &dir).pop().unwrap();
    fail(&mut st, job, old);
    assert_eq!(logged(&st, old), 1);
    assert_eq!(shown(&st).map(|r| r.is_err()), Some(true), "no chip: the result is an error");
    commit(&dir, "one\nfeature 2\n");
    let job = reload(&mut st, &dir).pop().unwrap();
    fail(&mut st, job, old);
    assert_eq!(logged(&st, old), 1, "not again for a new commit");
    commit(&dir, "one\nfeature 3\n");
    let job = reload(&mut st, &dir).pop().unwrap();
    fail(&mut st, job, "fatal: boom");
    assert_eq!(logged(&st, "fatal: boom"), 1, "a different error is logged");
}

#[test]
fn checking_another_branch_opens_a_report() {
    let dir = diverged("cc-manual");
    git(&dir, &["checkout", "-q", "main"]);
    let mut st = state();
    st.settings.conflict_check = false;
    open(&mut st, &dir);
    let fx = update(&mut st, Msg::ConflictCheck(ConflictCheckMsg::CheckBranch("feature".into())), Instant::now());
    let [Effect::CheckConflicts(job)] = fx.as_slice() else { panic!("{fx:?}") };
    assert!(job.manual);
    assert_eq!(job.request.ours, oid(&dir, "main"));
    assert_eq!((job.request.target.name.as_str(), job.request.target.kind), ("feature", TargetKind::Chosen));
    let job = CheckJob { workdir: job.workdir.clone(), request: job.request.clone(), manual: true, generation: 0 };
    deliver(&mut st, job);
    let Some(Modal::ConflictReport(r)) = &st.modal else { panic!("the report opens") };
    assert_eq!(r.result, Ok(Prediction::Conflicts(vec!["a.txt".into()])));
    assert_eq!(logged(&st, "Merging feature into main would stop with conflicts in 1 file:"), 1);

    // With another dialog open, a toast instead.
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    let fx = update(&mut st, Msg::ConflictCheck(ConflictCheckMsg::CheckBranch("feature".into())), Instant::now());
    super::tests::finish(&mut st, fx);
    assert!(matches!(st.modal, Some(Modal::Shortcuts { .. })));
    assert!(st.toasts.iter().any(|t| t.text.contains("would stop with conflicts")));
}

#[test]
fn manual_checks_skip_trust_and_the_preference() {
    let dir = diverged("cc-manual-untrusted");
    git(&dir, &["config", "core.sshCommand", "x"]);
    let mut st = state();
    st.settings.conflict_check = false;
    open(&mut st, &dir);
    let fx = update(&mut st, Msg::ConflictCheck(ConflictCheckMsg::CheckNow), Instant::now());
    assert!(matches!(fx.as_slice(), [Effect::CheckConflicts(j)] if j.manual && j.request.target.name == "main"), "{fx:?}");
    super::tests::finish(&mut st, fx);
    assert_eq!(shown(&st), conflicts_in_a(), "Check again also updates the automatic result");
    let fx = update(&mut st, Msg::ConflictCheck(ConflictCheckMsg::CheckBranch("main".into())), Instant::now());
    assert!(matches!(fx.as_slice(), [Effect::CheckConflicts(j)] if j.manual && j.request.target.kind == TargetKind::Chosen), "{fx:?}");
}

#[test]
fn refused_manual_checks() {
    let dir = diverged("cc-refused");
    git(&dir, &["checkout", "-q", "main"]);
    let mut st = state();
    open(&mut st, &dir);
    let check = |st: &mut AppState, m: ConflictCheckMsg| update(st, Msg::ConflictCheck(m), Instant::now());
    assert!(check(&mut st, ConflictCheckMsg::CheckNow).is_empty());
    assert_eq!(st.log.last().unwrap().text, "No upstream with new commits or default branch to compare with.");
    assert!(check(&mut st, ConflictCheckMsg::CheckBranch("main".into())).is_empty());
    assert_eq!(st.log.last().unwrap().text, "main is the checked-out branch.");
    assert!(check(&mut st, ConflictCheckMsg::CheckBranch("nope".into())).is_empty());
    assert_eq!(st.log.last().unwrap().text, "There is no branch nope.");
    git(&dir, &["checkout", "-q", "--detach"]);
    reload(&mut st, &dir);
    assert!(check(&mut st, ConflictCheckMsg::CheckBranch("feature".into())).is_empty());
    assert!(st.log.last().unwrap().text.contains("Check out a branch"));
}

#[test]
fn the_report_line_follows_the_language() {
    let dir = diverged("cc-german");
    git(&dir, &["checkout", "-q", "main"]);
    let mut st = state();
    st.settings.conflict_check = false;
    st.settings.language = "de".into();
    open(&mut st, &dir);
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    let fx = update(&mut st, Msg::ConflictCheck(ConflictCheckMsg::CheckBranch("feature".into())), Instant::now());
    super::tests::finish(&mut st, fx);
    let de = glowfly_core::i18n::Lang::De;
    let line = de.trn("conflict_check.headline.conflicts_files", 1, &[("theirs", &"feature"), ("ours", &"main")]);
    assert_eq!(logged(&st, &line), 1, "the log line is German: {line}");
    let toast = st.toasts.last().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.starts_with(&line), "the toast is German: {toast}");
}
