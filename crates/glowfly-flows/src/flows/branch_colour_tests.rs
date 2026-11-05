//! Branch colours through the app: worked out by the load, kept when a status read turns the
//! work tree dirty and the graph is laid out again with its WIP row, pinned from the branch menus
//! (saved per repository and recoloured at once, main too), and the colour-blind preset's
//! hue count reaching the snapshot from Preferences and on a tab's return.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use glowfly_core::branch_colour::Swatch;
use glowfly_core::settings::ColourPalette;
use glowfly_model::effect::Effect;
use glowfly_model::effect::SaveEffect;
use glowfly_model::msg::{GraphMsg, Msg, RepoMsg, TabsMsg, WatchMsg};
use glowfly_model::state::AppState;
use std::path::PathBuf;
use std::time::Instant;

/// main, with topic and fix-1 (whose names share a slot) one commit each on top of it, opened.
fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["checkout", "-q", "--", "a.txt"]);
    for b in ["topic", "fix-1"] {
        git(&dir, &["checkout", "-q", "-b", b, "main"]);
        std::fs::write(dir.join(format!("{b}.txt")), b).unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", b]);
    }
    git(&dir, &["checkout", "-q", "main"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn swatch(st: &AppState, key: &str) -> Swatch {
    st.snap.as_ref().unwrap().branch_swatch(key)
}

fn pin(st: &mut AppState, key: &str, slot: Option<u8>) -> Vec<Effect> {
    update(st, Msg::Graph(GraphMsg::PinColour { key: key.into(), slot }), Instant::now())
}

#[test]
fn colours_survive_the_clean_to_dirty_relayout() {
    let (mut st, dir) = opened("branch-colour");
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.graph.wip.is_none(), "clean");
    let keys = ["main", "topic", "fix-1"];
    let before: Vec<Swatch> = keys.iter().map(|k| snap.branch_swatch(k)).collect();
    assert_eq!(before[0], Swatch::Trunk);
    assert_ne!(before[1], before[2], "side by side, never one hue");

    std::fs::write(dir.join("a.txt"), "changed\n").unwrap();
    let workdir = st.workdir().unwrap();
    let changed =
        Msg::Scoped { workdir, msg: Box::new(Msg::Watch(WatchMsg::RepoChanged { full: false, paths: None, last_event: Instant::now() })) };
    let fx = update(&mut st, changed, Instant::now());
    let status = fx
        .into_iter()
        .find(|e| matches!(e, Effect::Load(glowfly_model::effect::LoadEffect::Status { .. })))
        .and_then(glowfly_runner::testing::execute)
        .expect("a status read");
    let _ = update(&mut st, status, Instant::now());

    let snap = st.snap.as_ref().unwrap();
    assert!(snap.graph.wip.is_some(), "the WIP row appears");
    let after: Vec<Swatch> = keys.iter().map(|k| snap.branch_swatch(k)).collect();
    assert_eq!(after, before);
    let wip = snap.graph.owners.wip.expect("the WIP row has an owner");
    assert_eq!(snap.graph.owners.key(wip), Some("main"), "the WIP row takes HEAD's branch");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pin_recolours_at_once_is_saved_and_survives_a_reopen() {
    let (mut st, dir) = opened("branch-pin");
    let automatic = swatch(&st, "topic");
    let fx = pin(&mut st, "topic", Some(9));
    assert!(fx.iter().any(|e| matches!(e, Effect::Save(SaveEffect::Settings(_)))), "saved");
    assert!(!fx.iter().any(|e| matches!(e, Effect::Load(glowfly_model::effect::LoadEffect::Repo { .. }))), "no reload");
    assert_eq!(swatch(&st, "topic"), Swatch::Slot(9));
    assert_ne!(swatch(&st, "fix-1"), Swatch::Slot(9), "the automatic neighbour moves off it");
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    assert_eq!(st.settings.branch_pins_for(&real).get("topic"), Some(&9));
    assert!(pin(&mut st, "topic", Some(9)).is_empty(), "already pinned: nothing to save");

    // A fresh start with the saved settings.
    let mut again = state();
    again.settings = st.settings.clone();
    drive(&mut again, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert_eq!(swatch(&again, "topic"), Swatch::Slot(9), "kept across loads");

    let fx = pin(&mut st, "topic", None);
    assert!(fx.iter().any(|e| matches!(e, Effect::Save(SaveEffect::Settings(_)))));
    assert_eq!(swatch(&st, "topic"), automatic, "Automatic restores the picked hue");
    assert!(st.settings.branch_pins.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pinning_main_is_saved_and_recolours_it() {
    let (mut st, dir) = opened("branch-pin-main");
    assert_eq!(swatch(&st, "main"), Swatch::Trunk);
    let fx = pin(&mut st, "main", Some(3));
    assert!(fx.iter().any(|e| matches!(e, Effect::Save(SaveEffect::Settings(_)))), "saved");
    assert_eq!(swatch(&st, "main"), Swatch::Slot(3));
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    assert_eq!(st.settings.branch_pins_for(&real).get("main"), Some(&3));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn automatic_returns_main_to_the_trunk() {
    let (mut st, dir) = opened("branch-unpin-main");
    let _ = pin(&mut st, "main", Some(3));
    let fx = pin(&mut st, "main", None);
    assert!(fx.iter().any(|e| matches!(e, Effect::Save(SaveEffect::Settings(_)))), "saved");
    assert_eq!(swatch(&st, "main"), Swatch::Trunk);
    assert!(st.settings.branch_pins.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn slots_past_the_palette_are_refused() {
    let (mut st, dir) = opened("branch-pin-past");
    st.settings.colour_palette = ColourPalette::ColourBlind;
    assert!(pin(&mut st, "topic", Some(10)).is_empty(), "past the 8 colour-blind hues");
    assert!(pin(&mut st, "main", Some(8)).is_empty(), "main too");
    assert!(st.settings.branch_pins.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn switching_to_the_colour_blind_preset_moves_a_trunk_neighbour_off_a_near_slot() {
    let (mut st, dir) = opened("branch-trunk-near");
    // A commit on main after the others forked, so fix-1's line runs beside main's.
    std::fs::write(dir.join("main.txt"), "main").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "main moves on"]);
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let edit = glowfly_model::testing::settings_edit(&st, |s| s.colour_palette = ColourPalette::ColourBlind);
    drive(&mut st, edit);
    let Swatch::Slot(k) = swatch(&st, "fix-1") else { panic!("fix-1 is automatic") };
    // The shell reports the new palette's slots that pass for the trunk; say fix-1's is one.
    let fx = update(&mut st, Msg::Graph(GraphMsg::TrunkNear(1 << k)), Instant::now());
    assert!(fx.is_empty(), "nothing to save or reload");
    assert_eq!(swatch(&st, "main"), Swatch::Trunk);
    assert!(matches!(swatch(&st, "fix-1"), Swatch::Slot(s) if s != k && s < 8), "fix-1 moves off the near slot");
    assert_eq!(st.snap.as_ref().unwrap().graph.colouring.trunk_near, 1 << k);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_colour_blind_preset_reaches_the_snapshot_and_a_parked_tab() {
    let (mut st, dir) = opened("branch-preset");
    let other = scratch_repo("branch-preset-other");
    drive(&mut st, Msg::Repo(RepoMsg::Open(other.clone())));
    assert_eq!(st.tabs.len(), 2);
    let edit = glowfly_model::testing::settings_edit(&st, |s| s.colour_palette = ColourPalette::ColourBlind);
    drive(&mut st, edit);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.graph.colouring.n(), 8);
    assert!(snap.graph.swatches.iter().all(|s| matches!(s, Swatch::Trunk) || matches!(s, Swatch::Slot(n) if *n < 8)));

    // The first tab was parked while the preset changed: it is recoloured when it comes back,
    // before its refresh lands.
    let _ = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let snap = st.snap.as_ref().unwrap();
    assert!(glowfly_core::paths::same_folder(&snap.workdir, &dir), "back on the first repository");
    assert_eq!(snap.graph.colouring.n(), 8);
    assert!(snap.graph.swatches.iter().all(|s| matches!(s, Swatch::Trunk) || matches!(s, Swatch::Slot(n) if *n < 8)));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}
