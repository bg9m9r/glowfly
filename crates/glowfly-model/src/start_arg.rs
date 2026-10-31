//! What a new window is asked to open, from the command line or handed over by the OS: a
//! repository folder or a `glowfly://` link, and the message that opens it. `main` reads it from
//! the command line and hands it to a window already running when there is one.

use crate::deep_link::DeepLinkMsg;
use crate::msg::{Msg, RepoMsg};
use std::path::PathBuf;

/// What the command line asks the new window to open; either goes to a window already running
/// first (`main::forwarded`).
#[derive(Clone, Debug, PartialEq)]
pub enum StartArg {
    /// A repository folder, made absolute.
    Path(PathBuf),
    /// A `glowfly://` link.
    Link(String),
}

impl StartArg {
    /// The message that opens it: the same message for the command line and for what the OS
    /// hands over.
    pub fn into_msg(self) -> Msg {
        match self {
            StartArg::Path(p) => Msg::Repo(RepoMsg::Open(p)),
            StartArg::Link(t) => Msg::DeepLink(DeepLinkMsg::Open(t)),
        }
    }

    /// What a second process hands to the running window for it.
    pub fn handed(&self) -> glowfly_core::instance::Handed {
        match self {
            StartArg::Path(p) => glowfly_core::instance::Handed::Folder(p.clone()),
            StartArg::Link(t) => glowfly_core::instance::Handed::Link(t.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_arguments_become_the_open_messages() {
        let p = glowfly_core::paths::temp_dir().join("glowfly-start-msg");
        assert!(matches!(StartArg::Path(p.clone()).into_msg(), Msg::Repo(RepoMsg::Open(q)) if q == p));
        let t = "glowfly://tag/v1?url=x".to_owned();
        assert!(matches!(StartArg::Link(t.clone()).into_msg(), Msg::DeepLink(DeepLinkMsg::Open(u)) if u == t));
    }
}
