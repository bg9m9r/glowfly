//! The automatic check's bookkeeping and the texts every result gets.

use super::*;
use git2::Oid;
use trellis_core::i18n::Lang;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn request(ours: u8, kind: TargetKind) -> CheckRequest {
    let name = if kind == TargetKind::Upstream { "origin/feature" } else { "main" };
    CheckRequest { branch: "feature".into(), ours: oid(ours), target: MergeTarget { name: name.into(), oid: oid(9), kind } }
}

fn report(req: CheckRequest, result: Result<Prediction, String>) -> ConflictReport {
    ConflictReport { request: req, result }
}

fn conflicts(paths: &[&str]) -> Result<Prediction, String> {
    Ok(Prediction::Conflicts(paths.iter().map(|p| p.to_string()).collect()))
}

#[test]
fn one_check_at_a_time_with_one_queued() {
    let mut c = ConflictChecks::default();
    let req = request(1, TargetKind::DefaultBranch);
    assert!(c.wants(&req));
    let g = c.start().unwrap();
    assert_eq!(c.start(), None, "a second start while one runs is queued");
    assert_eq!(c.start(), None);
    let f = c.finish(g, report(req.clone(), conflicts(&["a.txt"])));
    assert_eq!(f, Finished { accepted: true, queued: true, new_error: None });
    assert!(!c.wants(&req), "the same inputs are known");
    assert!(c.wants(&request(2, TargetKind::DefaultBranch)), "a new commit isn't");
    let g = c.start().unwrap();
    assert!(!c.finish(g, report(req.clone(), Ok(Prediction::Clean))).queued);
}

#[test]
fn shown_only_for_the_current_inputs() {
    let mut c = ConflictChecks::default();
    let req = request(1, TargetKind::DefaultBranch);
    assert_eq!(c.shown(Some(&req)), None);
    let g = c.start().unwrap();
    c.finish(g, report(req.clone(), conflicts(&["a.txt"])));
    assert_eq!(c.shown(Some(&req)).map(|r| &r.result), Some(&conflicts(&["a.txt"])));
    assert_eq!(c.shown(Some(&request(2, TargetKind::DefaultBranch))), None);
    assert_eq!(c.shown(None), None);
}

#[test]
fn invalidate_asks_again_but_keeps_showing() {
    let mut c = ConflictChecks::default();
    let req = request(1, TargetKind::DefaultBranch);
    let g = c.start().unwrap();
    c.finish(g, report(req.clone(), conflicts(&["a.txt"])));
    c.invalidate();
    assert!(c.wants(&req));
    assert!(c.shown(Some(&req)).is_some(), "no flicker");
    let g = c.start().unwrap();
    c.finish(g, report(req.clone(), Ok(Prediction::Clean)));
    assert!(!c.wants(&req));
}

#[test]
fn an_f5_during_a_check_is_still_owed() {
    let mut c = ConflictChecks::default();
    let req = request(1, TargetKind::DefaultBranch);
    let g = c.start().unwrap();
    c.invalidate();
    assert!(c.wants(&req));
    assert_eq!(c.start(), None);
    let f = c.finish(g, report(req.clone(), Ok(Prediction::Clean)));
    assert!(f.queued);
    assert!(c.wants(&req), "the queued request still runs");
}

#[test]
fn an_f5_is_answered_by_one_check_however_often_it_is_asked_for() {
    let mut c = ConflictChecks::default();
    let req = request(1, TargetKind::DefaultBranch);
    let g = c.start().unwrap();
    c.finish(g, report(req.clone(), conflicts(&["a.txt"])));
    c.invalidate();
    let g = c.start().unwrap();
    // Asked for again on every frame while it runs (Preferences open): queued.
    assert_eq!(c.start(), None);
    let f = c.finish(g, report(req.clone(), Ok(Prediction::Clean)));
    assert!(f.queued);
    assert!(!c.wants(&req), "that check answered the F5");
}

#[test]
fn clear_drops_everything_and_ignores_late_replies() {
    let mut c = ConflictChecks::default();
    let req = request(1, TargetKind::DefaultBranch);
    let g = c.start().unwrap();
    c.clear();
    assert!(c.is_empty());
    assert_eq!(c.finish(g, report(req.clone(), Ok(Prediction::Clean))), Finished::default());
    assert_eq!(c.shown(Some(&req)), None);
    assert!(c.start().is_some(), "not running any more");
}

#[test]
fn parking_keeps_the_result_and_drops_the_check_in_flight() {
    let mut c = ConflictChecks::default();
    let req = request(1, TargetKind::DefaultBranch);
    let g = c.start().unwrap();
    c.finish(g, report(req.clone(), conflicts(&["a.txt"])));
    let late = c.start().unwrap();
    let mut parked = c.parked();
    assert!(c.is_empty(), "nothing stays behind");
    assert!(parked.shown(Some(&req)).is_some());
    assert!(parked.wants(&req), "the dropped check (only an F5 starts one for known inputs) is owed");
    assert_eq!(parked.finish(late, report(req.clone(), Ok(Prediction::Clean))), Finished::default(), "the old reply is ignored");
    assert!(parked.start().is_some(), "nothing runs after a return");
    assert_ne!(c.start(), Some(late));
}

#[test]
fn a_repeated_error_is_reported_once() {
    let mut c = ConflictChecks::default();
    let old = "Predicting conflicts needs git 2.38 or newer; this is git 2.30.0.".to_owned();
    let g = c.start().unwrap();
    assert_eq!(c.finish(g, report(request(1, TargetKind::DefaultBranch), Err(old.clone()))).new_error, Some(old.clone()));
    let g = c.start().unwrap();
    assert_eq!(c.finish(g, report(request(2, TargetKind::DefaultBranch), Err(old.clone()))).new_error, None, "another commit");
    let g = c.start().unwrap();
    assert_eq!(c.finish(g, report(request(3, TargetKind::DefaultBranch), Err("boom".into()))).new_error, Some("boom".into()));
    let g = c.start().unwrap();
    c.finish(g, report(request(4, TargetKind::DefaultBranch), Ok(Prediction::Clean)));
    let g = c.start().unwrap();
    assert_eq!(c.finish(g, report(request(5, TargetKind::DefaultBranch), Err("boom".into()))).new_error, Some("boom".into()));
}

#[test]
fn store_manual_replaces_the_last_result() {
    let mut c = ConflictChecks::default();
    let req = request(1, TargetKind::DefaultBranch);
    c.invalidate();
    c.store_manual(report(req.clone(), Ok(Prediction::Clean)));
    assert!(!c.wants(&req));
    assert_eq!(c.shown(Some(&req)).map(|r| &r.result), Some(&Ok(Prediction::Clean)));
}

#[test]
fn headlines() {
    let req = request(1, TargetKind::DefaultBranch);
    let h = |r| headline(Lang::En, &report(req.clone(), r));
    assert_eq!(h(conflicts(&["a", "b"])), "Merging main into feature would stop with conflicts in 2 files:");
    assert_eq!(h(conflicts(&["a"])), "Merging main into feature would stop with conflicts in 1 file:");
    assert_eq!(h(conflicts(&[])), "Merging main into feature would stop with conflicts.");
    assert_eq!(h(Ok(Prediction::Clean)), "main merges into feature without conflicts.");
    assert_eq!(h(Ok(Prediction::UpToDate)), "feature already contains everything on main.");
    assert_eq!(h(Ok(Prediction::FastForward)), "feature has no commits that main lacks, so merging is a fast-forward.");
    assert_eq!(h(Err("bad\u{1b}[31m text".into())), "Couldn't check feature against main: bad[31m text");
}

#[test]
fn chip_tooltip_says_what_and_where() {
    let tip = |kind, result, remote| chip_tooltip(Lang::En, &report(request(1, kind), result), remote);
    assert_eq!(
        tip(TargetKind::DefaultBranch, conflicts(&["a", "b", "c"]), false),
        "Merging main into feature would stop with conflicts in 3 files.\nmain is this repository's default branch.\nClick to see the files and merge or rebase."
    );
    let one = tip(TargetKind::DefaultBranch, conflicts(&["a"]), false);
    assert!(one.starts_with("Merging main into feature would stop with conflicts in 1 file.\n"), "{one}");
    assert_eq!(
        tip(TargetKind::DefaultBranch, conflicts(&[]), false),
        "Merging main into feature would stop with conflicts.\nmain is this repository's default branch.\nClick for details."
    );
    let up = tip(TargetKind::Upstream, conflicts(&["a"]), true);
    assert!(up.starts_with("Merging origin/feature into feature would stop with conflicts in 1 file.\n"), "{up}");
    assert!(up.contains("\norigin/feature is the upstream of feature. Compared with the last fetched state of origin/feature.\n"), "{up}");
    assert_eq!(
        tip(TargetKind::Chosen, conflicts(&["a", "b"]), false),
        "Merging main into feature would stop with conflicts in 2 files.\nClick to see the files and merge or rebase.",
        "a chosen target has no note line"
    );
    let err = tip(TargetKind::DefaultBranch, Err("boom".into()), false);
    assert!(err.starts_with("Couldn't check feature against main: boom\n"), "{err}");
    assert!(err.ends_with("\nClick for details."), "{err}");
    assert_eq!(tip(TargetKind::Chosen, Ok(Prediction::Clean), false), "main merges into feature without conflicts.\nClick for details.");
    let r = report(request(1, TargetKind::DefaultBranch), conflicts(&["a", "b", "c"]));
    let de = chip_tooltip(Lang::De, &r, false);
    assert!(de.contains('3'), "{de}");
    assert_ne!(de, chip_tooltip(Lang::En, &r, false));
    assert_eq!(de.matches('\n').count(), 2, "the same three lines: {de}");
}

#[test]
fn target_notes() {
    let up = request(1, TargetKind::Upstream).target;
    assert_eq!(
        target_note(Lang::En, &up, "feature", true).unwrap(),
        "origin/feature is the upstream of feature. Compared with the last fetched state of origin/feature."
    );
    let main = request(1, TargetKind::DefaultBranch).target;
    assert_eq!(target_note(Lang::En, &main, "feature", false).unwrap(), "main is this repository's default branch.");
    let de = target_note(Lang::De, &up, "feature", true).unwrap();
    assert!(de.contains("origin/feature") && !de.contains("upstream of"), "{de}");
    let chosen = request(1, TargetKind::Chosen).target;
    assert_eq!(target_note(Lang::En, &chosen, "feature", false), None);
}

#[test]
fn file_list_is_capped() {
    let paths: Vec<String> = (0..60).map(|i| format!("f{i}.txt")).collect();
    let (shown, more) = file_list(Lang::En, &paths);
    assert_eq!(shown.len(), SHOWN_FILES);
    assert_eq!(more.as_deref(), Some("and 10 more"));
    let (shown, more) = file_list(Lang::En, &["a\u{7}b".into()]);
    assert_eq!((shown, more), (vec!["ab".to_owned()], None));
}

#[test]
fn actions_only_for_a_real_merge() {
    let req = request(1, TargetKind::DefaultBranch);
    assert!(offers_actions(&report(req.clone(), conflicts(&["a"]))));
    assert!(offers_actions(&report(req.clone(), Ok(Prediction::Clean))));
    assert!(!offers_actions(&report(req.clone(), Ok(Prediction::UpToDate))));
    assert!(!offers_actions(&report(req.clone(), Ok(Prediction::FastForward))));
    assert!(!offers_actions(&report(req, Err("x".into()))));
}
