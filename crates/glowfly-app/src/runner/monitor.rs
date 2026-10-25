//! The slow-status note's job for git's built-in file system monitor (`Effect::TurnOnMonitor`):
//! check, start, confirm and set it (`repo::turn_on_monitor`), then have git write its token (and
//! the untracked cache, when that was offered too) into the index.

use super::execute::{Cx, Execute};
use super::git_ops;
use crate::effect::MonitorJob;
use crate::msg::{LoadMsg, Msg};
use glowfly_core::repo;
use std::path::Path;

impl Execute for MonitorJob {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(turn_on(&self.cwd, &self.then))
    }
}

/// Turns the monitor on in `cwd`, then runs `then` (the untracked cache's commands) or, once the
/// monitor is watching, the status read that records its token.
pub(super) fn turn_on(cwd: &Path, then: &[Vec<String>]) -> Msg {
    let (monitor, mut output) = repo::turn_on_monitor(cwd);
    let after = match (then.is_empty(), monitor.is_watching()) {
        (false, _) => then.to_vec(),
        (true, true) => vec![repo::monitor_record_cmd()],
        (true, false) => Vec::new(),
    };
    let mut cmds_ok = true;
    if !after.is_empty() {
        let (ok, text) = git_ops::run_git(cwd, &after, None, false);
        cmds_ok = ok;
        if !text.is_empty() {
            output.push('\n');
            output.push_str(&text);
        }
    }
    let cache = (!then.is_empty()).then_some(cmds_ok);
    Msg::Load(LoadMsg::StatusSpedUp { output, monitor, cache })
}
