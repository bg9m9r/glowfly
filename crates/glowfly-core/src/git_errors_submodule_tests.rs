//! Tests for reading a failure caused by changes inside a submodule: each of git's messages,
//! ahead of the local-changes lines the submodule's own git prints, never a fetch failure; and
//! what real git prints for a recursing checkout, a submodule update and a recursing reset.

use super::super::{Cause, classify};
use crate::gitcli::{args, run_with_input};
use crate::testutil::{empty_global_config, git, with_submodule, write};
use std::path::Path;

fn paths(p: &[&str]) -> Option<Cause> {
    Some(Cause::SubmoduleChanges { paths: p.iter().map(|s| s.to_string()).collect() })
}

#[test]
fn each_message_names_its_submodule() {
    assert_eq!(classify("error: Cannot update submodule:\n\tlibs/lib\n\tlibs/other\n\nAborting\n"), paths(&["libs/lib", "libs/other"]));
    assert_eq!(classify("error: Submodule 'libs/lib' could not be updated.\n"), paths(&["libs/lib"]));
    assert_eq!(classify("fatal: submodule 'x' has dirty index\n"), paths(&["x"]));
    assert_eq!(classify("error: unable to checkout submodule 'vendor/z'\n"), paths(&["vendor/z"]));
    let sha = "17fb2840c6fc2bf96cece6ac5e3057be068cf07e";
    for verb in ["checkout", "merge", "rebase"] {
        let out = format!("fatal: Unable to {verb} '{sha}' in submodule path 'libs/lib'\n");
        assert_eq!(classify(&out), paths(&["libs/lib"]), "{verb}");
    }
    assert_eq!(classify("fatal: could not reset submodule index\n"), paths(&[]));
}

#[test]
fn the_submodule_wins_over_its_own_local_changes_lines() {
    let out = "error: Your local changes to the following files would be overwritten by checkout:\n\tlib.txt\nPlease commit your \
               changes or stash them before you switch branches.\nAborting\nfatal: Unable to checkout \
               '17fb2840c6fc2bf96cece6ac5e3057be068cf07e' in submodule path 'libs/lib'\n";
    assert_eq!(classify(out), paths(&["libs/lib"]));
    let twice = "error: Submodule 'a' could not be updated.\nerror: Cannot update submodule:\n\ta\n\tb\n";
    assert_eq!(classify(twice), paths(&["a", "b"]), "each path once, in order");
}

#[test]
fn a_fetch_failure_in_a_submodule_is_not_a_change_inside() {
    let out = "fatal: could not read from remote repository.\nfatal: Unable to fetch in submodule path 'libs/lib'\n";
    assert!(!matches!(classify(out), Some(Cause::SubmoduleChanges { .. })));
    let alone = "fatal: Unable to fetch in submodule path 'libs/lib'\n";
    assert!(!matches!(classify(alone), Some(Cause::SubmoduleChanges { .. })));
}

/// Runs `git <a>` in `dir` with an empty global config; it must fail. Returns its output.
fn fails(dir: &Path, a: &[&str]) -> String {
    let env = empty_global_config(dir);
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (ok, out) = run_with_input(dir, &args(a), "", &env);
    assert!(!ok, "git {a:?} should fail: {out}");
    out
}

/// A parent on main recording the submodule's first commit c0, with a branch `moved` that
/// records c1 (lib.txt changed); the submodule is checked out at `at` ("c0" or "c1") with
/// lib.txt edited. Returns (parent, the id of c1).
fn dirty_sub(name: &str, at: &str) -> (std::path::PathBuf, String) {
    let (parent, sub) = with_submodule(name);
    let head = |d: &Path| git2::Repository::open(d).unwrap().head().unwrap().target().unwrap().to_string();
    let c0 = head(&sub);
    git(&parent, &["checkout", "-q", "-b", "moved"]);
    write(&sub, "lib.txt", "c1\n");
    git(&sub, &["commit", "-q", "-am", "c1"]);
    let c1 = head(&sub);
    git(&parent, &["add", "libs/lib"]);
    git(&parent, &["commit", "-q", "-m", "move lib"]);
    git(&parent, &["checkout", "-q", "main"]);
    git(&sub, &["checkout", "-q", "-f", if at == "c0" { &c0 } else { &c1 }]);
    write(&sub, "lib.txt", "dirty\n");
    (parent, c1)
}

#[test]
fn a_recursing_checkout_stopped_by_changes_inside() {
    let (parent, _) = dirty_sub("submodule-error-checkout", "c0");
    let out = fails(&parent, &["-c", "submodule.recurse=true", "checkout", "moved"]);
    assert!(out.contains("Submodule 'libs/lib' could not be updated."), "{out}");
    assert_eq!(classify(&out), paths(&["libs/lib"]), "{out}");
}

#[test]
fn a_submodule_update_stopped_by_changes_inside() {
    let (parent, c1) = dirty_sub("submodule-error-update", "c0");
    git(&parent, &["checkout", "-q", "moved"]);
    let out = fails(&parent, &["submodule", "update", "--checkout"]);
    assert!(out.contains(&format!("Unable to checkout '{c1}' in submodule path 'libs/lib'")), "{out}");
    assert_eq!(classify(&out), paths(&["libs/lib"]), "{out}");
}

#[test]
fn a_recursing_reset_stopped_by_changes_inside() {
    let (parent, _) = dirty_sub("submodule-error-reset", "c1");
    git(&parent, &["checkout", "-q", "moved"]);
    // --hard discards the changes inside too; --keep stops for them.
    let out = fails(&parent, &["-c", "submodule.recurse=true", "reset", "--keep", "HEAD~1"]);
    assert!(out.contains("Submodule 'libs/lib' could not be updated."), "{out}");
    assert_eq!(classify(&out), paths(&["libs/lib"]), "{out}");
}
