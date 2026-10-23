# glowfly-app: entry points and layers (`crates/glowfly-app/src/`)

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here. Features that span every layer are in
[app-features.md](app-features.md); the views are in the `ui-*.md` files.

## Entry points

- `main.rs`: starts the window, or hands a `glowfly://` argument to the window already running; also runs as git's sequence editor (`--write-todo`), gpg.program wrapper (`GLOWFLY_GPG_WRAPPER`) and askpass helper, and has two command-line modes for scripts (`cli_update.rs`).
- `cli_update.rs`: `glowfly --version` and `glowfly --check-update <folder>` (exit 0 downloaded, 3 up to date, 1 failed, 2 usage): the installer checks' command-line modes, run before logging, settings and the window.
- `logging.rs`: the tracing setup, after the helper modes exit: warnings, errors and panics to `errors.log` (rotated; debug details with extended logging, switched at runtime), and the panic hook.
- `logging.rs` sends `RUST_LOG` to stderr, with each span's time as it closes (`glowfly=debug`: effects and the phases of a load or an index build; `glowfly=trace`: frames, their phases and each message).
- `build.rs`: bundles `CHANGELOG.md` into the program (`OUT_DIR`) and, for Windows, embeds the icon and version details in `glowfly.exe`.

## `msg/`: messages

- `Msg`: user intents and background results, one variant per area carrying that area's enum (`RepoMsg`, `DiffMsg`, `StageMsg`, …), one file per few areas.
- `init.rs`: `InitMsg`.
- `forge_filters.rs`: `FilterMsg`.
- `issue.rs`: `IssueMsg`, for the issue panel and the Create issue dialog.
- `onboarding.rs`: `OnboardingMsg`, carried by the first-run welcome and the tour.
- `dialog.rs`: `DialogMsg`, carried by a dialog's backdrop click, Esc or close button and the discard question.
- `op_preview.rs`: `OpPreviewMsg`, carried by the operation preview's open, result, Preview again and Apply.
- `notify.rs`: `NotifyMsg`, carried by holding and closing toasts and clicks on desktop notifications.
- A few cross-cutting messages live in `Msg` itself (`Tick`, `Scoped`, `Confirm`, `ToastAction`, `Logged`, …).

## `effect/`: effects

- `Effect`: everything slow or outside the process (`mod.rs`).
- The data some effects carry: `repo.rs` (with `LoadKind`), `forge.rs`, `forge_filters.rs` (PR and issue filter calls), `issue.rs` (the issue panel's and the Create issue dialog's tracker calls and posting a comment), `blame.rs`, `notify.rs` (desktop notifications).
- Grouped effects: the Clone dialog (`clone.rs`), the Init dialog (`init.rs`), the SUBMODULES section (`submodule.rs`), interactive rebase (`rebase.rs`), connected accounts (`account.rs`) and the stash list (`stash.rs`).
- `ui.rs`: the effects the UI shell carries out.
- `os.rs`: `OsEffect`, what is handed to the operating system: links, terminals, the file manager, file pickers.
- `prefs.rs`: `PrefsEffect`, the git config values and signing keys of Preferences.
- `op_preview.rs`: `OpPreviewJob`, the operation preview's job: the work folder and the request.
- `selection.rs`: `StackRequest`, a multi-selection's stacked list request.
- `pick.rs`: `PickJob`, a cherry-pick or revert run that skips commits changing nothing.
- `stack.rs`: `StackEffect`, `StackPushJob`: a stack's push with the remote tips it saw and whether pull requests follow.
- `names.rs`: `Effect::name`.
- `scope.rs`: `Effect::scope`.

## Per-area message and effect types

- `lfs.rs`, `git_env.rs`, `themes.rs`, `gitflow.rs`, `terminal.rs`, `submodule.rs`, `remote.rs`, `tag.rs`, `worktree.rs`, `ssh.rs`, `profile.rs`, `file_op.rs`, `patch.rs`, `tools.rs`, `conflict_check.rs`, `pull_request.rs`, `logs.rs`, `onboarding.rs`, `deep_link.rs`: per-area message and effect types of areas with their own data types (one `Msg` variant each).

## `state/`: plain data and pure rules

- `AppState`: plain data, one file per area; pure rules (why an action is blocked, form checks, labels) live next to their data.
- `stack_rows.rs`: the LOCAL section's stack tree.
- `stack.rs`: why a restack is refused.
- `stack_push.rs`: a stack's push targets and why Push stack is off.
- `stack_prs.rs`: a stack's pull request layers, the bottom layer's base, why Create pull requests for the stack is off and the run of create dialogs.
- `palette_stack.rs`: the palette's stack entries.
- `clone.rs`, `clone_service.rs`: the Clone dialog.
- `init.rs`: the Init dialog.
- `issue_words.rs`: the words of the issue panel and Create dialog per tracker.
- `rebase_merges.rs`: the rebase panel's merge rows.
- `rebase_onto.rs`: rebasing onto a graph commit.
- `stash_confirm.rs`: whether applying, popping or deleting a stash asks first, for every entry point.
- `auto_stash.rs`: the auto-stash prompt, its texts and the stashes waiting to go back.
- `palette_nav.rs`: the palette's page and end keys.
- `palette_selected.rs`: the palette's entries for the selected commit.
- `palette_suggested.rs`: the empty query's Recent, Suggested and All commands groups and which commands the repository's state suggests.
- `op_actions.rs`: the operation in progress: its banner commands.
- `op_steps.rs`: the WIP panel's steps (which operation, each step's state, which are open and their words).
- `op_strip.rs`: the strip's text.
- `op_confirm.rs`: the danger confirmations of Abort, Skip and Reset bisect, with when an abort keeps the resolved changes.
- `toolbar_tips.rs`: the toolbar's rules: why each button is off, the ahead/behind and last-fetched lines and the Push, Pull, Stash and Pop previews.
- `section_actions.rs`: what each left-panel section header's + does (its name, why it is off and the message it sends).
- `tip.rs`: the rich tooltip's data.
- `undo_preview.rs`: what Undo and Redo would change.
- `op_preview.rs`: the operation preview's dialog data, its title and headline, why Apply is off (busy, loading, nothing to do or stale) and the message Apply sends, the same as the direct menu item's.
- `load_more.rs`: a failed load for more history, the graph's row under the last commit (Load more, or the end row's sentence with Fetch full history and Load more) and how far a filtering search has looked.
- `AppState` holds no egui types; textures and fonts live in a `UiCache` owned by the UI layer.

## `update/`: state changes

- `update(state, msg, now) -> Vec<Effect>`: one file per area, plus headless tests (`*_tests.rs`, declared in `update/test_mods.rs`).
- `terminal_schemes.rs`: importing terminal color schemes and switching the terminal to the first one.
- `stack.rs`: restacking stacked branches: a rebase job whose results end in `rebase.rs::finished` and whose stopped runs wait in `st.restacks` for the banner's Continue or Skip.
- `stack_push.rs`: pushing a stack as one job with a log line per layer.
- `stack_prs.rs`: creating a stack's pull requests: the push first when a layer isn't on the code host's remote, then one create dialog per layer bottom first, skipping layers with an open one, and stopping on Cancel, Esc, a failed create or a tab switch.
- `op_progress.rs`: the strip's Show work in progress and the banner's confirmed Abort.
- `branch_colour.rs`: pinned branch colours and the preset's hue count brought into the snapshot at load, on Preferences edits and on a tab's return.
- `repo_colour.rs`: repository colours from the Color submenus.
- `op_preview.rs`: opening the operation preview, its result for the dialog that asked only, Preview again and Apply, with a merge on a git without `merge-tree` answered at once.
- `load_more.rs`: a failed load for more history kept with its automatic retries (its row in `ui/graph/more.rs`).
- `search_deeper.rs`: a filtering search loading further back in doubling batches.
- `wip_list.rs`: Up/Down between the Unstaged or Staged Files, moving on to the neighbouring file after S or U, and asking again for a file still opening (`renew_open`).

## `runner/`: background work

- `mod.rs`: runs effects on worker threads and reports back with `Msg`s; `execute.rs` maps each effect to its work; one file per kind of work.
- `op_preview.rs`: works out an operation preview with `glowfly_core::op_preview`.
- `pick.rs`: runs a cherry-pick or revert, skipping the commits that change nothing, and reports `JobMsg::PickDone`.
- `logs.rs`: the one ordered worker for the log files.
- Effects that change a repository, and fetches, keep it out of the repository pool while they run (`queue::holds`, `repo_pool::hold`); a closed tab lets go of its handles (`BackgroundEffect::Forget`).
- `sandbox.rs` (tests only): keeps the journeys' runner inside the test: saves go to the test's folder, effects that reach the network, the OS shell, the desktop, file pickers, the credential store or the global git config are dropped by name, and it counts the effects in flight.
- `watcher.rs` (in `src/`): the file watcher for the active tab's repository, owned by the runner; on Windows its thread also runs the poll's sweeps (`Effect::SweepWatch`, `watcher/sweep_tests.rs`).

## `platform.rs`: OS integration

- Recent repositories and which listed folders are missing, open tabs, terminals, the file manager.
- `platform/icon.rs`: the window icon.
- `platform/app_id.rs`: the installed Windows copy's app id, taken before the window opens.
- `platform/macos_events.rs`: the folders and links macOS hands Glowfly.app as Apple events, waiting in `platform/open_events.rs` until the runner attaches.
- `platform/native_menu.rs`: the macOS system menu bar showing Glowfly's menus: the facade, the queue of clicked items' tags and the hook that turns winit's default app menu off.
- `platform/macos_menu.rs`: the AppKit menus, built and updated from the plan, with the delegate that leaves every key to the window but Cmd+H and Option+Cmd+H.
- `platform/native_window/`: the Windows window procedure (see Window chrome in [flow-shell.md](flow-shell.md)).

## `keymap/`: key bindings

- `table.rs`: the binding table.
- `chord.rs`: chords and their text ("Cmd" and "Option" on macOS, through `KeyLabels`) and file forms.
- `active.rs`: the effective keymap with the user's overrides.
- `rebind.rs`: the rebinding rules.
- Shared by matching (`ui/shortcuts/global.rs`), the sheet, the palette, tooltips and Preferences > Keyboard.
- The Repository commands group binds palette commands and has no default keys; `state/palette_bindings.rs` maps commands and bindings both ways, and a press sends `PaletteMsg::RunBoundCommand`, which `update/palette_keys.rs` runs when the palette would list the entry.

## Test-only modules

- `smoke_probe.rs`, `ui/probe.rs`: the opt-in state file for the real-input smoke tests (`GLOWFLY_SMOKE_PROBE`, an absolute path; off otherwise, with no thread and AccessKit left alone).
- `ui/probe.rs` builds it as JSON from egui's current pass (AccessKit names and widget rects with their regions, the drawn panels including the drawn title bar, the window frame being drawn as `chrome`, the keyboard focus) and a few `AppState` fields; the shell offers it after each frame.
- `smoke_probe.rs` writes only changed ones, with a rising `seq`, atomically from its own thread, retrying a failed write (a reader holding the file on Windows) until it lands or a newer one replaces it.
- `journey/` (tests only): whole-app journeys: the real app (`GlowflyApp::for_test`) in an egui_kittest harness on fixture repositories.
- `journey/find.rs`: driving it by clicking and typing on widgets found by accessible name: a region of the window, the open command palette among them, and a pick among equal names.
- `journey/harness.rs`: waiting until the app is idle after each step.
- `journey/fixture.rs`: checking the screen and the repository on disk.
- `journey/` is in the crate because the package has no library target `tests/` could reach; `src/journey/probe.rs` dry-runs the smoke tests' labels.
- `glyph_check/` (tests only): every non-ASCII character in the app's and core's string literals and in the translation files has a glyph in the fonts egui draws with.
- `glyph_check/scan.rs` finds the characters, skipping comments and test code; `mod.rs` asks the fonts; `NEVER_DRAWN` lists the few that are never shown.
- `glyph_check/` installs the app's bundled fonts through `ui::theme::fonts::install`, as the app does.
