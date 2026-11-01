//! No two text snapshots alike: a text snapshot that is byte for byte another one's checks
//! nothing the other doesn't (a theme variant, whose look only its golden image judges, or a
//! hover or resize the text can't see). `IDENTICAL` lists the groups that are still alike; it
//! only shrinks. Make a new test assert what it is named for (a size, a position, a state)
//! instead of saving the same text again.

use super::compare::DIR;
use std::collections::{BTreeMap, BTreeSet};

/// The groups of text snapshots still byte-identical, each a set of names. Only shrinks: when a
/// group's snapshots differ or go, take them off (and lower `IDENTICAL_FILES`).
const IDENTICAL: &[&[&str]] = &[];

/// How many snapshots `IDENTICAL` names.
const IDENTICAL_FILES: usize = 0;

/// The approved text snapshots in `dir`, grouped by their text: the names of each text shared by
/// more than one.
fn alike(dir: &std::path::Path) -> Vec<BTreeSet<String>> {
    let mut by_text: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        let Some(stem) = name.strip_suffix(".txt") else { continue };
        if stem.ends_with(".new") || stem.ends_with(".diff") {
            continue;
        }
        let text = std::fs::read_to_string(dir.join(&name)).unwrap().replace("\r\n", "\n");
        by_text.entry(text).or_default().insert(stem.to_owned());
    }
    by_text.into_values().filter(|names| names.len() > 1).collect()
}

/// What is wrong with the groups found (`found`) against the allowed ones: a group not allowed,
/// or an allowed group that is no longer alike (to take off the list).
fn problems(found: &[BTreeSet<String>], allowed: &[&[&str]]) -> Vec<String> {
    let allowed: Vec<BTreeSet<String>> = allowed.iter().map(|g| g.iter().map(|n| n.to_string()).collect()).collect();
    let mut out = Vec::new();
    for group in found {
        if !allowed.iter().any(|a| group.is_subset(a)) {
            out.push(format!(
                "these text snapshots are byte-identical: {group:?}. Have the test assert what it is named for (or keep a theme variant as a golden image only, in goldens.rs's IMAGE_ONLY) instead"
            ));
        }
    }
    for a in &allowed {
        if !found.contains(a) {
            out.push(format!("{a:?} are no longer all alike: take them off IDENTICAL in identical.rs (it only shrinks)"));
        }
    }
    out
}

#[test]
fn no_two_text_snapshots_are_alike() {
    let names: Vec<&str> = IDENTICAL.iter().flat_map(|g| g.iter().copied()).collect();
    assert_eq!(names.len(), IDENTICAL_FILES, "IDENTICAL_FILES counts the names IDENTICAL lists (it only goes down)");
    assert_eq!(names.iter().collect::<BTreeSet<_>>().len(), names.len(), "a name is in two IDENTICAL groups");
    let problems = problems(&alike(std::path::Path::new(DIR)), IDENTICAL);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn problems_name_new_and_stale_groups() {
    let set = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
    assert!(problems(&[set(&["a", "b"])], &[&["a", "b"]]).is_empty());
    let new = problems(&[set(&["a", "b"]), set(&["c", "d"])], &[&["a", "b"]]);
    assert_eq!(new.len(), 1);
    assert!(new[0].contains("\"c\", \"d\""), "{new:?}");
    // A group now only partly alike, or no longer at all, has to come off the list.
    assert_eq!(problems(&[set(&["a", "b"])], &[&["a", "b", "c"]]).len(), 1);
    assert_eq!(problems(&[], &[&["a", "b"]]).len(), 1);
}

#[test]
fn alike_skips_new_and_diff_files() {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-identical-snaps-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, text) in [("a.txt", "x\n"), ("b.txt", "x\r\n"), ("a.new.txt", "x\n"), ("c.diff.txt", "x\n"), ("d.txt", "y\n")] {
        std::fs::write(dir.join(name), text).unwrap();
    }
    assert_eq!(alike(&dir), vec![["a", "b"].iter().map(|n| n.to_string()).collect::<BTreeSet<_>>()]);
    let _ = std::fs::remove_dir_all(&dir);
}
