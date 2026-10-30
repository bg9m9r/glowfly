//! The pull request panel's action forms (comment, review, merge) and their rules: why each
//! action or choice is unavailable, the labels and explanations, the merge methods each service
//! lists, the merge confirmation and the post-merge branch deletion; the review form's own rules
//! are in `review.rs`, the local branch's deletion in `pull_request_cleanup.rs`. Pure; the calls
//! are made by update/pull_request/calls.rs.

use super::gitflow_base::Deleting;
use super::pull_request::{PrAction, PrView, Tone};
use super::pull_request_cleanup::{LocalDelete, Merging, local_delete};
use crate::msg::Msg;
use glowfly_core::forge::{EntryKind, MergeMethod, MergeState, PrDetail, PrPage, PrState, ReviewEvent, ReviewStyle, Verdict};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;

/// The most characters a comment, review summary or merge message may have.
pub const MAX_BODY: usize = 65_536;
/// The most characters a merge commit title may have.
pub const MAX_TITLE: usize = 1_024;

pub(super) const WAITING: &str = "Waiting for the service…";

/// Why nothing can be merged when the repository allows no strategy Glowfly can send.
pub const NO_METHODS: &str = "None of this repository's merge strategies can be used from Glowfly yet; merge it on the service";

mod review;
pub use review::*;

/// What the user has typed and chosen in the panel's forms.
#[derive(Clone, Debug, PartialEq)]
pub struct PrForms {
    pub comment: String,
    pub review_body: String,
    pub review_event: ReviewEvent,
    pub merge_method: Option<MergeMethod>,
    pub merge_title: String,
    pub merge_message: String,
    pub delete_branch: bool,
    /// Delete the local branch too once merged (`pull_request_cleanup`).
    pub delete_local: bool,
    /// Reply drafts by code comment thread id, kept across reloads of the page.
    pub replies: std::collections::HashMap<String, String>,
}

impl Default for PrForms {
    fn default() -> Self {
        PrForms {
            comment: String::new(),
            review_body: String::new(),
            review_event: ReviewEvent::Comment,
            merge_method: None,
            merge_title: String::new(),
            merge_message: String::new(),
            delete_branch: false,
            delete_local: false,
            replies: Default::default(),
        }
    }
}

pub(super) fn too_long(s: &str, max: usize) -> bool {
    s.chars().count() > max
}

/// The page, or why it can't be acted on yet (not loaded, or another action in flight).
pub(super) fn ready(v: &PrView) -> Result<&PrPage, String> {
    let Some(page) = v.loaded() else { return Err(format!("The {} hasn't loaded yet", v.noun)) };
    if v.busy.is_some() {
        return Err(WAITING.to_owned());
    }
    Ok(page)
}

/// "This pull request is merged" or "…is closed"; None while it is open.
pub(super) fn not_open(v: &PrView, d: &PrDetail) -> Option<String> {
    match d.state {
        PrState::Open => None,
        PrState::Merged => Some(format!("This {} is merged", v.noun)),
        PrState::Closed => Some(format!("This {} is closed", v.noun)),
    }
}

/// Why the Comment button is disabled.
pub fn comment_blocked(v: &PrView) -> Option<String> {
    let page = match ready(v) {
        Ok(p) => p,
        Err(why) => return Some(why),
    };
    let text = v.forms.comment.trim();
    if page.detail.locked {
        Some("The conversation is locked on the service".to_owned())
    } else if text.is_empty() {
        Some("Write a comment first".to_owned())
    } else if too_long(&v.forms.comment, MAX_BODY) {
        Some("Comments can have at most 65,536 characters".to_owned())
    } else {
        None
    }
}

/// The line above the merge form: whether the service would merge it now, and its tone.
/// None unless the page is loaded and the pull request open.
pub fn mergeability(v: &PrView) -> Option<(Tone, String)> {
    let d = &v.loaded()?.detail;
    if d.state != PrState::Open {
        return None;
    }
    let (source, target) = (&v.pr.source, &v.pr.target);
    Some(if d.draft {
        (Tone::Warn, "Drafts can't be merged; mark it ready on the service".to_owned())
    } else if d.merge_state == MergeState::Conflicts {
        (Tone::Bad, format!("Conflicts with {target} must be resolved first"))
    } else if d.can_merge == Some(false) {
        (Tone::Neutral, "Only people with write access can merge".to_owned())
    } else {
        match d.merge_state {
            MergeState::Clean => (Tone::Good, format!("No conflicts with {target}")),
            MergeState::Conflicts => (Tone::Bad, format!("Conflicts with {target} must be resolved first")),
            MergeState::Blocked => (Tone::Warn, "The repository's rules block merging (required reviews or checks)".to_owned()),
            MergeState::Behind => (Tone::Warn, format!("{source} is behind {target}; the service may require updating it")),
            MergeState::Unstable => (Tone::Warn, "Some checks haven't passed".to_owned()),
            MergeState::Unknown => (Tone::Neutral, "The service is still checking whether this can merge".to_owned()),
        }
    })
}

/// The merge form's methods: every one GitHub has (the repository's choice decides which are
/// enabled), or only the GitLab project's or Bitbucket repository's own.
pub fn merge_choices(d: &PrDetail) -> Vec<MergeMethod> {
    match d.style {
        ReviewStyle::Verdicts => MergeMethod::GITHUB.to_vec(),
        ReviewStyle::Approvals { .. } | ReviewStyle::Votes { .. } => d.methods.clone(),
    }
}

/// Why merge method `m` can't be chosen.
pub fn method_blocked(d: &PrDetail, m: MergeMethod) -> Option<&'static str> {
    (!d.methods.contains(&m)).then_some("Not enabled in this repository's settings")
}

pub fn method_label(m: MergeMethod) -> &'static str {
    match m {
        MergeMethod::Merge => "Merge commit",
        MergeMethod::Squash => "Squash and merge",
        MergeMethod::Rebase => "Rebase and merge",
        MergeMethod::FastForward => "Fast-forward",
    }
}

/// One line on what merge method `m` does to `target`.
pub fn method_help(m: MergeMethod, target: &str) -> String {
    match m {
        MergeMethod::Merge => format!("All commits are added to {target} through a merge commit."),
        MergeMethod::Squash => format!("The commits are combined into one commit on {target}."),
        MergeMethod::Rebase => format!("Each commit is replayed onto {target}, with no merge commit."),
        MergeMethod::FastForward => format!("{target} moves forward to the source branch's last commit, with no merge commit."),
    }
}

/// The Merge button's label: "Merge pull request", or just "Merge" for a merge request.
pub fn merge_button(v: &PrView) -> String {
    if v.noun == "merge request" { "Merge".to_owned() } else { format!("Merge {}", v.noun) }
}

/// Why the Merge button is disabled; the first reason that applies.
pub fn merge_blocked(v: &PrView) -> Option<String> {
    let page = match ready(v) {
        Ok(p) => p,
        Err(why) => return Some(why),
    };
    let d = &page.detail;
    if let Some(why) = not_open(v, d) {
        return Some(why);
    }
    if d.draft {
        return Some("Drafts can't be merged; mark it ready on the service".to_owned());
    }
    match d.merge_state {
        MergeState::Conflicts => return Some(format!("Conflicts with {} must be resolved first", v.pr.target)),
        MergeState::Blocked => return Some("The repository's rules block merging (required reviews or checks)".to_owned()),
        _ => {}
    }
    if d.can_merge == Some(false) {
        return Some("Only people with write access can merge".to_owned());
    }
    if merge_choices(d).is_empty() {
        return Some(NO_METHODS.to_owned());
    }
    match v.forms.merge_method {
        None => return Some("Choose how to merge".to_owned()),
        Some(m) => {
            if let Some(why) = method_blocked(d, m) {
                return Some(why.to_owned());
            }
        }
    }
    if too_long(&v.forms.merge_title, MAX_TITLE) || too_long(&v.forms.merge_message, MAX_BODY) {
        return Some("The commit title or message is too long".to_owned());
    }
    None
}

/// Why the source branch can't be deleted along with the merge.
pub fn delete_after_merge_blocked(v: &PrView, default_branch: Option<&str>) -> Option<String> {
    let Some(page) = v.loaded() else { return Some(format!("The {} hasn't loaded yet", v.noun)) };
    let d = &page.detail;
    if !d.same_repo || !v.pr.head_repo.is_same() {
        Some("The source branch is on a fork".to_owned())
    } else if default_branch == Some(v.pr.source.as_str()) {
        Some(format!("{} is the repository's default branch", v.pr.source))
    } else if d.deletes_merged_branches {
        Some("The repository deletes merged branches itself".to_owned())
    } else {
        None
    }
}

/// Whether the merge will delete the source branch: asked for, and allowed.
pub fn deletes_after_merge(v: &PrView, default_branch: Option<&str>) -> bool {
    v.forms.delete_branch && delete_after_merge_blocked(v, default_branch).is_none()
}

/// The merge `v`'s form would make, for its local branch's rules; None until the page loads.
pub fn merging<'a>(v: &'a PrView, default_branch: Option<&'a str>) -> Option<Merging<'a>> {
    let d = &v.loaded()?.detail;
    let (remote, source, target) = (v.remote.as_str(), v.pr.source.as_str(), v.pr.target.as_str());
    // A fork's source, by the service's list or its page, is never the local branch here.
    let same_repo = d.same_repo && v.pr.head_repo.is_same();
    Some(Merging { remote, source, target, method: v.forms.merge_method, same_repo, default_branch })
}

/// How the merge will delete the local branch: asked for, and allowed.
pub fn deletes_local(t: Lang, v: &PrView, snap: Option<&Snapshot>, default_branch: Option<&str>) -> Option<LocalDelete> {
    let m = merging(v, default_branch).filter(|_| v.forms.delete_local)?;
    local_delete(t, snap?, &m)?.1.ok()
}

/// The confirmation the Merge button opens; accepting it sends `PullRequestMsg::Merge`.
pub fn merge_confirm(t: Lang, v: &PrView, snap: Option<&Snapshot>, default_branch: Option<&str>) -> Msg {
    let how = match v.forms.merge_method {
        Some(MergeMethod::Squash) => "squashing its commits",
        Some(MergeMethod::Rebase) => "rebasing its commits",
        Some(MergeMethod::FastForward) => "a fast-forward",
        _ => "a merge commit",
    };
    let title = glowfly_core::text::printable(&v.pr.title);
    let mut body = format!("Merge {} \u{201c}{title}\u{201d} into {} by {how}?", v.label, v.pr.target);
    if deletes_after_merge(v, default_branch) {
        body.push_str(&format!(" Then delete {} on {}.", v.pr.source, v.remote));
    }
    if let Some(d) = deletes_local(t, v, snap, default_branch) {
        let (branch, target) = (d.branch.as_str(), v.pr.target.as_str());
        body.push(' ');
        body.push_str(&match d.checkout {
            Some(_) => t.trf("pr.merge.confirm.delete_local_head", &[("branch", &branch), ("target", &target)]),
            None => t.trf("pr.merge.confirm.delete_local", &[("branch", &branch)]),
        });
    }
    Msg::confirm(&merge_button(v), body, "Merge", Msg::PullRequest(crate::pull_request::PullRequestMsg::Merge))
}

/// After a merge: the Delete button's confirmation, when the source branch is still on the
/// remote, in the same repository, not the default branch, and the repository doesn't delete
/// merged branches itself. Asks more firmly when the source is the Git Flow production or
/// development branch (`gitflow_base::guard`).
pub fn delete_branch_offer(t: Lang, v: &PrView, snap: &Snapshot, default_branch: Option<&str>) -> Option<Msg> {
    if v.state() != Some(PrState::Merged) || delete_after_merge_blocked(v, default_branch).is_some() {
        return None;
    }
    super::pull_request::Tips::of(snap, &v.remote, v.source_remote.as_deref(), &v.pr).source?;
    let (remote, source) = (v.remote.as_str(), v.pr.source.as_str());
    let confirm = Msg::confirm_danger(
        "Delete remote branch",
        format!("Delete branch {source} on {remote}? This affects everyone using the remote."),
        "Delete",
        Msg::git("Delete remote branch", &["push", remote, "--delete", source]),
    );
    Some(super::gitflow_base::guard(t, snap, source, Deleting::Remote(remote), confirm))
}

/// What an entry's author did, and its tone.
pub fn verdict_text(kind: &EntryKind) -> (&'static str, Tone) {
    match kind {
        EntryKind::Comment => ("commented", Tone::Neutral),
        EntryKind::Review(Verdict::Approved) => ("approved", Tone::Good),
        EntryKind::Review(Verdict::ChangesRequested) => ("requested changes", Tone::Bad),
        EntryKind::Review(Verdict::Commented) => ("reviewed", Tone::Neutral),
        EntryKind::Review(Verdict::Dismissed) => ("review dismissed", Tone::Neutral),
        EntryKind::Review(Verdict::ApprovalRemoved) => ("removed their approval", Tone::Neutral),
    }
}

/// Fits the forms to a freshly loaded page: a merge method the repository allows (the one last
/// used in this repository, `remembered`, when the form has none allowed and it is, else the
/// first), and a review choice the service lists (on GitHub and Bitbucket, no verdict the author
/// can't give).
pub fn prefill(forms: &mut PrForms, d: &PrDetail, remembered: Option<MergeMethod>) {
    if forms.merge_method.is_none_or(|m| !d.methods.contains(&m)) {
        forms.merge_method = remembered.filter(|m| d.methods.contains(m)).or_else(|| d.methods.first().copied());
    }
    let own = matches!(d.style, ReviewStyle::Verdicts | ReviewStyle::Votes { .. }) && d.is_author();
    if own || !review_choices(d).contains(&forms.review_event) {
        forms.review_event = ReviewEvent::Comment;
    }
}

/// Which action a failure belongs to, for its place in the panel.
pub fn error_for<'a>(v: &'a PrView, action: &PrAction) -> Option<&'a str> {
    v.action_error.as_ref().filter(|(a, _)| a == action).map(|(_, e)| e.as_str())
}

#[cfg(test)]
mod bitbucket_tests;
#[cfg(test)]
mod gitlab_tests;
#[cfg(test)]
pub(crate) mod tests;
