//! `perform` on a real screen: a scripted tour through the actions, and the two ways an action
//! is skipped instead of failing.

use super::super::super::fixture::Fixture;
use super::super::super::harness::Journey;
use super::super::action::{Action, Outside};
use super::super::probe::{self, took_effect};
use super::super::scene::{self, Scene};
use super::super::shadow::Shadow;
use super::{Done, perform};

fn start(tag: &str) -> (Scene, Journey) {
    let scene = scene::build(tag, 1);
    let j = Journey::start(Fixture::new(&format!("perform-{tag}")), Some(scene.app.clone()), vec![scene.app.clone()]);
    (scene, j)
}

/// Performs `action` and expects it to be performed.
fn run(j: &mut Journey, scene: &Scene, shadow: &mut Shadow, action: &Action) {
    let done = perform(j, scene, "app", shadow, action);
    assert!(matches!(done, Done::Performed), "{action}: {done:?}");
}

/// `run`, and checks the action's probe where there is one.
fn step(j: &mut Journey, scene: &Scene, shadow: &mut Shadow, action: Action) {
    let before = probe::read(&scene.app);
    run(j, scene, shadow, &action);
    let after = probe::read(&scene.app);
    assert_ne!(took_effect(&action, &before, &after), Some(false), "{action} did not take effect");
}

#[test]
fn scripted_tour() {
    let (scene, mut j) = start("tour");
    let mut shadow = Shadow::new(1);
    let outside = |o| Action::Outside(o);
    step(&mut j, &scene, &mut shadow, outside(Outside::Write { path: "a.txt".into(), lines: 2 }));
    assert!(shadow.live("app").len() == 2, "{:?}", shadow.live("app"));
    step(&mut j, &scene, &mut shadow, Action::SelectWip);
    step(&mut j, &scene, &mut shadow, Action::StageAll);
    step(&mut j, &scene, &mut shadow, Action::UnstageAll);
    step(&mut j, &scene, &mut shadow, Action::Stage("a.txt".into()));
    step(&mut j, &scene, &mut shadow, Action::Unstage("a.txt".into()));
    step(&mut j, &scene, &mut shadow, outside(Outside::Write { path: "src/b.txt".into(), lines: 1 }));
    step(&mut j, &scene, &mut shadow, Action::Stage("src/b.txt".into()));
    step(&mut j, &scene, &mut shadow, Action::Unstage("src/b.txt".into()));
    step(&mut j, &scene, &mut shadow, Action::StageAll);
    step(&mut j, &scene, &mut shadow, Action::Commit("msg 1".into()));
    assert!(shadow.summaries("app").contains("msg 1"));
    step(&mut j, &scene, &mut shadow, Action::CreateBranch("b1".into()));
    step(&mut j, &scene, &mut shadow, Action::Checkout("main".into()));
    step(&mut j, &scene, &mut shadow, Action::Merge("topic".into()));
    step(&mut j, &scene, &mut shadow, Action::ResolveTheirs("shared.txt".into()));
    step(&mut j, &scene, &mut shadow, Action::Continue);
    step(&mut j, &scene, &mut shadow, outside(Outside::Append { path: "crlf.txt".into(), lines: 1 }));
    let appended = std::fs::read_to_string(scene.app.join("crlf.txt")).unwrap();
    assert!(appended.contains("\r\n") && !appended.replace("\r\n", "").contains('\n'), "{appended:?}");
    step(&mut j, &scene, &mut shadow, Action::Stash);
    step(&mut j, &scene, &mut shadow, Action::Unstash);
    step(&mut j, &scene, &mut shadow, Action::Undo);
    step(&mut j, &scene, &mut shadow, Action::Redo);
}

#[test]
fn a_commit_types_over_the_message_an_undo_put_back() {
    let (scene, mut j) = start("retype");
    let mut shadow = Shadow::new(1);
    step(&mut j, &scene, &mut shadow, Action::Outside(Outside::Write { path: "a.txt".into(), lines: 1 }));
    step(&mut j, &scene, &mut shadow, Action::SelectWip);
    step(&mut j, &scene, &mut shadow, Action::StageAll);
    step(&mut j, &scene, &mut shadow, Action::Commit("msg 1".into()));
    step(&mut j, &scene, &mut shadow, Action::Undo);
    step(&mut j, &scene, &mut shadow, Action::Commit("msg 2".into()));
    assert_eq!(probe::read(&scene.app).head_summary.as_deref(), Some("msg 2"));
}

#[test]
fn missing_widget_is_skipped() {
    let (scene, mut j) = start("missing");
    let mut shadow = Shadow::new(1);
    for action in [Action::Checkout("nope".into()), Action::Continue, Action::Abort, Action::ResolveOurs("a.txt".into())] {
        let Done::Skipped(why) = perform(&mut j, &scene, "app", &mut shadow, &action) else { panic!("{action} was performed") };
        assert!(!why.is_empty(), "{action}");
    }
}

#[test]
fn outside_write_failure_is_skipped() {
    let (scene, mut j) = start("fail");
    let mut shadow = Shadow::new(1);
    // a.txt is a file, so nothing can be created below it on any OS.
    let action = Action::Outside(Outside::Create { path: "a.txt/x.txt".into(), lines: 1 });
    let Done::Skipped(why) = perform(&mut j, &scene, "app", &mut shadow, &action) else { panic!("the write succeeded") };
    assert!(why.contains("a.txt/x.txt"), "{why}");
    assert!(shadow.live("app").is_empty());
}

#[test]
fn less_common_actions_are_performed() {
    let (scene, mut j) = start("rest");
    let mut shadow = Shadow::new(1);
    let write = |path: &str| Action::Outside(Outside::Write { path: path.into(), lines: 1 });
    // Discard asks first; its dialog is answered by the action.
    step(&mut j, &scene, &mut shadow, write("a.txt"));
    step(&mut j, &scene, &mut shadow, Action::SelectWip);
    step(&mut j, &scene, &mut shadow, Action::Discard("a.txt".into()));
    // Amend takes the staged change into the head commit.
    step(&mut j, &scene, &mut shadow, write("a.txt"));
    step(&mut j, &scene, &mut shadow, Action::StageAll);
    step(&mut j, &scene, &mut shadow, Action::Amend);
    // A stash is dropped from its menu, then confirmed.
    step(&mut j, &scene, &mut shadow, write("a.txt"));
    step(&mut j, &scene, &mut shadow, Action::Stash);
    run(&mut j, &scene, &mut shadow, &Action::DropStash);
    step(&mut j, &scene, &mut shadow, Action::Confirm("Delete".into()));
    assert_eq!(probe::read(&scene.app).stashes, 0);
    // A stopped merge is given up.
    step(&mut j, &scene, &mut shadow, Action::Merge("topic".into()));
    run(&mut j, &scene, &mut shadow, &Action::Abort);
    if j.modal_open() {
        run(&mut j, &scene, &mut shadow, &Action::Confirm("Abort merge".into()));
    }
    assert!(probe::read(&scene.app).conflicted.is_empty());
}

#[test]
fn resolving_the_second_of_two_conflicts() {
    let (scene, mut j) = start("two");
    let mut shadow = Shadow::new(1);
    // topic and main both change shared.txt; make a.txt conflict too.
    use super::super::super::fixture::{commit_files, git, git_try};
    git(&scene.app, &["checkout", "-q", "topic"]);
    commit_files(&scene.app, &[("a.txt", "topic a\n")], "topic a");
    git(&scene.app, &["checkout", "-q", "main"]);
    commit_files(&scene.app, &[("a.txt", "main a\n")], "main a");
    assert!(!git_try(&scene.app, &["merge", "topic"]));
    step(&mut j, &scene, &mut shadow, Action::Refresh);
    let r = probe::read(&scene.app);
    let both: Vec<&String> = r.conflicted.iter().collect();
    assert_eq!(both.len(), 2, "{both:?}");
    let (first, second) = (both[0].clone(), both[1].clone());
    step(&mut j, &scene, &mut shadow, Action::SelectWip);
    step(&mut j, &scene, &mut shadow, Action::ResolveTheirs(second.clone()));
    let after = probe::read(&scene.app);
    assert!(after.conflicted.contains(&first) && !after.conflicted.contains(&second), "{:?}", after.conflicted);
}

#[test]
fn a_stash_just_made_is_dropped_from_its_menu() {
    // Right after the toolbar's Stash, the pointer has to rest on the row for a frame first.
    let (scene, mut j) = start("drop-new");
    let mut shadow = Shadow::new(1);
    step(&mut j, &scene, &mut shadow, Action::Outside(Outside::Write { path: "a.txt".into(), lines: 1 }));
    step(&mut j, &scene, &mut shadow, Action::Stash);
    run(&mut j, &scene, &mut shadow, &Action::DropStash);
    step(&mut j, &scene, &mut shadow, Action::Confirm("Delete".into()));
    assert_eq!(probe::read(&scene.app).stashes, 0);
}
