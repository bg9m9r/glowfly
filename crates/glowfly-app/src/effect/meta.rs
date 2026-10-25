//! What the runner needs to know about an effect before it runs, decided by each effect area
//! beside its type (`EffectMeta`), so a new variant is placed in that area's file only.
//! `Effect`'s own `name`, `scope`, `timed`, `queue`, `holds` and `stop_key` delegate to it
//! (`effect_match!` in areas.rs).

use super::partial::StopKey;
use std::path::{Path, PathBuf};

/// One effect area's metadata. `name`, `scope`, `timed`, `queue` and `sandbox` have no default on
/// purpose: each area decides them for every variant with an exhaustive match (no `_` arm), or
/// one answer for the whole area, so a new variant has to be placed.
pub trait EffectMeta {
    /// A short name for the worker thread (`glowfly-<name>`), the debug log and the sandbox's
    /// list of dropped effects.
    fn name(&self) -> &'static str;

    /// The repository whose tab the result belongs to (sent back as `Msg::Scoped`): results for
    /// a tab that is no longer active are dropped. None for global answers, jobs whose result
    /// must reach `update` whichever tab is active, and work outside any repository.
    fn scope(&self) -> Option<&PathBuf>;

    /// Whether the runner times it for the performance log; false when it waits on the user or
    /// the system.
    fn timed(&self) -> bool;

    /// The repository whose per-repository queue it runs on, when it can change that repository
    /// (index, working tree, refs), so it never runs at the same time as another change there.
    fn queue(&self) -> Option<&Path>;

    /// The repository it keeps out of the repository pool while it runs (`repo_pool::hold`):
    /// one it can change, and a fetch's too, after which git may repack.
    fn holds(&self) -> Option<&Path> {
        self.queue()
    }

    /// The key a runner registers its stop under (`Effect::StopGit`); None when it can't be
    /// stopped.
    fn stop_key(&self) -> Option<StopKey> {
        None
    }

    /// What the whole-app tests' runner does with it (runner/sandbox.rs).
    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule;
}

/// What the whole-app tests' runner does with an effect.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SandboxRule {
    /// Runs as in the app: repository and file work inside the test's folders.
    Run,
    /// A save of Glowfly's own files, written into the test's folder instead.
    Save,
    /// Reaches the network, the OS shell, the desktop, a file picker, the credential store or
    /// the global git config: dropped and recorded by name.
    Drop,
}

#[cfg(test)]
#[path = "meta_tests.rs"]
mod tests;
