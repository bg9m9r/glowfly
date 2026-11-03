//! The panel's steps and groups without a repository: adding and removing exec and break steps,
//! moving the selection, and forgetting a combined message whose group changed.

use super::NewStep;
use crate::state::rebase::{Editing, RebasePanel};
use git2::Oid;
use glowfly_core::rebase::{
    RebaseAction::{self, *},
    RebaseBase, RebasePlan, Remaining, TodoItem, TodoStep, Upstream,
};

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn c(n: u8, action: RebaseAction) -> TodoStep {
    TodoStep::Commit(TodoItem {
        oid: oid(n),
        action,
        summary: format!("c{n}"),
        message: format!("c{n}\n\nbody {n}\n"),
        new_message: None,
        combined_message: None,
        author: "Test".into(),
        time: 0,
    })
}

/// A loaded panel holding `steps`.
fn panel(steps: Vec<TodoStep>) -> RebasePanel {
    let mut p = RebasePanel::loading(RebaseBase::FromCommit(oid(1)));
    let plan = RebasePlan {
        head: oid(9),
        branch: "main".into(),
        upstream: Upstream::Root,
        onto_label: String::new(),
        steps: Vec::new(),
        held: Vec::new(),
    };
    p.fill(plan);
    p.original = steps.clone();
    p.items = steps;
    p
}

fn kinds(p: &RebasePanel) -> Vec<String> {
    p.items
        .iter()
        .map(|s| match s {
            TodoStep::Commit(i) => format!("{} {}", i.action.keyword(), i.summary),
            TodoStep::Exec(cmd) => format!("exec {cmd}"),
            TodoStep::Break => "break".into(),
            other => format!("{other:?}"),
        })
        .collect()
}

fn combined(p: &RebasePanel, i: usize) -> Option<&str> {
    p.items[i].commit().and_then(|c| c.combined_message.as_deref())
}

#[test]
fn steps_go_after_the_selection_or_its_group() {
    let mut p = panel(vec![c(1, Pick), c(2, Squash), c(3, Drop), c(4, Fixup), c(5, Pick)]);
    p.add_step(Some(0), NewStep::Break);
    assert_eq!(kinds(&p), ["pick c1", "squash c2", "drop c3", "fixup c4", "break", "pick c5"], "after the group's last row");
    assert_eq!((p.selected, p.editing), (Some(4), None));
    p.add_step(Some(2), NewStep::Exec);
    assert_eq!(p.items[4], TodoStep::Exec(String::new()), "a member's step goes after the group too");
    assert_eq!((p.selected, p.editing), (Some(4), Some(Editing::Command(4))), "an exec step opens its command editor");
    p.draft = "cargo test".into();
    p.save_message();
    assert_eq!(p.items[4], TodoStep::Exec("cargo test".into()));
    p.add_step(None, NewStep::Break);
    assert_eq!(p.items.last(), Some(&TodoStep::Break), "without a selection at the end");
    p.add_step(Some(6), NewStep::Exec);
    assert_eq!(p.insert_at(Some(6)), 7);
    assert_eq!(p.insert_at(Some(99)), p.items.len());
}

#[test]
fn an_empty_exec_goes_away_with_its_editor() {
    let mut p = panel(vec![c(1, Pick), c(2, Pick)]);
    p.add_step(Some(0), NewStep::Exec);
    assert_eq!(p.items.len(), 3);
    p.draft = "   ".into();
    p.save_message();
    assert_eq!(p.editing, Some(Editing::Command(1)), "an empty command can't be saved");
    p.close_editor();
    assert_eq!(kinds(&p), ["pick c1", "pick c2"]);
    // A command typed before stays when its editor is cancelled.
    p.add_step(Some(1), NewStep::Exec);
    p.draft = "make".into();
    p.save_message();
    p.open_editor(2);
    assert_eq!((p.editing, p.draft.as_str()), (Some(Editing::Command(2)), "make"));
    p.draft.clear();
    p.close_editor();
    assert_eq!(p.items[2], TodoStep::Exec("make".into()));
}

#[test]
fn only_steps_can_be_removed() {
    let mut p = panel(vec![c(1, Pick), TodoStep::Break, TodoStep::Exec("x".into()), c(2, Pick)]);
    p.selected = Some(3);
    p.remove_step(0);
    assert_eq!(p.items.len(), 4, "a commit stays");
    p.remove_step(1);
    assert_eq!(kinds(&p), ["pick c1", "exec x", "pick c2"]);
    assert_eq!(p.selected, Some(2), "the selection follows its row");
    p.selected = Some(1);
    p.remove_step(1);
    assert_eq!(kinds(&p), ["pick c1", "pick c2"]);
    assert_eq!(p.selected, Some(1), "the next row is selected");
}

#[test]
fn selection_moves_and_follows() {
    let mut p = panel(vec![c(1, Pick), c(2, Pick), c(3, Pick)]);
    assert_eq!((p.next_selection(1), p.next_selection(-1)), (Some(0), Some(2)), "from no selection: first or last");
    p.selected = Some(2);
    assert_eq!((p.next_selection(1), p.next_selection(-1)), (Some(2), Some(1)), "it stops at the ends");
    p.selected = Some(0);
    assert_eq!(p.next_selection(-1), Some(0));
    p.move_item(0, 2);
    assert_eq!(p.selected, Some(2), "the selected row moved");
    p.move_item(1, 0);
    assert_eq!(p.selected, Some(2));
    assert_eq!(panel(Vec::new()).next_selection(1), None);
}

#[test]
fn a_changed_group_forgets_its_combined_message() {
    let typed = |p: &mut RebasePanel| p.items[0].commit_mut().unwrap().combined_message = Some("Typed".into());
    let mut p = panel(vec![c(1, Pick), c(2, Squash), c(3, Fixup), c(4, Pick)]);
    typed(&mut p);
    p.move_item(3, 1);
    assert_eq!(combined(&p, 0), None, "a commit moved into the group");

    let mut p = panel(vec![c(1, Pick), c(2, Squash), c(3, Fixup), c(4, Pick)]);
    typed(&mut p);
    p.set_action(2, Pick);
    assert_eq!(combined(&p, 0), None, "a member left the group");

    let mut p = panel(vec![c(1, Pick), c(2, Squash), c(3, Fixup), c(4, Pick)]);
    typed(&mut p);
    p.items.insert(2, TodoStep::Break);
    let before = super::groups(&[c(1, Pick), c(2, Squash), c(3, Fixup), c(4, Pick)]);
    p.prune_combined(&before);
    assert_eq!(combined(&p, 0), None, "a step split the group");

    let mut p = panel(vec![c(1, Pick), c(2, Squash), c(3, Pick)]);
    typed(&mut p);
    p.set_action(0, Edit);
    p.add_step(Some(2), NewStep::Break);
    assert_eq!(combined(&p, 0), Some("Typed"), "changes outside the group keep it");
    p.set_action(1, Drop);
    assert_eq!(combined(&p, 0), None, "no longer heads a group");
}

#[test]
fn combined_editor_and_gits_default() {
    let mut p = panel(vec![c(1, Pick), c(2, Squash), c(3, Fixup)]);
    p.open_editor(0);
    assert_eq!(p.editing, Some(Editing::Combined(0)));
    assert_eq!(p.editor_row(), Some(2), "drawn under the group's last row");
    assert_eq!(p.draft, "c1\n\nbody 1\n\nc2\n\nbody 2");
    p.save_message();
    assert_eq!(combined(&p, 0), None, "git's own message stores nothing");
    p.set_action(0, Reword);
    assert_eq!(p.editing, Some(Editing::Combined(0)), "reword on a head opens the combined editor");
    p.draft = "Together".into();
    p.save_message();
    assert_eq!(combined(&p, 0), Some("Together"));
    p.open_editor(1);
    assert_eq!(p.editing, Some(Editing::Message(1)), "a member edits its own message");
    p.reset();
    assert_eq!(combined(&p, 0), None);
}

#[test]
fn remaining_drops_count_only_new_ones() {
    let mut p = RebasePanel::loading_remaining();
    let r = Remaining {
        branch: Some("main".into()),
        onto_label: "1a2b3c4 Base".into(),
        done: 2,
        locked: Vec::new(),
        locked_text: String::new(),
        steps: vec![c(1, Drop), c(2, Pick), c(3, Pick)],
        first_may_combine: true,
        todo: String::new(),
    };
    p.fill_remaining(r);
    assert_eq!(p.drops(), 0, "already dropped in git's list");
    p.set_action(2, Drop);
    assert_eq!(p.drops(), 1);
    assert_eq!(p.subtitle(glowfly_core::i18n::Lang::En), "3 steps left in the rebase of main onto 1a2b3c4, 2 done.");
    assert!(p.first_may_combine());
}

#[test]
fn the_header_says_the_order_and_where_the_list_ends() {
    use glowfly_core::i18n::Lang;
    let mut p = panel(vec![c(1, Pick), c(2, Pick)]);
    assert_eq!(p.subtitle(Lang::En), "Rebasing 2 commits of main from the first commit.");
    assert_eq!(p.order_hint(Lang::En), "Oldest first: commits are applied top to bottom, and the last becomes the tip of main.");
    assert_eq!(p.tip_text(Lang::En), "Tip of main");
    let plan = p.plan.as_mut().unwrap();
    plan.upstream = Upstream::Commit(oid(7));
    plan.onto_label = "b9eb369 Add app skeleton".into();
    assert_eq!(p.subtitle(Lang::En), "Rebasing 2 commits of main onto b9eb369.", "a commit by its short id alone");
    p.plan.as_mut().unwrap().onto_label = "release/2.0".into();
    assert_eq!(p.subtitle(Lang::En), "Rebasing 2 commits of main onto release/2.0.", "a branch by its name");
    for lang in [Lang::De, Lang::Es] {
        assert_ne!(p.subtitle(lang), p.subtitle(Lang::En));
        assert!(p.order_hint(lang).contains("main") && p.tip_text(lang).contains("main"), "{lang:?}");
    }

    let mut r = RebasePanel::loading_remaining();
    r.fill_remaining(Remaining {
        branch: None,
        onto_label: "1a2b3c4 Base".into(),
        done: 1,
        locked: Vec::new(),
        locked_text: String::new(),
        steps: vec![c(3, Pick)],
        first_may_combine: false,
        todo: String::new(),
    });
    assert_eq!(r.subtitle(Lang::En), "1 step left in the rebase of a detached HEAD onto 1a2b3c4, 1 done.");
    assert_eq!(r.order_hint(Lang::En), "Oldest first: the remaining steps run top to bottom, and the last becomes the new HEAD.");
    assert_eq!(r.tip_text(Lang::En), "New HEAD");
    r.remaining.as_mut().unwrap().branch = Some("feature".into());
    assert_eq!(r.order_hint(Lang::En), "Oldest first: the remaining steps run top to bottom, and the last becomes the tip of feature.");
    assert_eq!(r.tip_text(Lang::En), "Tip of feature");
}

fn ur(branch: &str) -> TodoStep {
    TodoStep::UpdateRef(format!("refs/heads/{branch}"))
}

#[test]
fn update_ref_rows_travel_with_their_commit() {
    let mut p = panel(vec![c(1, Pick), ur("lower"), c(2, Pick), c(3, Pick), ur("mid")]);
    assert!(!p.movable(1), "an update-ref row moves only with its commit");
    p.move_item(1, 0);
    assert_eq!(p.items[1], ur("lower"));
    p.selected = Some(0);
    p.move_item(0, 1);
    assert_eq!(kinds(&p)[..3], ["pick c2", "pick c1", "UpdateRef(\"refs/heads/lower\")"]);
    assert_eq!(p.selected, Some(1), "the selection follows its row");
    // Down onto mid's row: past c3 and mid.
    p.move_item(1, 4);
    assert_eq!(p.items[1..], [c(3, Pick), ur("mid"), c(1, Pick), ur("lower")]);
    // A step added after a commit goes below its update-ref rows.
    p.add_step(Some(1), NewStep::Break);
    assert_eq!(p.items[1..4], [c(3, Pick), ur("mid"), TodoStep::Break]);
    assert!(!p.items.iter().any(|s| matches!(s, TodoStep::Exec(_))));
}

/// A running rebase whose first remaining row is a branch's update (its commit is done):
/// moving a commit above it, or a squash group taking it in, leaves the branch elsewhere.
#[test]
fn edits_that_move_a_stacked_branch_are_named() {
    let mut p = panel(vec![ur("lower"), c(3, Pick), c(4, Pick)]);
    assert!(p.moved_refs().is_empty());
    p.move_item(2, 0);
    assert_eq!(p.items[..2], [c(4, Pick), ur("lower")]);
    assert_eq!(p.moved_refs(), ["lower"], "c4 now comes before lower's update");
    p.reset();
    p.set_action(1, Fixup);
    assert_eq!(p.moved_refs(), ["lower"], "c3 melds into lower's commit");
    p.set_action(1, Drop);
    assert!(p.moved_refs().is_empty(), "a dropped c3 leaves lower where it was");
}
