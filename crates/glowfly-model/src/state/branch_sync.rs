//! Keeping a branch's copy on its remote in step (update/branch_sync.rs): the Rename dialog's
//! form and whether it may rename the branch on its remote too (`remote_rename`: refused for the
//! remote's default branch, a Git Flow base, a branch behind its upstream and a name the remote
//! already has), the running rename's headline by the step it reached (`RenameJob`), and the
//! Delete remote branch dialog (`DeleteRemoteForm`, drawn by `ui/remote_branch_delete_dialog.rs`)
//! with the local branches that track the branch, whose upstreams it offers to unset.

use crate::msg::{BranchMsg, Msg};
use crate::state::touched::Touched;
use crate::state::{gitflow_base, remote_branch};
use git2::Oid;
use glowfly_core::gitflow::FlowBase;
use glowfly_core::i18n::Lang;
use glowfly_core::names::{RefClash, ref_clash};
use glowfly_core::repo::{BranchInfo, Snapshot};

/// The Rename dialog (`Modal::RenameBranch`): the branch, the name typed and whether to rename
/// it on its remote too.
#[derive(Clone, Debug, PartialEq)]
pub struct RenameForm {
    pub old: String,
    pub name: String,
    pub touched: Touched,
    /// Also on the remote: push the new name, then delete the old one there.
    pub remote: bool,
}

impl RenameForm {
    /// The dialog for local branch `old`, its name filled in and only the local branch renamed.
    pub fn new(old: String) -> Self {
        RenameForm { name: old.clone(), old, touched: Touched::default(), remote: false }
    }
}

/// Renaming a branch's upstream along with it: `old` on `remote` (at `tip`, as last fetched)
/// goes, and the new name is pushed there.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteRename {
    pub remote: String,
    pub old: String,
    pub tip: Oid,
}

impl RemoteRename {
    /// "remote/old".
    pub fn upstream(&self) -> String {
        format!("{}/{}", self.remote, self.old)
    }
}

/// Whether renaming local branch `b` to `new` may rename its upstream too: None when it has no
/// upstream on a remote (nothing to offer), else the plan, or why it is refused. The remote's
/// default branch and Git Flow's bases are never renamed there (others build on them), a
/// branch behind its upstream would delete the commits it lacks, and a name the remote already
/// has (or a folder of it) would be pushed onto someone else's branch.
pub fn remote_rename(t: Lang, snap: &Snapshot, b: &BranchInfo, new: &str) -> Option<Result<RemoteRename, String>> {
    let tracking = b.tracking.as_ref().filter(|x| x.is_remote())?;
    let (remote, old) = (tracking.remote.as_str(), tracking.merge.as_str());
    let upstream = format!("{remote}/{old}");
    let Some(tip) = remote_branch::exists(snap, remote, old) else {
        return Some(Err(t.trf("branch_sync.rename.gone", &[("upstream", &upstream)])));
    };
    if is_default(snap, remote, old) {
        return Some(Err(t.trf("branch_sync.rename.default", &[("branch", &old), ("remote", &remote)])));
    }
    for name in [b.name.as_str(), old] {
        match snap.gitflow.base_role(name) {
            Some(FlowBase::Production) => return Some(Err(t.trf("branch_sync.rename.production", &[("branch", &name)]))),
            Some(FlowBase::Development) => return Some(Err(t.trf("branch_sync.rename.development", &[("branch", &name)]))),
            None => {}
        }
    }
    if b.behind > 0 {
        let args: [(&str, &dyn std::fmt::Display); 2] = [("upstream", &upstream), ("branch", &b.name)];
        return Some(Err(t.trn("branch_sync.rename.behind", b.behind, &args)));
    }
    let names = snap.remotes.iter().find(|(r, _)| r == remote).map(|(_, l)| l.as_slice()).unwrap_or_default();
    if !new.is_empty() && new != b.name {
        let clash = ref_clash(new, names.iter().map(|(n, _)| n.as_str()));
        let other = |o: &str| format!("{remote}/{o}");
        match clash {
            Some(RefClash::Exists(o)) => return Some(Err(t.trf("branch_sync.rename.taken", &[("other", &other(&o))]))),
            Some(RefClash::Clashes(o) | RefClash::FolderCase { existing: o, .. }) => {
                return Some(Err(t.trf("branch_sync.rename.clash", &[("name", &new), ("other", &other(&o))])));
            }
            None => {}
        }
    }
    Some(Ok(RemoteRename { remote: remote.to_owned(), old: old.to_owned(), tip }))
}

/// Whether `name` is `remote`'s default branch: its recorded HEAD, or without one the main
/// branch Glowfly picks (`Snapshot::mainline`).
pub(crate) fn is_default(snap: &Snapshot, remote: &str, name: &str) -> bool {
    match snap.remote_heads.iter().find(|(r, _)| r == remote) {
        Some((_, head)) => head == name,
        None => snap.mainline().as_deref() == Some(name),
    }
}

/// A running rename that also moves the branch on its remote, for its headline.
#[derive(Clone, Debug, PartialEq)]
pub struct RenameJob {
    pub label: String,
    pub old: String,
    pub new: String,
    pub plan: RemoteRename,
}

impl RenameJob {
    /// What the job did, by the step it stopped at when it failed (0: the rename, 1: the push,
    /// 2: the delete), with `why` it failed. None when the rename itself failed: nothing else
    /// ran, and git's failure explains it.
    pub fn headline(&self, t: Lang, ok: bool, step: usize, why: &str) -> Option<String> {
        let (old, new, remote, upstream) = (&self.old, &self.new, &self.plan.remote, self.plan.upstream());
        let args: [(&str, &dyn std::fmt::Display); 5] =
            [("old", old), ("new", new), ("remote", remote), ("upstream", &upstream), ("why", &why)];
        match (ok, step) {
            (true, _) => Some(t.trf("branch_sync.rename.done", &args)),
            (false, 0) => None,
            (false, 1) => Some(t.trf("branch_sync.rename.push_failed", &args)),
            (false, _) => Some(t.trf("branch_sync.rename.delete_failed", &args)),
        }
    }
}

/// The Delete remote branch dialog (`Modal::DeleteRemoteBranch`): branch `name` on `remote`,
/// the local branches that track it and whether to unset their upstreams, and its Git Flow role.
#[derive(Clone, Debug, PartialEq)]
pub struct DeleteRemoteForm {
    pub remote: String,
    pub name: String,
    pub tracking: Vec<String>,
    pub unset: bool,
    pub role: Option<FlowBase>,
}

impl DeleteRemoteForm {
    /// The dialog for `remote`/`name`, offering to unset every local branch's upstream on it.
    pub fn new(snap: &Snapshot, remote: &str, name: &str) -> Self {
        let tracking = snap.locals.iter().filter(|b| remote_branch::tracks(b, remote, name)).map(|b| b.name.clone()).collect();
        let role = snap.gitflow.base_role(name);
        DeleteRemoteForm { remote: remote.to_owned(), name: name.to_owned(), tracking, unset: true, role }
    }

    pub fn title(&self, t: Lang) -> &'static str {
        self.role.map_or_else(|| t.tr("remote_branch.delete.title"), |r| gitflow_base::title(t, r))
    }

    pub fn body(&self, t: Lang) -> String {
        t.trf("remote_branch.delete.body", &[("branch", &self.name), ("remote", &self.remote)])
    }

    /// The warning that local branches track it, naming them; None when none does.
    pub fn tracking_note(&self, t: Lang) -> Option<String> {
        let (branches, upstream) = (self.tracking.join(", "), format!("{}/{}", self.remote, self.name));
        let args: [(&str, &dyn std::fmt::Display); 2] = [("branches", &branches), ("upstream", &upstream)];
        (!self.tracking.is_empty()).then(|| t.trn("remote_branch.delete.tracked", self.tracking.len(), &args))
    }

    /// The Unset upstream checkbox's text.
    pub fn unset_label(&self, t: Lang) -> String {
        let branches = self.tracking.join(", ");
        t.trn("remote_branch.delete.unset.check", self.tracking.len(), &[("branches", &branches)])
    }

    /// Git Flow's warning for a base branch, with what deleting it on the remote means.
    pub fn role_warning(&self, t: Lang) -> Option<String> {
        let role = self.role?;
        let remote = t.trf("gitflow.delete_base.remote.warn", &[("remote", &self.remote)]);
        Some(format!("{} {remote}", gitflow_base::warning(t, role, &self.name)))
    }

    pub fn button(&self, t: Lang) -> &'static str {
        if self.role.is_some() { t.tr("gitflow.delete_base.delete.button") } else { t.tr("common.delete.button") }
    }

    /// What the danger button sends: the delete, and the upstreams to unset after it.
    pub fn delete_msg(&self) -> Msg {
        let unset = if self.unset { self.tracking.clone() } else { Vec::new() };
        Msg::Branch(BranchMsg::DeleteRemote { remote: self.remote.clone(), name: self.name.clone(), unset })
    }
}
