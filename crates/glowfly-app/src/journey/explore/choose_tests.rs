//! `applicable` on a real screen: the app is started on a scene's repository.

use super::super::super::find::{Pick, Region};
use super::super::super::fixture::{Fixture, git_try, write};
use super::super::super::harness::Journey;
use super::super::action::{Action, Outside};
use super::super::probe;
use super::super::rng::Rng;
use super::super::scene::{self, Scene};
use super::super::shadow::Shadow;
use super::applicable;

/// A scene and the app open on its `app` repository (the app's own folders are apart from the scene's).
fn start(tag: &str) -> (Scene, Journey) {
    let scene = scene::build(tag, 1);
    let j = Journey::start(Fixture::new(&format!("choose-{tag}")), Some(scene.app.clone()), vec![scene.app.clone()]);
    (scene, j)
}

fn choices(j: &Journey, scene: &Scene, seed: u64) -> Vec<Action> {
    let r = probe::read(&scene.app);
    applicable(j, scene, "app", &r, &mut Rng::new(seed), &mut Shadow::new(seed)).into_iter().map(|(a, _)| a).collect()
}

#[test]
fn clean_main_offers_basics() {
    let (scene, j) = start("basics");
    let all = choices(&j, &scene, 5);
    assert!(all.iter().any(|a| matches!(a, Action::CreateBranch(_))), "{all:?}");
    assert!(all.contains(&Action::Checkout("topic".into())), "{all:?}");
    assert!(all.iter().any(|a| matches!(a, Action::Outside(Outside::Write { .. }))), "{all:?}");
    assert!(!all.iter().any(|a| matches!(a, Action::Commit(_) | Action::Continue)), "{all:?}");
}

#[test]
fn conflict_offers_resolution() {
    let (scene, mut j) = start("conflict");
    assert!(!git_try(&scene.app, &["merge", "topic"]), "the merge conflicts");
    j.refresh();
    let all = choices(&j, &scene, 5);
    assert!(all.contains(&Action::ResolveOurs("shared.txt".into())), "{all:?}");
    assert!(all.contains(&Action::Abort), "{all:?}");
    assert!(!all.iter().any(|a| matches!(a, Action::Commit(_))), "{all:?}");
}

#[test]
fn modal_open_limits_choices() {
    let (scene, mut j) = start("modal");
    write(&scene.app, "a.txt", "edited\n");
    j.refresh();
    j.branch_menu("topic", "Checkout");
    assert!(j.modal_open());
    // A dialog's buttons are enabled a moment after it opens.
    j.settle();
    let all = choices(&j, &scene, 5);
    assert!(all.contains(&Action::Confirm("Stash and check out".into())), "{all:?}");
    assert!(all.iter().all(|a| matches!(a, Action::Confirm(_))), "{all:?}");
    j.click("Cancel", Region::Anywhere, Pick::Only);
    assert!(!j.modal_open());
}

#[test]
fn choices_ignore_screen_order() {
    let (scene, j) = start("order");
    assert_eq!(choices(&j, &scene, 9), choices(&j, &scene, 9));
}
