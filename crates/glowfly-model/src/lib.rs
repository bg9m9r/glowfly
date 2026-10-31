//! Glowfly's model: the state, the messages, the effects and `update`, with no window and no
//! I/O of its own.
//!
//! Layers (see docs/ARCHITECTURE.md): `state` (plain data) ← `update` (pure state changes,
//! returns effects) → the runner (glowfly-runner, executes effects on worker threads) → `msg`
//! results back into `update`; the app's views draw the state and turn input into messages.

pub mod conflict_check;
pub mod custom_commands;
pub mod deep_link;
pub mod effect;
pub mod file_op;
pub mod git_env;
pub mod gitflow;
pub mod journal;
pub mod keymap;
pub mod lfs;
pub mod logs;
pub mod msg;
pub mod onboarding;
pub mod patch;
pub mod profile;
pub mod pull_request;
pub mod range_diff;
pub mod remote;
pub mod ssh;
pub mod start_arg;
pub mod state;
pub mod submodule;
pub mod tag;
pub mod terminal;
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod testing;
pub mod themes;
pub mod tools;
pub mod update;
pub mod worktree;
