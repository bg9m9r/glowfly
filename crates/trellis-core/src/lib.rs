//! Trellis's core: repository access (libgit2 + git CLI), commit-graph layout, hosting-service
//! integrations, settings and formatting helpers. Nothing here depends on the UI.

pub mod askpass;
pub mod avatars;
pub mod branch_ops;
pub mod clone_ops;
pub mod conflict_rows;
pub mod dates;
pub mod diff_rows;
pub mod error;
pub mod forge;
pub mod fuzzy;
pub mod gitcli;
pub mod gitflow;
pub mod graph;
pub mod highlight;
pub mod intraline;
pub mod lfs;
pub mod line_pairs;
pub mod matching;
pub mod persist;
pub mod rebase;
pub mod remote_ops;
pub mod repo;
pub mod settings;
pub mod shallow_ops;
pub mod signature_check;
pub mod signing;
pub mod sparse_ops;
pub mod spell;
pub mod ssh_keys;
pub mod stash_ops;
pub mod tabs;
pub mod tag_ops;
pub mod terminal;
#[cfg(test)]
pub(crate) mod testutil;
pub mod text;
pub mod trust;
pub mod undo;
pub mod watch;
pub mod whole_file;
pub mod worktree_ops;
