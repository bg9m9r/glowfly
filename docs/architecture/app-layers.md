# glowfly-app and glowfly-model: entry points and layers

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here. Features that span every layer are in
[app-features.md](app-features.md); the views are in the `ui-*.md` files.

`msg/`, `effect/`, `state/`, `update/`, `keymap/`, the per-area types, `start_arg.rs` and
`testing.rs` are glowfly-model's (`crates/glowfly-model/src/`); the app re-exports them under the
same names, so `crate::state::…` reads the same in both. The rest is glowfly-app's
(`crates/glowfly-app/src/`).

`runner/` with `watcher.rs` and `watcher/` is glowfly-runner's: `runner/x.rs` below is
`crates/glowfly-runner/src/x.rs`, and `runner/mod.rs` is its `lib.rs`; the app names it `crate::runner`.

`flows/` is glowfly-flows' (`crates/glowfly-flows/src/flows/`), a test-only crate.

## Entry points

- `main.rs`: starts the window, or hands a `glowfly://` argument to the window already running; also runs as git's sequence editor (`--write-todo`), gpg.program wrapper (`GLOWFLY_GPG_WRAPPER`) and askpass helper, and has two command-line modes for scripts (`cli_update.rs`).
- `cli_update.rs`: `glowfly --version` and `glowfly --check-update <folder>` (exit 0 downloaded, 3 up to date, 1 failed, 2 usage): the installer checks' command-line modes, run before logging, settings and the window.
- `logging.rs`: the tracing setup, after the helper modes exit: warnings, errors and panics to `errors.log` (rotated; debug details with extended logging, switched at runtime), and the panic hook.
- `logging.rs` sends `RUST_LOG` to stderr, with each span's time as it closes (`glowfly=debug`: effects and the phases of a load or an index build; `glowfly=trace`: frames, their phases and each message).
- `start_arg.rs` (glowfly-model): `StartArg`, what a new window is asked to open (a folder or a `glowfly://` link) and the message that opens it; `main.rs` reads it from the command line.
- `build.rs`: for Windows, embeds the icon and version details in `glowfly.exe`.
- glowfly-model's `lib.rs`: declares the model's modules; its `test-support` feature adds `testing.rs` and the few test helpers marked `#[doc(hidden)]` for other crates' tests (features only add items).
- glowfly-model's `build.rs`: bundles `CHANGELOG.md` into the program (`OUT_DIR`, read by `state/release_notes.rs`).

## `msg/`: messages

- `Msg`: user intents and background results, one variant per area carrying that area's enum (`RepoMsg`, `DiffMsg`, `StageMsg`, …), one file per few areas.
- `areas.rs`: the table of message areas (variant, enum, short name, handler and its arguments) that builds `Msg`, `Msg::on_inactive`, `Msg::area` and `update`'s routing (`area_router!` in update/mod.rs).
- `init.rs`: `InitMsg`.
- `forge_filters.rs`: `FilterMsg`.
- `issue.rs`: `IssueMsg`, for the issue panel and the Create issue dialog.
- `onboarding.rs`: `OnboardingMsg`, carried by the first-run welcome, the tour, the tips in context and Help > Getting started.
- `dialog.rs`: `DialogMsg`, carried by a dialog's backdrop click, Esc or close button and the discard question.
- `op_preview.rs`: `OpPreviewMsg`, carried by the operation preview's open, result, Preview again and Apply.
- `notify.rs`: `NotifyMsg`, carried by holding and closing toasts and clicks on desktop notifications.
- `lock.rs`: `LockMsg`, Delete lock file's check after a failed job, the confirmed delete and its result.
- `repo_groups.rs`: `GroupMsg`, making, renaming, colouring, ordering and removing repository groups, moving favorites between them and adding repositories to one.
- A few cross-cutting messages live in `Msg` itself (`Tick`, `Scoped`, `Confirm`, `ToastAction`, `Logged`, …).
- `msg/app.rs`: window-level actions from the menu bar and a drawn title bar, and the system's accessibility settings the shell read (update/app.rs).
- `msg/diff.rs`: messages of the diff view, the conflict view and the file history (update/diff.rs, update/highlight.rs, update/conflict.rs, update/history.rs).
- `msg/files.rs`: messages of the file menus: restoring a file from a commit, opening and showing working-tree files, and creating, renaming and deleting them (update/files.rs, update/work_files.rs).
- `msg/keys.rs`: messages of Preferences > Keyboard (update/keys.rs).
- `msg/layout.rs`: messages for the window's zoom, full screen, showing or hiding its panels and how the changed-file lists show their files (update/layout.rs).
- `msg/recent.rs`: messages for the recent and favorite repository lists (update/recent.rs).
- `msg/repos.rs`: messages for finding repositories in project folders, and fetching or pulling many listed repositories at once (update/repos.rs).
- `msg/work.rs`: messages for staging, committing and running git jobs (update/ops.rs, update/commit.rs, update/undo.rs, update/stash.rs).

## `effect/`: effects

- `Effect`: everything slow or outside the process (`mod.rs`): one variant per effect area, from the table in `areas.rs`, and the older flat variants.
- `meta.rs`: `EffectMeta`, what each area decides for its effects beside its type (name, scope, timing, queue, pool hold, stop key, the tests' sandbox rule); tests in `meta_tests.rs`.
- `areas.rs`: the table of effect areas; `effect_match!` runs one body for every area in `Effect`'s metadata, the sandbox's rule and the runner's dispatch.
- The data some effects carry: `repo.rs` (with `LoadKind`: opening, a refresh, or a load for more that goes on from the walk), `forge.rs`, `forge_filters.rs` (PR and issue filter calls), `issue.rs` (the issue panel's and the Create issue dialog's tracker calls and posting a comment), `blame.rs`, `hunk_patch.rs` (Copy hunk as patch's read, run by `runner/hunk_patch.rs`), `notify.rs` (desktop notifications).
- Grouped effects: the Clone dialog (`clone.rs`), the Init dialog (`init.rs`), the SUBMODULES section (`submodule.rs`), interactive rebase (`rebase.rs`), connected accounts (`account.rs`) and the stash list (`stash.rs`).
- `ui.rs`: the effects the UI shell carries out.
- `os.rs`: `OsEffect`, what is handed to the operating system: links, terminals, the file manager, file pickers.
- `prefs.rs`: `PrefsEffect`, the git config values and signing keys of Preferences; a page's save goes as one `WriteGitConfig` batch that reports back (`PrefsMsg::ConfigSaved`).
- `op_preview.rs`: `OpPreviewJob`, the operation preview's job: the work folder and the request.
- `trust_check.rs`: `TrustCheck`, what the runner checks just before a trust-gated effect starts; `Seal`, a check update records beside the effect it pushed; and `Checked` (`Effect::Checked`), an effect carrying its check to the runner.
- `selection.rs`: `SelectionEffect`: comparing two commits or a commit and the working tree, and a multi-selection's stacked list (`StackRequest`).
- `lfs.rs`: `LfsEffect`: whether git-lfs is installed and the LFS page's status.
- `branch.rs`: `BranchEffect`: counting the commits a branch delete would lose, for the Delete dialog, and the Push to all remotes job.
- `lock.rs`: `LockEffect`: checking a lock file a failed job named, and deleting it (on the repository's queue).
- `sparse.rs`: `SparseEffect`: the Sparse Checkout page's state and `git sparse-checkout`.
- `conflict.rs`: `ConflictEffect`: loading a conflicted file and counting the markers in working files.
- `worktree.rs`: `WorktreeEffect`: the worktree folder check, removing, moving and counting other worktrees' changes.
- `monitor.rs`: `MonitorJob` and `MonitorOff`, turning git's file system monitor on and off.
- `pick.rs`: `PickJob`, a cherry-pick or revert run that skips commits changing nothing.
- `stack.rs`: `StackEffect`, `StackPushJob`: a stack's push with the remote tips it saw and whether pull requests follow.
- `names.rs`: `Effect::name`, `timed` (flat variants; areas delegate).
- `scope.rs`: `Effect::scope`, `queue`, `holds` (flat variants, with the fetch and push rule; areas delegate).
- `sandbox.rs`: `Effect::sandbox`, what the whole-app tests' runner does with each effect (`SandboxRule`; flat variants, areas delegate).
- `effect/credential.rs`: saving and dropping HTTPS sign-ins typed in the credential prompt (runner/credentials.rs).
- `effect/repos.rs`: what repositories across folders ask the runner (runner/repos.rs): listing the project folders, and fetching or pulling one listed repository.

## Per-area message and effect types

- `lfs.rs`, `git_env.rs`, `themes.rs`, `gitflow.rs`, `terminal.rs`, `submodule.rs`, `remote.rs`, `tag.rs`, `worktree.rs`, `ssh.rs`, `profile.rs`, `file_op.rs`, `patch.rs`, `tools.rs`, `conflict_check.rs`, `pull_request.rs`, `logs.rs`, `onboarding.rs`, `deep_link.rs`: per-area message and effect types of areas with their own data types (one `Msg` variant each).
- `keymap.rs`: the keyboard bindings: every key Glowfly answers to and where it works (`table.rs`), their ids (`ids.rs`), chords with their display and settings-file forms (`chord.rs`), the effective keymap (`active.rs`) and the rules for changing a key (`rebind.rs`).
- `pull_request/line_comment.rs`: messages of a new code comment on lines of a pull request file's diff (the comment button on a diff line, the hunk header's Comment on N lines, and the comment box in the Files changed pane).

## `state/`: plain data and pure rules

- `AppState`: plain data, one file per area; pure rules (why an action is blocked, form checks, labels) live next to their data.
- `stack_rows.rs`: the LOCAL section's stack tree, kept between frames per snapshot and filter.
- `branch_delete.rs`: the branch Delete dialog's data (the tip, what would be lost, the Git Flow role), its words and the message its danger button sends.
- `branches/push.rs`: which branches a row or toast may force push (refused for the remote's default branch and a Git Flow base), the confirmation naming the commits the remote would lose, the toolbar's confirmation (warning plainly before it overwrites a default or base branch), and Push to all remotes' headline.
- `branches/rejected_push.rs`: the local branch a push rejected as behind was for, read from the job's push command, git's rejection line and its `To` URL (matched on push URLs, one remote only), for the toast's Force push with lease.
- `branch_sync.rs`: the Rename dialog's form and whether it may rename the branch on its remote too (refused for the default branch, a Git Flow base, a branch behind its upstream, a name the remote has), that job's headline by the step it reached, and the Delete remote branch dialog's data (the local branches tracking it, Unset upstream).
- `stack.rs`: why a restack is refused.
- `running_job.rs`: `RunningJob`, the git command sequence that is running and what follows its success.
- `stack_push.rs`: a stack's push targets and why Push stack is off.
- `stack_prs.rs`: a stack's pull request layers, the bottom layer's base, why Create pull requests for the stack is off and the run of create dialogs.
- `palette_stack.rs`: the palette's stack entries.
- `repo_groups.rs`: repository groups' members, the shown order of the favorites (groups first; the favorite keys follow it), a repository's group and colour, and the New group / Rename group form.
- `repo_group_picker.rs`: a group's Add repositories picker: what it offers (favorites outside the group, then Recent), its filter and the ticked rows.
- `clone.rs`, `clone_service.rs`: the Clone dialog.
- `init.rs`: the Init dialog.
- `issue_words.rs`: the words of the issue panel and Create dialog per tracker.
- `rebase_merges.rs`: the rebase panel's merge rows, and its header's texts: the subtitle, the order line and the tip row's "Tip of main".
- `rebase_update_refs.rs`: the rebase panel's update-ref rows, moving a commit with them, the stacked branches an edit leaves on another commit, and the branch aliases that hold Start back.
- `rebase_onto.rs`: rebasing onto a graph commit.
- `stash_confirm.rs`: whether applying, popping or deleting a stash asks first or is refused over conflicts, for every entry point, and the files the stash and the working tree both change; tests in `stash_confirm_tests.rs`.
- `auto_stash.rs`: the auto-stash prompt, its texts and the stashes waiting to go back.
- `job_spec.rs`: `JobSpec`, a git job for `start_job`, and `Retry`, Try again's copy of one with the tab it started in.
- `job_spec/labels.rs`: a job's English label shown in the UI language, for the headlines built from it.
- `palette_nav.rs`: the palette's page and end keys.
- `palette_cache.rs`: the open palette's command list kept between frames (checkout entries until the refs change, the ranking per query); tests in `palette_cache_tests.rs`.
- `palette_selected.rs`: the palette's entries for the selected commit.
- `palette_suggested.rs`: the empty query's Recent, Suggested and All commands groups and which commands the repository's state suggests.
- `op_actions.rs`: the operation in progress: its banner commands.
- `op_steps.rs`: the WIP panel's steps (which operation, each step's state, which are open and their words).
- `commit_stop.rs` (`commit::stop`): what a stopped rebase means for the commit form: no push after committing, Amend and continue at a stop to edit, and Amend offered only where git would amend HEAD.
- `commit_ready.rs` (`commit::ready`): whether the commit box can commit, the first missing step its caption names (staging, then the summary), and the commit button's label.
- `diff_header/cache.rs`: `HeaderCache`, kept in `UiCache::diff_header`: the diff header's change rows, line counts and file order, built again only when the diff, view mode, layout, folds, list (the WIP lists by `Snapshot::status_stamp`) or list mode change. Tests in `diff_header/cache_tests.rs`.
- `file_tree.rs`: a changed-file list grouped in folders: the tree's rows in order (compacted folder chains, folders first), and its file order with which files collapsed folders hide.
- `list_keys.rs`: the keys of a file list with the keyboard focus (`ListKey`), which of them each list takes (`KeySet`), the lists that only open files (`FileList`) and where a step, page or edge move lands in its visible order (`ListMove::target`), for any list built on `ui/list_focus.rs`.
- `file_lists.rs`: the lists that only open files as their keys see them: what each row opens (a diff, a conflict, a folder history file), in on-screen order, and the current row.
- `wip_lists.rs`: the Unstaged and Staged Files lists as their keys see them: their files in on-screen order, the file the keys act on, the Shift-selected files (`WipSpan`) and the move-on after a stage (`MoveOn`).
- `diff_text.rs`: the text selected in the open diff's code (`TextSel`: anchor, head, unit and side, keyed to the diff shown), its line places (`LineRef`), word bounds and `copy`, the text a selection copies (`copy_at_most` for the screen reader's value); `copy_lines` (the row menu's whole lines) and `select_all` (Ctrl+A).
- `file_folds.rs`: the folders collapsed in each changed-file list's tree (in `RepoSession::tab_view`, parked with the tab), and which list a diff was opened from.
- `op_strip.rs`: the strip's text.
- `conflict_keys.rs` (`state::conflict::keys`): what the conflict keys send: take ours, theirs or both for the current conflict, clear it, Mark resolved once enabled, and Continue (the strip's Commit merge or Continue, else the rebase's). Tests in `conflict_keys_tests.rs`; end to end in `flows/conflict_keys_tests.rs`.
- `conflict_strip.rs`: the strip while the conflict view is open: the operation in words, the files left, Abort and Commit merge or Continue with why it waits. Tests in `conflict_strip_tests.rs`.
- `op_confirm.rs`: the danger confirmations of Abort, Skip and Reset bisect, with when an abort keeps the operation's work for Undo.
- `toolbar_tips.rs`: the toolbar's rules: why each button is off, the ahead/behind and last-fetched lines and the Push, Pull, Stash and Unstash previews.
- `section_actions.rs`: what each left-panel section header's + does (its name, why it is off and the message it sends).
- `session.rs`: `RepoSession`, the per-tab views paired with their generation counters (`Gen`, `PrSlot`), the open diff as `DiffSlot` with its counter, the commit detail and draft, the repository reads, the selection (and whether a WIP selection was chosen, `wip_lit`, so its row is drawn selected), forge data, undo history and commit search, and the issue comment drafts kept after their panel closed (`issue_drafts`, `state/issue_comment.rs::KeptDrafts`), parked whole with the tab; `counters()` lists every counter (the reads' included) and `resume_after` keeps them rising across tabs and drops the forge replies in flight (`replies_dropped`); the diff's `shown_gen` is carried rather than counted. Tests in `session_tests.rs`; the switch itself in `flows/tabs_session_tests.rs`.
- `session_reads.rs`: `RepoReads`, what was read from the tab's repository (commit stats, line counts, signatures, conflict check): `parked` splits the results (which go with the tab, requests in flight dropped) from the counters (which stay behind), `clear` forgets them when another repository opens. Tests in `session_reads_tests.rs`.
- `session_view.rs`: `TabView`, the per-tab view state in `RepoSession::tab_view` (search text, ref filter, graph scroll, file-list folds, expanded empty sections, the diff's opened folds and top row with the mode and layout it was counted in, whether its history was freed while parked).
- `empty_sections.rs`: the left-panel sections that start collapsed while empty, and the flag each one's header toggles then.
- `tip.rs`: the rich tooltip's data.
- `system_a11y.rs` (`view::system_a11y`): the system's reduce-motion, high-contrast and text-size settings as the shell read them (`ViewState::system_a11y`, set by `AppMsg::SystemA11y`), and the values in effect with Preferences > Accessibility's choices: `motion_reduced`, `system_contrast`, `zoom_factor`. Tests in `update/tests/system_a11y_tests.rs`.
- `undo_preview.rs`: what Undo and Redo would change.
- `op_preview.rs`: the operation preview's dialog data, its title and headline, why Apply is off (busy, loading, nothing to do or stale) and the message Apply sends, the same as the direct menu item's.
- `refresh/recount.rs`: when other worktrees' changes are counted again after a reload that kept the snapshot (at most once per `RECOUNT_EVERY`).
- `load_more.rs`: a failed load for more history, the graph's row under the last commit (Load more, or the end row's sentence with Fetch full history and Load more) and how far a filtering search has looked.
- `trust_rows.rs` (`trust::rows`): the trust prompt's rows: a setting value's program word (`value_parts`) and the working-tree places a row's Git LFS recognition is checked against (`row_follows`); `trust.rs`'s tests are in `trust_tests.rs`.
- `AppState` holds no egui types; textures and fonts live in a `UiCache` owned by the UI layer.
- `state/activity_rows.rs`: the activity panel's rows, worked out from a tab's log: session headers, one row per run or note, and consecutive successful auto-fetches (routine runs) folded into one group.
- `state/am_banner.rs`: the Applying patches banner on the WIP row while a `git am` series is stopped: the stopped patch and what to do next (the title, which patch of how many, is `op_actions::banner_title`), and its buttons' English labels, the ids the jobs run under.
- `state/app_queries.rs`: `AppState`'s queries: whether hunk indexes are current and line operations blocked, the working folder, load options.
- `state/askpass.rs`: credential prompts from git and ssh (through the askpass helper) waiting for the user.
- `state/blame_find.rs`: find in the blame view and the file history's File view: the query, the lines that match, and the current match.
- `state/blocked.rs`: the reason shared by every action that can't start while another one runs.
- `state/build_status.rs`: CI status of branch tips: which tips are on screen, what the hosting service said about each commit, and when to ask again (update/build_status.rs).
- `state/diff_preview.rs`: the binary diff on screen's two sides: their sizes and decoded images, read in the background (`Effect::LoadBinarySides`, set only by update/diff_preview.rs) for the header's size chip and the before/after image preview.
- `state/files.rs`: the New file and Rename file dialogs' form: the typed path, the last error and whether a request is running; what status says about a path being deleted, the trash's name, the file manager's show-in-folder label, the offer of the home Trash (`TrashOffer`), and the message that asks to discard an Unstaged Files row (its menu item and Delete key send it).
- `state/forge_filter_form.rs`: the filter dialog (`Modal::ForgeFilter`): a saved filter's name and conditions while they are edited, and why it can't be saved yet.
- `state/forks.rs`: the fork dialogs and View on service: Add a fork (pick one of the repository's forks on its hosting service and add it as a remote) and Fork (fork the repository into the user's account, then add and fetch it).
- `state/gitflow_base.rs`: deleting the branch Git Flow names as production or development: the firmer danger confirmation (its own title, a warning and a "… anyway" button), and the title and warning the Delete branch, Delete remote branch and Remove worktree dialogs show.
- `state/gitflow_resume.rs`: a Finish stopped at a merge conflict, remembered for its tab with the steps still to run, and the texts of the window banner, the Git Flow panel row, the in-progress banner's line and the palette entry that offer to continue it (update/gitflow_resume.rs keeps it current).
- `state/graph_rows.rs`: which rows the commit graph lays out (`GraphRows`): this tab's WIP row, other worktrees' WIP rows above their HEAD commits, then the commits; or only the search's matching commits while it filters.
- `state/history_text.rs`: moving within the file history's File view: find, go to line, the cursor line and where the view scrolls next.
- `state/issue_form.rs`: the Create issue dialog (`Modal::CreateIssue`; update/issue_create.rs, ui/issue/create_dialog.rs): the tracker, the fields, the choices loaded from the service (`Effect::Issue`), the live checks, and why the dialog can't be opened at all.
- `state/keys_page.rs`: Preferences > Keyboard page state: the filter, the key being recorded, and the warning or notice under a row.
- `state/onboarding_tour.rs`: the tour of the main window: its five steps, the panel each one points at, and their texts (`tour.*` keys).
- `state/palette.rs`: the command palette (Ctrl+P): whether it is open, what is typed, which row is selected, and the commands it lists for the query.
- `state/palette_checkouts.rs`: the palette's checkout entries: local, remote and tag checkouts, and for Switch branch alone the local branches another worktree has (opening it).
- `state/palette_commands.rs`: the command palette's commands: which are available now, their labels, hidden keywords, details and the global keys that do the same (their current keys come from the effective keymap, `keymap/active.rs`).
- `state/palette_commands_rebase.rs`: the palette's history entries: restore the open rebase panel's original order, reword, squash into parent and drop the selected commit (when that edit can run), rebase the current branch onto it (plainly or interactively), and Continue, Skip and Edit remaining steps while a rebase is in progress.
- `state/palette_commands_repos.rs`: the palette's repository-list entries: favorites, then recent repositories, to open (each but the active one and missing ones), and the list commands: favorite or unfavorite the open repository, open all favorites in tabs, remove missing entries, find repositories in folders, and fetch or pull the favorites or every listed repository.
- `state/palette_commands_submodules.rs`: the palette's SUBMODULES entries: Add submodule… (even without submodules: the section and its menu only show with some), Initialize all and Update all while they have something to do, and per submodule Open, Initialize / Update / Restore, Edit… and Remove…, each with the row's status word.
- `state/palette_commands_worktrees.rs`: the palette's worktree entries: open another worktree, create one from the selected commit or HEAD, and remove, lock, unlock, move or prune them, offered by the same rules as the WORKTREES row menu.
- `state/palette_conflict.rs`: the palette's conflict entries, while files are conflicted or resolved during an operation: Mark all resolved, Mark <path> conflicted for each resolved file git can restore, and Mark all conflicted when there are several.
- `state/palette_files.rs`: the file history finder's rows: the tracked files plus every folder that holds them, a folder shown with a trailing '/'.
- `state/palette_gitflow.rs`: the palette's Git Flow entries: Configure, then once the repository is set up Open Git Flow panel, a Start entry per branch kind, a Start from issue entry per kind (while issues are assigned), a Finish entry per open flow branch, and Continue finishing a Finish stopped at a conflict once its merge is committed.
- `state/palette_issues.rs`: the palette's issue entries: Create issue… whenever the repository has a tracker (with the reason as its detail while creating is blocked) or is on Bitbucket Data Center (which has no tracker of its own: the detail says to choose Jira), and View issue… while the tracker has issue details in Glowfly and issues are listed.
- `state/palette_kinds.rs`: the palette's second steps: the sub-lists a command opens (tracked files, stashes, profiles, assigned issues, branches and tags to switch to) and free-text steps (a stash message, a commit count), with their titles, prompts and footers, and which command opens which step.
- `state/palette_link.rs`: the palette's entries for copying a glowfly:// link to the open repository and to the checked-out branch.
- `state/palette_logs.rs`: the palette's log entries: view the activity, error and performance logs, open the logs folder, export every log as one bundle, and turn extended logging on or off.
- `state/palette_onboarding.rs`: the palette's onboarding entries: "Show the welcome", the tips' on or off switch (whichever applies), "Show tips again" and Getting started always (even when hidden from the Help menu), and "Take the tour" (or "Resume the tour") while a repository is open.
- `state/palette_pr.rs`: the palette's pull request entries for the checked-out branch: create one, and push the branch first when the code host lacks it or some of its commits.
- `state/palette_profiles.rs`: the palette's profile switching: the "Switch profile…" entry and the rows of its sub-list.
- `state/palette_recent.rs`: the palette's per-session memory of recently run commands, and the stable key each plain command is remembered by.
- `state/palette_setup.rs`: the palette's entries that set up or configure a repository feature: Git LFS and sparse checkout (Git Flow has its own file).
- `state/palette_shallow.rs`: the palette's entries for a shallow clone: fetch its full history, or a number of older commits (a free-text step).
- `state/palette_stash.rs`: the palette's stash entries: Stash changes, Stash with message…, the Stash dialog's Stash staged changes… and Stash selected files… (while the tree is dirty), then Unstash the latest stash and the apply, unstash, delete and rename sub-lists (while there are stashes).
- `state/palette_tabs.rs`: the command palette's tab commands: new, reopen (the newest closed tab, or any one on the recently closed list), close, rename and move the active tab.
- `state/palette_tools.rs`: the palette's external tool entries: open the repository in the external editor, the open diff in the diff tool, the open conflict in the merge tool, Preferences > External tools (the editor, diff and merge tools) and Preferences > Terminal.
- `state/palette_updates.rs`: the palette's update entries: "View release notes" and "Check for updates" always, plus "Download Glowfly x.y.z" while a newer release is on offer (not to a Flatpak copy, which Flatpak updates) and, once it has downloaded, "Install Glowfly x.y.z and restart" or "Open the Glowfly x.y.z installer and quit" when it can be installed.
- `state/panel_layout.rs`: every drawer's default size and limits, in one place, and the arithmetic that fits the drawers into the window each frame.
- `state/patch_apply.rs`: the Apply patch dialog (`Modal::ApplyPatch`): what the file holds, how to apply it (its changes with `git apply` options, or as commits with `git am`), why it can't be applied now, and the result texts of both jobs and of the in-progress series' buttons.
- `state/path_hazards.rs`: rows Glowfly won't act on (`glowfly_core::repo::Hazard`): names that aren't UTF-8, and paths that share one file on disk with another index path.
- `state/prefs_save.rs`: the Preferences save model.
- `state/profiles.rs`: Preferences > Profile: which profile the page edits, the text buffers of its fields, the inline rename and New profile forms, and the texts the page, its dialog and toasts show (and the Integrations page's line naming the profile its accounts belong to).
- `state/publish.rs`: the "Publish to GitHub" dialog, offered when pushing a repository that has no remote.
- `state/pull_request_checks.rs`: the pull request panel's Checks section (update/pull_request/checks.rs, ui/pull_request/checks.rs): every check on the pull request's head commit as its service lists it, whether the section is open, when it was read, and the texts that describe it.
- `state/pull_request_commits.rs`: the pull request panel's Commits tab (ui/pull_request/commits.rs): its label, heading and texts.
- `state/rebase_banner.rs`: the rebase banner on the WIP row: its text (the step, the stopped commit and why, a failed command, the autostash note) and what its buttons send.
- `state/rebase_quick.rs`: history edits from the graph's menus (reword, squash, drop): why a menu entry is unavailable, the Reword and Squash dialog's form, and the question asked before a drop runs.
- `state/rebase_remaining.rs`: the rebase panel on the remaining steps of an interactive rebase in progress (`git rebase --edit-todo`): its texts, when it can open, and the request it sends.
- `state/rebase_steps.rs`: the rebase panel's exec and break steps, its row selection, and the squash groups a typed combined message belongs to.
- `state/refresh.rs`: keeping repository reloads from piling up: at most one load and one status read run for the active tab at a time, and requests made meanwhile are merged into one more run afterwards.
- `state/remotes.rs`: the REMOTE section's text and the Add/Edit remote dialog's form and rules, shared by the views and `update` so a disabled item or a refused request shows the same words.
- `state/search.rs`: the graph's commit search: the parsed query, its matches, the filter flag, the author list and the answers for its path terms.
- `state/selection_wip.rs`: one commit compared with the working directory (`Selection::VsWorkdir`): the commit row and the WIP row selected together, and the files that differ from the commit to the working tree (or, swapped, from the working tree to the commit).
- `state/shallow.rs`: shallow clones: which remote more history comes from, whether it can be fetched now, and the graph header's explanation.
- `state/signatures.rs`: signature checks of the active tab's commits, and what the details panel's badge says about one (label and tooltip), worked out without drawing.
- `state/ssh_marks.rs`: Preferences > SSH: what a key card marks about its key (the default key, the one the open repository uses, a key ~/.ssh/config names, a key ssh tries on its own, a key the agent holds), which keys live outside the .ssh folder and which were added with Use existing, the keys only the agent holds, and the state the Agent section's pill shows.
- `state/ssh_service.rs`: Preferences > SSH: adding a public key to a connected hosting-service account, through the inline form under the key's row.
- `state/ssh_use.rs`: Preferences > SSH: the key git uses (core.sshCommand, globally or for the open repository) and the local SSH agent.
- `state/ssh_windows.rs`: Preferences > SSH on Windows: whether to offer pointing git at Windows' own OpenSSH (which reads keys from the OpenSSH Authentication Agent service) through the global core.sshCommand, or to undo that.
- `state/stashes.rs`: the Rename stash dialog's form (`Modal::RenameStash`) and the Stash dialog's form (`Modal::StashPush`): what goes into a new stash, the files it lists and why it can't run.
- `state/status_counts.rs`: the status bar's counts, following the sidebar's lists: the pull requests and issues each section shows under its filter (`forge_filters::pr_view` and `issue_view`).
- `state/submodules.rs`: the SUBMODULES section's text and rules, shared by the view and `update`: the status word, the status dot's and the row's tooltips, why open, update, copy and edit are blocked, the update confirmation (with the commits it would leave behind) and the update command.
- `state/submodules/add.rs`: the Add submodule dialog's form and rules: the URL, the folder (checked as typed against the text and the snapshot, then on disk by the runner) and the optional branch.
- `state/submodules/batch.rs`: updating several submodules in one job: Initialize all, Update all (which asks before leaving commits behind) and the update after a checkout, pull, merge, rebase or reset (which leaves out any submodule where it would), with their texts, and what this session remembers about them.
- `state/submodules/edit.rs`: the Edit submodule dialog's form and rules: its URL and branch in .gitmodules, prefilled with what is there, and the `git submodule set-url` / `set-branch` commands a change runs.
- `state/submodules/remove.rs`: the Remove submodule confirmation: what it deletes, what would be lost (changes inside, commits on no remote-tracking branch, stashes), why removing is unavailable, and the messages for each way the removal can end.
- `state/tool_compare.rs`: rules of the external diff and merge tools: which programs are not set yet (`ToolSetup`: the entries stay enabled and open Preferences > External tools), when the merge tool entry is disabled and why, planning a request from the settings, and what a merge tool run's end says.
- `state/tool_git.rs`: rules and texts for git config's diff and merge tools, used when Preferences > External tools sets no program: what the log and toasts say, how a merge tool run ended (a command Glowfly ran in sh, or `git mergetool`), git's most telling line, and what the External tools page shows about the tool git config names.
- `state/trello.rs`: the Board picker of This repository > Issue tracker, for a Trello tracker.
- `state/view.rs`: transient view state: scroll targets, hover highlights, focus requests and which left-panel sections are expanded.
- `state/worktree_manage.rs`: the Remove and Lock worktree dialogs' forms, and the rules and words for removing, locking, unlocking and pruning worktrees, shared by the views and `update` so a disabled menu item and a refused request say the same thing.
- `state/worktree_move.rs`: the Move worktree dialog's form and the rules and words for moving a worktree, shared by the views and `update` so a disabled menu item and a refused request say the same thing.

## `update/`: state changes

- `update(state, msg, now) -> Vec<Effect>`: one file per area, plus headless tests: those that need no runner in `update/tests/` (`mod.rs` lists them), the ones that run effects (and those sharing helpers with them) in `flows/`; the shared fixtures are in `testing.rs`.
- `prefs.rs`: opening and closing Preferences, the settings file and the settings that apply at once, and the routing of the window's other messages; Accessibility's High contrast check and Show all shortcuts (tests in `update/tests/accessibility_tests.rs`, with the toast lifetimes).
- `toast_log.rs`: every toast leaves an activity line, unless the same update logged a line of its outcome already (the toast is then that line's).
- `prefs_nav.rs`: `enter_page`, the one way a Preferences page is shown (opening on it, its navigation row, its area's `ShowPage`) and what it reads as it opens; tests in `flows/prefs_pages_tests.rs`.
- `prefs_signing.rs`: Commit Signing's key list, Generate GPG key, Extend by 2 years and copying a public key (tests in `update/tests/prefs_signing_tests.rs`).
- `prefs_pick.rs`: the file and folder pickers and where each picked path goes.
- `open_link.rs`: a clicked Markdown link (`Msg::OpenLink`) opened, or first a confirmation naming the real host when its text names another (tests in `update/tests/open_link_tests.rs`).
- `diff_copy.rs`: Copy hunk as patch: the hunk read again from the repository (`Effect::CopyHunkPatch`) and copied when it is UTF-8 text, else a toast. Tests in `flows/diff_copy_tests.rs`.
- `terminal_schemes.rs`: importing terminal color schemes and switching the terminal to the first one.
- `stack.rs`: restacking stacked branches: a rebase job whose results end in `rebase.rs::finished` and whose stopped runs wait in `st.restacks` for the banner's Continue or Skip; a stop whose rebase ended another way ends as the quiet `EndRestack` job (back to the branch checked out before, stash put back, finished part recorded for undo).
- `auto_stash_restack.rs`: a restack's auto-stash as a pending entry while its stop waits (held from the ordinary put-back, saved for a restart), and telling how the restack's own put-back went.
- `stack_push.rs`: pushing a stack as one job with a log line per layer.
- `stack_prs.rs`: creating a stack's pull requests: the push first when a layer isn't on the code host's remote, then one create dialog per layer bottom first, skipping layers with an open one, and stopping on Cancel, Esc, a failed create or a tab switch.
- `op_progress.rs`: the strip's Show work in progress and the banner's confirmed Abort (a kept one runs as `StashEffect::AbortKeeping`).
- `branch_colour.rs`: pinned branch colours (main's too), the preset's hue count and the slots near the trunk the shell reports (`GraphMsg::TrunkNear`) brought into the snapshot at load, on Preferences edits, on a palette change and on a tab's return.
- `repo_colour.rs`: repository colours from the Color submenus.
- `repo_groups.rs`: repository groups: the dialogs, colours, order, Remove group, moving favorites in and out and adding repositories (tests in `update/tests/repo_groups_tests.rs`).
- `trust_seal.rs`: sealing trust-gated jobs, shells and previews for the runner's re-check (`push`), wrapping them and every read that lets git run (`read_in`) as `Effect::Checked` before update returns (`wrap`, from `update::update_sealed`), and holding work again when the re-check finds the repository no longer covered or what it brings in can't be compared (`TrustMsg::Stale`); tests in `flows/trust_recheck_tests.rs`, `trust_recheck_programs_tests.rs` (a tracked config program, a submodule's hook) and `trust_patch_export_tests.rs` (Create patch's export, sealed in `patch_changes.rs`).
- `op_preview.rs`: opening the operation preview, its result for the dialog that asked only, Preview again and Apply, with a merge on a git without `merge-tree` answered at once and a merge preview in a held repository waiting for the trust prompt (`trust::hold_waiting`), since merge-tree runs the repository's merge drivers.
- `pr_heads.rs`: mapping pull request heads to loaded commits on every load, load-more page and PR list: a full id by lookup only, the prefixes in one pass over the loaded commits (tests in `update/tests/pr_heads_tests.rs`).
- `load_more.rs`: a load for more history going on from the walk (the page taken in below the rows on screen), and a failed one kept with its automatic retries (its row in `ui/graph/more.rs`).
- `search_deeper.rs`: a filtering search loading further back in doubling batches.
- `wip_list.rs`: opening the file a WIP list key lands on, moving on to the nearest file after the open one was staged or unstaged as a whole (key, button or menu, alone or with the selected files), asking again for a file still opening (`renew_open`), and following a new file whose staged diff emptied to its untracked view (`emptied`).
- `wip_keys.rs`: the keys of a focused Unstaged or Staged Files list: moves, Shift selection, Space, Enter, Delete through the discard confirmation, and Left/Right to the other list keeping the place. Tests in `flows/wip_keys_tests.rs`.
- `file_list_keys.rs`: the moves of a focused commit, comparison, pull request, folder history, Conflicted or Resolved Files list, opening the file they land on as its click would (never leaving unsaved conflict picks). Tests in `flows/file_list_keys_tests.rs`.
- `branch_delete.rs`: opening the branch Delete dialog (counting first unless the checked-out branch contains the branch), taking in the count for the dialog that asked, and the delete job with its revision check.
- `branch_push.rs`: Force push with lease of any branch with an upstream (refused when its upstream or that upstream's tip changed since the confirmation), the toolbar's Force push of the checked-out branch, and Push to all remotes as one job naming the remotes that failed.
- `branch_sync.rs`: Rename with the remote as one job (rename, `push -u` of the new name, then the old name deleted under a lease) with a headline saying what is left when a step fails, and Delete remote branch with the tracking branches' upstreams unset after it.
- `branch_checkout.rs`: checkout of a local branch, and of a remote branch through a local branch tracking it (one chosen of several, `CheckoutTracking`) fast-forwarded first in the same job when only behind (ancestry check, then `update-ref` leased on the old tip), a diverged one checked out where it is with a note, or a new tracking branch; the New branch dialog opened tracking a remote branch. Checking out HEAD does nothing and shows no toast; nor does checking out a remote branch HEAD tracks, unless HEAD is only behind it (then HEAD is fast-forwarded): level, ahead or diverged, HEAD is left as it is. Tests in `flows/remote_checkout_tests.rs` and `update/tests/branch_checkout_tests.rs`.
- `branch_elsewhere.rs`: a checkout of a local branch another worktree has opens that worktree instead (its tab, else a new one; also mid-rebase or mid-bisect there), noted in the activity log; a missing worktree's branch is refused (once, however often clicked) with a toast offering Prune and check out (`BranchMsg::PruneThen`), which runs `git worktree prune` first in the checkout's job. Tests in `flows/worktree_branch_tests.rs`.
- `clone_https.rs`: clones from a host with a connected account: an https clone's SSH-named submodules over https too (`-c url.<https>.insteadOf`), and Clone over HTTPS as <user> after an SSH clone fails for a key or an unknown host key.
- `job_retry.rs`: Try again: the spec `start_job` kept, offered on lock-file and HTTPS sign-in failures and run again only in its own open, active tab.
- `conflict_count.rs`: a conflicts toast, its run and View conflicts taking the count and files of the first status read after the job, in place of git's CONFLICT lines; that read, or the next job, ends the wait.
- `lock_file.rs`: Delete lock file: the check after a lock failure, the button put first on the run and its toast, the confirmation, the delete's result and the job run again.
- `stage_folder.rs`: Stage folder and Unstage folder from a folder row of the WIP lists' tree (refused while a conflicted file is under it), and staging or unstaging the files a Shift move selected together.
- `update/blame_find.rs`: moving within one blame: find in the file, go to line, and the next or previous run.
- `update/blame_nav.rs`: moving between blames: Blame before this commit and Back, skipping commits, the whitespace, move and wrap toggles, the jump to the file's history, and the header's Diff and File back to the history the blame was opened over.
- `update/build_status.rs`: CI status of branch tips: asking the host about the tips on screen, a few at a time, and recording the answers (state/build_status.rs).
- `update/compare_wip.rs`: comparing one commit with the working directory (`Selection::VsWorkdir`): Ctrl-click on the WIP row or a commit, the commit menu's entry (also in the palette and the commit details), Swap direction, reading the file list in the chosen direction (again after every working-tree status read, so it follows edits), and keeping it across refreshes.
- `update/diff_preview.rs`: the binary diff's sizes and image previews: asked for when a binary diff is shown (`Effect::LoadBinarySides`), kept through a reload of the same file until the new sides arrive, ignored when they come back for a diff that is no longer on screen, and asked for again when a tab comes back with them still loading.
- `update/files.rs`: file actions from file menus: restoring a file as it was in a commit (after a confirmation, recorded for undo with the working copy it replaced), opening a working-tree file in its default program or showing it in the file manager, and creating, renaming and deleting working-tree files (in `work_files.rs`).
- `update/forge_filter_edit.rs`: the filter dialog (`Modal::ForgeFilter`): opening it for a new filter (starting from the one on show) or a saved one, loading its pickers' options, and saving.
- `update/gitflow_finish.rs`: Git Flow Finish: opening the dialog for a feature, release or hotfix branch, running the planned merges, tag and delete as one git job (it stops at the first failing step), and the headline when that job (or a Continue finish job) ends: the plan's summary, or which step stopped it and what is left, remembered for Continue finish when a merge conflicted (gitflow_resume.rs).
- `update/gitflow_resume.rs`: continuing a Finish stopped at a merge conflict: remembering the steps left for the job's tab (active or parked), following each snapshot (the merge seen in progress, committed, or the steps no longer applying), and running the steps left as one job once the merge is committed.
- `update/graph_filter.rs`: hidden and soloed refs in the graph, saved per repository: changing them saves the settings and reloads; each load drops refs that no longer exist.
- `update/history_find.rs`: moving within the file history's File view: find in the shown version and go to line, as the blame view does (`blame_find.rs`).
- `update/history_view.rs`: the history's right panel: the selected file of a folder entry (and stepping through its files), the Diff / File toggle and the File view's Text / Preview switch (both remembered in the settings), and the File view's text of the selected version (`Effect::LoadFileText`).
- `update/inactive.rs`: results for a tab that is no longer active: what each area keeps (`Msg::on_inactive`).
- `update/issue_comment.rs`: posting the issue panel's comment box and quoting a comment into it; the reply appends the posted comment or keeps the draft with the reason, also for a parked tab's panel or the draft kept for a closed one.
- `update/issue_create.rs`: the Create issue dialog: opening it for the active repository's tracker, loading the labels, people, types and epics it offers (per Jira project), creating the issue, and the reply: logged and toasted with a View action, cached, the ISSUES list refreshed, and the New branch dialog opened when asked.
- `update/keys.rs`: Preferences > Keyboard: recording a key for an action, checking it (`keymap::check`), settling a clash by swapping or taking the key, removing keys and resetting them.
- `update/logs_copy.rs`: the logs panel's Copy entry (the row menu, the open row's Copy) and Copy log (the header's More menu, the row menu).
- `update/logs_export.rs`: export logs: ask where to save (a worker thread runs the dialog), then the log worker writes every log as one bundle after the appends queued before it (runner/logs.rs), under a header naming the versions and the platform for a bug report.
- `update/palette.rs`: the command palette: opening and closing it, moving the selection, running a command through the message the rest of the app uses for it (remembering recently run ones), and the second steps two-step commands open (the file finder, the stash, profile and assigned-issue sub-lists, a stash message, a shallow clone's commit count).
- `update/patch_apply.rs`: applying a patch file: the open dialog, reading the chosen file (no git) for the Apply patch dialog, then one `RunGit` job: `git apply` with the chosen options, or `git am` for the series as commits, each with a check after a failure whether the changes are already there.
- `update/prefs_leave.rs`: the unsaved-changes guard of Preferences: a message that would leave a page with edits the Save / Revert bar hasn't saved is held, and "Save your changes?" asks what to do with them (Save, Discard or Cancel).
- `update/prefs_save.rs`: saving and reverting the open Preferences page (the Save / Revert bar, Ctrl/Cmd+S and Esc, and the "Save your changes?" answers), and the "Saved" tick's clock.
- `update/profile_setup.rs`: what follows a profile switch besides the identity: the app catching up with the preferences and connected accounts it loaded (`Settings::swap_profile_setup`), and keeping a shared sign-in while any profile still lists its account.
- `update/profiles.rs`: named profiles: seeding the first from the global identity and keeping the current one in step with git config while Preferences > Profile is open, adding, renaming, editing and deleting (asking first) profiles, and switching, which writes the profile's identity to the global git config in the background and, once that worked, swaps in its accounts and preferences (profile_setup.rs).
- `update/publish.rs`: publishing a repository that has no remote: create a GitHub repository, add it as `origin` and push the current branch to it.
- `update/rebase_quick.rs`: history edits from the graph's menus: preparing the rebase (`Effect::Rebase(PrepareEdit)`), asking first (the Reword and Squash dialog, the drop confirmation, stashing uncommitted changes), and running it as a rebase job whose result ends in `rebase::finished`.
- `update/rebase_remaining.rs`: the rebase panel on the remaining steps of a running interactive rebase: opening it, its list arriving, and Update steps running `git rebase --edit-todo` as the active job.
- `update/rebase_resume.rs`: the rebase banner's Continue and Skip commit: run as a rebase job whose result (`RebaseMsg::RebaseFinished`) says where the rebase stopped next, and the headlines for stopped rebases.
- `update/recent.rs`: the recent and favorite repository lists: removing and clearing recent entries, toggling, reordering and opening favorites, marking folders that are gone and removing them.
- `update/remote_forks.rs`: view a remote on its hosting service, and the fork dialogs: Add a fork (list the repository's forks, pick one, add it as a remote) and Fork (find the user's own fork or fork into their account, then add and fetch it).
- `update/remotes.rs`: the REMOTE section: Add and Edit remote (the dialog's form checked again against the current snapshot), Fetch one remote, and Remove after a confirmation.
- `update/repos.rs`: repositories across folders: the Find repositories dialog and its scan (repo_scan.rs), and fetch or pull across listed repositories (repo_sync.rs).
- `update/revert.rs`: reverting part of a commit or comparison diff into the working tree: Revert Hunk, Revert lines (the line selection) and Revert changes to this file (file-row menus; not for a commit compared with the working tree, whose rows have Restore), and their results, which are recorded in the tab's undo history.
- `update/search.rs`: the graph's search bar: parsing the query, its matches over the loaded history, stepping through them, filtering the graph to them, and the author picker.
- `update/search_paths.rs`: the search's path terms: asking the runner which loaded commits changed a matching path, merging its reports, and keeping the answers across query changes, reloads and tab switches.
- `update/shallow.rs`: shallow clones: fetching more history as a job, and what a reload after a deepen invalidates.
- `update/shortcuts.rs`: the staging, revert, commit-message and sheet keys.
- `update/signatures.rs`: commit signature checks for the details panel's badge.
- `update/ssh_pick.rs`: Preferences > SSH: Use existing (pick a key pair outside the .ssh folder in the OS file dialog, list it and remember it in `Settings::ssh_extra_keys`) and Remove from list (forget one again).
- `update/ssh_service.rs`: Preferences > SSH: adding a key's public half to a connected hosting-service account.
- `update/ssh_use.rs`: Preferences > SSH: the key git uses for SSH remotes (core.sshCommand, globally or for the open, trusted repository), the Use local SSH agent setting, and adding a key to the agent.
- `update/ssh_windows.rs`: Preferences > SSH on Windows: Use Windows' OpenSSH writes the global core.sshCommand that makes git run Windows' own ssh (which reads keys from the agent service), and Use Git's own ssh unsets it.
- `update/stash_push.rs`: the Stash dialog (`Modal::StashPush`): opening it on a choice, and stashing every change, only the staged changes or chosen files, optionally keeping the staged changes staged, as one git job (`stash_ops::push_command`), so hooks, config and Repository trust apply.
- `update/submodule_add.rs`: the Add submodule dialog: opening it, checking its folder on disk as the user types (the view only reads the answer), and running `git submodule add` as a job.
- `update/submodule_edit.rs`: the Edit submodule dialog: opening it prefilled with the submodule's .gitmodules URL and branch, and running `git submodule set-url` / `sync` / `set-branch` as one job.
- `update/submodule_remove.rs`: removing a submodule: reading it again (`SubmoduleEffect::CheckRemove`), the confirmation naming what goes and what would be lost, the removal as a job (`SubmoduleEffect::Remove`) and its result, with its own messages for each partial failure.
- `update/submodules.rs`: the SUBMODULES section: opening a submodule in a tab, and initializing or updating one as a git job.
- `update/tag_annotate.rs`: annotating a lightweight tag from the TAGS row menu: opening the dialog at the commit the snapshot has for the tag, and replacing the tag with an annotated one at that commit in one job.
- `update/tag_checkout.rs`: checking out a tag (detached): one git job named after the tag, its headline, and the one-time detached-HEAD warning with a "Create branch…" action after the first success.
- `update/terminal_tabs.rs`: terminal tabs: several shells per repository, each numbered with the lowest free number (named in state/terminal_names.rs).
- `update/terminal_view.rs`: what the terminal grid asks of its active tab: scrolling the history (by rows, or a screen at a time from the keyboard), copying, a finished mouse selection and selecting all (each offered as the PRIMARY selection, and copied too with Copy on select), clearing, pasting from the clipboard or from PRIMARY (a middle click), and remembering the panel's height.
- `update/tool_compare.rs`: the external diff and merge tools: opening a diff in the diff tool (an unscoped effect that reports only failures), and resolving a conflict in the merge tool as the single active job, ended through `ops::finish_job` when the program exits.
- `update/tool_git.rs`: git config's diff and merge tools, used when Preferences > External tools sets no program: a click reads git config first (`read_then`), and its result runs the tool git names, opens Preferences when it names none, or says why it can't run.
- `update/trello.rs`: the Board picker of This repository > Issue tracker, while Trello is the repository's tracker: each account's boards load once per Preferences session.
- `update/trust_inherit.rs`: passing trust on to the repositories Glowfly makes from a trusted one: a worktree it creates (whose config and hooks are its repository's) and submodules it checks out for the first time (like a clone Glowfly made).
- `update/work_files.rs`: creating, renaming, deleting and discarding (untracked) working-tree files: the New file and Rename dialogs (checked against the snapshot before anything runs), running the delete or discard a confirmation agreed to (the confirmations themselves are in `work_files_delete`), and the results (a toast, the dialog's inline error, or a log line).
- `update/work_files_delete.rs`: deleting working-tree files and discarding untracked ones: the confirmations (worded by what status says about the path and by whether the file goes to the Recycle Bin / Trash), the discard that discarding every line of an untracked file's diff sends instead, the follow-up that offers a permanent delete when the trash can't take a file, and the toasts that say how the file went.

## `runner/`: background work

- `mod.rs`: runs effects on worker threads and reports back with `Msg`s; one file per kind of work.
- `execute.rs`: `Execute` (each effect area's impl lives in its `runner/<area>.rs`) and `Cx`, what an effect may need from the runner (its trust gate and seams among them), with the flat variants' arms.
- `seams.rs`: `Seams`, what the work reaches outside the repositories with: the trash, the sequence editor, the path search's chunk time and where saves of settings, recent repositories and tabs go; `Seams::REAL` in the app, the stand-ins of `testing::SEAMS` in tests.
- `testing.rs` (tests only, and other crates' tests through glowfly-runner's `test-support` feature): the entry points tests run the runner through (`execute`, `execute_checked`, `sandboxed`, the stand-in trash `test_bin`, the saves folder `own_files`, the diff decoration), always with the stand-ins, refusing effects that reach outside the test (those the sandbox drops, bar a few that only read); `testing_tests.rs` checks they never reach the real trash, editor or config folder.
- `lfs.rs`, `selection.rs`: the `Execute` impls of `LfsEffect` and `SelectionEffect`.
- `branches.rs`: the `Execute` impl of `BranchEffect` (the count, and each push of Push to all remotes in turn).
- `lock.rs`: the `Execute` impl of `LockEffect` (`glowfly_core::lock_file`).
- `ui.rs`: the `Execute` impl of `UiEffect`, which only warns: the UI shell carries those out.
- `load.rs`: a repository load: only the status while the refs are unchanged, the next commits of the walk for a load for more, else everything, reusing the commits on screen.
- `op_preview.rs`: works out an operation preview with `glowfly_core::op_preview`.
- `pick.rs`: runs a cherry-pick or revert, skipping the commits that change nothing, and reports `JobMsg::PickDone`.
- `undo_run.rs`: undo and redo as the runner carries them out: plans the action as the repository is now and runs its git commands, through the trust gate when there is one (`UndoGit`), saying whether anything may have changed (`UndoRun::acted`).
- `undo_stash.rs`: Stash changes and undo: stops first when the stash list or index can't be read or intent-to-add files can't be stashed; stashes under a message with a nonce per run (untracked too only when the stash stays) and finds that stash anywhere in the list; runs the undo; after a merge, pull, cherry-pick, revert or rebase undo, or when nothing was undone, puts the changes back with `stash pop --index` only; every git command goes through the trust gate; otherwise the stash stays and the result names it and says why (`UndoStash`).
- `op_stops.rs`: notes where a merge, pull, cherry-pick, revert or rebase job's operation starts (after any auto-stash in front of it) and reports `UndoResult::Stopped` when it stops in progress; a plain abort reports where it ended (`UndoResult::Aborted`), and `UndoHistory::aborted` keeps the older entries only when that is the same place.
- `logs.rs`: the one ordered worker for the log files, attached to what `logging.rs` set up (`LogSetup`).
- `guard.rs`: panic recovery: a panicking effect still sends the result that ends its job or loading state (`Recovery::of`); tests in `guard_tests.rs`.
- `trust.rs`: reading the repositories made from a trusted one, and the re-check just before a sealed effect starts (`verify`, on its worker; a shell's in its session thread): a scan with libgit2 (checked-out submodules too, which git runs inside) compared with the decision it was sent under, and an incoming commit's hooks and config programs that follow the working tree; a read whose check fails reads without git where it can (a status read); a read's scan is reused while its stamp is unchanged (`trust::stamp`); tests in `trust_tests.rs`.
- `trust_gate.rs`: the commits an effect names (a pick, a rebase step, an undo) compared before it starts, and the per-command gate a job's git commands go through where hooks follow the working tree (`Gate::before`: a command it doesn't read held, a pull pinned to what its one fetch brought, each later command compared just before it runs, and the repository scanned again after any command but a fetch); a rebase panel step's `reset` and merge parents are compared too; Trust and run on a prompt that couldn't compare passes that reason at that command only (`Accepted`); tests in `trust_gate_tests.rs`.
- Effects that change a repository, and fetches, keep it out of the repository pool while they run (`Effect::holds`, `repo_pool::hold`); a closed tab lets go of its handles (`BackgroundEffect::Forget`).
- A line written to a built-in terminal holds its repository out of the pool while the shell runs a program, read from its activity probe (`runner/terminal_hold.rs`); what was typed is never read.
- `sandbox.rs`: keeps the journeys' runner (`testing::sandboxed`; none in the app) inside the test: saves go to the test's folder, effects that reach the network, the OS shell, the desktop, file pickers, the credential store or the global git config are dropped by name, and it counts the effects in flight.
- `watcher.rs` (glowfly-runner's `src/`): the file watcher for the active tab's repository, owned by the runner; on Windows its thread also runs the poll's sweeps (`Effect::SweepWatch`, `watcher/sweep_tests.rs`); `watcher/includes.rs` watches the folders of the config files the repository's config includes (`watcher/include_tests.rs`); a burst that touched the config also sends `WatchMsg::ConfigChanged` (`watcher/config_tests.rs`); `watcher/probe.rs` checks after starting that events arrive at all and sends `WatchMsg::Probed` (`watcher/probe_tests.rs`).
- `runner/askpass.rs`: the credential-prompt listener: started once with the app, it reports prompts from the askpass helper as `Msg`s and hands the dialog's answers back to the waiting helper.
- `runner/credentials.rs`: saving and dropping HTTPS sign-ins with git's credential helpers (`CredentialEffect`).
- `runner/expect.rs`: the check a job can carry (`Effect::RunGit { expect }`): a revision named by position, such as `stash@{2}`, must still be the commit the user picked, or nothing runs.
- `runner/files.rs`: restoring a file from a commit with an undo entry (`Effect::RestoreFile`), creating, renaming, deleting and discarding (untracked) a working-tree file (`Effect::WorkFile`), listing the tracked files (`Effect::LoadTrackedPaths`), and checking the Clone dialog's parent folder (`CloneEffect::CheckParent`).
- `runner/forge_calls.rs`: calls to hosting services: pull requests and issues (`Effect::Forge`), and connected accounts (`Effect::Account`): signing in, tokens, owners, creating a repository and a Trello account's boards.
- `runner/forge_filter_calls.rs`: calls of pull request and issue filters (`ForgeCall::Filter`): the signed-in user, issue searches and the filter dialog's options.
- `runner/forks.rs`: the fork dialogs' hosting-service calls (`Effect::Remote`): listing a repository's forks, finding the user's own fork and forking into the user's account.
- `runner/git_ops.rs`: local git operations run by the runner: staging and discarding ops, conflict changes such as Mark resolved, and running a list of git commands (through the trust gate too).
- `runner/instance.rs`: what reaches this window from outside once it runs: the links and folders a second Glowfly process hands to it (`glowfly_core::instance`), reported as `DeepLinkMsg::Open` and `DeepLinkMsg::OpenFolder`, and what the OS hands over as events (`platform::open_events`), reported as their command-line form would be.
- `runner/issues.rs`: the issue panel's and the Create issue dialog's calls to the tracker (`Effect::Issue`): an issue's details, the dialog's choices, creating an issue and posting a comment.
- `runner/keys.rs`: signing keys for the Preferences signing page: listing, generating and exporting them.
- `runner/patches.rs`: the patch file dialogs, saving a patch, reading one for the Apply patch dialog and the text of a commit's patch for the clipboard (`Effect::Patch`).
- `runner/path_search.rs`: the graph search's path terms: which loaded commits changed a matching path.
- `runner/profiles.rs`: switching profiles: writing a profile's identity to the global git config, then reading the config back for Preferences.
- `runner/queue.rs`: one queue per repository for the effects that change it (index, working tree, refs), so a stage, discard or conflict save never runs at the same time as a pull, a checkout or another edit of the same repository: each runs after the ones queued before it.
- `runner/revert.rs`: reverting part of a commit or comparison diff in the working tree (`Effect::Revert`), with the working copy saved before and after so the revert can be undone.
- `runner/saves.rs`: saving Glowfly's own files (settings, recent repositories, open tabs).
- `runner/signatures.rs`: checking a commit's signature with git (Effect::VerifySignature).
- `runner/stashes.rs`: stash list jobs (`Effect::Stash`): renaming an entry, putting an auto-stash back, reading and writing the auto-stashes waiting to go back across restarts, reading the files a stash changes for the question before applying it, and the abort that keeps an operation's work (also run by redoing one).
- `runner/stats.rs`: line counts for the graph's CHANGES column.
- `runner/submodules.rs`: the SUBMODULES section's effects (`Effect::Submodule`): reading one submodule (or all, for a batch update) again before its update or removal is confirmed, checking the Add dialog's folder, and removing one.
- `runner/terminal_out.rs`: a shell's output on its way to the UI thread, which parses it.
- `runner/tools/git_tool.rs`: running the diff and merge tools git config names (Preferences > External tools sets none).

## `runner/platform.rs` and `platform/`: OS integration

- `runner/platform.rs`: which listed folders are missing, the file manager; `runner/platform/terminal.rs`: terminals.
- `runner/platform/state_files.rs`: the recent-repositories and open-tabs files (a bad line costs only itself, a failed read stops saves over the file, private on Unix); its tests in `state_files_tests.rs`.
- `runner/platform/open_events.rs`: the inbox where the folders and links the OS hands over wait until the runner attaches.
- `platform/mod.rs`: the window's glue with the OS, only for the shell and `main.rs` (the files below).
- `platform/a11y_system.rs`: the system's reduce-motion, high-contrast and text-size settings, read for the shell (Windows system parameters and registry, AppKit's `NSWorkspace`, GNOME's gsettings on a thread) through a `Watch` whose reader tests replace.
- `platform/app_id.rs`: the installed Windows copy's app id, taken before the window opens.
- `platform/macos_events.rs`: the folders and links macOS hands Glowfly.app as Apple events, waiting in `runner/platform/open_events.rs` until the runner attaches (`platform::install_open_events`).
- `platform/native_menu.rs`: the macOS system menu bar showing Glowfly's menus: the facade, the queue of clicked items' tags and the hook that turns winit's default app menu off.
- `platform/macos_menu.rs`: the AppKit menus, built and updated from the plan, with the delegate that leaves every key to the window but Cmd+H and Option+Cmd+H.
- `platform/numpad_keys.rs`: Windows Alt codes kept from the pane keys: a winit message hook counts the keypad's digit presses (egui names them like the main row's) and the shell's `raw_input_hook` drops those Alt+digit presses; `platform::event_loop_hook` installs it beside macOS's menu hook.
- `platform/primary_selection.rs`: the Linux PRIMARY selection the terminal sets and middle-click pastes (smithay-clipboard on Wayland, arboard on X11).
- `platform/system_fonts.rs`: the system's Chinese, Japanese and Korean fonts, searched once on a startup thread and kept while each adds a script: mapped from a system font folder, read into memory from anywhere else.
- `platform/system_fonts/candidates.rs`: the known font files of each system, in the order the system language prefers, the Linux font folders searched for them (the system's first), and the system font folders that may be mapped: only those root or TrustedInstaller alone can write, so not macOS's `/Library/Fonts`.
- `platform/native_window/`: the Windows window procedure (see Window chrome in [flow-shell.md](flow-shell.md)).

## `keymap/`: key bindings

- `table.rs`: the binding table.
- `chord.rs`: chords and their text ("Cmd" and "Option" on macOS, through `KeyLabels`) and file forms.
- `active.rs`: the effective keymap with the user's overrides.
- `rebind.rs`: the rebinding rules.
- `tests.rs`, `override_tests.rs`: the table's rows, ids, chord text, clashes and filtering; loading, applying and saving the user's changes from settings.
- `conflict_tests.rs`: `When::Conflict` overlaps the browsing places but not the graph or a text field, and the conflict keys' rows.
- `pane_tests.rs`: the pane keys (Alt+1 to 5, Control on macOS, F6) clash with nothing, the menus' mnemonics included, and `When::Graph` never overlaps a diff's contexts; their update side in `flows/pane_focus_tests.rs`.
- Shared by matching (`ui/shortcuts/global.rs`), the sheet, the palette, tooltips and Preferences > Keyboard.
- The Repository commands group binds palette commands; Fetch, Pull, Push, New branch, Stash changes, Unstash and Switch branch have default keys; `state/palette_bindings.rs` maps commands and bindings both ways, and a press sends `PaletteMsg::RunBoundCommand`, which `update/palette_keys.rs` runs when the palette would list the entry.
- `keymap/ids.rs`: the binding ids, and the stable names the settings file knows them by.

## Test-only modules

- `smoke_probe.rs`, `ui/probe.rs`: the opt-in state file the installer checks read to see the app start and draw (`GLOWFLY_SMOKE_PROBE`, an absolute path; off otherwise, with no thread and AccessKit left alone).
- `ui/probe.rs` builds it as JSON from egui's current pass (AccessKit names and widget rects with their regions, the drawn panels including the drawn title bar, the window frame being drawn as `chrome`, the keyboard focus) and a few `AppState` fields; the shell offers it after each frame.
- `smoke_probe.rs` writes only changed ones, with a rising `seq`, atomically from its own thread, retrying a failed write (a reader holding the file on Windows) until it lands or a newer one replaces it.
- `testing.rs` (glowfly-model; tests only, and other crates' tests through `test-support`): the fixtures every test may use and that run no effect: `state()`, `scratch_repo`, `long_repo`, `clean_tree`, `keys`, `same_place`, the kit's `git`.
- `flows/` (tests only; glowfly-flows, whose `lib.rs` has only `cfg(test)` items: the model's modules and the runner under their app paths, and `mod flows`): `update`'s end-to-end tests, which run effects through the runner (`runner::testing`), and the tests that share helpers with them; `mod.rs` lists them (`pull_request/` for the pull request panel's), and `helpers.rs` has `drive`, `finish` and `finish_keeping` and the effects they skip. Out of `update/` so the model's own tests never need the runner.
- `runner_snapshots/` (tests only): the view snapshot tests whose state comes through the runner (`snapshot_tests_whole.rs`, `snapshot_tests_wrap.rs`, and one test each in `snapshot_tests_conflict_files.rs` and `snapshot_tests_search.rs`), on glowfly-ui's fixtures and approved snapshots.
- `journey/` (tests only): whole-app journeys: the real app (`GlowflyApp::for_test`) in an egui_kittest harness on fixture repositories.
- `journey/find.rs`: driving it by clicking and typing on widgets found by accessible name: a region of the window, the open command palette among them, and a pick among equal names.
- `journey/harness.rs`: waiting until the app is idle after each step.
- `journey/fixture.rs`: checking the screen and the repository on disk.
- `journey/graph_keys.rs`: the graph gets its keys back from Esc on a button or WIP list Tab reached (and that Esc does nothing else), from Tab, and from its pane key.
- `journey/` is in the crate because the package has no library target `tests/` could reach.
- `glyph_check/` (tests only): every non-ASCII character in every crate's string literals and in the translation files has a glyph in the fonts egui draws with.
- `glyph_check/scan.rs` finds the characters with the source checks' shared lexer (glowfly-ui's `tests/view_checks/support/lexer.rs`), skipping comments and test code; `mod.rs` asks the fonts; `NEVER_DRAWN` lists the few that are never shown.
- `glyph_check/` installs the app's bundled fonts through `ui::theme::fonts::install`, as the app does.
- `glyph_check/cjk_dates.rs`: every month, weekday and half-day word of the ja_JP, zh_CN and ko_KR dates has a glyph, in the tests' CJK font and in the machine's own where it has them.
