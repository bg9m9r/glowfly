//! The view snapshot tests whose state comes through the runner (an effect run with
//! `runner::testing`, or a diff decorated the way the runner decorates it), so they live with the
//! runner in the app crate; glowfly-ui's own view tests never use it. They share glowfly-ui's
//! fixture repository and renderer (`snapshot_fixtures`, from its `test-support` feature) and
//! compare against its approved snapshots.

use glowfly_ui::snapshot_fixtures as snapshot_tests;

mod snapshot_tests_conflict_files;
mod snapshot_tests_search;
mod snapshot_tests_whole;
mod snapshot_tests_wrap;
