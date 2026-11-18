//! What Delete lock file and Try again run again once a lock file is in the way: a failed job's
//! spec (`Retry`, update/job_retry.rs) or a working-tree op (`LocalRetry`,
//! update/lock_local.rs).

use super::job_spec::Retry;
use super::tabs::TabId;
use crate::effect::LocalOp;
use std::path::PathBuf;

/// What runs again after a lock file was in the way.
#[derive(Clone, Debug)]
pub enum LockRetry {
    Job(Box<Retry>),
    Local(Box<LocalRetry>),
}

/// A working-tree op that failed on a lock: run again only in `tab`, while it is active and
/// still shows `workdir`.
#[derive(Clone, Debug)]
pub struct LocalRetry {
    pub tab: TabId,
    pub workdir: PathBuf,
    pub op: LocalOp,
}
