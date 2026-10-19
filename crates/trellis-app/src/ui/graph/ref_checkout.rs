//! What a double-click on a graph row's ref chip checks out: a local branch itself, a remote
//! branch through its tracking local branch (made when there is none). A chip folding copies on
//! remotes checks out through its local branch, or for remote-only copies through the lead copy
//! (the tracked one, else the default remote's). Tags, a detached HEAD and labels of unknown
//! remotes check out nothing; the double-click stays a row click.

use super::ref_group::Chip;
use crate::msg::{BranchMsg, Msg};
use crate::state::remote_branch;
use trellis_core::repo::{RefKind, Snapshot};

/// The message a double-click on `chip` sends, if it checks anything out.
pub(super) fn double_click(chip: &Chip, snap: &Snapshot) -> Option<Msg> {
    let l = chip.label;
    match l.kind {
        RefKind::Local => Some(Msg::Branch(BranchMsg::Checkout(l.name.clone()))),
        RefKind::Remote => remote_branch::split(snap, &l.name)
            .map(|(remote, name)| Msg::Branch(BranchMsg::CheckoutRemote { remote: remote.to_owned(), name: name.to_owned() })),
        RefKind::Tag | RefKind::Head => None,
    }
}
