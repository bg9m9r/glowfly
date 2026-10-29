//! Checks for the trust plumbing's readers: a rebase or sequencer todo (what `--continue`
//! brings in), FETCH_HEAD (what a pinned pull merges), and the patch side (am's mailbox
//! detection, `git apply --numstat` output, and whether a patch's path reaches a hooks folder).

use super::text;
use crate::trust::Follow;
use crate::trust::follow::touches;
use crate::trust::incoming::{Todo, Unchecked, add_steps, is_mailbox};
use crate::trust::plumbing::numstat_paths;
use crate::trust::pull::fetched_in;
use git2::Oid;
use std::collections::HashSet;

/// The verbs whose first argument (after any options) is the commit the step replays.
const PICKS: [&str; 11] = ["pick", "p", "reword", "r", "edit", "e", "squash", "s", "fixup", "f", "revert"];

/// A made-up commit for the revision `spec`: the same spec always gives the same one.
fn fake_oid(spec: &str) -> Oid {
    use sha2::Digest;
    Oid::from_bytes(&sha2::Sha256::digest(spec.as_bytes())[..20]).expect("20 bytes are an object id")
}

/// The steps lines of a todo: trimmed, without blank lines and comments (as git reads them).
fn steps(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'))
}

/// A rebase or sequencer todo. Revisions resolve to made-up commits, except those holding a
/// `!`, which don't resolve. Invariants: no panic; every revision resolved is a word of the
/// todo (or `refs/rewritten/` and one); and when the todo reads, every pick step's commit was
/// resolved and listed (a revert's among the reverts), so no step is left unchecked, and a
/// todo with a merge or reset step has no pick list to simulate.
pub fn trust_todo(data: &[u8]) {
    let text = text(data);
    let words: HashSet<&str> = text.split_whitespace().collect();
    let mut asked: HashSet<String> = HashSet::new();
    let mut todo = Todo { picks: Some(Vec::new()), ..Todo::default() };
    let mut reverts = Vec::new();
    let mut resolve = |spec: &str| {
        let word = spec.strip_prefix("refs/rewritten/").unwrap_or(spec);
        assert!(words.contains(word) || words.contains(spec), "resolved {spec:?}, which isn't a word of the todo");
        asked.insert(spec.to_owned());
        if spec.contains('!') { Err(Unchecked::Unresolved(spec.to_owned())) } else { Ok(fake_oid(spec)) }
    };
    if add_steps(&text, &mut todo, &mut reverts, &mut resolve).is_err() {
        return;
    }
    for line in steps(&text) {
        let mut words = line.split_whitespace();
        let verb = words.next().unwrap_or_default();
        if matches!(verb, "merge" | "m" | "reset" | "t") {
            assert!(todo.picks.is_none(), "{line:?} left a pick list");
        }
        if !PICKS.contains(&verb) {
            continue;
        }
        let Some(commit) = words.find(|w| !w.starts_with('-')) else { continue };
        assert!(asked.contains(commit), "{line:?} read without resolving {commit:?}");
        let listed = if verb == "revert" { &reverts } else { &todo.commits };
        assert!(listed.contains(&fake_oid(commit)), "{line:?} brings in {commit:?}, which isn't listed");
    }
    if let Some(picks) = &todo.picks {
        assert!(picks.len() <= steps(&text).count());
    }
}

/// FETCH_HEAD. Invariants: no panic; when it reads, the commits listed are exactly its
/// non-blank lines without a `not-for-merge` mark, in order, each with the commit its first
/// field names.
pub fn trust_fetch_head(data: &[u8]) {
    let text = text(data);
    let Ok(fetched) = fetched_in(&text) else { return };
    let marked: Vec<&str> =
        text.lines().filter(|l| !l.trim().is_empty()).filter(|l| l.split('\t').nth(1).unwrap_or_default().is_empty()).collect();
    let lines: Vec<&str> = fetched.iter().map(|f| f.line.as_str()).collect();
    assert_eq!(lines, marked);
    for f in &fetched {
        let first = f.line.split('\t').next().unwrap_or_default();
        assert!(f.oid.to_string().starts_with(&first.to_ascii_lowercase()), "{:?} read as {}", f.line, f.oid);
    }
}

/// A patch, and `git apply --numstat -z` output. Invariants: no panic in the mailbox
/// detection; every path read from the numstat output is a NUL-free part of it; and a path
/// under the hooks folder is caught whatever follows it and however the folder is spelled
/// (letter case, the trailing dots Windows drops, the code points HFS+ ignores), while letter
/// case alone never changes the answer for any path.
pub fn trust_patch(data: &[u8]) {
    is_mailbox(data);
    let text = text(data);
    let paths = numstat_paths(&text);
    for p in &paths {
        assert!(!p.contains('\0') && text.contains(p.as_str()), "{p:?} isn't a path of the output");
    }
    let follows = [Follow { path: ".githooks".into(), tracked: None, hooks: true }];
    for p in paths.iter().map(String::as_str).chain(text.split(['\0', '\n']).take(64)) {
        assert_eq!(touches(p, &follows), touches(&p.to_ascii_uppercase(), &follows), "{p:?}");
        for folder in [".githooks", ".GitHooks", ".githooks.", ".git\u{200c}hooks", "./.githooks"] {
            assert!(touches(&format!("{folder}/{p}"), &follows), "{folder}/{p} isn't caught");
        }
    }
}
