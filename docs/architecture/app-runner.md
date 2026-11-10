# glowfly-runner and the app's platform glue

Part of [ARCHITECTURE.md](../ARCHITECTURE.md), split from [app-layers.md](app-layers.md) (the
model's layers). One line per file or topic: when you add or move a source file, update its line
here.

`runner/` with `watcher.rs` and `watcher/` is glowfly-runner's: `runner/x.rs` below is
`crates/glowfly-runner/src/x.rs`, and `runner/mod.rs` is its `lib.rs`; the other crates name it `glowfly_runner`.
`platform/` is glowfly-app's (`crates/glowfly-app/src/`).

## `runner/`: background work

- `mod.rs`: runs effects on worker threads and reports back with `Msg`s; one file per kind of work.
- `execute.rs`: `Execute` (each effect area's impl lives in its `runner/<area>.rs`) and `Cx`, what an effect may need from the runner (its trust gate and seams among them); `execute_with` runs an area's impl and scopes its result.
- `seams.rs`: `Seams`, what the work reaches outside the repositories with: the trash, the sequence editor, the path search's chunk time and where saves of settings, recent repositories and tabs go; `Seams::REAL` in the app, the stand-ins of `testing::SEAMS` in tests.
- `testing.rs` (tests only, and other crates' tests through glowfly-runner's `test-support` feature): the entry points tests run the runner through (`execute`, `execute_all` keeping what an effect sends before its result, `execute_checked`, `sandboxed`, the stand-in trash `test_bin`, the saves folder `own_files`, the diff decoration), always with the stand-ins, refusing effects that reach outside the test (those the sandbox drops, bar a few that only read); `testing_tests.rs` checks they never reach the real trash, editor or config folder.
- `lfs.rs`, `selection.rs`: the `Execute` impls of `LfsEffect` and `SelectionEffect`.
- `branches.rs`: the `Execute` impl of `BranchEffect` (the count, and each push of Push to all remotes in turn).
- `lock.rs`: the `Execute` impl of `LockEffect` (`glowfly_core::lock_file`).
- `control.rs`: `ControlEffect`s, carried out by `Runner::run` on the calling thread (watcher, credential answers, stops), never queued, so a stop or an answer reaches its job while that runs; its `Execute` impl only warns; tests in `control_tests.rs`.
- `network.rs`: the `Execute` impl of `NetworkEffect`: a hosting service's calls (`forge_calls.rs`) and avatar downloads.
- `ui.rs`: the `Execute` impl of `UiEffect`, which only warns: the UI shell carries those out.
- `load.rs`: the `Execute` impl of `LoadEffect` (the status reads are in `status.rs`), and a repository load: only the status while the refs are unchanged, the next commits of the walk for a load for more, else everything, reusing the commits on screen.
- `diff.rs`: the `Execute` impl of `DiffEffect`: a diff and its highlighting (`highlight.rs`), the unchanged lines between hunks (`diff_gaps.rs`) and a binary diff's sides.
- `job.rs`: the `Execute` impl of `JobEffect`: a job's git commands, with the expected revision's check, the undo record, the trust gate's re-run, a stop for network commands, the journal note and an auto-stash's saved record (`auto_stash_made.rs`); and undo or redo (`undo_run.rs`).
- `auto_stash_made.rs`: writes an auto-stash job's saved record as soon as its first command made the stash, before the next command, and sends `AutoStashMsg::Made`; tests in `auto_stash_made_tests.rs`.
- `op_preview.rs`: works out an operation preview with `glowfly_core::op_preview`.
- `pick.rs`: runs a cherry-pick or revert, skipping the commits that change nothing, and reports `JobMsg::PickDone`; an auto-stash in front gets its saved record once its push made the stash (`auto_stash_made.rs`).
- `undo_run.rs`: undo and redo as the runner carries them out: plans the action as the repository is now and runs its git commands (a batch's parts one by one), through the trust gate when there is one (`UndoGit`), saying whether anything may have changed (`UndoRun::acted`).
- `undo_stash.rs`: Stash changes and undo: stops first when the stash list or index can't be read or intent-to-add files can't be stashed; stashes under a message with a nonce per run (untracked too only when the stash stays) and finds that stash anywhere in the list; runs the undo; after a merge, pull, cherry-pick, revert or rebase undo, or when nothing was undone, puts the changes back with `stash pop --index` only; every git command goes through the trust gate; otherwise the stash stays and the result names it and says why (`UndoStash`).
- `op_stops.rs`: notes where a merge, pull, cherry-pick, revert or rebase job's operation starts (after any auto-stash in front of it) and reports `UndoResult::Stopped` when it stops in progress; a plain abort reports where it ended (`UndoResult::Aborted`), and `UndoHistory::aborted` keeps the older entries only when that is the same place.
- `logs.rs`: the one ordered worker for the log files, attached to what `logging.rs` set up (`LogSetup`).
- `guard.rs`: panic recovery: a panicking effect still sends the result that ends its job or loading state (`Recovery::of`), a read's (load, status, blame, line counts) as `ReadFailed::Panicked`; tests in `guard_tests.rs`.
- `trust.rs`: reading the repositories made from a trusted one, checking which decided folders still exist (`platform::check_folders`), and the re-check just before a sealed effect starts (`verify`, on its worker; a shell's in its session thread): a scan with libgit2 (checked-out submodules too, which git runs inside) compared with the decision it was sent under, and an incoming commit's hooks and config programs that follow the working tree; a read whose check fails reads without git where it can (a status read); a read's scan is reused while its stamp is unchanged (`trust::stamp`); tests in `trust_tests.rs`.
- `trust_gate.rs`: the commits an effect names (a pick, a rebase step, an undo) compared before it starts, and the per-command gate a job's git commands go through where hooks follow the working tree (`Gate::before`: a command it doesn't read held, a pull pinned to what its one fetch brought, each later command compared just before it runs, and the repository scanned again after any command but a fetch); a rebase panel step's `reset` and merge parents are compared too; Trust and run on a prompt that couldn't compare passes that reason at that command only (`Accepted`); tests in `trust_gate_tests.rs`.
- Effects that change a repository, and fetches, keep it out of the repository pool while they run (`Effect::holds`, `repo_pool::hold`); a closed tab lets go of its handles (`BackgroundEffect::Forget`).
- A line written to a built-in terminal holds its repository out of the pool while the shell runs a program, read from its activity probe (`runner/terminal_hold.rs`); what was typed is never read.
- `sandbox.rs`: keeps the journeys' runner (`testing::sandboxed`; none in the app) inside the test: saves go to the test's folder, effects that reach the network, the OS shell, the desktop, file pickers, the credential store or the global git config are dropped by name, and it counts the effects in flight and those that panicked (caught by `guard::guarded_noting` or unwound).
- `watcher.rs` (glowfly-runner's `src/`): the file watcher for the active tab's repository, owned by the runner; on Windows its thread also runs the poll's sweeps (`ControlEffect::SweepWatch`, `watcher/sweep_tests.rs`); `watcher/includes.rs` watches the folders of the config files the repository's config includes (`watcher/include_tests.rs`); a burst that touched the config also sends `WatchMsg::ConfigChanged` (`watcher/config_tests.rs`); `watcher/probe.rs` checks after starting that events arrive at all and sends `WatchMsg::Probed` (`watcher/probe_tests.rs`).
- `runner/askpass.rs`: the credential-prompt listener: started once with the app, it reports prompts from the askpass helper as `Msg`s and hands the dialog's answers back to the waiting helper.
- `runner/credentials.rs`: saving and dropping HTTPS sign-ins with git's credential helpers (`CredentialEffect`).
- `runner/expect.rs`: the check a job can carry (`JobEffect::RunGit { expect }`): a revision named by position, such as `stash@{2}`, must still be the commit the user picked, or nothing runs.
- `runner/files.rs`: the `Execute` impl of `FilesEffect`: restoring a file from a commit with an undo entry, creating, renaming, deleting and discarding (untracked) a working-tree file, listing the tracked files, a staging or conflict edit (`git_ops.rs`), reverting part of a diff (`revert.rs`) and discarding or deleting several files at once (`discard_paths.rs`, `delete_paths.rs`); and checking the Clone dialog's parent folder (`CloneEffect::CheckParent`).
- `runner/history.rs`: the `Execute` impl of `HistoryEffect` (a file's or folder's history; the path search and the line counts when they reach a worker rather than `Runner::run`'s own), and a file's text as of a commit for the File view, with its syntax spans and previews.
- `runner/forge_calls.rs`: calls to hosting services: pull requests and issues (`NetworkEffect::Forge`), and connected accounts (`Effect::Account`): signing in, tokens, owners, creating a repository and a Trello account's boards.
- `runner/forge_filter_calls.rs`: calls of pull request and issue filters (`ForgeCall::Filter`): the signed-in user, issue searches and the filter dialog's options.
- `runner/forks.rs`: the fork dialogs' hosting-service calls (`Effect::Remote`): listing a repository's forks, finding the user's own fork and forking into the user's account.
- `runner/delete_paths.rs`: a multi-file delete: each working copy through the single delete's core calls (staged-new ones also leave the index), in order, stopping at the first failure, with one undo entry for every file that went.
- `runner/discard_paths.rs`: a multi-file discard: tracked files through the single discard's `LocalOp::DiscardFile`, untracked ones deleted as the confirmation said, in order, stopping at the first failure, with one undo entry for every file discarded.
- `runner/git_ops.rs`: local git operations run by the runner: staging and discarding ops, conflict changes such as Mark resolved, and running a list of git commands (through the trust gate too).
- `runner/instance.rs`: what reaches this window from outside once it runs: the links and folders a second Glowfly process hands to it (`glowfly_core::instance`), reported as `DeepLinkMsg::Open` and `DeepLinkMsg::OpenFolder`, and what the OS hands over as events (`platform::open_events`), reported as their command-line form would be.
- `runner/issues.rs`: the issue panel's and the Create issue dialog's calls to the tracker (`Effect::Issue`): an issue's details, the dialog's choices, creating an issue and posting a comment.
- `runner/keys.rs`: signing keys for the Preferences signing page: listing, generating and exporting them.
- `runner/patches.rs`: the patch file dialogs, saving a patch, reading one for the Apply patch dialog and the text of a commit's patch for the clipboard (`Effect::Patch`).
- `runner/path_search.rs`: the graph search's path terms: which loaded commits changed a matching path.
- `runner/profiles.rs`: switching profiles: writing a profile's identity to the global git config, then reading the config back for Preferences.
- `runner/queue.rs`: one queue per repository for the effects that change it (index, working tree, refs), so a stage, discard or conflict save never runs at the same time as a pull, a checkout or another edit of the same repository: each runs after the ones queued before it.
- `runner/revert.rs`: reverting part of a commit or comparison diff in the working tree (`FilesEffect::Revert`), with the working copy saved before and after so the revert can be undone.
- `runner/saves.rs`: the `Execute` impl of `SaveEffect` and its own thread per save: saving Glowfly's own files (settings, recent repositories, open tabs), each file's saves in order; `save_in` writes one into a folder (the config folder, or the sandbox's), and what a save of recent.txt or tabs.txt did besides writing, or why it was refused, comes back as `RecentMsg::StateFileSaved` / `StateFileBlocked`; tests in `saves_tests.rs`.
- `runner/session.rs`: the `Execute` impl of `SessionEffect`: checking the recent repositories' folders and reading the restored tabs' groups.
- `runner/signatures.rs`: checking a commit's signature with git (`CommitEffect::VerifySignature`).
- `runner/commit.rs`: the `Execute` impl of `CommitEffect`: a commit's detail and its signature check (`signatures.rs`).
- `runner/stashes.rs`: stash list jobs (`Effect::Stash`): renaming an entry, putting an auto-stash back, reading and writing the auto-stashes waiting to go back across restarts, reading the files a stash changes for the question before applying it, and the abort that keeps an operation's work (also run by redoing one).
- `runner/stats.rs`: line counts for the graph's CHANGES column.
- `runner/svg_worker.rs` (tests in `svg_worker_tests.rs`): the generic preview `Worker` (one named thread per preview kind) and the one that draws SVG previews for `DiffEffect::BinarySides`: one drawing at a time, a newer request replacing the queued one, callers giving up after 3 s (the side shows "Can't preview this SVG") while the drawing runs on.
- `runner/font_worker.rs`: the font previews' own `Worker` thread, with the same 3 s deadline (the side shows "Can't preview this font").
- `runner/submodules.rs`: the SUBMODULES section's effects (`Effect::Submodule`): reading one submodule (or all, for a batch update) again before its update or removal is confirmed, checking the Add dialog's folder, and removing one.
- `runner/terminal_out.rs`: a shell's output on its way to the UI thread, which parses it.
- `runner/tools/git_tool.rs`: running the diff and merge tools git config names (Preferences > External tools sets none).

## `runner/platform.rs` and `platform/`: OS integration

- `runner/platform.rs`: which listed folders are missing, the file manager; `runner/platform/terminal.rs`: terminals.
- `runner/platform/state_files.rs`: what a session holds about the recent-repositories and open-tabs files: unread (a failed start read: each save tries one read and writes nothing while it fails, reporting the first refusal), skipped lines (the file is copied before the first save replaces it) and carried entries (what a late read found, merged into every save until the session saves them itself); a leading BOM is dropped; private on Unix. Tests in `state_files_tests.rs` and `state_files_held_tests.rs`.
- `runner/platform/state_files/recent.rs`: recent.txt: reading, parsing (with the lines skipped), saving with the carried entries merged and capped, and Clear recent (`clear_recent`), which empties it without them; old backups pruned at start.
- `runner/platform/state_files/tabs.rs`: tabs.txt: reading, parsing (with the lines skipped), and saving with the carried tabs appended after the open ones (never the active one); old backups pruned at start.
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
