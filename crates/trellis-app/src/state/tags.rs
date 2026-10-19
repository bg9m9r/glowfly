//! The New tag dialog's form, and the name and remote checks its view and `update` share; the
//! Annotate dialog's form and its check; the TAGS row menu's remote push and delete: the
//! confirmation, job labels and headlines; the tag checkout's headline and one-time
//! detached-HEAD warning. Pure.

use super::branches::default_remote;
use crate::msg::Msg;
use crate::tag::TagMsg;
use trellis_core::repo::Snapshot;
use trellis_core::tag_ops::{TagNameProblem, check_tag_name};

/// What the New tag dialog is editing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagForm {
    /// The commit to tag (a full sha).
    pub at: String,
    pub name: String,
    pub message: String,
    /// The message is required.
    pub annotated: bool,
    /// The remote picked for the push ("" when the repository has none).
    pub remote: String,
}

/// What is wrong with `name` (trimmed here) as a new tag, given the snapshot's tags; None when
/// it can be created.
pub fn tag_problem(snap: Option<&Snapshot>, name: &str) -> Option<TagNameProblem> {
    let existing: Vec<&str> =
        snap.map(|s| s.tags.iter().map(|(n, _)| n.as_str()).chain(s.other_tags.iter().map(String::as_str)).collect()).unwrap_or_default();
    check_tag_name(name.trim(), &existing).err()
}

/// The remote a new tag is pushed to: `form_remote` while it still exists, else the default
/// remote; None when there are no remotes.
pub fn push_remote(snap: &Snapshot, form_remote: &str) -> Option<String> {
    if snap.remotes.iter().any(|(r, _)| r == form_remote) { Some(form_remote.to_owned()) } else { default_remote(snap) }
}

/// What the Annotate dialog is editing: a lightweight tag to replace with an annotated one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnnotateForm {
    pub tag: String,
    /// The full sha of the commit the lightweight tag pointed at when the dialog opened.
    pub at: String,
    pub message: String,
}

/// Why lightweight tag `tag` at commit `at` (a full sha) can't be annotated any more, given the
/// snapshot; None when it can (or there is no snapshot to tell).
pub fn annotate_problem(snap: Option<&Snapshot>, tag: &str, at: &str) -> Option<String> {
    let snap = snap?;
    let Some((_, oid)) = snap.tags.iter().find(|(n, _)| n == tag) else {
        return Some(format!("Tag {tag} no longer exists."));
    };
    if !snap.lightweight_tags.contains(tag) {
        return Some(format!("Tag {tag} is already annotated."));
    }
    let now = oid.to_string();
    if now != at {
        let short = &now[..7.min(now.len())];
        return Some(format!("Tag {tag} has moved to {short}. Open Annotate again from the TAGS list."));
    }
    None
}

/// Whether a remote tag job pushes the tag or deletes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RemoteTagOp {
    Push,
    Delete,
}

/// The running remote tag job, kept so its end can be told in words that name the remotes.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteTagJob {
    pub label: String,
    pub op: RemoteTagOp,
    pub tag: String,
    /// The remotes in the order their commands run.
    pub remotes: Vec<String>,
}

/// "a", "a and b", or "a, b and c".
pub fn remote_list(remotes: &[String]) -> String {
    match remotes {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// The job's label: "Push tag v1 to origin" or "Delete tag v1 from origin and backup".
pub fn job_label(op: RemoteTagOp, tag: &str, remotes: &[String]) -> String {
    let list = remote_list(remotes);
    match op {
        RemoteTagOp::Push => format!("Push tag {tag} to {list}"),
        RemoteTagOp::Delete => format!("Delete tag {tag} from {list}"),
    }
}

impl RemoteTagJob {
    /// The headline when every remote succeeded.
    pub fn done(&self) -> String {
        let list = remote_list(&self.remotes);
        match self.op {
            RemoteTagOp::Push => format!("Pushed tag {} to {list}.", self.tag),
            RemoteTagOp::Delete => format!("Deleted tag {} from {list}.", self.tag),
        }
    }

    /// The headline when a remote failed: which one and why, then the remotes already done and
    /// those not tried. The runner echoes each command as "$ git …", so the number of echoes is
    /// the failed remote's number.
    pub fn failed(&self, output: &str) -> String {
        let echoes = output.lines().filter(|l| l.starts_with("$ git ")).count();
        let at = echoes.clamp(1, self.remotes.len().max(1)) - 1;
        let failed = self.remotes.get(at).map(String::as_str).unwrap_or("");
        let done = self.remotes.get(..at).unwrap_or_default();
        let rest = self.remotes.get(at + 1..).unwrap_or_default();
        let why = failure_reason(output);
        let mut text = match self.op {
            RemoteTagOp::Push => format!("Could not push tag {} to {failed}: {why}.", self.tag),
            RemoteTagOp::Delete => format!("Could not delete tag {} from {failed}: {why}.", self.tag),
        };
        if !done.is_empty() {
            let verb = if self.op == RemoteTagOp::Push { "Pushed to" } else { "Deleted from" };
            text.push_str(&format!(" {verb} {}.", remote_list(done)));
        }
        if !rest.is_empty() {
            text.push_str(&format!(" Not tried: {}.", remote_list(rest)));
        }
        text
    }
}

/// Why the last command failed, from its output (what follows the last "$ git " echo).
fn failure_reason(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let start = lines.iter().rposition(|l| l.starts_with("$ git ")).map_or(0, |i| i + 1);
    let section = &lines[start..];
    if section.iter().any(|l| l.contains("remote ref does not exist")) {
        return "the remote has no such tag".to_owned();
    }
    if section.iter().any(|l| l.contains("(already exists)")) {
        return "the remote already has a different tag with that name".to_owned();
    }
    let error = section.iter().find_map(|l| l.strip_prefix("error: ").or_else(|| l.strip_prefix("fatal: ")));
    let line = error.or_else(|| lines.iter().rfind(|l| !l.starts_with("$ ")).copied()).unwrap_or("");
    line.trim().trim_end_matches('.').to_owned()
}

/// The running tag checkout, kept so its end can name the tag and, the first time, warn about
/// detached HEAD.
#[derive(Clone, Debug, PartialEq)]
pub struct TagCheckoutJob {
    pub label: String,
    pub tag: String,
    /// The tag's commit when the checkout started, for the "Create branch…" action.
    pub oid: Option<git2::Oid>,
    /// The repository the checkout runs in, so the action only runs there.
    pub workdir: Option<std::path::PathBuf>,
}

/// What detached HEAD means, said once after the first tag checkout (and kept in the log).
pub const DETACHED_WARNING: &str = "HEAD is detached: commits you make now are on no branch and can be lost when you check out something else. Create a branch to keep them.";

/// The headline after checking out `tag`; `first` adds the detached-HEAD warning.
pub fn checkout_headline(tag: &str, first: bool) -> String {
    if first { format!("Checked out tag {tag}. {DETACHED_WARNING}") } else { format!("Checked out tag {tag} (detached HEAD)") }
}

/// The confirmation before deleting `tag` from `remotes`; accepting sends `TagMsg::DeleteRemote`.
pub fn delete_remote_confirm(tag: &str, remotes: Vec<String>) -> Msg {
    let (title, button) = match &remotes[..] {
        [one] => ("Delete tag from remote", format!("Delete from {one}")),
        _ => ("Delete tag from all remotes", "Delete from all remotes".to_owned()),
    };
    let body = format!(
        "Delete tag {tag} from {}? Your local tag stays, and anyone who already fetched it keeps their copy.",
        remote_list(&remotes)
    );
    Msg::confirm_danger(title, body, &button, Msg::Tag(TagMsg::DeleteRemote { tag: tag.to_owned(), remotes }))
}
