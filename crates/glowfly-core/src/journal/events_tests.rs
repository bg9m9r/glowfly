//! Reading reflog messages in every shape git writes them.

use super::events::{Event, parse};

#[test]
fn parses_every_message_shape() {
    let s = |x: &str| x.to_owned();
    let cases: Vec<(&str, Event)> = vec![
        ("commit: Fix the build", Event::Commit { subject: s("Fix the build") }),
        ("commit (initial): First", Event::Initial { subject: s("First") }),
        ("commit (amend): Fix: really", Event::Amend { subject: s("Fix: really") }),
        ("commit (merge): Merge branch 'x'", Event::MergeCommit { subject: s("Merge branch 'x'") }),
        ("checkout: moving from main to feature/login", Event::Checkout { from: s("main"), to: s("feature/login") }),
        ("reset: moving to HEAD~1", Event::Reset { to: s("HEAD~1") }),
        ("reset: moving to 1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4d5e6f1a2b", Event::Reset { to: s("1a2b3c4") }),
        ("merge feature: Fast-forward", Event::Merge { from: s("feature"), fast_forward: true }),
        ("merge origin/main: Merge made by the 'ort' strategy.", Event::Merge { from: s("origin/main"), fast_forward: false }),
        ("pull: Fast-forward", Event::Pull { rebase: false }),
        ("pull origin main: Merge made by the 'ort' strategy.", Event::Pull { rebase: false }),
        ("pull --rebase (finish): refs/heads/main onto 1a2b3c4", Event::Pull { rebase: true }),
        ("pull --rebase (pick): Change", Event::RebaseStep),
        ("rebase (start): checkout main", Event::RebaseStep),
        ("rebase (pick): Add login", Event::RebaseStep),
        ("rebase (reword): Add login", Event::RebaseStep),
        ("rebase (continue): Add login", Event::RebaseStep),
        ("rebase -i (start): checkout 1a2b3c4", Event::RebaseStep),
        ("rebase (finish): returning to refs/heads/feature", Event::RebaseFinish { onto: None }),
        (
            "rebase (finish): refs/heads/feature onto 9f8e7d6c5b4a9f8e7d6c5b4a9f8e7d6c5b4a9f8e",
            Event::RebaseFinish { onto: Some(s("9f8e7d6")) },
        ),
        ("rebase -i (finish): returning to refs/heads/feature", Event::RebaseFinish { onto: None }),
        ("rebase (abort): returning to refs/heads/feature", Event::RebaseAbort),
        ("cherry-pick: Port the fix", Event::CherryPick { subject: s("Port the fix") }),
        ("revert: Revert \"Port the fix\"", Event::Revert { subject: s("Revert \"Port the fix\"") }),
        ("branch: Created from HEAD", Event::BranchCreated { from: s("HEAD") }),
        ("branch: Created from refs/remotes/origin/x", Event::BranchCreated { from: s("origin/x") }),
        ("branch: Reset to main", Event::BranchReset { to: s("main") }),
        ("Branch: renamed refs/heads/old to refs/heads/new", Event::BranchRenamed { from: s("old") }),
        ("clone: from https://example.invalid/repo.git", Event::Clone),
        ("fetch origin: fast-forward", Event::Fetch),
        ("fetch: fast-forward", Event::Fetch),
        ("am: Apply the patch", Event::Am { subject: s("Apply the patch") }),
        ("Glowfly: restore main to 1a2b3c4", Event::Other),
        ("something else entirely", Event::Other),
        ("", Event::Other),
    ];
    for (message, want) in cases {
        assert_eq!(parse(message), want, "{message:?}");
    }
}

#[test]
fn only_rebase_starts_and_steps_are_hidden() {
    for m in ["rebase (start): checkout main", "rebase (pick): x", "rebase -i (squash): x", "rebase (edit): x"] {
        assert!(parse(m).hidden(), "{m}");
    }
    for m in ["rebase (finish): returning to refs/heads/x", "rebase (abort): returning to refs/heads/x", "commit: x", "nothing"] {
        assert!(!parse(m).hidden(), "{m}");
    }
}
