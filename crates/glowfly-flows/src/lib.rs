//! The end-to-end tests of `update`: they run effects through the runner (glowfly-runner's
//! `testing`), and the tests that share helpers with them. A crate of their own so the model's
//! tests never need the runner, and so they build and run without the app's window, views and
//! wgpu. Test code only: without `cfg(test)` this crate is empty. The module paths match the
//! app's (`crate::state::…`, `crate::runner::…`), where the tests lived before.

#[cfg(test)]
use glowfly_model::{
    conflict_check, custom_commands, deep_link, effect, git_env, gitflow, journal, keymap, lfs, logs, msg, onboarding, patch, profile,
    pull_request, range_diff, remote, ssh, start_arg, state, submodule, tag, terminal, testing, themes, tools, update, worktree,
};
#[cfg(test)]
use glowfly_runner as runner;

#[cfg(test)]
mod flows;
