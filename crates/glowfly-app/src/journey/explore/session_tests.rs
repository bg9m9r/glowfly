//! Sessions: the case text reads back, a seed decides the whole session, and a replay goes on
//! past actions that no longer apply.

use super::super::action::{Action, Outside};
use super::super::rules::Rule;
use super::{Checks, Outcome, Plan, actions, case_text, parse_case, report, run};

#[test]
fn case_round_trip() {
    let acts = vec![
        Action::Commit("say \"hi\"".into()),
        Action::Undo,
        Action::Outside(Outside::Append { path: "src/b.txt".into(), lines: 2 }),
        Action::Confirm("Stash and check out".into()),
    ];
    let text = case_text(5, Rule::Sound, &acts);
    assert!(text.contains("sound"), "{text}");
    assert_eq!(parse_case(&text), Ok((5, acts)));
}

#[test]
fn bad_cases_are_refused() {
    assert!(parse_case("undo\n").is_err(), "no seed line");
    assert!(parse_case("seed x\n").is_err());
    assert!(parse_case("seed 1\nfly away\n").is_err());
    assert_eq!(parse_case("# a comment\n\nseed 3\n"), Ok((3, Vec::new())));
}

#[test]
fn same_seed_same_session() {
    let checks = Checks { slow_every: 5 };
    let (a, b) = (run("d1", 11, Plan::Random { steps: 25 }, checks), run("d2", 11, Plan::Random { steps: 25 }, checks));
    assert!(a.broke.is_none(), "{}", report(&a, &actions(&a)));
    assert!(b.broke.is_none(), "{}", report(&b, &actions(&b)));
    assert_eq!(actions(&a), actions(&b));
    assert!(a.steps.len() >= 20, "the walk went on: {:?}", actions(&a));
}

#[test]
fn replay_reproduces() {
    let checks = Checks { slow_every: 5 };
    let first = run("r1", 11, Plan::Random { steps: 25 }, checks);
    assert!(first.broke.is_none(), "{}", report(&first, &actions(&first)));
    let again = run("r2", 11, Plan::Replay(actions(&first)), checks);
    assert!(again.broke.is_none(), "{}", report(&again, &actions(&again)));
    assert_eq!(again.steps.len(), first.steps.len());
}

#[test]
fn replay_skips_inapplicable() {
    let plan = Plan::Replay(vec![Action::Checkout("never-made".into()), Action::Commit("x".into())]);
    let o = run("skip", 11, plan, Checks { slow_every: 1 });
    assert!(o.broke.is_none(), "{}", report(&o, &actions(&o)));
    assert_eq!(actions(&o), [Action::Checkout("never-made".into()), Action::Commit("x".into())]);
    assert!(o.state.contains("main"), "the state names the branch: {}", o.state);
}

#[test]
fn the_report_names_the_rule_seed_and_actions() {
    use super::super::rules::Broken;
    let o = Outcome {
        seed: 9,
        steps: Vec::new(),
        broke: Some(Broken { rule: Rule::NoLostWork, step: 4, detail: "work lost from app".into() }),
        state: "branch main".into(),
    };
    let text = report(&o, &[Action::Stash, Action::Undo]);
    for part in ["no-lost-work", "step 4", "work lost from app", "seed 9", "stash\nundo", "branch main"] {
        assert!(text.contains(part), "{part:?} missing from:\n{text}");
    }
}

#[test]
fn a_dialogs_button_finishes_the_action_that_opened_it() {
    use super::super::{probe, scene};
    use super::{Judged, judged};
    let s = scene::build("judged", 1);
    let r = probe::read(&s.app);
    let at = |action: Action, mark: u64| Judged { tab: "app".into(), action, before: r.clone(), mark, undoable: None };
    let mut pending = None;
    assert!(judged(&mut pending, at(Action::DeleteBranch("topic".into()), 1), true, true).is_none(), "the dialog is open");
    assert!(judged(&mut pending, at(Action::Confirm("Delete anyway".into()), 2), false, true).is_none(), "still open");
    let done = judged(&mut pending, at(Action::Confirm("Delete anyway".into()), 3), true, false).expect("finished");
    assert_eq!((done.action, done.mark), (Action::DeleteBranch("topic".into()), 1), "judged with its own reading and mark");
    assert!(pending.is_none());

    assert!(judged(&mut pending, at(Action::Checkout("side".into()), 4), true, true).is_none());
    assert!(judged(&mut pending, at(Action::Confirm("Cancel".into()), 5), true, false).is_none(), "Cancel finishes nothing");
    assert!(pending.is_none());
    assert!(judged(&mut pending, at(Action::Confirm("Delete".into()), 6), true, false).is_none(), "nothing pending");
    let plain = judged(&mut pending, at(Action::Stash, 7), true, false).expect("no dialog");
    assert_eq!(plain.action, Action::Stash);
    assert!(judged(&mut pending, at(Action::Stash, 8), false, false).is_none(), "skipped");
}

#[test]
fn a_branch_deleted_through_its_dialog_passes() {
    let plan = Plan::Replay(vec![Action::DeleteBranch("topic".into()), Action::Confirm("Delete anyway".into())]);
    let o = run("delete", 3, plan, Checks { slow_every: 1 });
    assert!(o.broke.is_none(), "{}", report(&o, &actions(&o)));
    assert!(!o.state.contains("topic"), "topic is gone: {}", o.state);
}

#[test]
fn undo_after_continue_returns_to_before_the_operation() {
    // Continue finishes the rebase, and the app records the whole rebase: Undo goes back to before
    // it began, not to the rebase in progress.
    let text =
        "seed 1\nrebase \"side\"\ncheckout \"side\"\noutside create \"notes.txt\" +3\nresolve-ours \"shared.txt\"\nstash\ncontinue\nundo\n";
    // With a branch deleted during the rebase, one Undo doesn't bring the branch back too.
    let deleted = "seed 1\nrebase \"side\"\ncheckout \"side\"\noutside create \"notes.txt\" +3\ndelete-branch \"side\"\nconfirm \"Delete anyway\"\nselect-wip\nresolve-ours \"shared.txt\"\nstash\ncontinue\nundo\n";
    for (tag, case) in [("op-undo", text), ("op-undo-deleted", deleted)] {
        let (seed, list) = parse_case(case).unwrap();
        let o = run(tag, seed, Plan::Replay(list), Checks { slow_every: 1 });
        assert!(o.broke.is_none(), "{}", report(&o, &actions(&o)));
    }
}

#[test]
fn undo_after_a_step_of_two_entries_is_not_judged() {
    // Stash and check out records the checkout and then the put-back of the changes: one Undo
    // takes the changes back into the stash and stays on main.
    let text = "seed 1\ncreate-branch \"b456\"\noutside write \"a.txt\" +1\ncheckout \"main\"\nconfirm \"Stash and check out\"\nundo\n";
    let (seed, list) = parse_case(text).unwrap();
    let o = run("two-entries", seed, Plan::Replay(list), Checks { slow_every: 1 });
    assert!(o.broke.is_none(), "{}", report(&o, &actions(&o)));
    assert!(o.state.contains("branch: main"), "{}", o.state);
}
