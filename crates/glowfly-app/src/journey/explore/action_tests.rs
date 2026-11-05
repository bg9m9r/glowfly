//! Tests of the case text.

use super::*;

fn every_variant(s: &str) -> Vec<Action> {
    use Action::*;
    let s = || s.to_owned();
    vec![
        SelectWip,
        Stage(s()),
        Unstage(s()),
        StageAll,
        UnstageAll,
        StageHunk(s()),
        Commit(s()),
        Amend,
        CreateBranch(s()),
        Checkout(s()),
        DeleteBranch(s()),
        Merge(s()),
        Rebase(s()),
        Stash,
        Unstash,
        DropStash,
        Undo,
        Redo,
        Discard(s()),
        ResolveOurs(s()),
        ResolveTheirs(s()),
        Continue,
        Abort,
        Refresh,
        SwitchTab(s()),
        CloseTab(s()),
        Confirm(s()),
        Outside(self::Outside::Write { path: s(), lines: 3 }),
        Outside(self::Outside::Append { path: s(), lines: 0 }),
        Outside(self::Outside::Create { path: s(), lines: 255 }),
        Outside(self::Outside::Delete { path: s() }),
        Outside(self::Outside::GitCommit(s())),
        Outside(self::Outside::GitCheckout(s())),
        Outside(self::Outside::GitBranch(s())),
    ]
}

#[test]
fn round_trips() {
    for s in ["a.txt", "msg 7", "say \"hi\"", "back\\slash\\", "tab\t and ünïcode ✓", "", "\\\"", "+3"] {
        for a in every_variant(s) {
            assert_eq!(parse(&a.to_string()), Ok(a.clone()), "{a}");
        }
    }
}

#[test]
fn reads_the_documented_lines() {
    assert_eq!(parse("select-wip"), Ok(Action::SelectWip));
    assert_eq!(parse("commit \"msg 7\""), Ok(Action::Commit("msg 7".into())));
    assert_eq!(parse("outside append \"src/a.txt\" +3"), Ok(Action::Outside(Outside::Append { path: "src/a.txt".into(), lines: 3 })));
    assert_eq!(Action::Confirm("Stash and check out".into()).to_string(), "confirm \"Stash and check out\"");
}

#[test]
fn rejects_unknown() {
    let err = parse("fly away").unwrap_err();
    assert!(err.contains("fly away"), "{err}");
    for bad in ["", "stage", "stage a.txt", "commit \"x", "undo \"x\"", "outside write \"p\"", "outside nope \"p\"", "stage \"a\\n\""] {
        assert!(parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn text_has_no_root() {
    for a in every_variant("src/a.txt") {
        let t = a.to_string();
        assert!(!t.contains(":\\") && !t.starts_with('/'), "{t}");
    }
}
