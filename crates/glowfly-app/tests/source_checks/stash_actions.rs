//! Keeps the messages that act on one listed stash behind their builder. `StashMsg::Run` (apply,
//! pop or drop by position and commit) is built only by `state/stash_confirm.rs`, which decides
//! whether the action asks first, and handled only by `update/stash.rs`; `StashMsg::RenameDialog`
//! only by `state/stash_actions.rs` and handled by `update/stash.rs`. Every other file asks
//! `state/stash_actions.rs` for the message, so the STASHES menu, the palette and the toolbar
//! can't drift apart. Test code is skipped, and so are comments (through `code_lines`). Importing
//! the variants (`StashMsg::{`, `StashMsg::*`, `StashMsg as`) would hide the names, so those are
//! refused too.

use crate::hygiene::all_sources;
use crate::views::code_lines;

const MODEL: &str = "crates/glowfly-model/src/";

/// A guarded message and the files (under `MODEL`) that may name it.
const GUARDED: &[(&str, &[&str])] = &[
    ("StashMsg::Run", &["state/stash_confirm.rs", "update/stash.rs"]),
    ("StashMsg::RenameDialog", &["state/stash_actions.rs", "update/stash.rs"]),
];

/// The ways around naming a variant.
const ESCAPES: &[&str] = &["StashMsg::{", "StashMsg::*", "StashMsg as "];

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether `line` names `needle` as a whole path: not the tail of a longer name (`AutoStashMsg`)
/// and not the start of a longer one (`StashMsg::Runner`).
fn names(line: &str, needle: &str) -> bool {
    line.match_indices(needle).any(|(i, _)| {
        let before = line[..i].chars().next_back();
        let after = line[i + needle.len()..].chars().next();
        let ends_in_symbol = !needle.ends_with(is_word);
        !before.is_some_and(is_word) && (ends_in_symbol || !after.is_some_and(is_word))
    })
}

#[test]
fn stash_messages_are_built_by_the_stash_builder_only() {
    let mut found = Vec::new();
    let mut seen = Vec::new();
    for s in all_sources().iter().filter(|s| !s.test) {
        for (n, line) in code_lines(&s.text).iter().enumerate() {
            for (needle, allowed) in GUARDED {
                if !names(line, needle) {
                    continue;
                }
                let ok = s.path.strip_prefix(MODEL).is_some_and(|rel| allowed.contains(&rel));
                if ok {
                    seen.push((*needle, s.path.clone()));
                } else {
                    found.push(format!("{}:{}: {needle}", s.path, n + 1));
                }
            }
            for escape in ESCAPES {
                if names(line, escape) {
                    found.push(format!("{}:{}: {escape}", s.path, n + 1));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "get a stash's message from state/stash_actions.rs (`items`, `for_commit`, `pop_latest`), and name the variants in full: {found:?}"
    );
    let mut stale = Vec::new();
    for (needle, allowed) in GUARDED {
        for rel in *allowed {
            let path = format!("{MODEL}{rel}");
            if !seen.iter().any(|(n, p)| n == needle && *p == path) {
                stale.push(format!("{path} no longer names {needle}; remove it from GUARDED in tests/source_checks/stash_actions.rs"));
            }
        }
    }
    assert!(stale.is_empty(), "{stale:?}");
}

#[test]
fn the_check_tells_a_stash_message_from_its_neighbours() {
    assert!(names("Msg::Stash(StashMsg::Run { action, index, oid })", "StashMsg::Run"));
    assert!(names("crate::msg::StashMsg::RenameDialog { oid }", "StashMsg::RenameDialog"));
    assert!(!names("Msg::AutoStash(AutoStashMsg::Retry(r))", "StashMsg::Retry"));
    assert!(!names("AutoStashMsg::Run { .. }", "StashMsg::Run"));
    assert!(!names("StashMsg::Rename { oid, message }", "StashMsg::RenameDialog"));
    assert!(!names("StashMsg::Runner", "StashMsg::Run"));
    assert!(!names("Self::Run { .. }", "StashMsg::Run"));
    assert!(names("use crate::msg::StashMsg::{Run, Ask};", "StashMsg::{"));
    assert!(names("use crate::msg::StashMsg as M;", "StashMsg as "));
    let comment = code_lines("// StashMsg::Run\nfn f() {}\n");
    assert!(comment.iter().all(|l| !names(l, "StashMsg::Run")), "a comment doesn't count");
}
