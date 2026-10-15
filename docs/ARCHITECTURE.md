# Trellis architecture

Trellis is a native Git client (Rust, egui, libgit2). This document describes its structure and
the rules that keep it maintainable. Each module's `//!` comment says what it holds in more
detail.

## Goals

- **Small, focused files** (aim for < 400 lines) so a change, by a person or an AI, needs only
  the files for one area.
- **Layers enforced by the compiler and a test**: the UI cannot call git or HTTP directly, and
  core logic cannot depend on egui.
- **Testable without a window**: behavior lives in a pure `update` function; views get
  snapshot tests.
- **One way to do background work**: an `Effect`, run by the runner.

## Layout

Two crates. `trellis-core` is a library with no egui and no app state. `trellis-app` is the
binary (package `trellis`).

### trellis-core (`crates/trellis-core/src/`)

| Module | Holds |
| --- | --- |
| `repo/` | The repository model on libgit2: loading history, refs and status; diffs; staging, line staging, revert, restore; conflicts and their resolution; working-tree files; submodules, worktrees, remotes, shallow-clone boundaries. |
| `gitcli.rs` | The git CLI, for commands that honour hooks, credentials and signing. |
| `askpass/` | Credential prompts: classifying git's and ssh's questions, the helper protocol, the app's listener and the helper's client. |
| `branch_ops.rs`, `tag_ops.rs`, `remote_ops.rs`, `clone_ops.rs`, `shallow_ops.rs`, `worktree_ops.rs`, `stash_ops.rs`, `sparse_ops.rs`, `patch_ops.rs` | Command lines and input checks for branches, tags, remotes, cloning, fetching more of a shallow clone's history, worktrees and stashes (renaming a stash list entry by dropping and storing the top of `refs/stash` again); sparse checkout state, paths and command lines; patch files: format-patch export, git apply command lines and errors. |
| `lfs.rs`, `lfs/` | Git LFS: install and setup state, tracked patterns, LFS command lines. |
| `gitflow.rs`, `gitflow/` | Git Flow with plain git: config, checks, start and finish plans, versions. |
| `rebase/` | Interactive rebase: the plan, the todo file, running `git rebase -i`. |
| `undo.rs` | Undo and redo of Trellis's own operations. |
| `forge/` | Hosting services behind the `Forge` trait (GitHub, GitLab, Bitbucket Cloud and Data Center, Azure DevOps, Jira), each described once in a `ProviderInfo`. |
| `terminal/` | The built-in terminal: screen model, key encoding, PTY sessions. |
| `graph.rs` | Lane layout for the commit graph. |
| `diff_rows.rs`, `whole_file.rs`, `whole_file/`, `conflict_rows.rs`, `intraline.rs`, `line_pairs.rs`, `tabs.rs` | Row layout of diffs, the whole-file view and the conflict view; word-level changes; which removed line sits beside which added line; tab stops. |
| `highlight.rs`, `highlight/` | Syntax highlighting of diff lines (syntect), as token kinds. |
| `watch.rs` | Which file changes in a repository matter. |
| `trust.rs` | Repository trust: config keys and hooks, the repository's own and its checked-out submodules', that make git run programs (read with libgit2 only, at every full load). |
| `fuzzy.rs`, `matching.rs` | Matching for the command palette, filters and search. |
| `commit_query.rs` | Commit search queries: author terms, matching rows, the author list. |
| `settings.rs`, `settings/` | Preferences, graph column widths, per-repository graph filters and repository trust decisions. |
| `persist.rs` | Crash-safe writes of Trellis's own files and of files saved for the user (patches). |
| `dates.rs`, `spell.rs`, `signing.rs`, `avatars.rs`, `text.rs`, `error.rs` | Dates, spell checking, signing keys, avatar fetching, safe display of outside text, typed errors. |
| `ssh_keys.rs`, `ssh_keys/` | SSH key pairs in `~/.ssh`: listing them with `ssh-keygen` fingerprints, and generating Ed25519 keys with `ssh-keygen` (written under a temporary name, then moved into place). |
| `signature_check.rs` | Checking commit signatures through git (`%G?`), so the user's signing programs, keyrings and allowed signers file apply. |

### trellis-app (`crates/trellis-app/src/`)

| Module | Holds |
| --- | --- |
| `main.rs` | Starts the window; also runs as git's sequence editor (`--write-todo`) and askpass helper. |
| `msg/` | `Msg`: user intents and background results, one variant per area carrying that area's enum (`RepoMsg`, `DiffMsg`, `StageMsg`, …, one file per few areas), plus a few cross-cutting messages (`Tick`, `Scoped`, `Confirm`, `Logged`, …). |
| `effect/` | `Effect`: everything slow or outside the process (`mod.rs`), the data some effects carry (`repo.rs`, `forge.rs`), and `Effect::name` (`names.rs`). |
| `lfs.rs`, `gitflow.rs`, `terminal.rs`, `submodule.rs`, `remote.rs`, `tag.rs`, `worktree.rs`, `ssh.rs`, `file_op.rs`, `patch.rs` | Per-area message and effect types of areas with their own data types (one `Msg` variant each). |
| `state/` | `AppState`: plain data, one file per area. Pure rules (why an action is blocked, form checks, labels) live next to their data. |
| `update/` | `update(state, msg, now) -> Vec<Effect>`: one file per area, plus headless tests (`*_tests.rs`). |
| `runner/` | Runs effects on worker threads and reports back with `Msg`s (`mod.rs`), one file per kind of work. |
| `watcher.rs` | The file watcher for the active tab's repository, owned by the runner. |
| `platform.rs` | OS integration: recent repositories and which listed folders are missing, open tabs, terminals, the file manager. |
| `keymap.rs` | The keyboard binding table, shared by shortcuts, the palette and tooltips. |
| `ui/` | egui views, one file or folder per region, plus snapshot tests (`snapshot_tests*.rs`). `ui/mod.rs` is the shell: it owns the runner and dispatches messages. |

### Tests outside `src/`

| Path | Checks |
| --- | --- |
| `crates/trellis-app/tests/layering.rs` | No I/O in `ui/`, `update/` or `state/` (see Rules). |
| `crates/trellis-app/tests/sequence_editor.rs` | A rebase with the built executable as the sequence editor. |
| `crates/trellis-app/tests/askpass.rs` | The built executable as the askpass helper. |
| `crates/trellis-app/tests/snapshots/` | Approved UI images. |

### Data flow (Elm-style)

```
 view(&AppState) ──Msg──▶ update(&mut AppState, Msg) ──Vec<Effect>──▶ runner
       ▲                                                                   │
       └─────────────── Msg::…Done(result) ◀── worker thread ◀─────────────┘
```

- **Views** draw `AppState` and push `Msg`s. They change only text buffers and `view` state
  (scroll, hover, open sections, a drag's preview until its release sends a `Msg`), never domain
  state such as the log, the selection or a form's checks. They never touch disk, network,
  processes or the clock. They may use core types and pure helpers. Avatars, for example, are
  requested by the views, fetched through `Effect::FetchAvatars`, and turned into textures by the
  shell.
- **`update`** is the only place domain state changes. It never blocks and does no I/O. Anything
  slow or outside the process (git, HTTP, the file system) becomes an `Effect`.
- **The runner** (`runner/mod.rs`) starts one worker thread per effect and sends the result back
  as a `Msg`, waking the UI.
- **Tabs**: the active tab's repository data lives directly in `AppState`; switching tabs parks
  it in `state::tabs::Parked` and restores the other tab's. Results of repository-scoped effects
  (`Effect::scope`) come back as `Msg::Scoped`. If their tab is no longer active, only what the
  area's `on_inactive` returns is applied (a log line, an undo entry for that tab, a dialog's
  result); the rest is dropped.
  Paths are resolved on disk once, when a repository loads (`Snapshot::real_workdir`); `update`
  compares them with the pure `state::tabs::same_path`.
- **`AppState`** holds no egui types; textures and fonts live in a `UiCache` owned by the UI
  layer.

## Rules

1. `trellis-core` must not depend on `eframe`/`egui`.
2. `ui/`, `update/` and `state/` do no I/O. `tests/layering.rs` denies whole families (the file
   system, processes, network, threads, environment, clock, libgit2 access) and the core
   functions that do I/O, and names the file and line of each use. The few justified exceptions
   (the shell's startup in `ui/mod.rs`) are listed in its `ALLOWED` table with a reason.
3. New behavior gets an `update` test; new git or API logic gets a `trellis-core` test.
4. New views get an `egui_kittest` snapshot when practical. Views read time from
   `st.view.now` / `st.view.utc_offset` (never the system clock) so images are reproducible.
5. No global mutable state; pass values in. The one exception is the active palette
   (`ui::theme::color`): a thread-local flag the shell sets at the start of each frame, so
   renders on different threads (parallel snapshot tests) can't affect each other. Immutable
   data loaded once (the syntax definitions in `highlight/syntaxes.rs`) is not mutable state,
   and neither is the askpass environment in `gitcli` (`enable_askpass`): set once by the app
   shell at startup, never changed afterwards and never set in tests.
6. Each hosting provider implements `Forge`; the UI never matches on provider-specific types.
7. `scripts/check.ps1` (fmt, clippy -D warnings, tests) must pass before committing.
8. `trellis-core` returns `trellis_core::error::Error` (typed, via `thiserror`); decide behaviour
   from the variant (`is_auth()`, `is_not_found()`), never from message text.

## Recipe: adding a feature

1. **Message.** Add a variant for the user's intent to the area's message enum (`RepoMsg` in
   `msg/repo.rs`, `LfsMsg` in `lfs.rs`, …). A new area gets its own enum, one `Msg` variant
   carrying it, and an arm in `Msg::on_inactive`; only messages no area owns go in `Msg`
   itself.
2. **Update.** Handle it in the area's handler in `update/<area>.rs`, which matches its enum
   exhaustively (no `_` arm, so nothing is dropped silently); `update/mod.rs` only routes each
   `Msg` variant to its handler. Change state there. Keep pure rules (checks, labels, why
   something is blocked) in `state/<area>.rs`.
3. **Effect.** If the work is slow or leaves the process, add an `Effect` variant in
   `effect/mod.rs`, give it a name in `effect/names.rs`, decide in `Effect::scope` and the
   runner's `queue::repo_of` whether it is scoped to a tab and whether it changes the
   repository (both list every effect), and return it from `update`.
4. **Runner.** Execute it in `runner/mod.rs`, or in `runner/<area>.rs` for more than a line.
   The real work (git, files, HTTP) goes in `trellis-core`.
5. **Result.** The runner sends a result message back. Handle it in the same `update` file.
   Drop stale results: compare a generation number, or the request with what is still open.
   Say in the enum's `on_inactive` what the result still needs if its tab is no longer active.
6. **View.** Draw it in `ui/<region>.rs`: read state, push `Msg`s.
7. **Tests.** Write an `update` test (`update/<area>_tests.rs`). Add a core test on a fixture
   repository (see `scratch_repo`) for new git logic, and an `egui_kittest` snapshot for a new
   view.
8. **Settings.** For a new preference, add a field with a default in `settings.rs`
   (`#[serde(default)]`) and a control on the matching Preferences page (`ui/prefs/`).

## Migration status

- [x] 1. Baseline commit, `CLAUDE.md`, rustfmt config, check script, CI
- [x] 2. Extract `trellis-core` (git, graph, forge, settings, dates, spell, avatar fetching); `Forge` trait
- [x] 3. Single task runner replaces the per-feature thread/channel pairs
- [x] 4. Split `AppState` / `Msg` / `update` out of the `TrellisApp` struct, area by area
- [x] 5. Move views into `ui/` modules; remove global atomics (the palette flag is per thread)
- [x] 6. Headless `update` tests, `egui_kittest` snapshots, CI enforcing clippy
- [ ] 7. Resume `TODO.md`, one feature per commit with tests
