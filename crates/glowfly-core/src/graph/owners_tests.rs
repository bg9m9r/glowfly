use super::*;
use crate::graph::layout;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

/// Commit `n` with parents `parents` and summary `summary`.
fn c(n: u8, parents: &[u8], summary: &str) -> CommitInfo {
    CommitInfo {
        oid: oid(n),
        summary: summary.into(),
        body: Default::default(),
        author: crate::repo::Author::new("", ""),
        time: 0,
        parents: parents.iter().map(|&p| oid(p)).collect(),
    }
}

fn names<'a>(tips: &[(u8, &[&str])], mainline: Option<&str>) -> LaneNames<'a> {
    LaneNames {
        tips: tips.iter().map(|(n, keys)| (oid(*n), keys.iter().map(|k| (*k).to_owned()).collect())).collect(),
        mainline: mainline.map(str::to_owned),
        remotes: vec!["origin"],
        ..LaneNames::default()
    }
}

fn owners_of(commits: &[CommitInfo], names: &LaneNames) -> Owners {
    assign(&layout(commits, names.head, false, &[]), commits, names)
}

/// The owner of each commit row, as keys ("?" for an unnamed line).
fn row_keys(o: &Owners) -> Vec<&str> {
    o.commit.iter().map(|&i| o.key(i).unwrap_or("?")).collect()
}

fn named(o: &Owners, key: &str) -> u32 {
    o.owner_of_key(key).unwrap_or_else(|| panic!("no owner {key}: {:?}", o.list))
}

#[test]
fn main_chain_is_main() {
    let commits = [c(1, &[2], ""), c(2, &[3], ""), c(3, &[], "")];
    let o = owners_of(&commits, &names(&[(1, &["main"])], Some("main")));
    assert_eq!(row_keys(&o), ["main"; 3]);
    assert_eq!(o.mainline.as_deref(), Some("main"));
}

#[test]
fn feature_on_top_of_main() {
    let commits = [c(1, &[2], ""), c(2, &[3], ""), c(3, &[], "")];
    let o = owners_of(&commits, &names(&[(1, &["feature"]), (2, &["main"])], Some("main")));
    assert_eq!(row_keys(&o), ["feature", "main", "main"]);
    assert_eq!(o.lane_before(0, RowPos::commit(1)), Some(named(&o, "feature")));
    assert_eq!(o.lane_after(0, RowPos::commit(1)), Some(named(&o, "main")));
    assert_eq!(o.lane_before(0, RowPos::commit(0)), None, "nothing above the first tip");
}

#[test]
fn fork_point_belongs_to_main() {
    // feature's tip (lane 0) is newer than main's (lane 1); both come from 3.
    let commits = [c(1, &[3], ""), c(2, &[3], ""), c(3, &[], "")];
    let o = owners_of(&commits, &names(&[(1, &["feature"]), (2, &["main"])], Some("main")));
    assert_eq!(row_keys(&o), ["feature", "main", "main"]);
    assert_eq!(o.lane_before(0, RowPos::commit(2)), Some(named(&o, "feature")), "feature's line runs into the fork");
    assert_eq!(o.lane_after(0, RowPos::commit(2)), None, "the root ends every line");
}

#[test]
fn label_inside_main_history_does_not_recolour() {
    let commits = [c(1, &[2], ""), c(2, &[3], ""), c(3, &[], "")];
    let o = owners_of(&commits, &names(&[(1, &["main"]), (2, &["old"])], Some("main")));
    assert_eq!(row_keys(&o), ["main"; 3]);
    let o = owners_of(&commits, &names(&[(1, &["feature"]), (2, &["old"])], None));
    assert_eq!(row_keys(&o), ["feature"; 3], "nor another branch's line");
}

#[test]
fn merged_lane_named_from_message() {
    // 1 merges topic (2) into main; the topic branch itself is gone.
    let commits = [c(1, &[3, 2], "Merge branch 'topic'"), c(2, &[3], ""), c(3, &[], "")];
    let o = owners_of(&commits, &names(&[(1, &["main"])], Some("main")));
    assert_eq!(row_keys(&o), ["main", "topic", "main"]);
    assert_eq!(o.lane_after(1, RowPos::commit(0)), Some(named(&o, "topic")), "the second parent's lane");
}

#[test]
fn unnamed_merged_lane() {
    let commits = [c(1, &[3, 2], "Tidy up"), c(2, &[3], ""), c(3, &[], "")];
    let o = owners_of(&commits, &names(&[(1, &["main"])], Some("main")));
    assert_eq!(o.list[o.commit[1] as usize], Owner::Unnamed(oid(2)));
}

#[test]
fn wip_row_takes_head_branch() {
    let commits = [c(1, &[2], ""), c(2, &[], "")];
    let mut n = names(&[(1, &["feature"])], None);
    n.head_branch = Some("feature");
    n.head = Some(oid(1));
    let l = layout(&commits, n.head, true, &[]);
    let o = assign(&l, &commits, &n);
    assert_eq!(o.wip, Some(named(&o, "feature")));
    assert_eq!(o.commit[0], named(&o, "feature"));
    assert_eq!(o.lane_after(0, RowPos::wip()), Some(named(&o, "feature")));
}

#[test]
fn detached_wip_row_is_unnamed() {
    let commits = [c(1, &[2], ""), c(2, &[], "")];
    let mut n = names(&[], None);
    n.head = Some(oid(1));
    let l = layout(&commits, n.head, true, &[]);
    let o = assign(&l, &commits, &n);
    assert_eq!(o.wip.map(|w| o.list[w as usize].clone()), Some(Owner::Unnamed(oid(1))));
}

#[test]
fn worktree_wip_row_takes_its_branch() {
    let commits = [c(1, &[2], ""), c(2, &[], "")];
    let mut n = names(&[(1, &["main"])], Some("main"));
    n.worktree_branches = vec![Some("wt".to_owned()), None];
    let l = layout(&commits, Some(oid(1)), false, &[(0, oid(2)), (1, oid(2))]);
    let o = assign(&l, &commits, &n);
    assert_eq!(o.worktree_wips.len(), 2);
    assert_eq!(o.worktree_wips[0], named(&o, "wt"));
    assert_eq!(o.list[o.worktree_wips[1] as usize], Owner::Unnamed(oid(2)), "a detached worktree");
    assert_eq!(o.lane_after(1, RowPos::worktree(1)), Some(named(&o, "wt")));
    assert_eq!(row_keys(&o), ["main", "main"]);
}

#[test]
fn local_and_remote_share_an_owner() {
    // feature (1) and origin/feature (2) have diverged from 3.
    let commits = [c(1, &[3], ""), c(2, &[3], ""), c(3, &[], "")];
    let o = owners_of(&commits, &names(&[(1, &["feature"]), (2, &["feature"])], None));
    assert_eq!(o.commit[0], o.commit[1]);
    assert_eq!(o.list.iter().filter(|x| **x == Owner::Branch("feature".to_owned())).count(), 1);
    assert!(o.neighbours.is_empty(), "an owner is never its own neighbour: {:?}", o.neighbours);
}

#[test]
fn neighbours_recorded_for_side_by_side_lanes() {
    // a, b and c in lanes 0, 1 and 2 from one root.
    let commits = [c(1, &[4], ""), c(2, &[4], ""), c(3, &[4], ""), c(4, &[], "")];
    let o = owners_of(&commits, &names(&[(1, &["a"]), (2, &["b"]), (3, &["c"])], None));
    let (a, b, cc) = (named(&o, "a"), named(&o, "b"), named(&o, "c"));
    assert!(o.neighbours.contains(&(a.min(b), a.max(b))));
    assert!(o.neighbours.contains(&(b.min(cc), b.max(cc))));
    assert!(!o.neighbours.contains(&(a.min(cc), a.max(cc))), "lanes two apart: {:?}", o.neighbours);
}

#[test]
fn an_empty_history_has_no_owners() {
    let o = owners_of(&[], &names(&[], Some("main")));
    assert!(o.list.is_empty() && o.commit.is_empty() && o.neighbours.is_empty());
}

#[test]
fn owners_extended_match_owners_assigned_at_once() {
    // main merges feature (named by its message) and an octopus of two unnamed lines; a second
    // root comes in; worktrees sit at rows 0, 3 and 6, one of them on no branch.
    let commits = [
        c(1, &[2, 5], "Merge branch 'feature'"),
        c(2, &[3, 7, 8], "Merge commits"),
        c(5, &[6], ""),
        c(3, &[4], ""),
        c(7, &[4], ""),
        c(8, &[9], ""),
        c(6, &[4], ""),
        c(4, &[10, 11], "Merge branch 'docs'"),
        c(9, &[10], ""),
        c(11, &[], ""),
        c(10, &[], ""),
    ];
    let mut n = names(&[(1, &["main"]), (5, &["topic"]), (9, &["old"])], Some("main"));
    n.head_branch = Some("main");
    n.head = Some(oid(1));
    n.worktree_branches = vec![Some("wt".into()), None, Some("topic".into())];
    let worktrees = [(0, oid(1)), (1, oid(3)), (2, oid(6))];
    let lay = crate::graph::lanes::layout(&commits, Some(oid(1)), true, &worktrees);
    let whole = assign(&lay, &commits, &n);
    assert!(whole.neighbours.len() > 2 && whole.worktree_wips.len() == 3, "{whole:?}");
    for s in 0..=commits.len() {
        for t in s..=commits.len() {
            let mut parts = Owners::begin(&lay, &n);
            // The layout only as far as the rows replayed so far, as a page would have it.
            let mut part = crate::graph::lanes::start(Some(oid(1)), true, &worktrees);
            part.extend(&commits[..s], 0);
            parts.extend(&part, &commits[..s], 0, &n);
            part.extend(&commits[s..t], s);
            parts.extend(&part, &commits[..t], s, &n);
            part.extend(&commits[t..], t);
            parts.extend(&part, &commits, t, &n);
            assert_eq!(parts, whole, "split at {s} and {t}");
        }
    }
}
