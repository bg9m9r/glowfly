# Data flow: repositories, tabs, jobs and hosting services

Part of [ARCHITECTURE.md](../ARCHITECTURE.md), which has the Msg / update / Effect / runner loop
these follow. One line per topic: when you change how a flow works, update its lines here. The
shell's flows (dialogs, toasts, layout, window, menus, logs, links) are in
[flow-shell.md](flow-shell.md).

## Unchanged lines in a diff

- `TabView::diff_expanded` (in `RepoSession::tab_view`) holds the opened runs by key (a run's first new line, the same in the whole-file view and the hunk view) and, for the hunk view, the text of the gaps between hunks, read once per shown diff (`Effect::LoadDiffGaps`, `update/diff_gaps.rs`).
- E and Shift+E open the next closed run from the top or fold back the one there (`update/fold_keys.rs`).

## Next/previous change across files

- A step's file is `ViewState::diff_landing` until it loads; a refresh while it loads asks for the step's file again (`update/diff.rs::renew_step`) instead of reloading the old one.
- Once it lands, `ViewState::file_reveal` makes the file list it came from scroll to it once (`ui/file_reveal.rs`, guarded by the landing's generation).
- A plain open works the same way: its file is `ViewState::diff_opening` until it arrives, and a refresh meanwhile asks for it again (`update/wip_list.rs::renew_open`) while its list still holds it, so a quick click, a WIP list step or the move-on after S or U isn't lost.

## Tabs

- The active tab's repository data lives in `AppState`; switching tabs parks it in `state::tabs::Parked` and restores the other tab's. Everything per tab moves as one `RepoSession`: the views with generation counters, the open diff (`DiffSlot`), the commit detail and draft, the repository reads (`RepoReads`), the selection, forge data, undo history, commit search and the view state (`TabView`). Its counters on return move past both tabs' (`RepoSession::resume_after`). `Parked` adds only the snapshot and the Create issue dialog. After the swap `unpark` runs named hooks: `gitflow::snapshot_changed`, the selection reset without a snapshot, the diff top-row recount (`restore_diff_top`), the Create issue dialog and the graph scroll restore.
- Results of repository-scoped effects (`Effect::scope`) come back as `Msg::Scoped`.
- If their tab is no longer active, only what the area's `on_inactive` returns is applied (a log line, an undo entry for that tab, a dialog's result); the rest is dropped.
- Each tab's activity log lives outside `Parked`, in `Tab::log` (the active one's in `AppState::log`): `switch` and `activate` swap it, so a restored tab that hasn't loaded still gets lines.
- Paths are resolved on disk once, when a repository loads (`Snapshot::real_workdir`, with its identity `Snapshot::folder`).
- `update` compares paths with the pure `paths::same_spelling` plus the identities worker threads found (`AppState::same_folder`, `state/known_folders.rs`); it never folds case, which is the volume's business.
- Settings saved per repository are keyed by `real_workdir` through `settings::repo_keys`.
- Worktree tab groups: each `Tab::group` is its repository's common git dir (`Snapshot::common`, or for restored tabs `Effect::ReadTabGroups`, read at startup without loading them).
- The pure rules for tab group blocks, drops and regrouping are in `state/tab_groups.rs`, applied by `update/tab_groups.rs` and drawn by `ui/tabs.rs`.
- Worktrees and branches: a local branch another worktree has checked out is found by its exact name and `WorktreeInfo::current` (`state/worktree_branches.rs`), never by comparing folder spellings.
- Every UI checkout of a local branch goes through `BranchMsg::Checkout`, which refuses a branch another worktree has checked out.
- Moving a worktree (`update/worktree_move.rs`) points the tabs, the recent list, the closed tabs, favorites and `KnownFolders` at the new folder.
- Removing a worktree closes the other tabs showing it first (`update/worktree_manage.rs`).
- Each tab has a `TabId` for as long as it is open: what a request waiting for an answer names it by, since its index changes when tabs move or close.
- Closing a tab that holds work only in memory (a commit draft, conflict choices) or whose terminals run a program (asked about first, `update/terminal_busy.rs`) asks first (`state/tab_losses.rs`), and the confirmation carries a `CloseRequest` by id (`update/tabs_close.rs`).
- A tab's label (Rename tab, `update/tab_label.rs`) is saved with the open tabs in tabs.txt and kept on the recently closed list.

## Job follow-ups

- A job can carry what to do once it succeeds (`JobSpec`/`RunningJob` `then_push`, `then_open`, `then_pr`, `then_link`), handled in `update/ops.rs::finish_job`.
- Follow-ups that act on the repository run only while the job's tab is active and are logged otherwise.
- A follow-up that needs the refreshed snapshot waits for the reload: a push for a pull request sets `ForgeState::open_after_load`, and the next snapshot load opens the create dialog (`update/push_pr.rs::loaded`), never replacing another dialog.

## Pull requests across forks

- Detect lists the remotes on the code host's account as PR repositories (`ForgeState::pr_repos`, built by `state/pr_repos.rs`: the code host's first, one per repository).
- The base is a remote named `upstream`, else the code host's.
- A branch tracking a fork remote is pushed and measured there (`pr_repos::push_remote`, `state/push_pr.rs`'s `*_on` functions).
- The create dialog picks From / Into (`ui/create_pr_repos.rs`, `update/pr_repo_pick.rs`), and `update/push_pr.rs::create` sends the request to the Into repository with `NewPullRequest::source_repo` set to the fork.
- Each service's head on a fork is in `glowfly-core/src/forge/pr_heads.rs`.
- Push to your fork… goes through the Fork dialog with `ForkCreate::then_pr`, whose add job also pushes the branch (`update/fork_pr.rs`).

## Pull request source tip

- The pull request panel compares from the target's remote-tracking tip to a source tip the runner picks (`glowfly-core/src/repo/pr_compare.rs`).
- The source tip is the head commit the service listed when it is already in the object database, else the remote-tracking source branch (never a same-named branch once the page says the source is on a fork).
- The files and the commits in `target..source` come from one load.
- With no source here at all, the panel fetches the service's head ref once in the background, without prompts and writing no ref (`update/pull_request/source.rs`, `Provider::pr_head_ref`, `remote_ops::fetch_ref_cmd`), then compares again.
- The source notes and their buttons are in `state/pull_request_source.rs`.
- A Commits tab row selects its commit into the commit panel without closing the pull request panel (`update/pull_request/commits.rs`), and that commit's file diffs open inside the tab.
- The review and merge forms' rules are in `state/pull_request_actions.rs` (review choices in `pull_request_actions/review.rs`).

## New code comments on diff lines

- A pull request file's diff (`DiffView::comments`, from `state/pull_request_line_comment.rs::diff_comments`) offers a comment button on hovered lines (`ui/diff/comment_add.rs`).
- The draft (`PrView::line_draft`, one per panel) is anchored by file line numbers and the source tip it was started on, never by row indexes, so a diff reload keeps it.
- Only lines the normal hunks show can take a comment, because the service compares within the same hunks.
- Posting goes through `calls::act` like the panel's other actions (`update/pull_request/line_comment.rs`).
- `forge::line_comments_in_app` gates the service apart from `reviews_in_app`.
- A single comment's thread gets a provisional `new:<comment id>` id until the page reloads, when `adopt_new_threads` hands its reply draft, open state and focus to the real thread.
- An answer for a parked tab is applied to that tab's parked panel.

## Code comments on the local version's lines

- When the files were loaded from a source tip that isn't the service's head, the threads' lines are mapped from the head onto it in the background (`glowfly-core/src/repo/line_map.rs`: a zero-context tree-to-tree diff that follows renames; `PrEffect::LineMaps`, `update/pull_request/line_map.rs`).
- The map is kept with the key it was made for (`state/pull_request_line_map.rs::MapKey`: head, local tip, base, target and the threads' paths) and used only while that key is still wanted, so a moved tip or new threads ask again.
- `Placement::Mapped` then puts the markers, location texts and go-to on the mapped lines (`state/pull_request_threads.rs`).
- Deleted-line numbers are kept only while the head's merge base with the target is the local diff's base.

## Operation journal

- The timeline is git's reflogs plus Glowfly's notes.
- Only the runner writes notes, and only around the work that rewrites history: a `RunGit` whose job carries a `journal` op, Undo and Redo, and the start of an interactive rebase (`runner/journal.rs::around`).
- The `journal` op is worked out from the command as the user gave it, before the auto-stash wrap, in `update/git_job.rs` and `update/auto_stash.rs::wrap`, else by `update/jobs.rs::start_job` from the job's undo record and the command of that kind; never for a job in another repository.
- The note records the tip of the branch the op moves (else HEAD) just before the run and is kept only when that moved; its time comes from the runner's clock, never from `update` or a view.
- Restore is an ordinary git job with an undo record (`state/journal_restore.rs`), so it lands in the journal and Undo takes it back.
- An open journal reloads on every `LoadKind::Refresh` of its tab and when the tab comes back; a reload or a preview that answers an older request (`JournalView::generation`) is dropped.
- The Lost commits tab runs `git fsck` (read-only, through gitcli, never with `--lost-found`, which writes into the repository) only while it shows.
- A refresh while the States tab shows marks the lost list stale (`LostView::stale`), and showing the tab again reads it then.

## Range-diff

- The view sits in the central order between the rebase panel and the journal (Esc closes it before the journal under it).
- It closes the diff, conflict, history, blame, rebase, pull request and issue views when it opens; opening the journal or Git Flow closes it.
- It is parked with its tab and asked again when the tab comes back (a new generation, so an older result is dropped).
- A refresh never reloads it: it compares the fixed tips it was opened with, and Retry asks again.
- In an untrusted repository it waits in `Phase::Held` and asks for trust, and starts once the repository is trusted (`update/range_diff.rs::trust_released`).

## Paced second passes

- Work too slow to wait for rides behind the result that needs it.
- On services that read checks and votes one pull request at a time, the list carries them for the first `CHECKED_PRS`.
- `update/pr_details.rs` then queues the rest and sends one `ForgeCall::PrDetails` chunk at a time, each reply filling its pull requests in and sending the next.
- What was read is cached by head commit and age (`state/pr_details.rs`), so a list refresh re-reads only what changed or aged.
- A failed chunk stops the pass without touching the host's status.

## Profile folder rules

- A rule is an `includeIf.gitdir` entry in the global config, so it holds on the command line too.
- Glowfly never writes a repository's own identity, and removes it only after the Use <profile> here confirmation.
- Which profile applies is read from git (`repo_identity::read`), never worked out by Glowfly alone; Glowfly's own matching only says what to expect.
- A mismatch places the entries again once per git dir and session.

## Dashboard

- Global state (`state/dashboard.rs`), never parked.
- Scan, then one Fetch effect, with credentials resolved outside any repository.
- Opening a PR waits for that tab's list (`pending_pr`).
- Which repositories and calls a refresh plans is in `state/dashboard_plan.rs`; the rows the view draws in `state/dashboard_rows.rs`.

## Find repositories

- Global state (`state/repo_scan.rs`, `st.repo_scan`), never parked; the project folders are a setting.
- Each scan is one `Effect::Repos` (`runner/repos.rs`) whose results carry each repository's `FolderId`, learned into `KnownFolders` so the "open" tags match a tab that shows the same folder under another spelling.

## Fetch or pull across repositories

- Global state (`state/repo_sync.rs`, `st.repo_sync`) with a row per favorite or listed repository.
- `update/repo_sync.rs` starts at most `MAX_RUNNING` `ReposEffect::Sync`s and starts the next as each `ReposMsg::Synced` comes back (stale generations are ignored).
- A pull queues behind the repository's other jobs.
- A saved "don't trust" skips a repository without running anything.
- Closing the dialog leaves the run going; its end is then told in a toast that opens it again.

## Incremental status

- The watcher (`watcher.rs`) collects the work-tree paths of a burst when nothing else changed (no `info/exclude`, `.gitignore`, `.gitattributes`, HEAD or ref change, no lost events, at most `watch::MAX_WATCH_PATHS`) and sends them in `WatchMsg::RepoChanged { paths }`.
- `update/refresh_paths.rs` asks for `Effect::LoadStatus { paths: Some }`.
- The runner (`runner/status.rs`) reads only the unstaged rows under them (`repo::status_of_paths`, widened to a submodule's or nested repository's root) or, when they can't be read on their own, the whole status in the same effect.
- `update` merges the rows into the lists on screen (`repo::merge_unstaged`) and keeps the staged list.
- A burst that also changed the index sends `WatchMsg::IndexChanged { paths }`.
- Every whole read keeps a memo of the index entries it started from (`Snapshot::index_memo`, `repo::IndexMemo`, read just before the status).
- `update/refresh_index.rs` asks for `Effect::LoadIndexStatus`, which compares the index with the memo (path, stage, mode, id, skip-worktree, intent-to-add; not stat data).
- It reads both lists' rows only for the changed paths, the burst's paths and the staged additions, deletions and renames a rename may pair with (`repo::status_after_index_change`, merged by `merge_staged` and `merge_unstaged`, storing the new memo).
- A conflicted or non-UTF-8 changed entry, a read set past the cap or one that can't be read alone, or no memo reads the whole status.
- Glowfly's own refreshes and the polls read everything; reads asked for while one runs merge (`state::refresh::Wanted`).
- A whole read slower than `repo::SLOW_STATUS` shows a note once per repository (`update/status_tip.rs`) whose button turns on git's untracked cache in a job.
- On Windows and macOS with git 2.36 or newer, the note's button also turns on git's built-in file system monitor (`Effect::TurnOnMonitor`: `git fsmonitor--daemon` started and confirmed, core.fsmonitor=true set only once it is watching, reported as `LoadMsg::StatusSpedUp`).
- On Linux the note suggests Watchman with git's fsmonitor-watchman hook, in words only.

## Loading more history and reloads

- Load more, scrolling near the end, Try again, the automatic retries, Keep searching, a filtering search's batches and the index's `load_through` raise `extra_commits` and ask for `Depth::More` (`update/refresh.rs::more`).
- When nothing but the commit count changed (`state::refresh::only_more`: the snapshot has a walk frontier), the load is `LoadKind::More` with the snapshot's `known` refs and its `Snapshot::prior`.
- The runner (`runner/load.rs`) checks `refs_key`: unchanged, it calls `repo::load_more`, which resumes the walk from the frontier for the next commits only, and answers `LoadMsg::HistoryExtended`.
- `update/load_more.rs::extended` applies the page with `Snapshot::extend_history`: the commits and rows appended, the lanes and owners laid out on from the last row, colours resolved again; the selection, scroll and search cursor stay. A large history's old commit list is freed on a worker, and a worker relayout in flight is asked for again.
- A refresh merged into the load for more (`RefreshState::status_after`) reads the status once the page lands.
- Moved refs, or a new commit that is a child of a loaded one (`NeedsFullLoad`, clock skew), give a full load at the larger count, reported as `RepoLoaded` with `Refresh`.
- Every full load for a refresh passes `Snapshot::prior`: the walk takes commits already on screen by oid (`repo::load_reusing`) instead of reading them again. F5 and opening a repository read everything fresh.

## Other worktrees' changes

- Their WIP rows never hold up a load: `repo::worktree_counts` counts them during the load only while their index is small (`LOAD_COUNT_INDEX` entries, read from the index header) and the load's budget (`LOAD_COUNT_BUDGET`) lasts, and marks the rest `ChangesRead::Pending`.
- Right after the load `update/worktree_counts.rs` asks for one `WorktreeEffect::CountChanges` (scoped; one per tab at a time, `RefreshState::counting`, kept across unchanged refreshes).
- The runner reads each through git with `COUNT_TIMEOUT` when git may run there, otherwise through libgit2.
- `WorktreeMsg::ChangesCounted` fills in the counts for the same `refs_key` and lays the graph out again.
- A read that times out, or one libgit2 would have to make of a huge index, is `TooLarge` and shown as not counted until the next full load.

## Submodules after a job

- `update/submodule_batch.rs` keeps submodules updated after a checkout, pull, merge, rebase or reset (Preferences > General, overridden per repository).
- `start_job` and the rebase launches mark the job in `st.submodules.after_job` when `submodule_ops::moves_work_tree` says it can change the recorded commits.
- `finish_job` then reads the submodules again (`SubmoduleEffect::CheckAll`, scoped) when that job succeeded in the tab that is still active, and the update leaves out any submodule where it would strand commits.
- The mark is matched by label and workdir, so one left by a job the trust prompt held never fires for another.
- `opened` offers (once per session) to initialize a newly opened repository's submodules.

## Auto-stash

- Files: `update/auto_stash.rs`, `update/auto_stash_saved.rs`, `update/auto_stash_restack.rs`, `update/pr_checkout.rs`, `state/auto_stash.rs`, `ui/auto_stash_dialog.rs`, `glowfly-core/src/stash_ops/saved.rs`.
- One guard for checkout, pull, merge, plain rebase, cherry-pick, revert and hard reset, and for Git Flow start and a pull request's checkout (`start_guarded_as` with their own `AutoStashFor`).
- Each entry point passes its single command and a retry message to `gate`.
- Only staged changes and changed tracked files count (`Snapshot::has_tracked_changes`): untracked files and an unstaged submodule row don't.
- On a dirty tree with nothing in progress the gate opens `Modal::AutoStash` holding the retry (Always stash skips it, except for a hard reset), and the answer reruns the retry with `st.auto_stash.approved` set for just that call.
- `wrap` puts `git stash push -m <message>` before the command in the same job (same label, undo recorded as the original command) and marks it (`making`, dropped with a held job).
- A guarded command the gate lets run unstashed (the snapshot showed nothing to stash) is recorded with its retry (`unstashed`, matched by label and workdir when the job ends).
- If git then says changes were in the way, the failure toast's Stash and <op> sends `AutoStashMsg::Retry`, which reruns it with the stash approved.
- `finish_job` turns the mark into a pending entry armed for the next load generation.
- `seen` runs after each snapshot of that tab, finds the stash, and once the operation is over (merge committed or aborted, rebase continued) and no job runs, puts it back as the quiet job `StashEffect::PutBack` (`git stash pop --index`, then a plain pop with a notice when the staged state no longer fits).
- An entry that has to wait is written to the worktree's saved records (`StashEffect::SaveAutoStashes`); opening the repository after a restart reads them (`ReadAutoStashes`) and, once it is ready, offers each listed stash (`Modal::ConfirmChoice`: Put back, Leave in stash list, Cancel asks again next open).
- A plain single-command job started outside the guard (`start_job`, `started_plain`) records its retry too, so its "would be overwritten" toast still offers Stash and <op>.
- The put-back's result becomes a toast with Show stash when it conflicted or failed, and the stash then stays in the list (marked auto in STASHES).
- Interactive rebases and graph edits keep their own `--autostash` and skip their stash question with Always stash on.
- A restack stashes once itself (`glowfly-core/src/stack/restack_end.rs`) and puts the stash back on the branch checked out before; while a stop waits, its stash is a pending entry the ordinary put-back leaves alone (`auto_stash_restack::held`), saved for a restart like the others.

## Custom commands

- Every full load (`update/repo.rs`, after the trust check), Reload custom commands and a watcher burst touching `.glowfly/` send `CustomCommandEffect::Load` with a new `generation`; the answer is dropped when it is older or for another folder.
- Loaded commands get their keys (checked against the keymap and each other) and their problems, logged and toasted only when the set changes (`CustomCommands::reported`).
- A run fills the placeholders from the row (`vars_for`) and may ask first (`Modal::Confirm`).
- A run is then an ordinary job: a git command through `start_job` (`Effect::RunGit` with prompts), a shell command through `begin_job` as `CustomCommandEffect::RunShell`, answered with `JobMsg::JobDone`.
- The repository's own commands in a repository not yet trusted wait in `trust::hold`; in a held repository the jobs' own trust gate holds every command.
