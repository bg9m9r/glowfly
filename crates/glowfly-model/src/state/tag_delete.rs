//! The Delete tag dialog's form: the tag, the commit it pointed at when the dialog opened and a
//! tick per configured remote to delete it from too; the button's label and the message it
//! sends. Also the headline of its job, which deletes the local tag first and then each ticked
//! remote's copy, read from the runner's "$ git …" echoes. Pure.

use super::tags::{RemoteTagJob, failure_reason, remote_list_in};
use crate::msg::Msg;
use crate::tag::TagMsg;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;

#[cfg(test)]
#[path = "tag_delete_tests.rs"]
mod tests;

/// What the Delete tag dialog is editing.
#[derive(Clone, Debug, PartialEq)]
pub struct DeleteTagForm {
    pub tag: String,
    /// The commit the tag pointed at when the dialog opened.
    pub at: git2::Oid,
    /// Every configured remote, in the snapshot's order, and whether it is ticked.
    pub remotes: Vec<(String, bool)>,
}

impl DeleteTagForm {
    /// The form for `tag` with every remote unticked; None when the snapshot has no such tag
    /// (also a tag that points at a tree or blob, which the TAGS list never shows).
    pub fn new(snap: &Snapshot, tag: &str) -> Option<Self> {
        let (_, at) = snap.tags.iter().find(|(n, _)| n == tag)?;
        let remotes = snap.remotes.iter().map(|(r, _)| (r.clone(), false)).collect();
        Some(Self { tag: tag.to_owned(), at: *at, remotes })
    }

    /// The ticked remotes, in list order.
    pub fn ticked(&self) -> Vec<String> {
        self.remotes.iter().filter(|(_, on)| *on).map(|(r, _)| r.clone()).collect()
    }

    /// The danger button's label: "Delete", or "Delete here and on 2 remotes".
    pub fn button(&self, t: Lang) -> String {
        match self.remotes.iter().filter(|(_, on)| *on).count() {
            0 => t.tr("common.delete.button").to_owned(),
            n => t.trn("tag.delete.button_remotes", n, &[]),
        }
    }

    /// What the danger button sends.
    pub fn delete_msg(&self) -> Msg {
        Msg::Tag(TagMsg::Delete { tag: self.tag.clone(), at: self.at, remotes: self.ticked() })
    }
}

/// The output of each command the runner echoed ("$ git …"): the local delete first, then one
/// section per remote.
fn sections(output: &str) -> Vec<Vec<&str>> {
    let mut out: Vec<Vec<&str>> = Vec::new();
    for line in output.lines() {
        if line.starts_with("$ git ") {
            out.push(Vec::new());
        } else if let Some(last) = out.last_mut() {
            last.push(line);
        }
    }
    out
}

/// The headline when the Delete tag job `job` (with remotes) ended. A remote that answered
/// git's "non-existent ref" warning had no such tag and is named so. None when no command ran
/// (the job's `expect` refused it), which leaves the generic failure headline.
pub fn headline(t: Lang, job: &RemoteTagJob, ok: bool, output: &str) -> Option<String> {
    let parts = sections(output);
    let tag: &dyn std::fmt::Display = &job.tag;
    let list = |remotes: &[String]| remote_list_in(t, remotes);
    if ok {
        let lacked = |i: usize| parts.get(i + 1).is_some_and(|s| s.iter().any(|l| l.contains("non-existent ref")));
        let (mut done, mut none): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
        for (i, r) in job.remotes.iter().enumerate() {
            if lacked(i) { none.push(r.clone()) } else { done.push(r.clone()) }
        }
        let mut text = if done.is_empty() {
            t.trf("tag.delete.done_here", &[("tag", tag)])
        } else {
            t.trf("tag.delete.done_remotes", &[("tag", tag), ("remotes", &list(&done))])
        };
        if !none.is_empty() {
            text.push(' ');
            text.push_str(&t.trf("tag.delete.had_none", &[("tag", tag), ("remotes", &list(&none))]));
        }
        return Some(text);
    }
    let why = failure_reason(output);
    match parts.len() {
        0 => None,
        1 => Some(t.trf("tag.delete.failed_local", &[("tag", tag), ("why", &why)])),
        n => {
            let at = (n - 2).min(job.remotes.len().saturating_sub(1));
            let failed = job.remotes.get(at).map(String::as_str).unwrap_or_default();
            let mut text = t.trf("tag.delete.failed_remote", &[("tag", tag), ("remote", &failed), ("why", &why)]);
            let done = job.remotes.get(..at).unwrap_or_default();
            if !done.is_empty() {
                text.push(' ');
                text.push_str(&t.trf("tag.delete.deleted_from", &[("remotes", &list(done))]));
            }
            let rest = job.remotes.get(at + 1..).unwrap_or_default();
            if !rest.is_empty() {
                text.push(' ');
                text.push_str(&t.trf("tag.delete.not_tried", &[("remotes", &list(rest))]));
            }
            Some(text)
        }
    }
}
