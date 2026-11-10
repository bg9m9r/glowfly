//! Each rule fires on the fault it exists for, and stays quiet on correct behaviour.

use super::super::super::fixture::{Fixture, git_try};
use super::super::super::harness::Journey;
use super::super::probe;
use super::super::scene::{self, Scene};
use super::super::shadow::Shadow;
use super::{Broken, Rule, errors_reported, no_crash, no_lost_work, screen_agrees, sound, undo_returns};

fn start(tag: &str) -> (Scene, Journey) {
    let scene = scene::build(tag, 1);
    let j = Journey::start(Fixture::new(&format!("rules-{tag}")), Some(scene.app.clone()), vec![scene.app.clone()]);
    (scene, j)
}

#[test]
fn sound_passes_then_fails() {
    let scene = scene::build("sound", 1);
    assert_eq!(sound(&scene.app), Ok(()));
    let r = git2::Repository::open(&scene.app).unwrap();
    let tree = r.head().unwrap().peel_to_commit().unwrap().tree_id().to_string();
    let loose = scene.app.join(".git/objects").join(&tree[..2]).join(&tree[2..]);
    assert!(loose.exists(), "the head's tree is a loose object: {}", loose.display());
    std::fs::remove_file(&loose).unwrap();
    assert!(sound(&scene.app).is_err());
}

#[test]
fn lost_work_fires() {
    let scene = scene::build("lost", 1);
    let mut shadow = Shadow::new(1);
    let m = shadow.marker();
    shadow.add("app", m.clone());
    std::fs::write(scene.app.join("a.txt"), format!("alpha\n{m}\n")).unwrap();
    assert_eq!(no_lost_work(&scene.app, "app", &shadow), Ok(()));
    std::fs::write(scene.app.join("a.txt"), "alpha\n").unwrap();
    let err = no_lost_work(&scene.app, "app", &shadow).unwrap_err();
    assert!(err.contains(&m), "{err}");
}

#[test]
fn screen_disagrees_after_unrefreshed_outside_checkout() {
    let (scene, mut j) = start("screen");
    assert_eq!(screen_agrees(&j, &probe::read(&scene.app)), Ok(()));
    assert!(git_try(&scene.app, &["checkout", "-q", "topic"]), "topic exists in the scene");
    let r = probe::read(&scene.app);
    assert!(screen_agrees(&j, &r).is_err());
    j.refresh();
    assert_eq!(screen_agrees(&j, &r), Ok(()));
}

#[test]
fn undo_mismatch_fires() {
    let scene = scene::build("undo", 1);
    let a = probe::read(&scene.app);
    assert_eq!(undo_returns(&a, &a.clone()), Ok(()));
    let mut b = a.clone();
    b.head = None;
    assert!(undo_returns(&a, &b).is_err());
}

#[test]
fn silent_failure_fires() {
    let fail = [(false, "Could not do it".to_owned())];
    assert!(errors_reported(Some(false), &[], &[]).is_err());
    assert_eq!(errors_reported(Some(false), &fail, &[]), Ok(()));
    assert_eq!(errors_reported(Some(false), &[], &["it failed".to_owned()]), Ok(()));
    assert!(errors_reported(Some(true), &fail, &[]).is_err());
    assert_eq!(errors_reported(Some(true), &[], &[]), Ok(()));
    assert_eq!(errors_reported(None, &fail, &[]), Ok(()));
}

#[test]
fn no_crash_is_quiet_on_a_clean_session() {
    let (_scene, j) = start("crash");
    assert_eq!(no_crash(&j), Ok(()));
}

#[test]
fn rules_have_their_names() {
    use Rule::*;
    let names = [NoCrash, Sound, NoLostWork, ScreenAgrees, UndoReturns, ErrorsReported].map(Rule::name);
    assert_eq!(names, ["no-crash", "sound", "no-lost-work", "screen-agrees", "undo-returns", "errors-reported"]);
    let b = Broken { rule: Sound, step: 3, detail: "x".into() };
    assert_eq!((b.rule.name(), b.step, b.detail.as_str()), ("sound", 3, "x"));
}

#[test]
fn log_marks_separate_old_entries_from_new() {
    let (scene, mut j) = start("marks");
    std::fs::write(scene.app.join("a.txt"), "changed\n").unwrap();
    let mark = j.log_mark();
    j.commit_all("a change");
    assert!(j.log_mark() > mark, "the commit logged an entry");
    assert!(j.errors_logged_since(mark).is_empty());
    assert!(j.toasts_since_action().iter().all(|(ok, _)| *ok));
}

#[test]
fn a_resolved_merge_needs_no_view_conflicts() {
    let (scene, mut j) = start("merge");
    assert!(!git_try(&scene.app, &["merge", "topic"]), "topic conflicts with the head");
    j.refresh();
    j.select_wip();
    let r = probe::read(&scene.app);
    assert!(!r.conflicted.is_empty());
    assert_eq!(screen_agrees(&j, &r), Ok(()), "conflicts shown");
    for path in &r.conflicted {
        assert!(git_try(&scene.app, &["checkout", "--ours", "--", path]));
        assert!(git_try(&scene.app, &["add", "--", path]));
    }
    j.refresh();
    let r = probe::read(&scene.app);
    assert!(r.conflicted.is_empty());
    assert_eq!(r.state, git2::RepositoryState::Merge);
    assert_eq!(screen_agrees(&j, &r), Ok(()), "merge still open, every conflict resolved");
}

#[test]
fn screen_disagrees_about_files_changed_outside() {
    let (scene, mut j) = start("files");
    std::fs::write(scene.app.join("a.txt"), "changed\n").unwrap();
    j.refresh();
    assert_eq!(screen_agrees(&j, &probe::read(&scene.app)), Ok(()));
    assert!(git_try(&scene.app, &["add", "a.txt"]));
    let r = probe::read(&scene.app);
    let err = screen_agrees(&j, &r).unwrap_err();
    assert!(err.contains("staged files"), "{err}");
    j.refresh();
    assert_eq!(screen_agrees(&j, &r), Ok(()));
}
