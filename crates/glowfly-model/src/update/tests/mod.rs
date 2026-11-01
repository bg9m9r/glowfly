//! The update tests, one module per file in this folder; the shared helpers are in helpers.rs.
//! The glob import keeps the `super::…` paths the test files use (`super::update`, `super::tests`,
//! the handlers) working; a file declared from its handler (with `#[path]`) is not listed here.

#[allow(unused_imports)]
use super::*;

pub(in crate::update) use crate::testing::*;

pub(super) mod avatars_tests;
pub(super) mod diff_lines_tests;
pub(super) mod diff_pairs_tests;
pub(super) mod editor_tests;
pub(super) mod git_program_tests;
pub(super) mod graph_columns_tests;
pub(super) mod keys_tests;
pub(super) mod layout_tests;
pub(super) mod layout_window_tests;
pub(super) mod onboarding_tests;
pub(super) mod op_steps_tests;
pub(super) mod palette_profile_tests;
pub(super) mod palette_theme_tests;
pub(super) mod pr_details_tests;
pub(super) mod pr_heads_tests;
pub(super) mod prefs_signing_tests;
pub(super) mod repo_colour_tests;
pub(super) mod repo_groups_tests;
pub(super) mod terminal_schemes_tests;
