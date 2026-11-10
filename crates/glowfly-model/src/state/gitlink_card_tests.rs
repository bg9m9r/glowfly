//! Tests for the gitlink card's text: each kind of move's title and lists, the counts in the
//! singular and plural, the line for commits not listed (and past the count's cap), the
//! warning for changes inside, why commits can't be listed, and German throughout.

use super::*;
use git2::Oid;
use glowfly_core::repo::GITLINK_LIST_MAX;

const EN: Lang = Lang::En;

fn oid(c: char) -> Oid {
    Oid::from_str(&c.to_string().repeat(40)).unwrap()
}

/// `n` listed commits (subjects c1…cn) of `total`.
fn run(n: usize, total: usize, capped: bool) -> CommitRun {
    let commits = (1..=n).map(|i| (Oid::from_str(&format!("{i:040x}")).unwrap(), format!("c{i}"))).collect();
    CommitRun { commits, total, capped }
}

fn moved(kind: MoveKind) -> GitlinkMove {
    GitlinkMove {
        path: "libs/lib".into(),
        old: Some(oid('a')),
        new: Some(oid('b')),
        kind,
        ahead: CommitRun::default(),
        behind: CommitRun::default(),
        dirty: false,
        type_change: false,
    }
}

fn forward(n: usize) -> GitlinkMove {
    GitlinkMove { ahead: run(n, n, false), ..moved(MoveKind::Forward) }
}

#[test]
fn a_forward_move_lists_its_commits() {
    let c = text(EN, &forward(2));
    assert_eq!(c.mark, Some(Mark::Forward));
    assert_eq!(c.title, "Submodule moved forward 2 commits");
    assert_eq!(c.ids, Some(("aaaaaaa".into(), "bbbbbbb".into(), "a".repeat(40), "b".repeat(40))));
    let g = &c.groups[0];
    assert_eq!((g.heading.as_str(), g.more.as_deref()), ("Commits it moves over", None));
    assert_eq!(g.rows[0], ("0000000".into(), "c1".into(), format!("{:040x}", 1)));
    assert_eq!(g.rows.len(), 2);
    assert!(c.explain.starts_with("A submodule is a repository of its own."), "{}", c.explain);
    assert_eq!((c.warning, c.reason, c.base), (None, None, None));
    assert_eq!(text(EN, &forward(1)).title, "Submodule moved forward 1 commit");
}

#[test]
fn a_backward_move() {
    let c = text(EN, &GitlinkMove { behind: run(3, 3, false), ..moved(MoveKind::Backward) });
    assert_eq!(c.mark, Some(Mark::Backward));
    assert_eq!(c.title, "Submodule moved back 3 commits");
    assert_eq!(c.groups.len(), 1);
    assert_eq!(c.groups[0].heading, "Commits it goes back over");
}

#[test]
fn sideways_moves_with_and_without_a_base() {
    let base = Some((oid('c'), "start here".to_owned()));
    let m = GitlinkMove { ahead: run(1, 1, false), behind: run(2, 2, false), ..moved(MoveKind::Sideways { base }) };
    let c = text(EN, &m);
    assert_eq!(c.mark, Some(Mark::Fork));
    assert_eq!(c.title, "Submodule moved to another line of history: 1 commit added, 2 left out");
    let headings: Vec<&str> = c.groups.iter().map(|g| g.heading.as_str()).collect();
    assert_eq!(headings, ["Only on the new commit", "Only on the old commit"]);
    assert_eq!(c.base.as_deref(), Some("Both start from ccccccc start here"));
    let unrelated = text(EN, &GitlinkMove { kind: MoveKind::Sideways { base: None }, ..m });
    assert_eq!(unrelated.title, "Submodule moved to a commit with unrelated history");
    assert_eq!(unrelated.base, None);
}

#[test]
fn added_removed_and_the_same_commit() {
    let added = text(EN, &GitlinkMove { old: None, ..moved(MoveKind::Added) });
    assert_eq!((added.title.as_str(), added.ids.is_none()), ("Submodule added at bbbbbbb", true));
    let removed = text(EN, &GitlinkMove { new: None, ..moved(MoveKind::Removed) });
    assert_eq!(removed.title, "Submodule removed (was at aaaaaaa)");
    let same = text(EN, &GitlinkMove { new: Some(oid('a')), dirty: true, ..moved(MoveKind::Same) });
    assert_eq!(same.title, "Submodule still at aaaaaaa, with changes inside");
    assert!(same.ids.is_none(), "one commit: no move to show");
    assert!(same.warning.as_deref().is_some_and(|w| w.contains("uncommitted changes inside")));
}

#[test]
fn long_lists_say_what_they_leave_out() {
    let c = text(EN, &GitlinkMove { ahead: run(GITLINK_LIST_MAX, 60, false), ..moved(MoveKind::Forward) });
    assert_eq!(c.title, "Submodule moved forward 60 commits");
    assert_eq!(c.groups[0].more.as_deref(), Some("10 earlier commits not shown"));
    let one = text(EN, &GitlinkMove { ahead: run(GITLINK_LIST_MAX, 51, false), ..moved(MoveKind::Forward) });
    assert_eq!(one.groups[0].more.as_deref(), Some("1 earlier commit not shown"));
    let capped = text(EN, &GitlinkMove { ahead: run(GITLINK_LIST_MAX, GITLINK_COUNT_MAX, true), ..moved(MoveKind::Forward) });
    assert_eq!(capped.title, "Submodule moved forward more than 10000 commits");
    assert_eq!(capped.groups[0].more.as_deref(), Some("More than 9950 earlier commits not shown"));
}

#[test]
fn unknown_moves_say_why() {
    let reason = |u| text(EN, &moved(MoveKind::Unknown(u))).reason.unwrap();
    assert_eq!(text(EN, &moved(MoveKind::Unknown(GitlinkUnknown::Shallow))).title, "Submodule moved from aaaaaaa to bbbbbbb");
    assert!(reason(GitlinkUnknown::NotCheckedOut).contains("isn't checked out here"));
    assert!(reason(GitlinkUnknown::Refused).contains("Glowfly doesn't follow"));
    assert_eq!(
        reason(GitlinkUnknown::Missing { old: false, new: true }),
        "The submodule's repository doesn't have bbbbbbb. Fetch in the submodule to list the commits."
    );
    assert!(reason(GitlinkUnknown::Missing { old: true, new: false }).contains("doesn't have aaaaaaa."));
    assert!(reason(GitlinkUnknown::Missing { old: true, new: true }).contains("neither aaaaaaa nor bbbbbbb"));
    assert!(reason(GitlinkUnknown::Shallow).contains("shallow"));
}

#[test]
fn german_has_every_text() {
    let base = Some((oid('c'), "s".to_owned()));
    let kinds = [
        forward(2),
        GitlinkMove { ahead: run(GITLINK_LIST_MAX, GITLINK_COUNT_MAX, true), ..moved(MoveKind::Forward) },
        GitlinkMove { behind: run(GITLINK_LIST_MAX, 70, false), ..moved(MoveKind::Backward) },
        GitlinkMove { ahead: run(1, 1, false), behind: run(1, 1, false), ..moved(MoveKind::Sideways { base }) },
        moved(MoveKind::Sideways { base: None }),
        GitlinkMove { old: None, ..moved(MoveKind::Added) },
        GitlinkMove { new: None, ..moved(MoveKind::Removed) },
        GitlinkMove { dirty: true, ..moved(MoveKind::Same) },
        moved(MoveKind::Unknown(GitlinkUnknown::NotCheckedOut)),
        moved(MoveKind::Unknown(GitlinkUnknown::Refused)),
        moved(MoveKind::Unknown(GitlinkUnknown::Missing { old: true, new: true })),
        moved(MoveKind::Unknown(GitlinkUnknown::Missing { old: false, new: true })),
        moved(MoveKind::Unknown(GitlinkUnknown::Shallow)),
    ];
    for m in &kinds {
        let (en, de) = (text(EN, m), text(Lang::De, m));
        assert_ne!(en.title, de.title, "{:?}", m.kind);
        assert_ne!(en.explain, de.explain);
        assert!(!m.dirty || en.warning != de.warning, "{:?}", m.kind);
        assert!(!matches!(m.kind, MoveKind::Unknown(_)) || en.reason != de.reason, "{:?}", m.kind);
        for (e, d) in en.groups.iter().zip(&de.groups) {
            assert_ne!(e.heading, d.heading);
            assert!(e.more.is_none() || e.more != d.more, "{:?}", m.kind);
        }
        assert!(en.base.is_none() || en.base != de.base);
    }
}
