//! The SUBMODULES section's text and rules, shared by the view and `update`: the status word,
//! the tooltip, why open, update, copy and edit are blocked, the update confirmation (with the
//! commits it would leave behind) and the update command. The Add submodule dialog's rules are in
//! `add.rs`, the Edit dialog's in `edit.rs`, the Remove submodule confirmation's in `remove.rs`,
//! and batch updates (Initialize all, Update all, updating after checkouts) in `batch.rs`.

use super::AppState;
use git2::Oid;
use glowfly_core::git_version::Feature;
use glowfly_core::gitcli::args;
use glowfly_core::repo::{LEFT_BEHIND_MAX, SubmoduleInfo, SubmoduleState};
use glowfly_core::settings::SubmoduleUpdates;
use glowfly_core::submodule_ops::is_relative_url;

pub mod add;
pub mod batch;
pub mod edit;
pub mod remove;

/// How many commits a confirmation names.
const NAMED: usize = 5;

/// The first 7 hex characters of `oid`.
pub(crate) fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// The word at the right of a row; None when in sync (and committed). An added submodule's
/// starts with "new".
pub fn row_word(sm: &SubmoduleInfo) -> Option<String> {
    let word = match sm.state {
        SubmoduleState::InSync => None,
        SubmoduleState::OutOfSync => Some("out of sync"),
        SubmoduleState::Dirty => Some("changes"),
        SubmoduleState::DirtyOutOfSync => Some("changes · out of sync"),
        SubmoduleState::Uninitialized => Some("not initialized"),
        SubmoduleState::Missing => Some("folder missing"),
    };
    match (sm.added, word) {
        (true, Some(w)) => Some(format!("new · {w}")),
        (true, None) => Some("new".to_owned()),
        (false, w) => w.map(str::to_owned),
    }
}

/// A row's hover text: what its state means, then its name, URL and commits.
pub fn tooltip(sm: &SubmoduleInfo) -> String {
    let recorded = short(sm.recorded);
    let checked = sm.checked_out.map(short);
    let mut out = String::new();
    if sm.added {
        out.push_str(&if sm.is_initialized() {
            format!("Added and checked out, but not committed yet; commit .gitmodules and {} to record it.\n", sm.path)
        } else {
            "Added but not committed yet, and not checked out; Initialize clones it.\n".to_owned()
        });
    }
    out.push_str(&match sm.state {
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
        SubmoduleState::Uninitialized => {
            format!("An empty folder: never initialized, or deinitialized. Initialize clones it and checks out {recorded}.")
        }
        SubmoduleState::Missing => format!(
            "The folder is gone, but this repository still records the submodule. Restore checks it out again at {recorded}; \
             Remove submodule… drops it."
        ),
    });
    out.push('\n');
    if sm.name != sm.path {
        out.push_str(&format!("\nName: {}", sm.name));
    }
    match &sm.url {
        Some(u) => {
            out.push_str(&format!("\nURL: {u}"));
            if let Some(c) = sm.clone_url.as_deref().filter(|c| is_relative_url(u) && c != u) {
                out.push_str(&format!("\nResolves to: {c}"));
            }
        }
        None => out.push_str("\nURL: none in .gitmodules"),
    }
    if let Some(b) = &sm.branch {
        out.push_str(&format!("\nBranch: {b}"));
    }
    out.push_str(&format!("\nRecorded commit: {recorded}"));
    out.push_str(&format!("\nChecked-out commit: {}", checked.as_deref().unwrap_or("none")));
    out
}

/// Why the submodule can't be opened in a tab: its folder holds no repository yet, so opening
/// it would find the parent instead.
pub fn open_blocked(sm: &SubmoduleInfo) -> Option<&'static str> {
    match sm.state {
        SubmoduleState::Missing => Some("Its folder is missing. Restore it first."),
        SubmoduleState::Uninitialized => Some("Not checked out yet. Initialize it first."),
        _ => None,
    }
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

/// What Copy URL copies: the URL git clones from (a relative one resolved), else .gitmodules'.
pub fn copy_url_text(sm: &SubmoduleInfo) -> String {
    sm.clone_url.clone().or_else(|| sm.url.clone()).unwrap_or_default()
}

/// Why Edit submodule… is unavailable; `git_blocked` is why this git can't (too old).
pub fn edit_blocked(sm: &SubmoduleInfo, git_blocked: Option<&str>) -> Option<String> {
    if sm.url.is_none() { Some(".gitmodules has no entry for it.".to_owned()) } else { git_blocked.map(str::to_owned) }
}

/// The menu item that runs `update_args`.
pub fn update_label(sm: &SubmoduleInfo) -> &'static str {
    match sm.state {
        SubmoduleState::Missing => "Restore folder",
        SubmoduleState::Uninitialized => "Initialize",
        _ => "Update to recorded commit",
    }
}

/// The job's label, in the toast and the activity log.
pub fn job_label(sm: &SubmoduleInfo) -> String {
    let verb = match sm.state {
        SubmoduleState::Missing => "Restore",
        SubmoduleState::Uninitialized => "Initialize",
        _ => "Update",
    };
    format!("{verb} submodule {}", sm.path)
}

/// What the SUBMODULES section shows beyond the snapshot: this repository's choice about
/// updating after checkouts and pulls, the global setting it may follow, and why this git can't
/// edit a submodule.
pub struct SectionCx {
    pub updates: SubmoduleUpdates,
    pub global: bool,
    pub edit_git: Option<String>,
}

pub fn section_cx(st: &AppState) -> SectionCx {
    let own = st.snap.as_ref().and_then(|s| st.settings.submodule_updates_for(&s.real_workdir));
    SectionCx {
        updates: SubmoduleUpdates::from_override(own),
        global: st.settings.submodules.keep_updated,
        edit_git: st.git.blocked(Feature::SubmoduleEdit),
    }
}

/// "1 commit", "3 commits", or "50 or more commits" when the list was cut off.
fn commit_count(sm: &SubmoduleInfo) -> String {
    count_commits(sm.left_behind.len())
}

/// "1 commit", "3 commits", or "50 or more commits" for a list cut off at `LEFT_BEHIND_MAX`.
pub(crate) fn count_commits(n: usize) -> String {
    match n {
        1 => "1 commit".to_owned(),
        LEFT_BEHIND_MAX => format!("{LEFT_BEHIND_MAX} or more commits"),
        n => format!("{n} commits"),
    }
}

/// The first commits of `list` as indented "<short id> <summary>" lines, each after a line
/// break, then "and [at least] N more" for the rest ("at least" when the list was cut off at
/// `LEFT_BEHIND_MAX`).
pub(crate) fn named_commits(list: &[(Oid, String)]) -> String {
    let mut out = String::new();
    for (id, summary) in list.iter().take(NAMED) {
        out.push_str(&format!("\n    {} {summary}", short(*id)));
    }
    if list.len() > NAMED {
        let at_least = if list.len() == LEFT_BEHIND_MAX { "at least " } else { "" };
        out.push_str(&format!("\n    and {at_least}{} more", list.len() - NAMED));
    }
    out
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
    out.push_str(&named_commits(&sm.left_behind));
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

/// `git submodule update --init --checkout -- :(literal)<path>` (the path is never a
/// glob: `lib[1]` would update `lib1`; `git --literal-pathspecs` doesn't reach `submodule update`,
/// as of git 2.54, but the pathspec magic does): clones it if needed and checks out the
/// recorded commit. `--checkout` overrides a `submodule.<name>.update` of rebase, merge or none,
/// so the action does what its label says.
pub fn update_args(path: &str) -> Vec<String> {
    args(&["submodule", "update", "--init", "--checkout", "--", &format!(":(literal){path}")])
}

#[cfg(test)]
mod tests;
