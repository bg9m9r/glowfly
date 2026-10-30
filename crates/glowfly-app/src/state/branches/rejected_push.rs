//! The local branch a push that git rejected as behind was for (`rejected_local`), for the
//! failure toast's Force push with lease. It is read from the job's own push command and git's
//! `! [rejected] <src> -> <dst>` line, and found only when the source is a local branch, the
//! push went to its upstream's remote and `<dst>` is its upstream's name: a push to another
//! remote, or of another branch to the same name, offers no force push.

use crate::state::branches;
use glowfly_core::repo::{BranchInfo, Snapshot};

/// The short name of `r` (`refs/heads/x` is `x`).
fn short(r: &str) -> &str {
    r.strip_prefix("refs/heads/").unwrap_or(r)
}

/// git's rejections of a push as behind ("fetch first", "non-fast-forward") in `output`, as
/// (source, destination) short names.
fn behind_rejections(output: &str) -> Vec<(&str, &str)> {
    output.lines().filter_map(behind_rejection).collect()
}

/// One line's rejection as behind, as (source, destination).
fn behind_rejection(l: &str) -> Option<(&str, &str)> {
    let rest = l.trim().strip_prefix("! [rejected]")?;
    let (refs, reason) = rest.split_once('(')?;
    let reason = reason.trim_end().strip_suffix(')')?;
    if !matches!(reason, "fetch first" | "non-fast-forward") {
        return None;
    }
    let (src, dst) = refs.split_once(" -> ")?;
    Some((short(src.trim()), short(dst.trim())))
}

/// The one remote a push goes to at the URL of the last `To <url>` line before git's first
/// rejection in `output`: matched on each remote's push URLs (its `pushurl`s, or else its
/// `url`s, where git sends a push). None when no remote, or more than one, pushes there.
fn printed_remote<'a>(snap: &'a Snapshot, output: &str) -> Option<&'a str> {
    let before = output.lines().take_while(|l| behind_rejection(l).is_none());
    let url = before.filter_map(|l| l.strip_prefix("To ")).last()?.trim().trim_end_matches('/');
    let pushes_there = |r: &&glowfly_core::repo::RemoteInfo| {
        let urls = if r.push_urls.is_empty() { &r.urls } else { &r.push_urls };
        urls.iter().any(|u| u.trim_end_matches('/') == url)
    };
    let mut found = snap.remote_info.iter().filter(pushes_there);
    match (found.next(), found.next()) {
        (Some(r), None) => Some(r.name.as_str()),
        _ => None,
    }
}

/// The local branch the failed push job `cmd` (its one command) was for, when git rejected it
/// as behind in `output`: the rejection's source is that branch (the command's refspec agrees),
/// the push went to its upstream's remote (named in the command, or else by the `To` line) and
/// the rejected destination is its upstream's name. None otherwise.
pub fn rejected_local<'a>(snap: &'a Snapshot, cmd: &[String], output: &str) -> Option<&'a BranchInfo> {
    if glowfly_core::gitcli::subcommand(cmd) != Some("push") {
        return None;
    }
    let at = cmd.iter().position(|a| a == "push")?;
    let positional: Vec<&str> = cmd[at + 1..].iter().map(String::as_str).filter(|a| !a.starts_with('-')).collect();
    // One rejection and at most one refspec, so the button names the one branch git refused.
    let rejections = behind_rejections(output);
    let ([(src, dst)], 0..=2) = (rejections.as_slice(), positional.len()) else { return None };
    if let Some(spec) = positional.get(1) {
        let from = spec.trim_start_matches('+').split(':').next().unwrap_or_default();
        if short(from) != *src {
            return None;
        }
    }
    let remote = match positional.first() {
        Some(r) => *r,
        None => printed_remote(snap, output)?,
    };
    let b = snap.locals.iter().find(|b| b.name == *src)?;
    let up = branches::remote_tracking(b)?;
    (up.remote == remote && up.merge == *dst).then_some(b)
}
