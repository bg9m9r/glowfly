# glowfly-core: git commands and operations (`crates/glowfly-core/src/`)

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here.

## The git CLI

- `gitcli.rs`: the git CLI, for commands that honour hooks, credentials and signing; reports every command's timing to the observer the app sets.
- `gitcli/wait.rs` waits for a child: pipes drained on their own threads, a timeout, and a `Stop` the caller can ask.
- `output_until` runs a command that can be stopped, and `process_tree.rs` ends it with everything it started (a new session and its process group on Unix: SIGTERM, then SIGKILL after 3 s; a job object on Windows).
- `stop_cleanup.rs` removes the ref locks and temporary pack and object files a stopped fetch left behind (only files made after the run started).

## Partial clones (`partial.rs`, `history_size.rs`, `forge/repo_size.rs`)

- Partial clones (`--filter=blob:none`): the promisor remote.
- The check before a libgit2 read (a diff's sides, the File view's version, blame's versions from `git log --raw` without lazy fetching) that reports `Error::MissingContent` instead of failing.
- The fetch commands that download exactly those objects (batches of 200, the noop negotiator).
- Registering the `partialclone` extension with libgit2 at startup.
- What Fetch full history may download: the hosting service's reported size or, for a source on this computer, its object store on disk, and the 500 MiB line from which the app asks first.

## Command lines and checks (`*_ops.rs`)

- `branch_ops.rs`, `tag_ops.rs`, `remote_ops.rs`: command lines and input checks for branches, tags and remotes.
- `clone_ops.rs`: cloning, with the sparse clone job: the clone, then the folders to check out.
- `shallow_ops.rs`: fetching more of a shallow clone's history.
- `worktree_ops.rs`: worktrees; moving one: `worktree_ops/moving.rs`.
- `submodule_ops.rs`: submodules: the Add dialog's fields; removing one (deinit, `git rm`, then its stored module, with what would be lost); `set-url` / `set-branch` for editing one; batch `submodule update` command lines; resolving relative URLs; which commands move the work tree, after which submodules may be updated.
- `stash_ops.rs`: renaming a stash list entry by dropping and storing the top of `refs/stash` again; stash push command lines: all, staged only, chosen paths, optionally keeping the staged changes.
- `stash_ops/renames.rs`: a chosen staged rename stashed whole: its old path put back from HEAD after the stash, refused beforehand when something is on disk there.
- `stash_ops/auto.rs`: auto-stashes: which single commands the guard covers (plus Git Flow start and finish, a pull request's checkout and a restack, named by their entry points), their fixed English stash messages, and putting a stash back with `git stash pop --index` once nothing is in progress (a plain pop when the staged state no longer fits), telling restored, restored unstaged, conflicted, failed and gone apart; tests in `auto_tests.rs` and `auto_put_back_tests.rs`.
- `stash_ops/saved.rs`: the auto-stashes waiting to go back, kept across restarts in the worktree's `glowfly/auto-stashes.json`; tests in `saved_tests.rs`.
- `stash_ops/paths.rs`: the files a stash changes (its working-tree, index and untracked-files commits), for the question before applying it; tests in `paths_tests.rs`.
- `sparse_ops.rs`: sparse checkout state, paths and command lines.
- `patch_ops.rs`, `patch_ops/`: format-patch export of commits or one file of a commit; uncommitted changes through git diff (all, staged or unstaged, chosen paths, untracked files); clipboard text; reading what a patch file holds without git.
- `patch_ops/`: git apply command lines for each option (index, three-way, `.rej` files, reverse) and their errors, and git am series with where a stopped one is.
- `pick_ops.rs`: command lines for cherry-picking and reverting commits from the graph (several at once, a merge against a chosen mainline parent, skipping a stopped commit), run through gitcli so hooks, config and signing apply; which picking is stopped (`Picking`).
- `pick_empty.rs`: telling a commit that changes nothing from a conflict and skipping it (`skip_empty`, with `--skip` from git 2.23; an older git ends a single pick cleanly) so the rest of the run goes on, counted in a `PickResult`.
- `init_ops.rs`, `init_ops/`: new repository files and the init job: `.gitignore` templates, license texts, what the chosen folder already holds, and init, write (never overwriting) and commit.

## Git LFS and Git Flow

- `lfs.rs`, `lfs/`: Git LFS: install and setup state, tracked patterns, LFS command lines.
- `lfs/pointer.rs`: pointer files: parsed from bytes, read from a blob without inflating large ones or from a working file, sizes for the screen with `size_text`, and a diff's two sides as `LfsChange`.
- `gitflow.rs`, `gitflow/`: Git Flow with plain git: config, checks, start and finish plans, continuing a finish stopped at a conflict, the production/development roles, versions, branch groups by kind.

## Interactive rebase (`rebase/`)

- Plan: from a commit, onto a branch or onto a commit.
- Todo: commit rows, exec and break steps, reword and combined-message amends.
- Ranges with merges recreated through `--rebase-merges` (`merges.rs`: label, reset and merge steps).
- Run (with `--autostash`, and Continue / Skip); quick edits chosen in the graph (reword, squash, drop).
- Progress: the step and the stopped commit of a rebase in progress, and a failed exec's command.
- The remaining steps of a rebase in progress and `--edit-todo`: `remaining.rs`.

## Stacked branches (`stack/`)

- `link` (pure) recognises which local branches are stacked on which from first-parent history and reflogs, and `read` gathers its input with libgit2: the base is the main branch, Git Flow's production and development branches and every remote's branch of those names.
- `plan`, `restack`: planning and running a restack with `git rebase --update-refs` through gitcli, one step per leaf that needs moving (and per layer above it that is itself behind), and going on after a stop.
- `restack_end`: a restack's one auto-stash (made before the first step, kept with the steps left after a stop) and its end: the branch checked out before checked out again and the stash put back there with `pop --index`, or kept in the list when HEAD can't get there; `end_stopped` for a stop whose rebase was aborted or finished outside; tests in `restack_stash_tests.rs`.
- `push`: pushing every layer through gitcli, each classified against the remote tip last seen (new, fast-forward, rewritten here per its reflog and replaced with a ref-specific `--force-with-lease`, or behind, diverged and gone, never pushed), one `git push --porcelain` per (remote, `-u`) group and each layer's outcome read from its ref line.
- `porcelain`: the push command lines and the porcelain reading.
- `Snapshot::stacks` holds a full load's stacks.

## Undo and redo (`undo.rs`, `undo/`)

- Undo and redo of Glowfly's own operations.
- `kinds.rs` recognises their command lines; `action.rs` holds the recorded actions and their labels; `record.rs` the state before and after; `planning.rs` plans the reversal.
- `refs.rs`: branches and tags.
- `moved.rs`: merges, pulls, cherry-picks, reverts and rebases.
- `stash.rs`: stashes, pops, applies and drops.
- `reset.rs`: resets.
- `saved.rs`: file, hunk and line discards and file restores.
- `reverted.rs`: reverts of part of a commit in the working tree.
- `resolved.rs`: saved conflict resolutions.
- `deleted.rs`: deletes and Unstage and delete.
- `rebased.rs`: interactive rebases, graph edits and rebases continued from the banner are recorded around their run.
- `resumed.rs`: a merge, cherry-pick or revert finished after conflicts (Continue, Skip or a commit) is recorded as one move from the HEAD it started at.
- `abort_keeping_commands`: aborting a merge, single cherry-pick or single revert with every conflict resolved and no edits outside its files stashes its changes first, recorded as a stash push so Undo pops them back.

## Operation journal (`journal.rs`, `journal/`)

- `reflog.rs`: HEAD's and the local branches' reflogs read with libgit2.
- `events.rs`: their messages turned into events.
- `notes.rs`: Glowfly's own notes kept beside them in `<git dir>/glowfly/journal.jsonl`: what an operation was and the tip it moved just before it, appended and trimmed.
- `timeline.rs`: merged newest first into one timeline with one row per state: a HEAD entry folds into the branch entry it moved with, a note becomes its own "Before …" row.
- `preview.rs`: a state's first-parent history and what Restore would leave behind or bring back.
- `restore.rs`: the command lines that restore a state.
- `reach.rs`: which rows' commits no ref, worktree HEAD or stash entry reaches any more: one revwalk that pushes the rows' commits and hides every tip.
- `lost.rs`: the lost commits themselves: `git fsck --connectivity-only --no-reflogs` through gitcli, the stash list's own entries dropped, dropped stashes recognised by their shape, and the `stash apply` / `stash store` lines that keep one.

## Range-diff (`range_diff.rs`, `range_diff/`)

- Two versions of a branch compared commit by commit with `git range-diff --no-color --abbrev=40 <old-base>..<old> <new-base>..<new>` (or `<base> <old> <new>` when both sides share a base, so an ancestor pair gives a one-sided list) through gitcli, with a timeout, git's own words on failure and `Feature::RangeDiff` when git lacks it.
- `bases.rs` finds a ref's last rewrite in its reflog (the newest entry that isn't a fast-forward), the mainline both ranges are cut at (else their merge base) and the 300-commit cap.
- `parse.rs` reads the pairs and each changed pair's interdiff as a `FileDiff`.
- Read only: no ref, index or file changes; the app runs it only in a trusted repository.

## Previews and checks

- `conflict_check.rs`: predicting merge conflicts with `git merge-tree` (nothing in the working tree, index or refs changes), and choosing the branch to compare the checked-out branch with.
- `op_preview/`: previewing a merge, rebase, reset, cherry-pick or drop.
- `op_preview/mod.rs`: the request and the result's types, the checks every preview makes first (no operation in progress, a born HEAD that still matches the request) and the entry point `preview`.
- `ops.rs`: merge, reset and cherry-pick, one function per operation; `ops_rebase.rs`: rebase and drop (a drop replays `rebase::prepare_edit`'s steps, so the preview follows the plan the drop itself runs).
- `replay.rs`: replaying commits one by one, with merges, labels and resets, as git would.
- `shape.rs`: the rows of the history afterwards, the commits arriving and the commits no ref will reach any more.
- Every object a preview writes with libgit2 goes to an in-memory backend (`add_new_mempack_backend`) added before anything is written and dropped with the repository, and commits are made with no ref to update, so the files, index, refs and object store on disk never change (`tests.rs` checks that).
- A previewed merge's conflicts are predicted with `git merge-tree` as `conflict_check.rs` does, which changes no files, index or refs either.
- `signature_check.rs`: checking commit signatures through git (`%G?`), so the user's signing programs, keyrings and allowed signers file apply.

## Several repositories

- `repo_scan.rs`: finding the Git repositories under project folders (the Find repositories dialog): a bounded breadth-first walk that reads only folder listings and `.git` entries, never follows links or junctions, and doesn't descend into a repository it found.
- `repo_sync.rs`: fetching or pulling (fast-forward only) one listed repository that may not be open, never prompting: skipped when it has no remotes or upstream, or when its config or hooks run programs its trust decision doesn't cover; tells "updated" from "up to date" by comparing its references before and after.

## Trust and custom commands

- `trust.rs`: repository trust: config keys and hooks, the repository's own and its checked-out submodules', that make git run programs (read with libgit2 only, at every full load).
- `trust/print.rs`: the print of a scan (one short hash per item) a Trusted decision is saved with, so a scan with anything new asks again (`trust::allows`).
- `custom_commands.rs`, `custom_commands/`: custom commands (`docs/CUSTOM_COMMANDS.md`): reading the user's and the repository's `commands.toml` (links refused, size and count capped).
- `parse.rs`: strict TOML with each problem's line.
- `expand.rs`: the placeholders checked and filled in, into git arguments or single-quoted sh text.
- `shell.rs`: finding POSIX sh (`sh_for`: `/bin/sh`, or Git for Windows' `sh.exe` beside git) and running a shell command with its script in `GLOWFLY_COMMAND`.
