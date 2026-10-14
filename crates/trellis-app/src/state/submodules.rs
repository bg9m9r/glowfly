//! The SUBMODULES section's text and rules, shared by the view and `update`: the status word,
//! the tooltip, why open, update and copy are blocked, the update confirmation (with the commits
//! it would leave behind) and the update command.

use git2::Oid;
use trellis_core::gitcli::args;
use trellis_core::repo::{LEFT_BEHIND_MAX, SubmoduleInfo, SubmoduleState};

/// How many left-behind commits the confirmation names.
const NAMED: usize = 5;

/// The first 7 hex characters of `oid`.
fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// The word at the right of a row; None when in sync.
pub fn state_word(s: SubmoduleState) -> Option<&'static str> {
    match s {
        SubmoduleState::InSync => None,
        SubmoduleState::OutOfSync => Some("out of sync"),
        SubmoduleState::Dirty => Some("changes"),
        SubmoduleState::DirtyOutOfSync => Some("changes · out of sync"),
        SubmoduleState::Uninitialized => Some("not initialized"),
    }
}

/// A row's hover text: what its state means, then its name, URL and commits.
pub fn tooltip(sm: &SubmoduleInfo) -> String {
    let recorded = short(sm.recorded);
    let checked = sm.checked_out.map(short);
    let mut out = match sm.state {
        SubmoduleState::InSync => "Checked out at the commit this repository records.".to_owned(),
        SubmoduleState::OutOfSync | SubmoduleState::DirtyOutOfSync => {
            let now = checked.clone().unwrap_or_default();
            let mut s = format!(
                "Checked out at {now} while this repository records {recorded}. Updating checks out {recorded}; committing the \
                 submodule path here records {now} instead."
            );
            if sm.is_dirty() {
                s.push_str("\nIt also has uncommitted changes or untracked files inside; open it to commit or discard them.");
            }
            if !sm.left_behind.is_empty() {
                let count = commit_count(sm);
                let (verb, them) = if sm.left_behind.len() == 1 { ("is", "it") } else { ("are", "them") };
                s.push_str(&format!("\n{count} here {verb} on no branch or tag; updating would leave {them} behind."));
            }
            s
        }
        SubmoduleState::Dirty => "Uncommitted changes or untracked files inside; open it to commit or discard them.".to_owned(),
        SubmoduleState::Uninitialized => format!("Not checked out yet; Initialize clones it and checks out {recorded}."),
    };
    out.push('\n');
    if sm.name != sm.path {
        out.push_str(&format!("\nName: {}", sm.name));
    }
    match &sm.url {
        Some(u) => out.push_str(&format!("\nURL: {u}")),
        None => out.push_str("\nURL: none in .gitmodules"),
    }
    out.push_str(&format!("\nRecorded commit: {recorded}"));
    out.push_str(&format!("\nChecked-out commit: {}", checked.as_deref().unwrap_or("none")));
    out
}

/// Why the submodule can't be opened in a tab: its folder holds no repository yet, so opening
/// it would find the parent instead.
pub fn open_blocked(sm: &SubmoduleInfo) -> Option<&'static str> {
    (!sm.is_initialized()).then_some("Not checked out yet. Initialize it first.")
}

/// Why Initialize / Update to recorded commit is unavailable.
pub fn update_blocked(sm: &SubmoduleInfo) -> Option<&'static str> {
    if sm.url.is_none() {
        Some(".gitmodules has no entry for it, so git can't initialize or update it.")
    } else if sm.is_initialized() && !sm.is_out_of_sync() {
        Some("Already checked out at the recorded commit.")
    } else {
        None
    }
}

pub fn copy_url_blocked(sm: &SubmoduleInfo) -> Option<&'static str> {
    sm.url.is_none().then_some(".gitmodules has no URL for it.")
}

/// The menu item that runs `update_args`.
pub fn update_label(sm: &SubmoduleInfo) -> &'static str {
    if sm.is_initialized() { "Update to recorded commit" } else { "Initialize" }
}

/// The job's label, in the toast and the activity log.
pub fn job_label(sm: &SubmoduleInfo) -> String {
    if sm.is_initialized() { format!("Update submodule {}", sm.path) } else { format!("Initialize submodule {}", sm.path) }
}

/// "1 commit", "3 commits", or "50 or more commits" when the list was cut off.
fn commit_count(sm: &SubmoduleInfo) -> String {
    match sm.left_behind.len() {
        1 => "1 commit".to_owned(),
        LEFT_BEHIND_MAX => format!("{LEFT_BEHIND_MAX} or more commits"),
        n => format!("{n} commits"),
    }
}

/// The confirmation before updating a submodule that is checked out at another commit. When
/// that would leave commits behind, it says so and names them.
pub fn confirm_update_body(sm: &SubmoduleInfo) -> String {
    let now = sm.checked_out.map(short).unwrap_or_default();
    let recorded = short(sm.recorded);
    if sm.left_behind.is_empty() {
        return format!(
            "{} is at {now} now. Updating checks out the recorded commit {recorded} (detached HEAD). Branches inside the \
             submodule are left as they are.",
            sm.path
        );
    }
    let one = sm.left_behind.len() == 1;
    let mut out = format!(
        "{} is at {now} now. No branch or tag inside it contains {}, so updating to the recorded commit {recorded} leaves {} \
         behind:\n",
        sm.path,
        if one { "that commit" } else { "these commits" },
        if one { "it" } else { "them" },
    );
    for (id, summary) in sm.left_behind.iter().take(NAMED) {
        out.push_str(&format!("\n    {} {summary}", short(*id)));
    }
    if sm.left_behind.len() > NAMED {
        let at_least = if sm.left_behind.len() == LEFT_BEHIND_MAX { "at least " } else { "" };
        out.push_str(&format!("\n    and {at_least}{} more", sm.left_behind.len() - NAMED));
    }
    out.push_str(&format!(
        "\n\nAfterwards only the submodule's reflog finds {}, until it expires. To keep {0}, cancel, open the submodule and \
         create a branch at {now}.",
        if one { "it" } else { "them" }
    ));
    out
}

/// The confirmation's button: plain, or naming what is left behind.
pub fn confirm_update_button(sm: &SubmoduleInfo) -> String {
    if sm.left_behind.is_empty() { "Update".to_owned() } else { format!("Update and leave {} behind", commit_count(sm)) }
}

/// `git submodule update --init -- <path>`: clones it if needed and checks out the recorded commit.
pub fn update_args(path: &str) -> Vec<String> {
    args(&["submodule", "update", "--init", "--", path])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(state: SubmoduleState, name: &str, url: Option<&str>) -> SubmoduleInfo {
        let checked_out = match state {
            SubmoduleState::Uninitialized => None,
            SubmoduleState::OutOfSync | SubmoduleState::DirtyOutOfSync => Some(Oid::from_str(&"b".repeat(40)).unwrap()),
            _ => Some(Oid::from_str(&"a".repeat(40)).unwrap()),
        };
        SubmoduleInfo {
            name: name.into(),
            path: "libs/core".into(),
            url: url.map(str::to_owned),
            recorded: Oid::from_str(&"a".repeat(40)).unwrap(),
            checked_out,
            state,
            left_behind: Vec::new(),
        }
    }

    #[test]
    fn state_words() {
        assert_eq!(state_word(SubmoduleState::InSync), None);
        assert_eq!(state_word(SubmoduleState::OutOfSync), Some("out of sync"));
        assert_eq!(state_word(SubmoduleState::Dirty), Some("changes"));
        assert_eq!(state_word(SubmoduleState::DirtyOutOfSync), Some("changes · out of sync"));
        assert_eq!(state_word(SubmoduleState::Uninitialized), Some("not initialized"));
    }

    #[test]
    fn tooltip_lines() {
        let t = tooltip(&info(SubmoduleState::OutOfSync, "libs/core", Some("../core.git")));
        assert!(t.starts_with("Checked out at bbbbbbb while this repository records aaaaaaa."), "{t}");
        assert!(t.contains("\nURL: ../core.git") && t.contains("\nRecorded commit: aaaaaaa"), "{t}");
        assert!(t.contains("\nChecked-out commit: bbbbbbb") && !t.contains("Name:"), "{t}");

        let t = tooltip(&info(SubmoduleState::Uninitialized, "core", None));
        assert!(t.contains("\nName: core") && t.contains("\nURL: none in .gitmodules"), "{t}");
        assert!(t.contains("\nChecked-out commit: none") && t.contains("checks out aaaaaaa"), "{t}");
    }

    #[test]
    fn labels_and_rules() {
        let un = info(SubmoduleState::Uninitialized, "libs/core", Some("u"));
        let ok = info(SubmoduleState::InSync, "libs/core", Some("u"));
        let dirty = info(SubmoduleState::Dirty, "libs/core", Some("u"));
        let out = info(SubmoduleState::DirtyOutOfSync, "libs/core", Some("u"));
        assert_eq!((update_label(&un), job_label(&un)), ("Initialize", "Initialize submodule libs/core".to_owned()));
        assert_eq!((update_label(&ok), job_label(&ok)), ("Update to recorded commit", "Update submodule libs/core".to_owned()));
        assert!(open_blocked(&un).is_some() && open_blocked(&dirty).is_none());
        assert_eq!(update_blocked(&un), None);
        assert_eq!(update_blocked(&out), None);
        assert_eq!(update_blocked(&ok), Some("Already checked out at the recorded commit."));
        assert_eq!(update_blocked(&dirty), Some("Already checked out at the recorded commit."));
        assert!(update_blocked(&info(SubmoduleState::Uninitialized, "x", None)).unwrap().contains(".gitmodules"));
        assert!(copy_url_blocked(&ok).is_none() && copy_url_blocked(&info(SubmoduleState::InSync, "x", None)).is_some());
        let body = confirm_update_body(&out);
        assert!(body.contains("bbbbbbb") && body.contains("aaaaaaa") && body.contains("Branches inside"), "{body}");
        assert_eq!(confirm_update_button(&out), "Update");
        assert_eq!(update_args("libs/core"), ["submodule", "update", "--init", "--", "libs/core"]);
    }

    fn with_left_behind(n: usize) -> SubmoduleInfo {
        let mut sm = info(SubmoduleState::OutOfSync, "libs/core", Some("u"));
        sm.left_behind = (0..n).map(|i| (Oid::from_str(&format!("{:x}", i + 1).repeat(40)[..40]).unwrap(), format!("work {i}"))).collect();
        sm
    }

    #[test]
    fn left_behind_commits_are_named() {
        let one = with_left_behind(1);
        let body = confirm_update_body(&one);
        assert!(body.contains("No branch or tag inside it contains that commit") && !body.contains("Branches inside"), "{body}");
        assert!(body.contains("\n    1111111 work 0") && body.contains("To keep it, cancel"), "{body}");
        assert!(body.contains("create a branch at bbbbbbb"), "{body}");
        assert_eq!(confirm_update_button(&one), "Update and leave 1 commit behind");
        let t = tooltip(&one);
        assert!(t.contains("\n1 commit here is on no branch or tag; updating would leave it behind."), "{t}");

        let seven = with_left_behind(7);
        let body = confirm_update_body(&seven);
        assert!(body.contains("these commits") && body.contains("\n    5555555 work 4") && !body.contains("work 5"), "{body}");
        assert!(body.contains("\n    and 2 more\n"), "{body}");
        assert_eq!(confirm_update_button(&seven), "Update and leave 7 commits behind");

        let many = with_left_behind(LEFT_BEHIND_MAX);
        let body = confirm_update_body(&many);
        assert!(body.contains("\n    and at least 45 more\n"), "{body}");
        assert_eq!(confirm_update_button(&many), "Update and leave 50 or more commits behind");
        let t = tooltip(&many);
        assert!(t.contains("\n50 or more commits here are on no branch or tag; updating would leave them behind."), "{t}");
    }
}
