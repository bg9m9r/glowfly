//! The command palette's tests: its command list, keys and groups, and each command's steps
//! (sub-lists, free-text steps) as they run through the runner. `palette_tests` and `stash_tests`
//! have helpers other areas' tests use too.

mod blame_tests;
mod checkout_tests;
mod compare_tests;
mod conflict_tests;
mod folder_tests;
mod groups_tests;
mod issue_tests;
mod keys_tests;
mod more_tests;
mod nav_tests;
pub(super) mod palette_tests;
mod pick_tests;
mod rebase_tests;
mod recent_tests;
mod repos_tests;
pub(super) mod stash_tests;
mod submodule_tests;
mod switch_branch_tests;
mod worktree_tests;
