use super::*;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn c(n: u8, parents: &[u8]) -> CommitInfo {
    CommitInfo {
        oid: oid(n),
        summary: "".into(),
        body: "".into(),
        author: crate::repo::Author::new("", ""),
        time: 0,
        parents: parents.iter().map(|&p| oid(p)).collect(),
    }
}

/// Histories newest first: a chain, a branch merged back, an octopus over three lines, and a
/// second root merged in.
fn histories() -> Vec<Vec<CommitInfo>> {
    vec![
        vec![c(1, &[2]), c(2, &[3]), c(3, &[])],
        vec![c(1, &[2, 5]), c(2, &[3]), c(5, &[6]), c(3, &[4]), c(6, &[4]), c(4, &[7]), c(7, &[])],
        vec![c(1, &[2, 3, 4]), c(9, &[2]), c(2, &[5]), c(3, &[5]), c(4, &[6]), c(5, &[6, 8]), c(6, &[7]), c(8, &[]), c(7, &[])],
    ]
}

#[test]
fn extending_a_layout_matches_laying_it_out_at_once() {
    for commits in histories() {
        let n = commits.len();
        // Worktrees at rows 0, 2 and 5 (when the history is that long), and one never reached.
        let mut worktrees: Vec<(usize, Oid)> =
            [0, 2, 5].iter().filter(|&&r| r < n).enumerate().map(|(k, &r)| (k, commits[r].oid)).collect();
        worktrees.push((7, oid(200)));
        for (head, wip) in [(Some(commits[0].oid), true), (Some(commits[0].oid), false), (None, true)] {
            let whole = layout(&commits, head, wip, &worktrees);
            for s in 0..=n {
                let mut parts = start(head, wip, &worktrees);
                parts.extend(&commits[..s], 0);
                parts.extend(&commits[s..], s);
                assert_eq!(parts, whole, "split at {s} of {n}, wip {wip}");
            }
            assert!(!whole.tail.waiting.is_empty(), "the worktree outside the history still waits");
        }
    }
}
