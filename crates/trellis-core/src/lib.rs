//! Trellis's core: repository access (libgit2 + git CLI), commit-graph layout, hosting-service
//! integrations, settings and formatting helpers. Nothing here depends on the UI.

pub mod avatars;
pub mod conflict_rows;
pub mod dates;
pub mod diff_rows;
pub mod error;
pub mod forge;
pub mod gitcli;
pub mod graph;
pub mod highlight;
pub mod intraline;
pub mod repo;
pub mod settings;
pub mod signing;
pub mod spell;
#[cfg(test)]
pub(crate) mod testutil;
pub mod undo;
