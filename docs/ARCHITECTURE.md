# Glowfly architecture

Glowfly is a native Git client (Rust, egui, libgit2). This document describes its structure and
the rules that keep it maintainable. Each module's `//!` comment says what it holds in more
detail. `docs/PLATFORMS.md` holds the rules for OS and file-system differences (paths, files,
processes, the environment, git).

## Goals

- **Small, focused files** (aim for < 400 lines) so a change, by a person or an AI, needs only
  the files for one area.
- **Layers enforced by the compiler and a test**: the UI cannot call git or HTTP directly, and
  core logic cannot depend on egui.
- **Testable without a window**: behavior lives in a pure `update` function; views get
  snapshot tests.
- **One way to do background work**: an `Effect`, run by the runner.

## Layout

Five crates. `glowfly-core` is a library with no egui and no app state. `glowfly-model` is a
library with the app's state, messages, effects and `update`, and no egui. `glowfly-app` is the
binary (package `glowfly`).
`glowfly-runner` is a library that carries out the model's effects on worker threads, with no egui.
`glowfly-flows` holds only tests: `update`'s end-to-end tests, run through the runner.

### Per-area files (`docs/architecture/`)

What each module and file holds is listed one line per file or topic in these files, so a change
reads only its area and two changes landing together merge line by line. When you add or move a
source file, update its line in the matching file.

- [core-repo.md](architecture/core-repo.md): `glowfly-core`'s repository model (`repo/`): loading and status, diffs, history, staging and revert, conflicts, submodules, worktrees.
- [core-ops.md](architecture/core-ops.md): the git CLI (`gitcli.rs`), partial clones, command lines (`*_ops.rs`), LFS, Git Flow, rebase, stacks, undo, the journal, range-diff, operation previews, trust, custom commands, several-repository scans and syncs.
- [core-display.md](architecture/core-display.md): the commit graph and branch colours, diff, whole-file and conflict rows, syntax highlighting, image and Markdown previews, search and the history index, watching for changes.
- [core-system.md](architecture/core-system.md): which git runs, paths and errors, credentials and signing, SSH keys, one window and `glowfly://` links, the terminal, settings, identity and profiles, files and logs, translations and dates, external tools, Glowfly's own updates.
- [core-forge.md](architecture/core-forge.md): hosting services (`forge/`).
- [app-layers.md](architecture/app-layers.md): `glowfly-app`'s and `glowfly-model`'s entry points (`main.rs`, `start_arg.rs`, `cli_update.rs`, `logging.rs`, `build.rs`) and its layers: `msg/`, `effect/`, `state/`, `update/`, `runner/`, `platform/` (window glue) and `runner/platform.rs` (OS helpers), `keymap/`, and the test-only modules (`journey/`, `glyph_check/`, the smoke probe).
- [app-features.md](architecture/app-features.md): features whose files span every layer: updates, the journal, custom commands, partial clones, the history index, range-diff, profile folder rules, multi-commit selection.
- [ui-shell.md](architecture/ui-shell.md): the shell (`shell/`), window layout, toolbar, repository tabs, terminal panel.
- [ui-views.md](architecture/ui-views.md): dialogs with their own files, blame and the File view, Markdown, the graph, commit details and the WIP panel, the diff, object menus, snapshot tests.
- [ui-widgets.md](architecture/ui-widgets.md): `widgets/`, rows and lists, disabled reasons, tooltips, empty and banner states, the dialog frame.
- [ui-theme.md](architecture/ui-theme.md): `theme/`: controls and modals, colours, fonts and the type scale, spacing and tokens.
- [tests.md](architecture/tests.md): tests outside `src/`, the fuzz targets and the installer checks.
- [flow-repo.md](architecture/flow-repo.md): how repository flows work: diffs, tabs, job follow-ups, pull requests, the journal, range-diff, the dashboard, several repositories, incremental status, worktrees, submodules, auto-stash, custom commands.
- [flow-shell.md](architecture/flow-shell.md): how the shell's flows work: dialogs, toasts, zoom and panels, window chrome, menus and menu keys, quitting, notifications, credential and signing prompts, LFS notices, logs, deep links, releases and updates.

### Finding things

- A module's `//!` comment says what it holds; start there once you know the file.
- To find a file, search the `docs/architecture/` files for its area's words or its name; each file or topic is on one line.
- To find a feature's code, look for its message enum (`msg/<area>.rs`), its handler (`update/<area>.rs`) and its state (`state/<area>.rs`); [app-features.md](architecture/app-features.md) lists the features spread over many files.
- Git and network work is in `glowfly-core` (`repo/`, `gitcli.rs`, `forge/`); drawing is in `ui/`.
- OS and file-system rules and their helpers: [PLATFORMS.md](PLATFORMS.md). Colours: [COLOUR.md](COLOUR.md).

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
- **`AppState`** holds no egui types; textures and fonts live in a `UiCache` owned by the UI
  layer.
- **Each area's flows** (tabs, jobs, status, dialogs, toasts, menus, logs, links, …) are in
  [flow-repo.md](architecture/flow-repo.md) and [flow-shell.md](architecture/flow-shell.md).

## Rules

1. `glowfly-core` must not depend on `eframe`/`egui`.
2. `ui/`, `update/` and `state/` do no I/O. `tests/layering.rs` denies whole families (the file
   system, processes, network, threads, environment, clock, libgit2 access) and the core
   functions that do I/O, and names the file and line of each use. The shell (`src/shell/`), which
   owns the window, the clock and the startup loads, is outside them; the few justified exceptions
   are listed in its `ALLOWED` table (`tests/layering/rules.rs`) with a reason.
3. New behavior gets an `update` test; new git or API logic gets a `glowfly-core` test. A flow that
   crosses views, the runner and git gets a journey step (`src/journey/`).
4. New views get an `egui_kittest` snapshot when practical. Views read time from
   `st.view.now` / `st.view.utc_offset` (never the system clock) so images are reproducible.
5. No global mutable state; pass values in. The one exception is the active palette
   (`ui::theme::color`): a thread-local copy of a `Palette`, set by reference
   (`color::set_palette`) at the start of each frame, so renders on different threads (parallel
   snapshot tests) can't affect each other. Palettes built from theme files are values owned by
   the shell's `ThemeCache`, worked out again only when the theme, the theme files or the OS
   setting change. The colour families, what each colour is for, the reserved hue bands and the
   contrast floors are in [COLOUR.md](COLOUR.md). Immutable
   data loaded once (the syntax definitions in `highlight/syntaxes.rs`) is not mutable state,
   and neither are the askpass environment in `gitcli` (`enable_askpass`) and gitcli's run
   observer (`observe_runs`, which hands timings to the log worker): each set once by the app
   shell at startup, never changed afterwards and never set in tests. The PATH children get
   (`env_path`) is likewise computed once, on first use, and never changed. The git program
   (`git_program`) is the one setting that changes while Glowfly runs: the shell sets it from
   the saved setting at startup, and afterwards only the runner sets it, after checking the new
   program (`GitEnvEffect::Use`); tests set it only to the git already on PATH. The counter
   behind `TabId::next` only hands out distinct ids; nothing depends on their values.
6. Each hosting provider implements `Forge`; the UI never matches on provider-specific types.
7. `scripts/check.ps1` (fmt, clippy -D warnings, tests) must pass before committing.
8. `glowfly-core` returns `glowfly_core::error::Error` (typed, via `thiserror`); decide behaviour
   from the variant (`is_auth()`, `is_not_found()`), never from message text.
9. Code that touches paths, files, processes, the environment or git follows
   `docs/PLATFORMS.md`, and uses the helpers it names (`paths`, `persist`, `names`, `repo/rpath.rs`,
   `env_path`, …) rather than writing new ones.
10. Views and update build user-visible text with `st.settings.lang()` and `tr` / `trf` / `trn`
    (keys in `crates/glowfly-core/locales/`, see its README). Translated text is never matched
    against git output or used as an egui id; ids use fixed `from_id_salt` strings. Write each
    key as a literal inside the call (`t.tr("conflict.panes.ours")`, never a key held in a
    variable): `tests/i18n_keys.rs` finds the keys the code uses that way.

## Recipe: adding a feature

1. **Message.** Add a variant for the user's intent to the area's message enum (`RepoMsg` in
   `msg/repo.rs`, `LfsMsg` in `lfs.rs`, …). A new area gets its own enum and one row in
   `msg/areas.rs` (the variant, the enum, the area's short name and its handler with the
   arguments it takes), which builds `Msg`, `Msg::on_inactive`, `Msg::area` and the `update`
   routing; only messages no area owns go in `Msg` itself.
2. **Update.** Handle it in the area's handler in `update/<area>.rs`, which matches its enum
   exhaustively (no `_` arm, so nothing is dropped silently); `update/mod.rs` only routes each
   `Msg` variant to its handler. Change state there. Keep pure rules (checks, labels, why
   something is blocked) in `state/<area>.rs`.
3. **Effect.** If the work is slow or leaves the process, add a variant to the area's effect
   type in `effect/<area>.rs` (or the area's own module) and decide its name, scope (whether
   its result belongs to a tab), timing, queue (whether it changes the repository; holds and
   the stop key when they differ) and sandbox rule in that file's `EffectMeta` impl, whose
   matches list every variant. A new effect area is one row in `effect/areas.rs`. Never add a
   flat `Effect` variant. Return the effect from `update`. An effect that runs git
   as the single active job starts through `start_job` or `begin_job` (`update/jobs.rs`),
   which checks that no job is running and gates it on the trust decision of the repository it
   runs in, the active one or another (`trust::gate` in `update/trust.rs`): it may wait for the
   trust prompt or be refused, so check their `Started` result rather than `st.job`. A job that
   makes repositories from a trusted one (a worktree, submodules) says so after it starts
   (`update/trust_inherit.rs`), and they are trusted when it succeeds. Work that waits outside
   the job gate for the active repository to be trusted, held built-in terminal shells
   (`SessionStatus::Held`), Open in external terminal, a conflict check or merge preview the user asked for (`TrustSession::waiting`) and the LFS
   and Sparse Checkout pages' git reads (`LfsPage::held`, `SparseStatus::held`), starts in
   `trust::released`. Jobs, shells, external terminals, conflict checks and merge previews
   that pass are pushed with a seal (`update/trust_seal.rs::push`), and update wraps them, and
   every read that lets git run in a repository, in `Effect::Checked`: the runner scans the
   repository again just before they start and sends them back held when the decision no
   longer covers it (a read reads without git or fails). Where hooks follow the working tree,
   a job's git commands also pass a per-command gate (`runner/trust_gate.rs`).
4. **Runner.** Execute it in the area's `Execute` impl in `runner/<area>.rs`. The real work
   (git, files, HTTP) goes in `glowfly-core`.
5. **Result.** The runner sends a result message back. Handle it in the same `update` file.
   Drop stale results: compare a generation number, or the request with what is still open.
   Say in the enum's `on_inactive` what the result still needs if its tab is no longer active.
6. **View.** Draw it in `ui/<region>.rs`: read state, push `Msg`s.
7. **Tests.** Write an `update` test in `update/tests/<area>_tests.rs`, registered with one
   `mod` line in `update/tests/mod.rs` (one that runs effects through the runner, with `drive`
   or `finish`, goes in glowfly-flows' `flows/<area>_tests.rs`, listed in `flows/mod.rs`); snapshot tests go in `ui/snapshot_tests/`, registered
   in its `mod.rs`. Add a core test on a fixture
   repository (see `scratch_repo`; build fixtures with the test kit's `git`, `testkit/`) for new
   git logic, and an `egui_kittest` snapshot for a new view (harnesses from
   `ui::snapshot_check::builder()`). A flow that crosses views, the runner and git gets a
   journey step (`src/journey/`): give a painted widget or a hint-only field an accessible name
   (`ui/access.rs`) so the journey can find it.
8. **Settings.** For a new preference, add a field with a default in `settings.rs`
   (`#[serde(default)]`) and a control on the matching Preferences page (`ui/prefs/`), built
   from `ui/prefs/form.rs` (rows, sections, labelled checkboxes). App settings apply at once.
   Values a page writes to git or repository config wait for the Save / Revert bar instead:
   `state/prefs_save.rs` says whether the open page has unsaved edits, `update/prefs_save.rs`
   saves and reverts them (Ctrl/Cmd+S and Esc too), showing "Saved" only once git reports the
   write succeeded and git's error on the page when it doesn't, `update/prefs_leave.rs` asks
   "Save your changes?" before leaving such a page, and `ui/prefs/save_bar.rs` draws the bar
   and the "Saved" tick.

## Migration status

- [x] 1. Baseline commit, `CLAUDE.md`, rustfmt config, check script, CI
- [x] 2. Extract `glowfly-core` (git, graph, forge, settings, dates, spell, avatar fetching); `Forge` trait
- [x] 3. Single task runner replaces the per-feature thread/channel pairs
- [x] 4. Split `AppState` / `Msg` / `update` out of the `GlowflyApp` struct, area by area
- [x] 5. Move views into `ui/` modules; remove global atomics (the palette flag is per thread)
- [x] 6. Headless `update` tests, `egui_kittest` snapshots, CI enforcing clippy
- [x] 7. Resume the backlog, one feature per commit with tests (now GitHub issues)
