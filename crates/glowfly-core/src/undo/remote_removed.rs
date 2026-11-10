//! A removed remote: recorded with its whole config section and the branch settings that named
//! it, put back value by value on undo (its remote-tracking branches come back with a fetch,
//! which the app offers), removed again on redo while its section is as it was put back.

use super::planning::stale;
use super::remote_config::{RemoteConfig, read, restore_cmds};
use super::{Action, Plan};
use crate::error::Result;
use git2::Repository;

/// The removal of remote `name`, when it had a section before (`before`) and has none now.
pub(super) fn after(repo: &Repository, name: String, before: Option<RemoteConfig>) -> Result<Option<Action>> {
    let Some(config) = before else { return Ok(None) };
    Ok(read(repo, &name)?.is_none().then_some(Action::RemoveRemote { name, config }))
}

/// Writes remote `name`'s recorded section back while no remote of that name is configured, or
/// (redoing) removes it again while its section is the one put back.
pub(super) fn plan(repo: &Repository, name: &str, config: &RemoteConfig, redo: bool) -> Result<Plan> {
    let now = read(repo, name)?;
    if !redo {
        return Ok(match now {
            Some(_) => stale(format!("A remote named {name} exists again")),
            None => Plan::Run(restore_cmds(name, config, repo)?),
        });
    }
    Ok(match now {
        None => stale(format!("Remote {name} no longer exists")),
        Some(c) if c.entries != config.entries => stale(format!("Remote {name} has changed since it was put back")),
        Some(_) => Plan::Run(vec![crate::remote_ops::remove_cmd(name)]),
    })
}
