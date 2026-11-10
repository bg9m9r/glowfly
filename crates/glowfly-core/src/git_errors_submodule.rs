//! A git command stopped by changes inside a submodule (a checkout or reset that recurses into
//! submodules, `git submodule update`): the paths git names, from its messages. Pure.

use super::{Cause, listed, quoted_after};

/// `Cause::SubmoduleChanges` when one of `lines` (no echoes or hints) is git's message for a
/// submodule whose changes stopped the command, with the paths git names, in order and once
/// each (none for "could not reset submodule index").
pub(super) fn changes(lines: &[&str]) -> Option<Cause> {
    let mut found = false;
    let mut paths: Vec<String> = Vec::new();
    let mut add = |p: &str| {
        if !p.is_empty() && !paths.iter().any(|q| q == p) {
            paths.push(p.to_owned());
        }
    };
    let header = |l: &str| l.contains("cannot update submodule");
    let list = listed(lines, header);
    if lines.iter().any(|l| l.trim_end().to_lowercase().ends_with("cannot update submodule:")) {
        found = true;
        list.iter().for_each(|p| add(p));
    }
    for l in lines {
        if let Some(p) = named(l) {
            found = true;
            add(p);
        } else if l.contains("could not reset submodule index") {
            found = true;
        }
    }
    found.then_some(Cause::SubmoduleChanges { paths })
}

/// The path one of git's submodule lines names.
fn named(l: &str) -> Option<&str> {
    if l.contains("' could not be updated.") {
        return quoted_after(l, "Submodule '");
    }
    if l.contains("' has dirty index") {
        return quoted_after(l, "ubmodule '");
    }
    if let Some(p) = quoted_after(l, "nable to checkout submodule '") {
        return Some(p);
    }
    // git submodule update's checkout, merge or rebase inside the submodule.
    let updating = ["checkout", "merge", "rebase"].iter().any(|v| l.contains(&format!("Unable to {v} '")));
    if updating && l.contains("' in submodule path '") {
        return quoted_after(l, "' in submodule path '");
    }
    None
}

#[cfg(test)]
#[path = "git_errors_submodule_tests.rs"]
mod tests;
