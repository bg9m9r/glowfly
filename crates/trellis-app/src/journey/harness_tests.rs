//! The journey harness's own checks: the test app starts idle and keeps to its folders, outside
//! effects are dropped, saves land in the test's folder, and a failed lookup says what is shown.

use super::find::{self, Pick, Region};
use super::fixture::{Fixture, git};
use super::harness::Journey;

/// The names in `dir`, sorted.
fn names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn starts_on_the_welcome_screen_and_settles() {
    let j = Journey::start(Fixture::new("welcome"), None, Vec::new());
    j.see("Clone a repository", Region::Central);
    j.see("Open a repository", Region::Central);
    assert!(j.dropped().is_empty(), "nothing reached outside the test: {:?}", j.dropped());
    let saved = names(&j.fx.config());
    assert!(saved.iter().all(|n| ["settings.json", "recent.txt", "tabs.txt"].contains(&n.as_str())), "{saved:?}");
    assert_eq!(names(&j.fx.root), ["config", "remote", "work"], "nothing else was written next to them");
}

#[test]
fn os_effects_are_dropped() {
    let mut j = Journey::start(Fixture::new("dropped"), None, Vec::new());
    j.click("Open a repository", Region::Central, Pick::Only);
    assert_eq!(j.dropped(), ["pick-folder"], "the folder picker never opens");
    assert!(j.state().job.is_none());
}

#[test]
fn saves_land_in_the_test_folder() {
    let fx = Fixture::new("saves");
    let repo = fx.work().join("app");
    fx.init(&repo, &[("a.txt", "one\n")]);
    git(&repo, &["status", "--short"]);
    let j = Journey::start(fx, Some(repo.clone()), vec![repo.clone()]);
    j.see("app", super::find::Region::Toolbar);
    let recent = std::fs::read_to_string(j.config_file("recent.txt")).expect("recent.txt is saved in the test's folder");
    let listed = crate::platform::parse_recent(&recent);
    assert!(listed.iter().any(|p| Journey::same(p, &repo)), "{listed:?}");
    let tabs = std::fs::read_to_string(j.config_file("tabs.txt")).expect("tabs.txt is saved in the test's folder");
    let (paths, _, _) = crate::platform::parse_tabs(&tabs);
    assert!(paths.iter().any(|p| Journey::same(p, &repo)), "{paths:?}");
}

#[test]
#[should_panic(expected = "Clone a repository")]
fn lookup_failure_lists_visible_labels() {
    let mut j = Journey::start(Fixture::new("lookup"), None, Vec::new());
    j.click("No such button", Region::Anywhere, Pick::Only);
}

#[test]
fn picks_one_of_several_widgets_by_position() {
    use eframe::egui;
    use egui_kittest::Harness;
    let mut h = Harness::new_ui(|ui| {
        ui.horizontal(|ui| {
            let _ = ui.button("Go");
            ui.label("first row");
            let _ = ui.button("Go");
        });
        ui.horizontal(|ui| {
            ui.label("second row");
            let _ = ui.button("Go");
        });
        ui.add(egui::Button::new("Alone"));
    });
    h.run();
    let x = |pick| find::centre(&find::find(&h, "Go", true, Region::Anywhere, pick).unwrap()).unwrap();
    assert!(find::find(&h, "Go", true, Region::Anywhere, Pick::Only).is_none(), "three match");
    assert!(find::find(&h, "Alone", true, Region::Anywhere, Pick::Only).is_some());
    assert_eq!(find::count(&h, "Go", true, Region::Anywhere), 3);
    let (first, last) = (x(Pick::First), x(Pick::Last));
    assert!(first.y < last.y, "tree order is drawing order");
    let (left, right) = (x(Pick::Leftmost), x(Pick::Rightmost));
    assert!(left.x < right.x);
    let second = x(Pick::InRowOf("second row"));
    assert_eq!(second, last, "the Go on the second row's line");
    let near = x(Pick::InRowOf("first row"));
    assert!(near.y == first.y, "a Go on the first row's line");
    assert!(find::labels_in(&h, Region::Anywhere).contains(&"second row".to_owned()));
}

#[test]
fn the_palette_is_its_own_region() {
    let fx = Fixture::new("palette-region");
    let repo = fx.work().join("app");
    fx.init(&repo, &[("a.txt", "one\n")]);
    let mut j = Journey::start(fx, Some(repo.clone()), vec![repo]);
    j.open_palette("refresh");
    assert!(j.state().palette.is_some(), "the palette's key opens it");
    assert_eq!(j.count("Refresh", Region::Palette), 1, "the palette's row");
    assert_eq!(j.count("Refresh", Region::Central), 0, "the palette's rows aren't in the centre");
    assert_eq!(j.count("Refresh the repository", Region::Palette), 0, "the toolbar isn't in the palette");
    j.click("Refresh", Region::Palette, Pick::Only);
    assert!(j.state().palette.is_none(), "running a row closes the palette");
    j.run_command("refresh", "Refresh");
    assert!(j.state().palette.is_none());
    assert_eq!(j.count("Refresh", Region::Palette), 0, "a closed palette has no region");
}
