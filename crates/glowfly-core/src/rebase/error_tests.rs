//! `RebaseError`: every variant shows the English its refusal always showed, git's and the
//! system's errors keep their own words, `Busy` names every operation in progress, and the
//! helper's refusal file reads back as written (or as its raw text).

use super::*;
use crate::error::Error;
use crate::rebase::{in_progress_text, operation_noun};
use git2::RepositoryState;

/// The variants without fields, with the sentence each always showed.
fn sentences() -> Vec<(RebaseError, &'static str)> {
    use RebaseError as E;
    vec![
        (E::Stale, STALE),
        (E::Changed, CHANGED),
        (E::Detached, DETACHED),
        (E::FirstCombines, FIRST_COMBINES),
        (E::EmptyReword, EMPTY_REWORD),
        (E::NotOnBranch, NOT_ON_BRANCH),
        (E::SomeNotOnBranch, SOME_NOT_ON_BRANCH),
        (E::MergeSelected, MERGE_SELECTED),
        (E::SomeMergeSelected, SOME_MERGE_SELECTED),
        (E::SideLine, SIDE_LINE),
        (E::SomeSideLine, SOME_SIDE_LINE),
        (E::BranchTip, BRANCH_TIP),
        (E::SomeBranchTips, SOME_BRANCH_TIPS),
        (E::SplitByBranch, SPLIT_BY_BRANCH),
        (E::CombineAfterMerge, COMBINE_AFTER_MERGE),
        (E::LostToReset, LOST_TO_RESET),
        (E::RootSquash, ROOT_SQUASH),
        (E::ParentMerge, PARENT_MERGE),
        (E::NotConsecutive, NOT_CONSECUTIVE),
        (E::TooFewToSquash, TOO_FEW_TO_SQUASH),
        (E::DropsEverything, DROPS_EVERYTHING),
        (E::NotRebasing, NOT_REBASING),
        (E::EmptyCombined, EMPTY_COMBINED),
        (E::EmptyExec, EMPTY_EXEC),
        (E::ExecOneLine, EXEC_ONE_LINE),
        (E::CombineAfterStep, COMBINE_AFTER_STEP),
        (E::NotInteractive, NOT_INTERACTIVE),
        (E::UnsupportedSteps, UNSUPPORTED_STEPS),
        (E::UpdateRefInGroup, UPDATE_REF_IN_GROUP),
        (E::RefsChanged, REFS_CHANGED),
        (E::TodoMoved, TODO_MOVED),
        (E::NotUtf8Branch, NOT_UTF8),
        (E::StackChanged, crate::stack::STALE_PLAN),
        (E::StillRebasing, crate::stack::STILL_REBASING),
        (E::NoCommitsYet, "The branch has no commits yet."),
        (E::EditTodoFailed, "git rebase --edit-todo failed"),
    ]
}

fn left(name: &str, at: Option<&str>) -> LeftOut {
    LeftOut { name: name.into(), checked_out_at: at.map(str::to_owned) }
}

/// The variants with fields, with the English each always showed.
fn formatted() -> Vec<(RebaseError, String)> {
    use RebaseError as E;
    let held = |elsewhere| E::Held { alias: "alias".into(), target: "mid".into(), elsewhere };
    vec![
        (E::CommitNotOnBranch { commit: "1a2b3c4".into() }, "1a2b3c4 is not on the current branch".into()),
        (
            E::NothingToRebase { branch: "main".into(), onto: "1a2b3c4 c4".into() },
            "main has no commits that aren't already on 1a2b3c4 c4".into(),
        ),
        (E::BranchNotFound { name: "x".into() }, Error::NotFound("branch x".into()).to_string()),
        (E::CommitNotFound { commit: "y".into() }, Error::NotFound("commit y".into()).to_string()),
        (
            held(true),
            "alias points at mid, which is checked out in another worktree; git would move mid with this rebase (rebase.updateRefs). Delete alias or switch that worktree to another branch first.".into(),
        ),
        (
            held(false),
            "alias points at mid, which isn't a local branch; git would move it with this rebase (rebase.updateRefs). Delete alias first.".into(),
        ),
        (
            E::NotMoved { branches: vec![left("lower", None), left("mid", None)] },
            "git won't move lower, mid with this rebase, though the list does: check rebase.updateRefs and the git you run, and try again.".into(),
        ),
        (
            E::NotMoved { branches: vec![left("lower", None), left("mid", Some("'/work/wt'"))] },
            "git won't move lower, mid (checked out at '/work/wt') with this rebase (rebase.updateRefs), though the list does. Switch that worktree to another branch, or turn rebase.updateRefs off, and try again.".into(),
        ),
        (E::Said { said: "fatal: no rebase in progress".into() }, "fatal: no rebase in progress".into()),
        (E::Panicked { effect: "rebase".into(), panic: "boom".into() }, "Glowfly hit an internal error (rebase): boom".into()),
        (E::Other { said: "something else".into() }, "something else".into()),
    ]
}

const STATES: [RepositoryState; 12] = {
    use RepositoryState::*;
    [
        Clean,
        Merge,
        Revert,
        RevertSequence,
        CherryPick,
        CherryPickSequence,
        Bisect,
        Rebase,
        RebaseInteractive,
        RebaseMerge,
        ApplyMailbox,
        ApplyMailboxOrRebase,
    ]
};

#[test]
fn every_variant_shows_its_old_english() {
    for (e, english) in sentences() {
        assert_eq!(e.to_string(), english, "{e:?}");
    }
    for (e, english) in formatted() {
        assert_eq!(e.to_string(), english, "{e:?}");
    }
    let alias = HeldAlias { alias: "alias".into(), target: "mid".into(), elsewhere: true };
    assert_eq!(alias.text(), formatted()[4].1);
}

#[test]
fn in_progress_shows_the_old_sentence_for_every_state() {
    for state in STATES {
        let old = operation_noun(state).map(|op| format!("Finish or abort the {op} in progress before starting an interactive rebase."));
        assert_eq!(in_progress_text(state), old, "{state:?}");
        assert_eq!(Busy::of(state).map(|op| RebaseError::InProgress { op }.to_string()), old, "{state:?}");
    }
    let am = RebaseError::InProgress { op: Busy::PatchApplication };
    assert_eq!(am.to_string(), "Finish or abort the patch application in progress before starting an interactive rebase.");
}

#[test]
fn busy_names_every_operation_in_progress() {
    use RepositoryState::*;
    let nouns: Vec<(RepositoryState, Option<&str>)> = STATES.iter().map(|s| (*s, operation_noun(*s))).collect();
    assert_eq!(
        nouns,
        [
            (Clean, None),
            (Merge, Some("merge")),
            (Revert, Some("revert")),
            (RevertSequence, Some("revert")),
            (CherryPick, Some("cherry-pick")),
            (CherryPickSequence, Some("cherry-pick")),
            (Bisect, Some("bisect")),
            (Rebase, Some("rebase")),
            (RebaseInteractive, Some("rebase")),
            (RebaseMerge, Some("rebase")),
            (ApplyMailbox, Some("patch application")),
            (ApplyMailboxOrRebase, Some("patch application")),
        ]
    );
    assert_eq!(Busy::of(Clean), None);
    assert_eq!(Busy::of(ApplyMailboxOrRebase), Some(Busy::PatchApplication));
    assert_eq!(Busy::of(CherryPickSequence), Some(Busy::CherryPick));
}

#[test]
fn errors_keep_their_own_words() {
    let shown = Error::Git(git2::Error::from_str("object not found")).to_string();
    assert_eq!(RebaseError::from(Error::Git(git2::Error::from_str("object not found"))), RebaseError::Said { said: shown.clone() });
    assert_eq!(RebaseError::from(git2::Error::from_str("object not found")), RebaseError::Said { said: shown });
    let io = || std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access is denied");
    let shown = Error::Io(io()).to_string();
    assert_eq!(RebaseError::from(Error::Io(io())), RebaseError::Said { said: shown.clone() });
    assert_eq!(RebaseError::from(io()), RebaseError::Said { said: shown });
    for e in [Error::NotFound("commit 1a2b3c4".into()), Error::Unsupported("can't encode \"✓\"".into()), Error::Other("odd".into())] {
        let shown = e.to_string();
        let typed = RebaseError::from(e);
        assert_eq!(typed, RebaseError::Other { said: shown.clone() });
        assert_eq!(typed.to_string(), shown);
    }
}

#[test]
fn a_refusal_file_reads_back_as_written() {
    let dir = crate::paths::temp_dir().join(format!("glowfly-rebase-refusal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let prepared = dir.join("todo");
    let every = sentences().into_iter().map(|(e, _)| e).chain(formatted().into_iter().map(|(e, _)| e));
    let busy = [Busy::Merge, Busy::Rebase, Busy::CherryPick, Busy::Revert, Busy::PatchApplication, Busy::Bisect];
    for e in every.chain(busy.map(|op| RebaseError::InProgress { op })) {
        write_refusal(&prepared, &e).unwrap();
        let text = std::fs::read_to_string(dir.join("todo.error")).unwrap();
        assert_eq!(read_refusal(&text), e, "{text}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refusal_that_isnt_one_is_its_own_words() {
    let said = "The commits to rebase changed; reload the list and try again.";
    assert_eq!(read_refusal(&format!("{said}\n")), RebaseError::Said { said: said.into() });
    assert_eq!(read_refusal(""), RebaseError::Said { said: String::new() });
    assert_eq!(read_refusal("{\"Unknown\":1}"), RebaseError::Said { said: "{\"Unknown\":1}".into() });
}
