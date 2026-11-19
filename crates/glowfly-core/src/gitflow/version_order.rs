//! The order of release and hotfix branches: by the version in the name (digit runs as numbers,
//! a suffixed version before the bare one), not by the name's text.

use super::{FlowKind, GitFlowSettings, short_name};

/// One piece of a name. The variant order is the rank at the place two names first differ:
/// a word (a suffix such as "-rc.") < the name ending there < a "." before a further part < a
/// number.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Part<'a> {
    Text(&'a str),
    End,
    Dot,
    /// A digit run without its leading zeros: its length, then its digits. Never parsed.
    Num(usize, &'a str),
}

/// What a release or hotfix short name sorts by.
///
/// Names without any ASCII digit all compare equal (they keep snapshot order among themselves)
/// and sit before every name with a digit: the highest version is then always the last row of a
/// group, and it matches the rule that text ranks before numbers. Because `Part::End` closes
/// every key and occurs only last, no key is a prefix of another, so the derived lexicographic
/// `Ord` is a total order (a pairwise compare with a "suffix comes first" special case can
/// loop: 1.2.0rc1 < 1.2.0 < 1.2.0.1 < 1.2.0rc1).
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct VersionKey<'a> {
    /// false for a name without a digit.
    numbered: bool,
    parts: Vec<Part<'a>>,
}

/// The sort key of a short name: one leading `v` or `V` directly before a digit is ignored, then
/// the name is cut into digit runs and runs of anything else. Digits are ASCII only.
pub(super) fn version_key(short: &str) -> VersionKey<'_> {
    let body = match short.as_bytes() {
        [b'v' | b'V', d, ..] if d.is_ascii_digit() => &short[1..],
        _ => short,
    };
    if !body.bytes().any(|c| c.is_ascii_digit()) {
        return VersionKey { numbered: false, parts: Vec::new() };
    }
    let mut parts = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 {
            // The cut is at an ASCII byte or the end, so always on a char boundary.
            let (run, tail) = rest.split_at(digits);
            let n = run.trim_start_matches('0');
            parts.push(Part::Num(n.len(), n));
            rest = tail;
        } else {
            let other = rest.bytes().take_while(|c| !c.is_ascii_digit()).count();
            let (text, tail) = rest.split_at(other);
            parts.push(if text == "." { Part::Dot } else { Part::Text(text) });
            rest = tail;
        }
    }
    parts.push(Part::End);
    VersionKey { numbered: true, parts }
}

/// Puts each versioned kind's branches in version order within the places that kind holds;
/// features, and which place holds which kind, stay.
pub(super) fn order_versioned<'b>(s: &GitFlowSettings, list: &mut [(FlowKind, &'b str)]) {
    for kind in FlowKind::ALL.into_iter().filter(|k| k.versioned()) {
        let at: Vec<usize> = (0..list.len()).filter(|i| list[*i].0 == kind).collect();
        let mut names: Vec<&'b str> = at.iter().map(|i| list[*i].1).collect();
        names.sort_by_cached_key(|n| version_key(short_name(s, kind, n)));
        for (i, name) in at.into_iter().zip(names) {
            list[i].1 = name;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitflow::finish_tests::defaults;

    fn sorted<'a>(names: &[&'a str]) -> Vec<&'a str> {
        let mut v = names.to_vec();
        v.sort_by_cached_key(|n| version_key(n));
        v
    }

    fn lt(a: &str, b: &str) {
        assert!(version_key(a) < version_key(b), "{a} should sort before {b}");
    }

    #[test]
    fn digit_runs_compare_as_numbers() {
        assert_eq!(sorted(&["1.10.1", "1.10.0", "1.9.0", "2", "10"]), ["1.9.0", "1.10.0", "1.10.1", "2", "10"]);
    }

    #[test]
    fn leading_zeros_are_ignored() {
        assert_eq!(version_key("1.02"), version_key("1.2"));
        lt("1.002", "1.10");
        assert_eq!(version_key("0"), version_key("000"));
        lt("0", "1");
    }

    #[test]
    fn a_leading_v_is_ignored() {
        assert_eq!(version_key("v1.2.0"), version_key("1.2.0"));
        assert_eq!(version_key("V1.2.0"), version_key("1.2.0"));
        lt("v1.9.0", "1.10.0");
        assert_eq!(version_key("vnext"), version_key("next"));
        assert_ne!(version_key("version2"), version_key("ersion2"));
    }

    #[test]
    fn a_suffix_comes_before_the_bare_version() {
        lt("1.2.0-rc.1", "1.2.0");
        lt("1.2.0rc1", "1.2.0");
        lt("1.2.0.rc1", "1.2.0");
        lt("1.2.0", "1.2.0.1");
        lt("1.2", "1.2.0");
        lt("1.2.0-rc.1", "1.2.0.1");
        lt("1.2.0-rc.2", "1.2.0-rc.10");
        lt("1.2.0-beta.1", "1.2.0-rc.1");
        lt("1.2.0-rc", "1.2.0-rc.1");
        lt("1.2.0", "1.2.1-rc.1");
    }

    #[test]
    fn names_without_digits_are_equal_and_first() {
        assert_eq!(version_key("next"), version_key("candidate"));
        lt("next", "0.0.1");
        lt("next", "rc-1");
        assert_eq!(sorted(&["zeta", "1.0", "alpha"]), ["zeta", "alpha", "1.0"]);
    }

    #[test]
    fn text_sorts_before_numbers_and_as_text() {
        lt("rc-2", "rc-10");
        lt("rc-10", "1.0");
        lt("1.0-a", "1.0-b");
        lt("1.0-B", "1.0-a");
    }

    #[test]
    fn very_long_digit_runs_do_not_overflow() {
        lt("18446744073709551615", "18446744073709551616");
        lt(&"9".repeat(60), &format!("1{}", "0".repeat(60)));
        lt(&format!("1{}", "0".repeat(60)), &format!("1{}1", "0".repeat(59)));
        assert_eq!(version_key(&format!("{}1", "0".repeat(100))), version_key("1"));
        let _ = version_key(&"7".repeat(5000));
    }

    #[test]
    fn non_ascii_text_is_text() {
        let _ = version_key("1.0-\u{fc}");
        let _ = version_key("\u{fc}1");
        assert!(!version_key("\u{ff11}\u{ff12}").numbered);
        assert!(!version_key("").numbered);
    }

    #[test]
    fn the_order_does_not_depend_on_the_input_order() {
        let names = ["rc-2", "1.2", "1.2.0-rc.1", "1.2.0rc1", "1.2.0", "1.2.0.1", "1.10.0", "v2.0.0", "10"];
        let expected = sorted(&names);
        let mut reversed = names;
        reversed.reverse();
        assert_eq!(sorted(&reversed), expected);
        for by in 1..names.len() {
            let mut rotated = names;
            rotated.rotate_left(by);
            assert_eq!(sorted(&rotated), expected);
        }
        let pos = |n: &str| expected.iter().position(|e| *e == n).unwrap();
        assert!(pos("1.2.0rc1") < pos("1.2.0") && pos("1.2.0") < pos("1.2.0.1"));
    }

    #[test]
    fn order_versioned_keeps_each_kinds_places() {
        use FlowKind::{Feature, Hotfix, Release};
        let mut list = [
            (Feature, "feature/b"),
            (Hotfix, "hotfix/1.10.0"),
            (Feature, "feature/a"),
            (Hotfix, "hotfix/1.9.0"),
            (Release, "release/2.0"),
            (Release, "release/1.0"),
        ];
        order_versioned(&defaults(), &mut list);
        assert_eq!(
            list,
            [
                (Feature, "feature/b"),
                (Hotfix, "hotfix/1.9.0"),
                (Feature, "feature/a"),
                (Hotfix, "hotfix/1.10.0"),
                (Release, "release/1.0"),
                (Release, "release/2.0"),
            ]
        );
        let s = GitFlowSettings { release: "rel-".into(), ..defaults() };
        let mut list = [(Release, "rel-10"), (Release, "rel-9")];
        order_versioned(&s, &mut list);
        assert_eq!(list, [(Release, "rel-9"), (Release, "rel-10")]);
    }
}
