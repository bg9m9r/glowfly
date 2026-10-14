//! Messages of the TAGS section's remote actions (state/tags.rs, update/tags.rs, ui/tags.rs).

#[derive(Clone, Debug, PartialEq)]
pub enum TagMsg {
    /// Push `tag` to each of `remotes` in one job (one remote from "Push to X", every remote
    /// from "Push to all remotes").
    Push { tag: String, remotes: Vec<String> },
    /// Confirmed: delete `tag` from each of `remotes` in one job (git push <r> --delete
    /// refs/tags/<tag>).
    DeleteRemote { tag: String, remotes: Vec<String> },
}
