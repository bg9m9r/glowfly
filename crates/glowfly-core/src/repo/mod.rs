//! Repository model built on libgit2.
//! Reads (history, refs, status, diffs) and index/working-tree edits happen here; operations
//! that should run hooks or talk to remotes go through `gitcli`.
//!
//! The types are here (the diff ones in `diff_types`, a loaded commit and its shared author in
//! `commit_info`, `Snapshot`, `LoadOptions` and `CommitDetail` in `snapshot`, `Snapshot`'s
//! queries in `snapshot_impl`);
//! the work is split by area: `load` (history, refs, status; `walk` and `walk_order` read the
//! history, `walk_reuse` takes what an earlier load read, `load_more` goes on from where a cut
//! walk stopped, `branch_merged` which local branches HEAD contains), `branch_colours` (laying the graph out with each lane's owner and colour), `status_paths` and `status_merge`
//! (re-reading only some work-tree paths), `status_speed` (what speeds git's status up) and `fsmonitor`
//! (git's built-in file system monitor), `diff` (and `diff_view`, a file's diff for the viewer, narrowed to it by `diff_scope`; `diff_gaps`, the unchanged lines between hunks), `line_counts` (lines added and removed per
//! file of a commit or of the working tree),
//! `history` (one file's commits, following renames back and forwards: `history_forward`; a folder's: `folder_history`), `file_at` (a file's text at a commit) and `file_bytes` (its bytes, binary or not, for the image preview), `blame`, `index` and `lines` (staging, line staging), `revert`, `restore`, `conflict`, `resolve`, `resolve_undo` (resolved files and their stage ids, Mark resolved and Mark
//! conflicted), `markers` (conflict markers left in working files) and `conflict_saved` (the
//! picks a file saved with markers holds), `template`
//! (commit.template), `tool_sides` (versions for external tools) and `binary_sides` (a binary diff's
//! sizes and image previews), `touching` (commits that
//! changed a searched path), `workfile`, `workfile_mv`, `workfile_rm`, `workfile_unrm` (putting a deleted file back for undo), `noclobber` and
//! `removal` (working-tree files), `tracked`,
//! `attrs` (gitattributes, read from the source `attr_source` picks for the version shown), `submodules` (and `submodule_add`, the checks on a new submodule's
//! folder), `worktrees` (and `worktree_counts`, other worktrees' changes), `remotes` and
//! `shallow`; `pr_compare` (a pull request's source tip, files and commits), `line_map` (a file's lines mapped across commits, following renames, for pull request threads), `divergence` (the commits on each side of a branch and its upstream, from the
//! loaded graph), `unmerged` (the commits deleting a branch would leave on no branch) and `fetch_time` (when FETCH_HEAD was written). Paths: `entry`
//! (file rows and their hazards), `rpath` (git's path bytes), `exact` (index edits that match
//! one path exactly) and `collide` (paths a file system can't keep apart).

use git2::Oid;

mod attr_source;
mod attrs;
mod binary_sides;
mod blame;
mod blame_porcelain;
pub mod branch_colours;
mod branch_merged;
mod collide;
mod commit_detail;
mod commit_info;
mod common_dir;
mod conflict;
mod conflict_saved;
mod diff;
mod diff_gaps;
mod diff_scope;
mod diff_types;
mod diff_view;
mod divergence;
mod entry;
mod exact;
mod fetch_time;
mod file_at;
mod file_bytes;
mod folder_history;
mod fsmonitor;
mod graph_cache;
mod history;
mod history_forward;
mod index;
mod index_ext;
mod index_memo;
mod lfs_marks;
mod line_counts;
mod line_map;
mod lines;
mod load;
mod load_more;
mod markers;
mod merge_msg;
mod noclobber;
mod op_state;
mod pr_compare;
mod push_config;
mod refs_key;
mod remotes;
mod removal;
mod resolve;
mod resolve_undo;
mod restore;
mod revert;
mod revert_file;
mod revert_merge;
mod rpath;
mod safe_names;
mod shallow;
mod shown_ws;
mod snapshot;
mod snapshot_impl;
mod stacked;
mod status;
mod status_cli;
mod status_index;
mod status_merge;
mod status_paths;
mod status_speed;
mod submodule_add;
mod submodules;
mod template;
mod tool_sides;
mod touching;
mod tracked;
mod trash;
mod unmerged;
pub(crate) mod walk;
pub(crate) mod walk_order;
mod walk_reuse;
mod workfile;
mod workfile_disk;
mod workfile_mv;
mod workfile_rename;
mod workfile_rm;
mod workfile_unrm;
mod worktree_counts;
mod worktrees;

pub use binary_sides::{BinarySide, BinarySides, PREVIEW_MAX_BYTES, binary_sides};
pub use blame::{BLAME_MAX_BYTES, Blame, BlameCommit, BlameOptions, BlameOrigin, BlameRun, blame_file};
pub use collide::{Collision, Folding, name_key, probe};
pub use commit_detail::commit_detail;
pub use common_dir::{CommonDir, repo_name_of};
pub use conflict::{
    Chunk, ConflictContent, ConflictFile, ConflictIds, Operation, Segment, Side, SideLabels, Unmergeable, load_conflict, marker_size,
};
// The merged-text parser, for the fuzz targets (src/fuzzing/).
#[cfg(any(test, feature = "fuzzing"))]
pub(crate) use conflict::parse_merged;
pub use conflict_saved::{RESTORE_MAX, saved_picks};
pub use diff::{
    BranchChanges, Comparison, branch_changes, commit_stats, commit_stats_until, compare, compare_files, compare_from_workdir,
    compare_workdir, load_diff, load_diff_with,
};
use diff::{head_tree, path_opts, workdir_opts};
pub use diff_gaps::load_gap_text;
pub use diff_types::{DiffLine, DiffTarget, FileDiff, Hunk, HunkOp, LoadDiffOptions, empty_tree, is_empty_tree};
pub use diff_view::{ViewDiff, load_diff_for_view, load_listed_diff};
pub use divergence::Divergence;
pub use entry::{FileEntry, Hazard};
pub use file_at::{BINARY_NOT_SHOWN, FILE_VIEW_MAX_BYTES, FileAt, FileAtOptions, LfsState, file_at, file_at_with};
pub use file_bytes::{FileBytes, file_bytes_at};
pub use folder_history::{folder_file_today, folder_history};
pub use fsmonitor::{
    FSMONITOR_DAEMON, MonitorOffer, MonitorOutcome, MonitorTrouble, daemon_built, daemon_in_build_options, monitor_disable_cmd,
    monitor_enable_cmd, monitor_offer, monitor_offer_for, monitor_record_cmd, monitor_start_cmd, monitor_status_cmd, monitor_stop_cmd,
    stop_monitor, turn_off_monitor, turn_on_monitor,
};
pub use graph_cache::GraphCache;
pub use history::{FileHistory, HistoryEntry, HistoryStart, file_history};
pub use history_forward::name_at_head;
pub use index::{
    WholeDiscard, apply_hunk, discard_file, discard_untracked, stage_all, stage_file, stage_paths, unstage_paths, whole_discard,
};
pub use index_ext::RecordMemo;
pub use index_memo::{IndexDelta, IndexMemo, MAX_INDEX_MEMO};
pub use line_counts::{FileLineCounts, LineCount, MAX_COUNTED_FILES, WipLineCounts, commit_line_counts, wip_line_counts};
pub use line_map::{LineChange, LineMap, PrLineMaps, line_maps, pr_line_maps};
pub use lines::{apply_lines, apply_shown};
pub use load::{load, load_cached, load_reusing, messages_between};
pub use load_more::{HistoryPage, MoreLoaded, load_more};
pub use markers::{MARKER_SCAN_MAX, conflict_markers, marker_regions};
pub use merge_msg::merge_message;
pub use op_state::{BranchUse, HeldBranch, held_branch, state};
pub use pr_compare::{PR_COMMITS_MAX, PrCompare, RangeCommit, RangeCommits, SourceFrom, SourceTip, is_commit_prefix, pr_compare};
pub use push_config::{PushRemotes, push_remotes};
pub(crate) use refs_key::submodule_stamps;
pub use refs_key::{RACY_WINDOW, open_file_stamp, refs_key, stamp_trustworthy};
pub use remotes::RemoteInfo;
pub(crate) use removal::TRASH_NAME;
pub use removal::{Deleted, Removal, Trasher};
pub use resolve::{Choice, conflict_ids, render_output, resolve_with_side, write_partial_resolution, write_resolution};
pub(crate) use resolve_undo::path_batches;
pub use resolve_undo::{ConflictSummary, ResolvedFile, conflict_summary, mark_conflicted, mark_resolved, resolve_undo_ids, resolved_paths};
pub use restore::{RestoredFrom, restore_file};
pub use revert::{ALREADY_REVERTED, RevertPart, RevertRequest, Reverted, revert};
pub(crate) use rpath::shown as shown_path;
pub use safe_names::is_safe_ref_name;
pub use stacked::*;
pub use status::{ChangeCounts, status_and_index, status_and_index_with, status_at};
pub use status_index::{IndexStatus, status_after_index_change};
pub use status_merge::{covers, merge_staged, merge_unstaged};
pub use status_paths::{CLI_PATH_BYTES, PathsStatus, status_of_paths};
pub use status_speed::{
    SLOW_STATUS, SlowStatus, StatusSpeedups, UNTRACKED_CACHE_ALL, slow, status_speedups, untracked_cache_cmds, untracked_cache_helps,
    untracked_cache_works,
};
pub use submodule_add::check_submodule_path;
pub use submodules::{LEFT_BEHIND_MAX, SubmoduleInfo, SubmoduleState, submodules_at, submodules_if_any};
pub use template::{CommitTemplate, commit_template};
pub use tool_sides::{DiffSides, MergeSides, SideSource, TOOL_FILE_TIMEOUT, blob_bytes, diff_sides, is_conflicted, merge_sides};
pub use touching::{Touching, commits_touching};
pub use tracked::tracked_paths;
pub use trash::{TrashProblem, TrashRefusal, Untrashed, Untrasher, home_trash, os_trash, os_untrash};
pub use unmerged::unmerged_count;
pub(crate) use workfile::contained;
pub use workfile::{
    PathProblem, Written, check_new_path, check_new_path_for, create_file, delete_file, new_path_warning, new_path_warning_for,
};
pub use workfile_mv::rename_file_staged;
pub use workfile_rename::rename_file;
pub use workfile_rm::{delete_staged_new_file, unstage_gone_new_file};
pub use workfile_unrm::{StagedNew, index_clear_for, restage_new, staged_new, staged_new_matches, undelete_file, untrash_file};
pub use worktree_counts::{COUNT_TIMEOUT, ChangesRead, WorktreeCount, count_worktree};
pub use worktrees::{WorktreeInfo, worktree_wip_heads, worktrees_at};

pub use commit_info::{Author, Authors, BODY_PREVIEW, CommitInfo, Parents, Text};
pub use snapshot::{CommitDetail, LoadOptions, Snapshot, next_status_stamp};
pub use walk_reuse::{Frontier, Prior};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefKind {
    Head,
    Local,
    Remote,
    Tag,
}

#[derive(Clone)]
pub struct RefLabel {
    pub name: String,
    /// Full ref name, e.g. refs/heads/main (empty for a detached HEAD label).
    pub full: String,
    pub kind: RefKind,
    pub is_head: bool,
}

/// A branch's configured upstream (`branch.<name>.remote` and `.merge`).
#[derive(Clone, Debug, PartialEq)]
pub struct Tracking {
    /// The remote name, or "." when the upstream is a local branch.
    pub remote: String,
    /// The upstream branch name on that remote, without refs/heads/.
    pub merge: String,
}

impl Tracking {
    /// Whether the upstream is on a remote rather than a local branch.
    pub fn is_remote(&self) -> bool {
        self.remote != "."
    }

    /// "remote/merge", or just the branch name for a local upstream.
    pub fn display(&self) -> String {
        if self.is_remote() { format!("{}/{}", self.remote, self.merge) } else { self.merge.clone() }
    }
}

#[derive(Clone, PartialEq)]
pub struct BranchInfo {
    pub name: String,
    pub oid: Option<Oid>,
    pub is_head: bool,
    /// The upstream ref's short name (e.g. origin/x), when it exists.
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    /// The configured upstream, present even when its remote-tracking ref is gone.
    pub tracking: Option<Tracking>,
    /// Full name of the resolved upstream ref, e.g. refs/remotes/origin/x; None when
    /// `upstream` is None.
    pub upstream_ref: Option<String>,
    /// Whether HEAD contains the branch tip, so deleting the branch loses no commits. True for
    /// the checked-out branch; false when HEAD is unborn. Worked out only for branches whose
    /// upstream is a remote-tracking ref (deleting both is what asks); false for the others.
    pub merged: bool,
}

#[derive(Clone)]
pub struct StashInfo {
    pub index: usize,
    pub message: String,
    pub oid: Oid,
}

#[cfg(test)]
mod branch_colours_tests;
#[cfg(test)]
mod branch_merged_tests;
#[cfg(test)]
mod bytes_tests;
#[cfg(test)]
mod collide_tests;
#[cfg(test)]
mod common_dir_tests;
#[cfg(test)]
mod compare_root_tests;
#[cfg(test)]
mod conflict_saved_tests;
#[cfg(test)]
mod conflict_tests;
#[cfg(test)]
mod convert_tests;
#[cfg(test)]
mod convert_write_tests;
#[cfg(test)]
mod diff_tests;
#[cfg(test)]
mod discard_tests;
#[cfg(test)]
mod divergence_tests;
#[cfg(test)]
mod exact_tests;
#[cfg(test)]
mod file_at_tests;
#[cfg(test)]
mod file_bytes_tests;
#[cfg(test)]
mod folder_history_tests;
#[cfg(test)]
mod history_forward_tests;
#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod line_map_tests;
#[cfg(test)]
mod lines_tests;
#[cfg(test)]
mod lines_whole_tests;
#[cfg(test)]
mod load_filter_tests;
#[cfg(test)]
mod load_more_tests;
#[cfg(test)]
mod markers_tests;
#[cfg(test)]
mod merge_msg_tests;
#[cfg(test)]
mod op_state_tests;
#[cfg(test)]
mod pr_compare_tests;
#[cfg(test)]
mod refs_key_tests;
#[cfg(test)]
mod remotes_tests;
#[cfg(test)]
mod removal_tests;
#[cfg(test)]
mod resolve_eol_tests;
#[cfg(test)]
mod resolve_tests;
#[cfg(test)]
mod resolve_undo_tests;
#[cfg(test)]
mod restore_lfs_tests;
#[cfg(test)]
mod restore_tests;
#[cfg(test)]
mod revert_file_tests;
#[cfg(test)]
mod revert_tests;
#[cfg(test)]
mod revert_workdir_tests;
#[cfg(test)]
mod revert_ws_tests;
#[cfg(test)]
mod safe_names_tests;
#[cfg(test)]
mod shallow_tests;
#[cfg(test)]
mod shown_ws_apply_tests;
#[cfg(test)]
mod sparse_status_tests;
#[cfg(test)]
mod status_cli_tests;
#[cfg(test)]
mod submodules_tests;
#[cfg(test)]
mod template_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod touching_tests;
#[cfg(test)]
mod tracked_tests;
#[cfg(test)]
mod unmerged_tests;
#[cfg(test)]
mod walk_reuse_tests;
#[cfg(test)]
mod walk_tests;
#[cfg(test)]
mod whole_tests;
#[cfg(test)]
mod workfile_case_tests;
#[cfg(test)]
mod workfile_exact_tests;
#[cfg(test)]
mod workfile_mv_tests;
#[cfg(test)]
mod workfile_rm_gone_tests;
#[cfg(test)]
mod workfile_rm_tests;
#[cfg(test)]
mod workfile_tests;
#[cfg(test)]
mod workfile_unrm_tests;
#[cfg(test)]
mod worktree_held_tests;
#[cfg(test)]
mod worktrees_tests;
