//! The palette's submodule entries: per submodule by state, the batch entries while they have
//! something to do, and Add submodule… without any.

use super::submodule_tests::{PATH, cleanup, deinit, opened};
use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::{Effect, SubmoduleEffect};
use crate::msg::{Msg, PaletteMsg, RepoMsg};
use crate::state::AppState;
use crate::state::palette::{PaletteCommand as C, entries};
use crate::submodule::{Batch, SubmoduleMsg};
use std::time::Instant;

fn labels(st: &AppState) -> Vec<String> {
    entries(st).into_iter().map(|e| e.label).collect()
}

fn has(st: &AppState, label: &str) -> bool {
    labels(st).iter().any(|l| l == label)
}

#[test]
fn entries_follow_each_submodule() {
    let (mut st, parent, lib) = opened("smp-entries");
    for l in [format!("Open submodule {PATH}"), format!("Edit submodule {PATH}…"), format!("Remove submodule {PATH}…")] {
        assert!(has(&st, &l), "{l}: {:?}", labels(&st));
    }
    assert!(!has(&st, &format!("Update submodule {PATH}")), "in sync");
    assert!(!has(&st, "Initialize all submodules") && !has(&st, "Update all submodules"));

    deinit(&mut st, &parent);
    assert!(has(&st, &format!("Initialize submodule {PATH}")) && has(&st, "Initialize all submodules"));
    assert!(has(&st, "Update all submodules"));
    assert!(!has(&st, &format!("Open submodule {PATH}")));
    let init = entries(&st).into_iter().find(|e| e.label == format!("Initialize submodule {PATH}")).unwrap();
    assert_eq!(init.detail.as_deref(), Some("not initialized"));

    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(all(Batch::InitAll))), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::Submodule(SubmoduleEffect::CheckAll { batch: Batch::InitAll, .. }))), "{fx:?}");
    cleanup(&[&parent, &lib]);
}

fn all(batch: Batch) -> C {
    C::Submodule(SubmoduleMsg::Batch { batch, confirmed: false })
}

#[test]
fn without_submodules_only_add() {
    let dir = scratch_repo("smp-none");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let subs: Vec<String> = labels(&st).into_iter().filter(|l| l.contains("ubmodule")).collect();
    assert_eq!(subs, ["Add submodule…"]);
    cleanup(&[&dir]);
}
