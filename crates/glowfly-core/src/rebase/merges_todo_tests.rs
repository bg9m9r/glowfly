//! Pure tests of the label, reset and merge steps: how they are written, checked and compared.

use super::*;
use git2::Oid;
use std::path::Path;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn item(n: u8, action: RebaseAction) -> TodoItem {
    TodoItem {
        oid: oid(n),
        action,
        summary: format!("commit {n}"),
        message: format!("commit {n}\n"),
        new_message: None,
        combined_message: None,
        author: "Test".into(),
        time: 1_700_000_000,
    }
}

fn c(n: u8, action: RebaseAction) -> TodoStep {
    TodoStep::Commit(item(n, action))
}

fn merge(n: u8, action: RebaseAction, parents: Vec<MergeParent>) -> TodoStep {
    TodoStep::Merge(MergeStep { item: item(n, action), parents })
}

fn label(n: u8) -> TodoStep {
    TodoStep::Label { name: format!("at-{n}"), at: oid(n) }
}

#[cfg(windows)]
const MSG_DIR: &str = "C:\\repo\\.git\\glowfly-rebase";
#[cfg(windows)]
const QUOTED_DIR: &str = "C:/repo/.git/glowfly-rebase";
#[cfg(not(windows))]
const MSG_DIR: &str = "/repo/.git/glowfly-rebase";
#[cfg(not(windows))]
const QUOTED_DIR: &str = "/repo/.git/glowfly-rebase";

/// A list in the shape `merges::script` makes: two lines and the merge joining them.
fn list(merge_action: RebaseAction) -> Vec<TodoStep> {
    use RebaseAction::Pick;
    vec![
        TodoStep::Reset(ResetTo::Onto),
        c(1, Pick),
        label(1),
        TodoStep::Reset(ResetTo::Commit(oid(9))),
        c(2, Pick),
        merge(3, merge_action, vec![MergeParent::Label("at-1".into()), MergeParent::Commit(oid(8))]),
        TodoStep::Reset(ResetTo::NewRoot),
        TodoStep::Reset(ResetTo::Label("at-1".into())),
    ]
}

#[test]
fn build_writes_labels_resets_and_merges() {
    let (text, files) = build(&list(RebaseAction::Pick), Path::new(MSG_DIR), false);
    let want = [
        "label onto".to_owned(),
        "reset onto".into(),
        format!("pick {} commit 1", oid(1)),
        "label at-1".into(),
        format!("reset {}", oid(9)),
        format!("pick {} commit 2", oid(2)),
        format!("merge -C {} at-1 {} # commit 3", oid(3), oid(8)),
        "reset [new root]".into(),
        "reset at-1".into(),
    ];
    assert_eq!(text.lines().collect::<Vec<_>>(), want);
    assert!(files.is_empty());
    let (plain, _) = build(&[c(1, RebaseAction::Pick)], Path::new(MSG_DIR), false);
    assert!(!plain.contains("label onto"), "only a list with structure names the new base");
}

#[test]
fn dropped_and_reworded_merges() {
    let (text, _) = build(&list(RebaseAction::Drop), Path::new(MSG_DIR), false);
    assert!(!text.contains("merge "), "a dropped merge writes nothing: {text}");

    let mut steps = list(RebaseAction::Reword);
    if let TodoStep::Merge(m) = &mut steps[5] {
        m.item.new_message = Some("Merged\n\n".into());
    }
    let (text, files) = build(&steps, Path::new(MSG_DIR), false);
    let lines: Vec<&str> = text.lines().collect();
    let base = format!("'{QUOTED_DIR}/reword-{}.base'", oid(3));
    assert_eq!(lines[6], format!("exec git rev-parse HEAD >{base}"));
    assert!(lines[7].starts_with(&format!("merge -C {}", oid(3))), "{text}");
    assert!(lines[8].starts_with("exec h=$(git rev-parse") && lines[8].contains(&format!("reword-{}.txt", oid(3))), "{text}");
    assert_eq!(files, [(Path::new(MSG_DIR).join(format!("reword-{}.txt", oid(3))), "Merged\n".to_owned())]);

    // A reword that keeps the message is a plain merge.
    let (text, files) = build(&list(RebaseAction::Reword), Path::new(MSG_DIR), false);
    assert!(!text.contains("exec") && files.is_empty(), "{text}");
}

#[test]
fn a_squash_after_structure_is_refused() {
    use RebaseAction::*;
    let after_merge = [c(1, Pick), merge(2, Pick, vec![]), c(3, Squash)];
    assert_eq!(problem(&after_merge).as_deref(), Some(COMBINE_AFTER_MERGE));
    let after_label = [c(1, Pick), label(1), c(3, Fixup)];
    assert_eq!(problem(&after_label).as_deref(), Some(COMBINE_AFTER_MERGE));
    let after_reset = [c(1, Pick), TodoStep::Reset(ResetTo::Onto), c(3, Squash)];
    assert_eq!(problem(&after_reset).as_deref(), Some(COMBINE_AFTER_MERGE));
    assert_eq!(problem(&[c(1, Pick), TodoStep::Break, c(3, Squash)]).as_deref(), Some(COMBINE_AFTER_STEP));
    assert_eq!(problem(&[merge(2, Pick, vec![]), c(3, Pick)]), None);
    assert_eq!(problem(&list(Pick)), None);
}

#[test]
fn a_commit_a_reset_throws_away_is_refused() {
    use RebaseAction::*;
    // Moved between its line's closing label and the next reset.
    let mut after_label = list(Pick);
    let moved = after_label.remove(1);
    after_label.insert(2, moved);
    assert_eq!(problem(&after_label).as_deref(), Some(LOST_TO_RESET), "{after_label:?}");
    // Moved above the first reset.
    let mut above = list(Pick);
    let moved = above.remove(4);
    above.insert(0, moved);
    assert_eq!(problem(&above).as_deref(), Some(LOST_TO_RESET));
    // A dropped commit there loses nothing; a dropped merge keeps nothing.
    let mut dropped = after_label.clone();
    dropped[2] = c(1, Drop);
    assert_eq!(problem(&dropped), None);
    let dropped_merge = [TodoStep::Reset(ResetTo::Onto), c(1, Pick), merge(2, Drop, vec![]), TodoStep::Reset(ResetTo::Onto)];
    assert_eq!(problem(&dropped_merge).as_deref(), Some(LOST_TO_RESET));
    // Commits after the last label or merge end the branch.
    assert_eq!(problem(&[TodoStep::Reset(ResetTo::Onto), c(1, Pick), label(1), c(2, Pick)]), None);
}

#[test]
fn unchanged_lists_with_structure() {
    let original = list(RebaseAction::Pick);
    assert!(is_unchanged(&original, &original));
    assert!(!is_unchanged(&list(RebaseAction::Drop), &original), "a dropped merge is a change");
    let mut moved = original.clone();
    moved.swap(4, 1);
    assert!(!is_unchanged(&moved, &original));
    assert!(is_unchanged(&list(RebaseAction::Reword), &original), "a reword keeping its message changes nothing");
}

#[test]
fn step_helpers() {
    let m = merge(3, RebaseAction::Pick, vec![]);
    assert_eq!((m.oid(), m.commit().is_none(), m.item().is_some(), m.is_structure()), (Some(oid(3)), true, true, true));
    assert_eq!((label(1).oid(), label(1).is_structure()), (None, true));
    assert!(!c(1, RebaseAction::Pick).is_structure());
}
