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
                          status), diff (details, comparisons between two commits, file
                          diffs with optional whitespace filtering, line counts), history
                          (commits that changed a file, following renames), index (staging),
                          lines (partial patches for line staging), revert (a commit or
                          comparison hunk's inverse applied to the working tree), restore (one
                          file written from a commit, or its parent, into the working tree), conflict
                          (reading an index conflict into chunks), resolve (picks, output, write
                          and stage), template (commit.template: the prefill and its comment char),
                          workfile (creating, renaming and deleting a working-tree file; path
                          checks), workfile_mv (a rename staged through git mv), workfile_rm
                          (deleting a staged new file or a staged rename's new path with its
                          index entries, and unstaging one whose working copy is already
                          gone), noclobber (a move that never replaces an existing file),
                          tracked (the paths in the index, for the palette's file finder)
    gitcli.rs             the git CLI (commands that honour hooks, credentials, signing);
                          passes the askpass environment to every git child once enabled
    askpass/              credential prompts: prompt (classifying what git or ssh asks),
                          protocol (the helper's JSON line protocol and token), server (the
                          app's loopback listener), client (the helper's side)
    branch_ops.rs         branch command lines (rename, upstream, fast-forward without
                          checkout, push, delete with the remote branch), name checks,
                          fast-forward state
    tag_ops.rs            tag name checks (ref-format rules with a reason, existing and
                          clashing tags) and the tag and tag-push command lines
    rebase/               interactive rebase: types (mod.rs), plan (the commits to replay),
                          todo (checking and writing the todo list, reword message files, and
                          installing it over git's as the sequence-editor helper), run
                          (`git rebase -i` with the prepared todo; where a stopped rebase is)
    undo.rs               undo/redo of Trellis's own operations: records state around a job,
                          plans the git commands that reverse it (checked against the repo)
    graph.rs              lane layout for the commit graph
    fuzzy.rs              fuzzy matching and ranking for the command palette (word starts,
                          runs, hidden keywords)
    diff_rows.rs          inline and side-by-side row layout of a diff
    whole_file.rs         whole-file view: change blocks mapped onto hunks, folds of long
                          unchanged runs (folds.rs), their rows, and mapping rows to lines
                          so fold changes keep the view in place (anchor.rs)
    conflict_rows.rs      row layout of the conflict view (aligned ours/theirs panes, output)
    highlight.rs          syntax highlighting of diff lines (syntect), as token kinds:
                          inline and background passes, hunks parsed in context from the
                          old text (highlight/context.rs, parse.rs), reuse/carry in
                          highlight/reuse.rs
    intraline.rs          word-level changes inside paired removed/added diff lines
    tabs.rs               tab stops for drawing diff text (lines keep raw tabs), with a byte
                          map so syntax spans and changed-word ranges follow the expansion
    watch.rs              which file changes in a repository matter (status or full reload),
                          and a cache of folders .gitignore ignores
    forge/                hosting services behind the `Forge` trait (github, gitlab,
                          bitbucket, azure, jira); model and trait in mod.rs, plus remote
                          (URL detection), auth (credentials), refs (issue references), http,
                          publish (creating a repository on the service)
    settings.rs, dates.rs, spell.rs, signing.rs, avatars.rs (fetching); settings/graph_filter
                          (hidden and soloed refs per repository)
  trellis-app/             binary
    state/                AppState (plain data), ForgeState, PrefsState, tabs (parked per-repo
                          data for inactive tabs), undo (per-tab undo history), lines (diff
                          line selection), conflict (the open conflict and its picks),
                          history (the open file history), watch (what the file watcher
                          follows), selection (several selected commits: Ctrl/Shift-click
                          rules, cherry-pick checks), branches (why branch actions are blocked,
                          push targets), rebase (the interactive rebase panel: the list being
                          edited and the rules for starting it), askpass (credential
                          prompts waiting for the user; global, not parked with a tab),
                          diff_nav (next/previous change across files: the file list a
                          diff came from, where a step lands), commit (the commit draft, its
                          readiness rules, button label and command line), palette (open
                          state and modes: the command list or a command's sub-list, ranked
                          per query), palette_commands (the commands available now: labels,
                          hidden keywords, details and shortcut hints), files (the
                          New file and Rename file dialogs' path form), tags (the New tag
                          dialog's form, its name check and push remote)
    msg.rs                Msg: user intents and background results
    effect.rs             Effect: everything slow or outside the process
    runner/               executes effects on worker threads, reports back as Msg (mod.rs);
                          config (reading git config), rebase (loading and running an
                          interactive rebase, with Trellis as the sequence editor), askpass
                          (starting the credential-prompt listener, answering prompts), os
                          (URLs, terminals, opening files and showing them in the file manager),
                          files (creating, renaming and deleting a working-tree file; listing
                          the tracked files), highlight.rs: decorate + background highlighting
                          (a newer HighlightDiff cancels the older)
    watcher.rs            notify watcher for the active tab's repository, debounced in its own
                          thread, owned by the runner (Effect::Watch)
    update/               update(state, msg, now) -> Vec<Effect>: repo, ops, forge, prefs, tabs,
                          undo, diff, conflict, history, watch, selection (several commits:
                          the two-commit comparison, cherry-picking them), branches (the local
                          branch menu's operations), rebase (the interactive rebase panel and
                          its job), graph_filter (hidden and soloed refs, saved per repository),
                          askpass (queueing, answering and cancelling credential prompts; what a
                          finished job clears), files (restoring a file from a commit, opening
                          or showing a working-tree file; work_files: create, rename, delete),
                          commit (committing, stage-all-and-commit, push after committing, the
                          template prefill), palette (opening the command palette, moving,
                          running a command, entering and leaving a sub-list and loading its
                          items), highlight (starting the background highlighting pass,
                          attaching its result if the same diff is still shown), tags
                          (opening the New tag dialog, creating the tag and pushing it in
                          one job, the push setting)
                          (+ headless tests in update/tests.rs)
    ui/                   egui views: toolbar, sidebar, graph/ (select: click modifiers and
                          arrow keys), details, commit_box (the WIP commit message, options,
                          button and Ctrl+Enter shortcuts), details_multi (several commits), diff/ (header,
                          inline, split, syntax, style: font, row height and gutters from
                          Preferences > Editor),
                          conflict, history, rebase/ (the interactive rebase panel; rows:
                          actions, message editor, drag to reorder), modals, menus,
                          branch_menu (local branch menu), branch_dialogs (rename, upstream),
                          ref_visibility (hide/solo menu items, row markers and banner),
                          file_menu (file actions in file rows' menus, commit and comparison
                          rows included: restore, open, show, new file in this folder, rename,
                          delete), file_dialogs (new file, rename file), tag_dialog (new tag,
                          plain or annotated, name checks, push to a remote),
                          askpass (the credential prompt, drawn above every other dialog),
                          palette (the Ctrl+P command palette overlay),
                          status, prefs/ (editor: the Editor page), theme, widgets
                          (+ snapshot_tests.rs, snapshot_tests_diff.rs, snapshot_tests_revert.rs,
                          snapshot_tests_select.rs, snapshot_tests_whole.rs,
                          snapshot_tests_branch.rs, snapshot_tests_rebase.rs,
                          snapshot_tests_editor.rs, snapshot_tests_askpass.rs,
                          snapshot_tests_file_menu.rs, snapshot_tests_commit.rs,
                          snapshot_tests_palette.rs, snapshot_tests_file_dialogs.rs,
                          snapshot_tests_tag.rs)
    platform.rs           OS integration (recent repos, terminal, file manager, showing a
                          file selected in it)
    main.rs               starts the window; `trellis --write-todo <prepared> <todo>` instead
                          installs a prepared todo list for git (no window) and exits; with
                          TRELLIS_ASKPASS set (or `--askpass <prompt>`) it is git's and ssh's
                          askpass helper: asks the running app and prints the answer
  trellis-app/tests/snapshots/   approved UI images
  trellis-app/tests/layering.rs  fails if a view under ui/ does I/O itself (see Rules)
  trellis-app/tests/sequence_editor.rs  runs a rebase with the built executable as the
                                 sequence editor
  trellis-app/tests/askpass.rs   runs the built executable as the askpass helper, alone and
                                 under `git credential fill`
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
  it in `state::tabs::Parked` and restores the other tab's (reordering moves parked data with
  the tab; `active_tab` follows the active tab). Results of repository-scoped
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
   data loaded once (the syntax definitions in `highlight.rs`) is not mutable state, and
   neither is the askpass environment in `gitcli` (`enable_askpass`): set once by the app
   shell at startup, never changed afterwards and never set in tests.
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
