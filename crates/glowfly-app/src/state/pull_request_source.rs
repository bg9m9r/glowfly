//! Where the pull request panel's files and commits come from (update/pull_request/source.rs):
//! the key a comparison was asked for, the source tip it was shown from (the listed head when it
//! is already here, else the remote-tracking branch), the background fetch of the service's head
//! ref, and the notes above the lists when the local branches don't show the pull request as the
//! service has it. Pure.

use super::pull_request::{Files, PrView};
use git2::Oid;
use glowfly_core::forge::PrState;
use glowfly_core::repo::{RangeCommits, Snapshot, SourceFrom, SourceTip, is_commit_prefix};

/// What a comparison is asked from: the target's remote-tracking tip, the remote-tracking
/// source branch's (None when it belongs to another repository) and the listed head.
#[derive(Clone, PartialEq, Debug)]
pub struct FilesKey {
    pub target: Option<Oid>,
    pub branch: Option<Oid>,
    pub head: Option<String>,
}

/// The source tip the loaded files and commits were compared to, and the commits.
#[derive(Clone, PartialEq, Debug)]
pub struct Shown {
    pub source: SourceTip,
    pub commits: RangeCommits,
}

/// The fetch of the service's head ref for this panel copy (at most one automatic try).
#[derive(Clone, PartialEq, Debug, Default)]
pub enum HeadFetch {
    #[default]
    Idle,
    Running,
    /// git's last message.
    Failed(String),
    Done,
}

/// The key the panel compares from now.
pub fn files_key(v: &PrView) -> FilesKey {
    let head = v.pr.head.clone().filter(|h| is_commit_prefix(h));
    let on_fork_remote = v.source_remote.as_deref().is_some_and(|r| r != v.remote);
    FilesKey { target: v.tips.target, branch: if fork_known(v) && !on_fork_remote { None } else { v.tips.source }, head }
}

/// The service's list or its loaded page says the source branch is in another repository (a
/// fork), so the code host's remote-tracking branch with its name isn't it (the fork's own
/// remote's is, when it is a remote here).
pub fn fork_known(v: &PrView) -> bool {
    !v.pr.head_repo.is_same() || v.loaded().is_some_and(|p| !p.detail.same_repo)
}

/// Why the local branches may not show the pull request as the service has it, or how the
/// panel made up for that.
#[derive(Clone, PartialEq, Debug)]
pub enum OutOfSync {
    TargetMissing,
    SourceMissing,
    /// The source branch is gone because it was deleted once merged.
    SourceDeleted,
    /// The local source tip is not the head the service reported, which isn't here.
    SourceDiffers {
        local: Oid,
        head: String,
    },
    /// Shown from the listed head, already here: the branch is missing (`None`), elsewhere, or
    /// on a fork (`fork`).
    HeadUsed {
        branch: Option<Oid>,
        head: Oid,
        fork: bool,
    },
    HeadFetching,
    HeadFetchFailed(String),
    /// The head ref was fetched, but the listed head wasn't in it.
    HeadNotSent {
        head: String,
    },
}

impl OutOfSync {
    pub fn text(&self, v: &PrView) -> String {
        let (r, pr, noun, label) = (&v.remote, &v.pr, v.noun, &v.label);
        // The remote the source branch is on (a fork's, when it is a remote here).
        let s = v.source_remote.as_deref().unwrap_or(r);
        match self {
            OutOfSync::TargetMissing => format!("{r}/{} isn't in this repository. Fetch {r} to compare.", pr.target),
            OutOfSync::SourceMissing if head_fetchable(v) => {
                format!("{s}/{} isn't in this repository. Fetch {label}'s head from {r} to compare.", pr.source)
            }
            OutOfSync::SourceMissing if s != r.as_str() => format!("{s}/{} isn't in this repository. Fetch {s} to compare.", pr.source),
            OutOfSync::SourceMissing => {
                format!("{r}/{} isn't in this repository. Fetch {r} to compare. A branch on a fork can't be fetched from {r}.", pr.source)
            }
            OutOfSync::SourceDeleted => format!("{s}/{} was deleted after the merge.", pr.source),
            OutOfSync::SourceDiffers { local, head } => format!(
                "{s}/{} is at {}, but the {noun} is at {}. Fetch {s} to see its latest changes.",
                pr.source,
                short(&local.to_string()),
                short(head)
            ),
            OutOfSync::HeadUsed { fork: true, head, .. } => {
                format!("This {noun} comes from a fork, so this shows its head commit {}.", short(&head.to_string()))
            }
            OutOfSync::HeadUsed { branch: None, head, .. } => {
                format!("{s}/{} isn't in this repository, so this shows the {noun}'s head commit {}.", pr.source, short(&head.to_string()))
            }
            OutOfSync::HeadUsed { branch: Some(b), head, .. } => format!(
                "{s}/{} is at {}; this shows the {noun}'s head commit {}, which is already here.",
                pr.source,
                short(&b.to_string()),
                short(&head.to_string())
            ),
            OutOfSync::HeadFetching => format!("Fetching {label}'s head from {r}…"),
            OutOfSync::HeadFetchFailed(e) => format!("Couldn't fetch {label}'s head from {r}: {e}"),
            OutOfSync::HeadNotSent { head } => format!(
                "Fetched {label}'s head from {r}, but its commit {} wasn't in it: the {noun} may have changed since the list was read.",
                short(head)
            ),
        }
    }
}

pub(super) fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

/// The notes to show above the file and commit lists, one per problem. None while the files
/// load, so nothing flashes before the result.
pub fn out_of_sync(v: &PrView) -> Vec<OutOfSync> {
    let mut notes = Vec::new();
    if v.tips.target.is_none() {
        notes.push(OutOfSync::TargetMissing);
    }
    if v.files == Files::Loading {
        return notes;
    }
    let key = files_key(v);
    if let Some(s) = v.shown.as_ref().filter(|s| s.source.from == SourceFrom::Head && v.source_tip().is_some()) {
        notes.push(OutOfSync::HeadUsed { branch: key.branch, head: s.source.oid, fork: fork_known(v) });
        return notes;
    }
    match &v.head_fetch {
        HeadFetch::Running => notes.push(OutOfSync::HeadFetching),
        HeadFetch::Failed(e) => notes.push(OutOfSync::HeadFetchFailed(e.clone())),
        HeadFetch::Done if v.files == Files::Missing && key.head.is_some() => {
            notes.push(OutOfSync::HeadNotSent { head: key.head.clone().unwrap_or_default() })
        }
        _ => match (key.branch, &v.pr.head) {
            (None, _) if v.state() == Some(PrState::Merged) => notes.push(OutOfSync::SourceDeleted),
            (None, _) => notes.push(OutOfSync::SourceMissing),
            (Some(local), Some(head)) if !v.pr.head_matches(&local.to_string()) => {
                notes.push(OutOfSync::SourceDiffers { local, head: head.clone() })
            }
            _ => {}
        },
    }
    notes
}

/// How the notes banner is colored.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum NoteTone {
    Info,
    Progress,
    Warning,
}

/// Info when the notes only say the head was used; Progress while the head is fetched.
pub fn tone(notes: &[OutOfSync]) -> NoteTone {
    let calm = |n: &OutOfSync| matches!(n, OutOfSync::HeadUsed { .. } | OutOfSync::HeadFetching);
    if !notes.iter().all(calm) {
        NoteTone::Warning
    } else if notes.contains(&OutOfSync::HeadFetching) {
        NoteTone::Progress
    } else {
        NoteTone::Info
    }
}

/// Whether the banner offers "Fetch <remote>": not when its notes only say the head was used
/// or is being fetched.
pub fn offers_fetch(notes: &[OutOfSync]) -> bool {
    !notes.iter().all(|n| matches!(n, OutOfSync::HeadUsed { .. } | OutOfSync::HeadFetching))
}

/// Whether the service keeps a head ref for it and its head is known.
pub fn head_fetchable(v: &PrView) -> bool {
    v.head_ref.is_some() && files_key(v).head.is_some()
}

/// Whether the banner offers "Fetch #7's head": the head can be fetched and some note says the
/// source isn't shown as the service has it.
pub fn offers_head_fetch(v: &PrView, notes: &[OutOfSync]) -> bool {
    head_fetchable(v)
        && notes.iter().any(|n| {
            matches!(
                n,
                OutOfSync::SourceMissing
                    | OutOfSync::SourceDeleted
                    | OutOfSync::SourceDiffers { .. }
                    | OutOfSync::HeadFetchFailed(_)
                    | OutOfSync::HeadNotSent { .. }
            )
        })
}

/// Why "Fetch #7's head" is disabled: the remote is gone or has no URL, the repository isn't
/// `trusted`, or a head fetch is running.
pub fn head_fetch_blocked(snap: &Snapshot, v: &PrView, trusted: bool) -> Option<String> {
    fetch_blocked(snap, &v.remote, true).or_else(|| {
        if !trusted {
            Some("Trust this repository to fetch from it.".to_owned())
        } else if v.head_fetch == HeadFetch::Running {
            Some("Fetching…".to_owned())
        } else {
            None
        }
    })
}

/// The head button's label: "Fetch #7's head".
pub fn head_button(v: &PrView) -> String {
    format!("Fetch {}'s head", v.label)
}

/// Why the panel's Fetch button is disabled.
pub fn fetch_blocked(snap: &Snapshot, remote: &str, idle: bool) -> Option<String> {
    match super::remotes::find(snap, remote) {
        None => Some(format!("{remote} is no longer a remote")),
        Some(info) => super::remotes::fetch_blocked(info, idle).map(str::to_owned),
    }
}

#[cfg(test)]
mod tests;
