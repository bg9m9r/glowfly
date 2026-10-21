//! Reading range-diff's output: pair headers in every shape git prints, the interdiff's outer
//! and inner markers, and the malformed lines that are refused.

use super::*;
use crate::range_diff::{Pair, Side};

const CAPTURED: &str = "\
1:  6f32963 ! 1:  71ef454 one: add d
    @@ Metadata
     Author: T <t@e>
\x20\x20\x20\x20\x20
      ## Commit message ##
    -    one: add d
    +    one: add d (edited)
\x20\x20\x20\x20\x20
      ## f.txt ##
     @@
      a
      b
      c
    -+d
    ++D changed
2:  930ec2f < -:  ------- two: add g
-:  ------- > 2:  c4a1c83 four: add z
3:  8f624f6 = 3:  5a6a9e8 three: add h
-:  ------- > 4:  f272a54 four: add z
";

fn lines(d: &FileDiff) -> Vec<(char, &str)> {
    d.hunks.iter().flat_map(|h| h.lines.iter().map(|l| (l.origin, l.text.as_str()))).collect()
}

#[test]
fn the_captured_output() {
    let p = pairs(CAPTURED).unwrap();
    let statuses: Vec<Status> = p.iter().map(|p| p.status).collect();
    assert_eq!(statuses, [Status::Changed, Status::Removed, Status::Added, Status::Unchanged, Status::Added]);
    assert_eq!(p[0].old, Some((1, "6f32963".into())));
    assert_eq!(p[0].new, Some((1, "71ef454".into())));
    assert_eq!(p[0].subject, "one: add d", "the old commit's subject, as git printed it");
    assert_eq!(p[1].new, None);
    assert_eq!(p[1].old, Some((2, "930ec2f".into())));
    assert_eq!(p[2].old, None);
    assert_eq!(p[4].new, Some((4, "f272a54".into())));
    assert!(p[1..].iter().all(|p| p.interdiff.is_none()));
    let d = p[0].interdiff.as_ref().unwrap();
    assert_eq!(d.hunks.len(), 1);
    assert_eq!(d.hunks[0].header, "@@ Metadata");
    let l = lines(d);
    assert_eq!(l[0], (' ', "Author: T <t@e>"));
    assert_eq!(l[1], (' ', ""), "a 5-space line is an empty context line");
    for want in [
        (' ', " ## Commit message ##"),
        ('-', "    one: add d"),
        ('+', "    one: add d (edited)"),
        (' ', "@@"),
        ('-', "+d"),
        ('+', "+D changed"),
    ] {
        assert!(l.contains(&want), "{want:?} in {l:?}");
    }
    assert!(d.hunks[0].lines.iter().all(|l| l.old.is_none() && l.new.is_none()));
}

#[test]
fn wide_indexes_and_full_ids() {
    let out = " 1:  bbea3cc =  1:  8bd4050 c1\n10:  b955eea = 10:  d38c5cc c10\n11:  132ccea <  -:  ------- c11\n -:  ------- > 11:  0eacfbb c11\n";
    let p = pairs(out).unwrap();
    assert_eq!(p.len(), 4);
    assert_eq!((p[0].old.clone(), p[0].subject.as_str()), (Some((1, "bbea3cc".into())), "c1"));
    assert_eq!(p[1].new, Some((10, "d38c5cc".into())));
    assert_eq!((p[2].status, p[2].new.clone()), (Status::Removed, None));
    assert_eq!((p[3].status, p[3].new.clone()), (Status::Added, Some((11, "0eacfbb".into()))));
    let (a, b) = ("a".repeat(40), "b".repeat(40));
    let dashes = "-".repeat(40);
    let full = format!("1:  {a} ! 1:  {b} fix:  two  spaces: and colons\n2:  {b} < -:  {dashes} \n-:  {dashes} > 2:  {a} \n");
    let p = pairs(&full).unwrap();
    assert_eq!(p[0].old, Some((1, a.clone())));
    assert_eq!(p[0].subject, "fix:  two  spaces: and colons");
    assert_eq!(p[1].subject, "", "an empty subject");
    assert_eq!((p[2].new.clone(), p[2].subject.as_str()), (Some((2, a.clone())), ""));
    let bare = format!("1:  {a} = 1:  {b}\n");
    assert_eq!(pairs(&bare).unwrap()[0].subject, "", "no space after the hex either");
}

#[test]
fn empty_and_old_style_output() {
    assert!(pairs("").unwrap().is_empty());
    let old = "1:  6f32963 ! 1:  71ef454 one\n    @@ -1,3 +1,3 @@\n     x\n    -a\n    +b\n    @@ f.txt: ctx\n    +c\n";
    let p = pairs(old).unwrap();
    let d = p[0].interdiff.as_ref().unwrap();
    let headers: Vec<&str> = d.hunks.iter().map(|h| h.header.as_str()).collect();
    assert_eq!(headers, ["@@ -1,3 +1,3 @@", "@@ f.txt: ctx"]);
    assert_eq!(lines(d), [(' ', "x"), ('-', "a"), ('+', "b"), ('+', "c")]);
    let implicit = pairs("1:  6f32963 ! 1:  71ef454 one\n    +x\n    \n").unwrap();
    let d = implicit[0].interdiff.as_ref().unwrap();
    assert_eq!(d.hunks[0].header, "", "lines before any @@ go in a hunk without a header");
    assert_eq!(lines(d), [('+', "x"), (' ', "")], "a bare 4-space line is empty context");
    let bang = pairs("1:  6f32963 ! 1:  71ef454 one\n2:  8f624f6 = 2:  5a6a9e8 two\n").unwrap();
    assert_eq!(bang[0].interdiff, None, "a '!' with no body has no interdiff");
}

#[test]
fn malformed_lines_are_errors_naming_the_line() {
    for bad in [
        "    @@ Metadata\n",
        "garbled header\n",
        "1:  6f32963 < 2:  71ef454 subject\n",
        "-:  ------- > -:  ------- subject\n",
        "1:  6f32963 > 1:  71ef454 subject\n",
        "1:  6f32963 = -:  ------- subject\n",
        "1:  6f32963 ? 1:  71ef454 subject\n",
        "1:  xyz1234 = 1:  71ef454 subject\n",
    ] {
        let e = pairs(bad).unwrap_err().to_string();
        assert!(e.starts_with("Couldn't read git range-diff's output: "), "{e}");
        assert!(e.contains(&printable(bad)), "{e} names {bad:?}");
    }
}

#[test]
fn counts_take_only_outer_changes() {
    let p = pairs(CAPTURED).unwrap();
    let side = |s: &Option<(u32, String)>| s.as_ref().map(|(n, _)| Side { index: *n, oid: git2::Oid::zero(), summary: String::new() });
    let pair = |r: &RawPair| Pair { status: r.status, old: side(&r.old), new: side(&r.new), interdiff: r.interdiff.clone() };
    let c = pair(&p[0]).counts();
    assert_eq!((c.added, c.removed), (2, 2), "the outer +/- lines, not the inner '+d'");
    assert_eq!(pair(&p[1]).counts(), Default::default());
}
