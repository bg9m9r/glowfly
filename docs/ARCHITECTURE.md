# Trellis architecture

Trellis is a native Git client (Rust, egui, libgit2). This document describes the target
structure, the rules that keep it maintainable, and the migration from the original
single-crate prototype.

## Goals

- **Small, focused files** (aim for < 400 lines) so a change, by a person or an AI, needs only
  the files for one area.
- **Layers enforced by the compiler**: the UI cannot call git or HTTP directly, and core logic
  cannot depend on egui.
- **Testable without a window**: behavior lives in a pure `update` function; views get
  snapshot tests.
- **One way to do background work.**

## Layout

```
crates/
  trellis-core/            library: no egui, no app state
    repo/                 repository model on libgit2: types (mod.rs), load (history, refs,
                          status), diff (details, file diffs, line counts), index (staging),
                          lines (partial patches for line staging), conflict (reading an index
                          conflict into chunks), resolve (picks, output, write and stage)
    gitcli.rs             the git CLI (commands that honour hooks, credentials, signing)
    undo.rs               undo/redo of Trellis's own operations: records state around a job,
                          plans the git commands that reverse it (checked against the repo)
    graph.rs              lane layout for the commit graph
    diff_rows.rs          inline and side-by-side row layout of a diff
    conflict_rows.rs      row layout of the conflict view (aligned ours/theirs panes, output)
    highlight.rs          syntax highlighting of diff lines (syntect), as token kinds
    intraline.rs          word-level changes inside paired removed/added diff lines
    forge/                hosting services behind the `Forge` trait (github, gitlab,
                          bitbucket, azure, jira); model and trait in mod.rs, plus remote
                          (URL detection), auth (credentials), refs (issue references), http,
                          publish (creating a repository on the service)
    settings.rs, dates.rs, spell.rs, signing.rs, avatars.rs (fetching)
  trellis-app/             binary
    state/                AppState (plain data), ForgeState, PrefsState, tabs (parked per-repo
                          data for inactive tabs), undo (per-tab undo history), lines (diff
                          line selection), conflict (the open conflict and its picks)
    msg.rs                Msg: user intents and background results
    effect.rs             Effect: everything slow or outside the process
    runner.rs             executes effects on worker threads, reports back as Msg
    update/               update(state, msg, now) -> Vec<Effect>: repo, ops, forge, prefs, tabs,
                          undo, diff, conflict
                          (+ headless tests in update/tests.rs)
    ui/                   egui views: toolbar, sidebar, graph/, details, diff/ (inline, split, syntax),
                          conflict, modals, menus,
                          status, prefs/, theme, widgets (+ snapshot_tests.rs)
    platform.rs           OS integration (recent repos, terminal, file manager)
  trellis-app/tests/snapshots/   approved UI images
  trellis-app/tests/layering.rs  fails if a view under ui/ does I/O itself (see Rules)
```

### Data flow (Elm-style)

```
 view(&AppState) ──Msg──▶ update(&mut AppState, Msg) ──Vec<Effect>──▶ runner
       ▲                                                                   │
       └─────────────── Msg::…Done(result) ◀── worker thread ◀─────────────┘
```

- **Views** never mutate `AppState` directly, except transient widget buffers such as text
  fields, and never call `trellis_core` operations that touch disk or network. They may use core
  types and pure helpers. `tests/layering.rs` enforces this: it fails on threads, processes,
  files, HTTP or the core functions that do I/O anywhere under `ui/` (the shell's startup in
  `ui/mod.rs` and the snapshot tests are exempt). Avatars, for example, are requested by the
  views, fetched through `Effect::FetchAvatars`, and turned into textures by the shell.
- **`update`** is the only place state changes. It never blocks. Anything slow (git commands,
  HTTP, large diffs) becomes an `Effect`.
- **The runner** executes effects on a worker pool and sends results back as `Msg`s.
- **Tabs**: the active tab's repository data lives directly in `AppState`; switching tabs parks
  it in `state::tabs::Parked` and restores the other tab's. Results of repository-scoped
  effects (`Effect::scope`) come back as `Msg::Scoped` and are dropped if their tab is no
  longer active; that tab refreshes when selected again.
- **`AppState`** holds no egui types; textures and fonts live in a `UiCache` owned by the UI
  layer.

## Rules

1. `trellis-core` must not depend on `eframe`/`egui`.
2. New behavior gets an `update` test; new git or API logic gets a `trellis-core` test.
3. New views get an `egui_kittest` snapshot when practical. Views read time from
   `st.view.now` / `st.view.utc_offset` (never the system clock) so images are reproducible.
4. No global mutable state; pass values in. The one exception is the active palette
   (`ui::theme::color`): a thread-local flag the shell sets at the start of each frame, so
   renders on different threads (parallel snapshot tests) can't affect each other. Immutable
   data loaded once (the syntax definitions in `highlight.rs`) is not mutable state.
5. Each hosting provider implements `Forge`; the UI never matches on provider-specific types.
6. `scripts/check.ps1` (fmt, clippy -D warnings, tests) must pass before committing.
7. `trellis-core` returns `trellis_core::error::Error` (typed, via `thiserror`); decide behaviour
   from the variant (`is_auth()`, `is_not_found()`), never from message text.

## Migration status

- [x] 1. Baseline commit, `CLAUDE.md`, rustfmt config, check script, CI
- [x] 2. Extract `trellis-core` (git, graph, forge, settings, dates, spell, avatar fetching); `Forge` trait
- [x] 3. Single task runner replaces the per-feature thread/channel pairs
- [x] 4. Split `AppState` / `Msg` / `update` out of the `TrellisApp` struct, area by area
- [x] 5. Move views into `ui/` modules; remove global atomics (the palette flag is per thread)
- [x] 6. Headless `update` tests, `egui_kittest` snapshots, CI enforcing clippy
- [ ] 7. Resume `TODO.md`, one feature per commit with tests
