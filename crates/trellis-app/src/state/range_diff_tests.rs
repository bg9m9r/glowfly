//! The Range-diff view's words: short ref names, the counts, the cut, the refusals and the
//! selection on load.

use super::*;
use trellis_core::range_diff::Range;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn pair(status: Status) -> Pair {
    Pair { status, old: None, new: None, interdiff: None }
}

fn compared(statuses: &[Status], mainline: Option<&str>) -> Compared {
    Compared {
        old: Range { base: oid(1), tip: oid(2) },
        new: Range { base: oid(3), tip: oid(4) },
        mainline: mainline.map(str::to_owned),
        rewrite: None,
        pairs: statuses.iter().map(|s| pair(*s)).collect(),
    }
}

#[test]
fn short_names_of_refs() {
    assert_eq!(short_ref("refs/heads/feature/x"), "feature/x");
    assert_eq!(short_ref("refs/remotes/origin/main"), "origin/main");
    assert_eq!(short_ref("HEAD"), "HEAD");
}

#[test]
fn counts_cut_and_first_changed() {
    let t = Lang::En;
    let c = compared(&[Status::Unchanged, Status::Removed, Status::Changed, Status::Added, Status::Changed], Some("main"));
    assert_eq!(counts_line(t, &c), "Changed 2 · Dropped 1 · New 1 · Same 1");
    assert_eq!(base_line(t, &c), "Each side's own commits since main");
    assert_eq!(first_changed(&c), Some(2));
    let split = compared(&[Status::Unchanged], None);
    assert_eq!(base_line(t, &split), format!("Commits since the two versions split at {}", short(oid(1))));
    assert_eq!(first_changed(&split), None);
    assert_eq!(status_label(t, Status::Removed), "Dropped");
}

#[test]
fn refusals_as_sentences() {
    let t = Lang::En;
    let no = outcome_failure(t, &Outcome::NoRewrite { refname: "refs/heads/feature".into() }).unwrap();
    assert_eq!(no, "There is no earlier version of feature to compare with: its reflog shows no rebase, amend, reset or force-push.");
    assert_eq!(outcome_failure(t, &Outcome::NoSharedHistory).unwrap(), "The two versions share no history.");
    let many = outcome_failure(t, &Outcome::TooMany { old: 301, new: 12 }).unwrap();
    assert!(many.contains("at most 300") && many.contains("300+") && many.contains("12"), "{many}");
    assert_eq!(outcome_failure(t, &Outcome::Compared(compared(&[], None))), None);
}

#[test]
fn the_subtitle_names_both_ends() {
    let t = Lang::En;
    let settings = Settings::default();
    let c = compared(&[], None);
    let tips = Origin { ends: Ends::Tips { old: oid(2), new: oid(4) }, label: None, at: None };
    assert_eq!(subtitle(t, &tips, &c, &settings, 0, 0), format!("{} and {}", short(oid(2)), short(oid(4))));
    let state = Origin { label: Some("feature".into()), at: Some(0), ..tips };
    let s = subtitle(t, &state, &c, &settings, 0, 0);
    assert!(s.starts_with(&format!("feature at {} (", short(oid(2)))) && s.ends_with(&format!("and now at {}", short(oid(4)))), "{s}");
    let rewrite =
        trellis_core::range_diff::Rewrite { before: oid(2), at: 0, message: "rebase (finish): refs/heads/feature onto 1234".into() };
    let c = Compared { rewrite: Some(rewrite), ..c };
    let origin = Origin { ends: Ends::LastRewrite { refname: "refs/heads/feature".into() }, label: Some("feature".into()), at: None };
    let s = subtitle(t, &origin, &c, &settings, 0, 0);
    assert!(s.starts_with(&format!("feature: before its last rewrite ({}, ", short(oid(2)))), "{s}");
    assert!(s.contains(&format!("and now ({}) · ", short(oid(4)))), "the reflog event follows: {s}");
}
