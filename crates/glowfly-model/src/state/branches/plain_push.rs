//! Where a plain push of a branch goes (`BranchInfo::push`, worked out by glowfly-core's
//! `push_target`), why git refuses it, and the tooltip notes on it (a configured push remote
//! that is missing, matching and mirror pushes). Only branches with a remote upstream whose
//! ref exists use it; the others keep the explicit pushes to the default remote.

use super::upstream_label;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, PushDest, PushRefusal, PushRule, PushTarget};

/// `b`'s plain push target, for a branch whose upstream is on a remote and still exists.
pub fn resolved(b: &BranchInfo) -> Option<&PushTarget> {
    b.push.as_ref().filter(|_| super::remote_tracking(b).is_some() && b.upstream.is_some())
}

/// Where the plain push goes, when git accepts it.
pub fn dest(b: &BranchInfo) -> Option<(&PushTarget, &PushDest)> {
    resolved(b).and_then(|p| Some((p, p.dest.as_ref().ok()?)))
}

/// Whether `d` is `b`'s upstream itself.
pub fn to_upstream(b: &BranchInfo, d: &PushDest) -> bool {
    d.tracking_ref.is_some() && d.tracking_ref == b.upstream_ref
}

/// The (ahead, behind) counts against `d`: the branch's own when `d` is its upstream (the
/// counts the rest of the app shows), else the push target's.
pub fn counts(b: &BranchInfo, d: &PushDest) -> (usize, usize) {
    if to_upstream(b, d) { (b.ahead, b.behind) } else { (d.ahead, d.behind) }
}

/// Why git refuses a plain push of `b`, in `t`; None when it doesn't (or `b` isn't resolved).
pub fn refused(t: Lang, b: &BranchInfo) -> Option<String> {
    let p = resolved(b)?;
    p.dest.as_ref().err().map(|r| refusal_text(t, b, p, r))
}

/// Whether git refuses the plain push of `b`.
pub fn refused_any(b: &BranchInfo) -> bool {
    resolved(b).is_some_and(|p| p.dest.is_err())
}

fn refusal_text(t: Lang, b: &BranchInfo, p: &PushTarget, r: &PushRefusal) -> String {
    let upstream = upstream_label(b).unwrap_or_default();
    let key = p.key.as_deref().unwrap_or_default();
    let mut args: Vec<(&str, &dyn std::fmt::Display)> =
        vec![("branch", &b.name), ("remote", &p.remote), ("key", &key), ("upstream", &upstream)];
    match r {
        PushRefusal::Nothing => t.trf("branch_push.plain.nothing", &args),
        PushRefusal::Invalid(v) => {
            args.push(("value", v));
            t.trf("branch_push.plain.invalid", &args)
        }
        PushRefusal::NotUpstreamRemote { upstream_remote } => {
            args.push(("upstream_remote", upstream_remote));
            t.trf("branch_push.plain.not_upstream_remote", &args)
        }
        PushRefusal::NameDiffers { merge } => {
            args.push(("merge", merge));
            t.trf("branch_push.plain.name_differs", &args)
        }
        PushRefusal::NotOnRemote => t.trf("branch_push.plain.not_on_remote", &args),
        PushRefusal::NotInRefspecs => t.trf("branch_push.plain.not_in_refspecs", &args),
        PushRefusal::NoRemote => t.trf("branch_push.plain.no_remote", &args),
    }
}

/// Why pushing `b` to `d` (not its upstream) would be refused or do nothing, as last fetched:
/// it is behind there, or level. None when the push creates the ref or has commits to send.
pub fn blocked_elsewhere(t: Lang, b: &BranchInfo, p: &PushTarget, d: &PushDest) -> Option<String> {
    d.tip?;
    let label = p.label(d);
    let args: &[(&str, &dyn std::fmt::Display)] = &[("branch", &b.name), ("dest", &label)];
    let (ahead, behind) = counts(b, d);
    if behind > 0 {
        return Some(t.trf("branch_push.plain.behind", args));
    }
    (ahead == 0).then(|| t.trf("branch_push.plain.up_to_date", args))
}

/// The tooltip's note on `b`'s plain push: a configured push remote that isn't a remote here
/// (so Push goes to the upstream), or a matching or mirror push, which sends more than `b`.
pub fn note(t: Lang, b: &BranchInfo) -> Option<String> {
    let p = resolved(b)?;
    if let Some((key, value)) = &p.missing {
        let dest = p.dest.as_ref().map(|d| p.label(d)).unwrap_or_else(|_| upstream_label(b).unwrap_or_default());
        return Some(t.trf("toolbar.tip.push_missing_remote", &[("key", key), ("remote", value), ("dest", &dest)]));
    }
    match p.dest.as_ref().ok()?.rule {
        PushRule::Matching => Some(t.trf("toolbar.tip.push_matching", &[("remote", &p.remote)])),
        PushRule::Mirror => Some(t.trf("toolbar.tip.push_mirror", &[("remote", &p.remote)])),
        PushRule::Default | PushRule::Refspec => None,
    }
}

#[cfg(test)]
#[path = "plain_push_tests.rs"]
mod tests;
