# glowfly-core: the repository model (`crates/glowfly-core/src/repo/`)

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here.

## Loading and status

- `repo/` is the repository model on libgit2: loading history, refs and status.
- Re-reading only some work-tree paths: `status_paths.rs`, merged by `status_merge.rs`.
- After an index change, both lists' rows of the paths whose entries changed since the memo of the last read: `index_memo.rs`, `status_index.rs`.
- What speeds git's status up and the slow-read note's commands: `status_speed.rs`.
- Git's built-in file system monitor (which OS and git have it, starting and confirming `git fsmonitor--daemon`, reading its refusals): `fsmonitor.rs`.
- Rows whose stored content is a Git LFS pointer get `FileEntry::lfs`, and a viewer diff with a pointer side carries its `LfsChange` (`lfs_marks.rs`).
- The commits one branch has and another lacks, walked in the loaded graph and marked incomplete when it runs off its end: `divergence.rs`.
- When the repository last fetched, from FETCH_HEAD's time: `fetch_time.rs`.
- `repo/walk_order.rs`: the history walk's order (newest committer time first, ties in the order found, each commit once, parents as git2's `parents()` yields them), shared by the graph's walk (`repo/walk.rs`) and the search index, so an index position says how many commits the graph must load to show it.
- `repo/commit_info.rs`: a loaded commit, about 104 bytes plus its texts. The walk reads each signature once and interns the (name, email) in an `Authors` table, so a history's commits share one `Author` per person; summary and body are boxed `Text`, one parent is kept inline (`Parents`), as are a graph row's first two edges (`graph::Edges`). The search index keeps its own author table of the same `Author`s.
- `Snapshot::lay_out_graph` (`repo/branch_colours.rs`) lays the graph out with its owners and colours, at load and when the work tree turns clean or dirty. `Snapshot::commits` is shared (`Arc<[CommitInfo]>`) so workers read it without a copy.
- Paths go through `repo/rpath.rs` (see `docs/PLATFORMS.md`).
- `repo_pool.rs` (in `src/`): the workers' pool of open repositories, so the refs fingerprint, status, path status, commit detail, diffs and line counts don't open the repository and parse its index each time. Lent to one thread at a time, keyed by the path as given and how it was opened; `repo_pool/fresh.rs` checks a handle before lending it again (config files, `.git` link, git dir, alternates, the index file). Idle handles go after 5 s, past 8 (2 per key), on `forget` (a tab closed) and during a `hold` (an effect that changes the repository, or a fetch): on Windows an open handle keeps old packs from being deleted and folders from being removed.

## Diffs

- A commit against its parent, another commit or the empty tree (`empty_tree`, for a line of commits reaching the first one), or the working tree either way round: `compare_workdir`, `compare_from_workdir`.
- The `linguist-language` override read from the commit's own `.gitattributes` for commit and comparison diffs: `attrs.rs`.
- The unchanged lines between a diff's hunks, read for the hunk view: `diff_gaps.rs`.
- The lines added and removed per file of a commit or of the working tree's staged and unstaged files, libgit2 only and capped at 2,000 files: `line_counts.rs`.
- Each commit's own changes against its first parent, newest first, for a selection that isn't one line of history: `stacked.rs`.
- A binary diff's two sides for its preview: each side's size (a blob's from the object header, a working file's from its metadata) and, when small enough and not an LFS pointer, its decoded image: `binary_sides.rs`.
- The copies an external diff or merge tool opens are written from `repo/tool_sides.rs` (see `external_tools/` in [core-system.md](core-system.md)).
- A pull request's source tip and its compare: `repo/pr_compare.rs`; code comment lines mapped from the head onto a local tip: `repo/line_map.rs` (see [flow-repo.md](flow-repo.md)).

## History and file contents

- Following a file's renames backwards and forwards: `history.rs`, `history_forward.rs`.
- A folder's moves backwards: `folder_history.rs`.
- A file's text at a commit for the File view (the version before a deleting commit; a Git LFS file's content through `git cat-file --filters` when git may run unasked): `file_at.rs`, which reads through `file_bytes.rs`; that gives the bytes, binary or not, for the File view's image preview.
- Blame through `git blame --porcelain`: each run's origin (the commit's path and the parent to step back to); whitespace, move and skipped-commit options.

## Changing files

- Staging, line staging and restore; working-tree files.
- Revert of a hunk, lines or a whole file of a commit diff, merged three ways with later edits: `revert_file.rs`, `revert_merge.rs`.
- Revert of a hunk or lines of a commit-to-working-tree diff: never merged.
- A hunk or lines shown with whitespace hidden are staged, discarded or reverted as shown, with the unchanged lines' exact bytes from the unfiltered diff: `shown_ws.rs`.

## Conflicts

- Conflicts and their resolution, keeping git's resolve-undo record: the resolved files, the stage ids of one record, Mark resolved and Mark conflicted: `resolve_undo.rs`.
- Conflict markers left in working files: `markers.rs`.
- The picks a file saved with markers holds: `conflict_saved.rs`.
- A merge's MERGE_MSG without its comment lines, read with libgit2 from the worktree's own git dir to prefill the commit draft: `merge_msg.rs`.

## Submodules, worktrees, remotes

- Submodules: their state, including a missing folder and one added but not committed, the .gitmodules branch and the URL git clones from, a relative one resolved.
- Whether a new submodule can go in a folder: the index, the disk, .gitignore, a leftover stored module.
- Worktrees, and other worktrees' changes, counted within the load's limits or in the background: `worktree_counts.rs`.
- Remotes and shallow-clone boundaries.
