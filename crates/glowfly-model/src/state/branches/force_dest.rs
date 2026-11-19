//! Where a force push with lease of a branch goes: the one rule the toolbar's Force push, a
//! branch row's, the rejected push toast's and each layer of Push stack share. A branch whose
//! plain push resolves (`plain_push`) force pushes to where that plain push goes, which config
//! may make another remote than its upstream's (`remote.pushDefault`, `branch.<b>.pushRemote`);
//! git refusing the plain push, or no remote-tracking ref to lease against, refuses the force
//! push too. A branch whose plain push isn't resolved keeps the explicit target (its upstream,
//! or for the toolbar the default remote). Tests: update/tests/force_push_tests.rs.

use super::{plain_push, remote_tracking};
use crate::state::{branches, remote_branch};
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, Snapshot};
use glowfly_core::text::redact_credentials;

/// The ref a force push replaces, and what the lease expects there.
#[derive(Clone, Debug, PartialEq)]
pub struct ForceDest {
    /// A remote's name, or "." for this repository.
    pub remote: String,
    /// The full ref replaced there (`refs/heads/main`).
    pub dest_ref: String,
    /// Its tip as the snapshot last saw it: the lease (None: not there).
    pub seen: Option<Oid>,
    /// Commits there that the branch lacks (0 without a tip).
    pub behind: usize,
    /// "fork/main".
    pub label: String,
    /// Whether the push adds -u: only a branch tracking nothing, published.
    pub set_upstream: bool,
    /// Whether it is the branch's own upstream (or where Publish sends it).
    pub upstream: bool,
}

impl ForceDest {
    /// The branch replaced, when `dest_ref` is one.
    pub fn branch(&self) -> Option<&str> {
        self.dest_ref.strip_prefix("refs/heads/")
    }

    /// Whether this is `dest` (a full ref, or a branch's name) on `remote`.
    pub fn is(&self, remote: &str, dest: &str) -> bool {
        self.remote == remote && self.dest_ref == full_ref(dest)
    }
}

/// `dest` as a full ref: a name without `refs/` is a branch.
fn full_ref(dest: &str) -> String {
    if dest.starts_with("refs/") { dest.to_owned() } else { format!("refs/heads/{dest}") }
}

/// `dest` on `remote` for a message: "fork/main" for a branch, "main" on this repository,
/// "fork refs/for/main" for another ref.
pub fn label(remote: &str, dest: &str) -> String {
    let full = full_ref(dest);
    match (full.strip_prefix("refs/heads/"), remote) {
        (Some(b), ".") => b.to_owned(),
        (Some(b), r) => format!("{r}/{b}"),
        (None, r) => format!("{r} {full}"),
    }
}

/// The destination of `b`'s plain push, or why it can't be force pushed; None when the plain
/// push isn't resolved.
fn by_plain_push(t: Lang, b: &BranchInfo) -> Option<Result<ForceDest, String>> {
    let p = plain_push::resolved(b)?;
    let d = match &p.dest {
        Err(_) => return Some(Err(plain_push::refused(t, b).unwrap_or_default())),
        Ok(d) => d,
    };
    if d.tracking_ref.is_none() {
        let dest = redact_credentials(&p.label(d));
        return Some(Err(t.trf("branch_push.force.no_lease", &[("branch", &b.name), ("dest", &dest)])));
    }
    Some(Ok(ForceDest {
        remote: p.remote.clone(),
        dest_ref: d.dest_ref.clone(),
        seen: d.tip,
        behind: plain_push::counts(b, d).1,
        label: p.label(d),
        set_upstream: false,
        upstream: plain_push::to_upstream(b, d),
    }))
}

/// `dest` on `remote` as an explicit push sends it.
fn explicit(snap: &Snapshot, b: &BranchInfo, remote: &str, dest: &str, set_upstream: bool) -> ForceDest {
    let tracks = remote_tracking(b).is_some_and(|t| t.remote == remote && t.merge == dest);
    ForceDest {
        remote: remote.to_owned(),
        dest_ref: format!("refs/heads/{dest}"),
        seen: remote_branch::exists(snap, remote, dest),
        behind: if tracks { b.behind } else { 0 },
        label: format!("{remote}/{dest}"),
        set_upstream,
        upstream: true,
    }
}

/// Where a row's or toast's force push of `b` goes, or why it is refused: its plain push's
/// destination, else its upstream (which needs a remote that exists).
pub fn of(t: Lang, snap: &Snapshot, b: &BranchInfo) -> Result<ForceDest, String> {
    if let Some(found) = by_plain_push(t, b) {
        return found;
    }
    let Some(tr) = remote_tracking(b) else {
        return Err(t.trf("branch_push.force.no_upstream", &[("branch", &b.name)]));
    };
    if !snap.remotes.iter().any(|(r, _)| *r == tr.remote) {
        return Err(t.trf("branch_push.force.no_remote", &[("remote", &tr.remote)]));
    }
    Ok(explicit(snap, b, &tr.remote, &tr.merge, false))
}

/// Where the toolbar's force push of the checked-out `b` goes: as `of`, but a branch with no
/// resolved plain push goes to its upstream or else the default remote (with -u for a branch
/// tracking nothing). Ok(None): there is no remote, so Publish opens.
pub fn of_head(t: Lang, snap: &Snapshot, b: &BranchInfo) -> Result<Option<ForceDest>, String> {
    if let Some(found) = by_plain_push(t, b) {
        return found.map(Some);
    }
    Ok(branches::push_target(b, snap, None).map(|(remote, dest, set_upstream)| explicit(snap, b, &remote, &dest, set_upstream)))
}
