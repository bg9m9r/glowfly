//! The stash builder: menu order, what each item sends on a clean tree, a dirty one and over
//! conflicts, finding a stash again by its commit, and Unstash the latest.

use super::*;
use crate::state::stash_confirm::tests::{EN, NOW, clock, fixture, listed, oid, with_stash};
use crate::state::stashes::drop_confirm_body;
use glowfly_core::repo::FileEntry;
use glowfly_core::settings::Settings;

fn menu(snap: &Snapshot) -> [StashItem; 4] {
    items(snap, clock(&Settings::default()), &snap.stashes[0])
}

#[test]
fn items_come_in_menu_order_with_delete_as_the_danger_item() {
    let (_, snap) = fixture("actions-order");
    let snap = with_stash(snap);
    let items = menu(&snap);
    let options: Vec<StashOption> = items.iter().map(|i| i.option).collect();
    assert_eq!(options, [StashOption::Apply, StashOption::Pop, StashOption::Rename, StashOption::Drop]);
    let danger: Vec<bool> = items.iter().map(|i| i.danger).collect();
    assert_eq!(danger, [false, false, false, true]);
    assert!(items.iter().all(|i| i.blocked.is_none()));
}

#[test]
fn a_clean_tree_sends_the_job_the_dialog_and_the_delete_confirmation() {
    let (_, snap) = fixture("actions-clean");
    let mut snap = with_stash(snap);
    snap.stashes[0].index = 3;
    let [apply, pop, rename, drop] = menu(&snap);
    assert!(matches!(apply.msg, Msg::Stash(StashMsg::Run { action: StashAction::Apply, index: 3, oid: o }) if o == oid()));
    assert!(matches!(pop.msg, Msg::Stash(StashMsg::Run { action: StashAction::Pop, index: 3, oid: o }) if o == oid()));
    assert!(matches!(rename.msg, Msg::Stash(StashMsg::RenameDialog { oid: o }) if o == oid()));
    assert!(matches!(&drop.msg, Msg::ConfirmDanger { title, then, .. }
        if title == "Delete stash" && matches!(**then, Msg::Stash(StashMsg::Run { action: StashAction::Drop, index: 3, .. }))));
}

#[test]
fn a_dirty_tree_asks_before_apply_and_pop() {
    let (_, snap) = fixture("actions-dirty");
    let mut snap = with_stash(snap);
    snap.unstaged.push(FileEntry::new("a.txt", 'M'));
    let [apply, pop, rename, drop] = menu(&snap);
    for (item, want) in [(apply, StashAction::Apply), (pop, StashAction::Pop)] {
        let Msg::Stash(StashMsg::Ask(ask)) = item.msg else { panic!("not the question") };
        assert_eq!(ask.run, listed(0));
        assert_eq!(ask.action, want);
    }
    assert!(matches!(rename.msg, Msg::Stash(StashMsg::RenameDialog { .. })));
    assert!(matches!(drop.msg, Msg::ConfirmDanger { .. }));
}

#[test]
fn conflicts_block_apply_and_pop_only() {
    let (_, snap) = fixture("actions-conflicts");
    let mut snap = with_stash(snap);
    snap.unstaged.push(FileEntry::new("a.txt", '!'));
    let why = apply_blocked(&snap, EN).expect("blocked over conflicts");
    let [apply, pop, rename, drop] = menu(&snap);
    for item in [apply, pop] {
        assert_eq!(item.blocked, Some(why));
        assert!(matches!(&item.msg, Msg::Logged { ok: false, text } if text == why));
    }
    assert!(rename.blocked.is_none() && drop.blocked.is_none());
}

#[test]
fn for_commit_finds_the_stash_again_or_gives_none() {
    let (_, snap) = fixture("actions-for-commit");
    assert!(for_commit(&snap, clock(&Settings::default()), StashOption::Apply, oid()).is_none(), "no stashes");
    let snap = with_stash(snap);
    for item in menu(&snap) {
        let msg = for_commit(&snap, clock(&Settings::default()), item.option, oid()).expect("listed");
        let same = matches!(
            (&msg, &item.msg),
            (Msg::Stash(StashMsg::Run { .. }), Msg::Stash(StashMsg::Run { .. }))
                | (Msg::Stash(StashMsg::RenameDialog { .. }), Msg::Stash(StashMsg::RenameDialog { .. }))
                | (Msg::ConfirmDanger { .. }, Msg::ConfirmDanger { .. })
        );
        assert!(same, "{:?}", item.option);
    }
    assert!(for_commit(&snap, clock(&Settings::default()), StashOption::Pop, Oid::zero()).is_none(), "gone");
}

#[test]
fn pop_latest_names_the_newest_stash_by_position_and_commit() {
    let (_, snap) = fixture("pop-latest");
    assert!(pop_latest(&snap, EN).is_none(), "no stashes");
    let snap = with_stash(snap);
    let m = pop_latest(&snap, EN).unwrap();
    assert!(matches!(m, Msg::Stash(StashMsg::Run { action: StashAction::Pop, index: 0, oid: o }) if o == oid()));
}

#[test]
fn every_entry_point_sends_the_same_run() {
    let (_, snap) = fixture("actions-entry-points");
    let mut snap = with_stash(snap);
    snap.stashes[0].index = 2;
    let [apply, pop, ..] = menu(&snap);
    let o = oid();
    let is =
        |msg: &Msg, want: StashAction| matches!(msg, Msg::Stash(StashMsg::Run { action, index: 2, oid }) if *action == want && *oid == o);
    let by_commit = |option| for_commit(&snap, clock(&Settings::default()), option, o).expect("listed");
    let latest = pop_latest(&snap, EN).expect("a stash");
    assert!(is(&pop.msg, StashAction::Pop) && is(&by_commit(StashOption::Pop), StashAction::Pop) && is(&latest, StashAction::Pop));
    assert!(is(&apply.msg, StashAction::Apply) && is(&by_commit(StashOption::Apply), StashAction::Apply));
}

#[test]
fn delete_names_when_the_stash_was_made() {
    let (_, snap) = fixture("actions-delete-when");
    let mut snap = with_stash(snap);
    let settings = Settings::default();
    let body = |msg: Msg| match msg {
        Msg::ConfirmDanger { body, .. } => body,
        _ => panic!("not the confirmation"),
    };
    let both = |snap: &Snapshot| {
        let [.., drop] = items(snap, clock(&settings), &snap.stashes[0]);
        (body(drop.msg), body(for_commit(snap, clock(&settings), StashOption::Drop, oid()).unwrap()))
    };
    snap.stashes[0].time = Some(NOW - 3 * 86_400);
    let want = "Delete \"On main: wip\", stashed 3 days ago? Undo brings it back while this tab is open.";
    assert_eq!(both(&snap), (want.to_owned(), want.to_owned()));
    snap.stashes[0].time = None;
    let undated = drop_confirm_body(EN, "On main: wip", None);
    assert_eq!(both(&snap), (undated.clone(), undated));
}
