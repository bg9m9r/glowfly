# Trellis: notes for AI assistants

Native Git client in Rust (egui + libgit2). Read `docs/ARCHITECTURE.md`
before structural changes. `ROADMAP.md` is the source of truth for the goal, milestones and
status; `TODO.md` is the backlog of items, the current milestone first.

## Commands

- `pwsh scripts/check.ps1`: fmt check, clippy (`-D warnings`), all tests. Run before every commit.
- `cargo run --release -- <repo path>`: run the app. With no path it reopens the last repo.
- `UPDATE_SNAPSHOTS=1 cargo test -p trellis snapshot`: re-render UI snapshots into
  `crates/trellis-app/tests/snapshots/`. Open the PNGs and check them before committing.
- `UPDATE_ICONS=1 cargo test -p trellis --test app_icon`: redraw the app icon into `assets/icon/`
  (PNGs, `trellis.ico`, `trellis.icns`) after changing `tests/app_icon/draw.rs`.
- `UPDATE_NOTICES=1 cargo test -p trellis-core notice`: regenerate the syntax-definition licenses
  in `THIRD-PARTY-NOTICES.md` after updating the `two-face` crate.
- `RUST_LOG=trellis=debug`: log background effects as they run.
- `TRELLIS_LIVE_REMOTE=<url> cargo test live_remote -- --ignored --nocapture`: read-only check
  against a real hosting service.

## Worktrees

Make every change in a git worktree, never in the main checkout (`C:/projects/trellis`):

1. `git worktree add ../trellis-worktrees/<slug> -b <branch>` from the main checkout's HEAD.
2. Work, run `scripts/check.ps1` and commit there. Set `CARGO_TARGET_DIR=C:/projects/trellis/target`
   to reuse the main build output (one build at a time).
3. When done: `git -C C:/projects/trellis merge --ff-only <branch>`, then
   `git worktree remove ../trellis-worktrees/<slug>` and `git branch -d <branch>`.
   Unfinished work: commit it as WIP on its branch, remove the worktree, keep the branch.

`git worktree list` should show only the main checkout when nothing is in progress.

## Where code goes

- Git reads/writes and CLI calls: `trellis-core` (`repo.rs`, `gitcli.rs`). Network APIs:
  `trellis-core/src/forge/`. Nothing in `trellis-core` may depend on egui.
- New behaviour: add a variant to the area's message enum (`msg/<area>.rs`, e.g. `RepoMsg`;
  its handler in `update/<area>.rs` matches it exhaustively, and its `on_inactive` decides
  what a result needs for an inactive tab), return an `Effect` for anything slow (add the
  effect to `effect/mod.rs` and execute it in `runner/mod.rs`), and have the runner report
  back with a result message. Test it in `update/tests.rs`.
- Drawing: `ui/<region>.rs`. Views take `&mut AppState` only for text buffers and `view`
  state, plus `UiCache` for textures, and push `Msg`s for everything else. No I/O in views;
  `tests/layering.rs` checks.
- Paths, files, processes, the environment or git calls: follow `docs/PLATFORMS.md` (the OS and
  file-system rules and the helpers to use).
- Settings: add a field with a default in `settings.rs` (`#[serde(default)]` keeps old files
  loading), then a control on the matching Preferences page.

## Conventions

- Commit messages use Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`,
  `ci:`, `chore:`); release-please derives versions and the CHANGELOG from them. Don't edit
  the version in `Cargo.toml`, `version.txt` or `CHANGELOG.md` by hand.

- When you tick an item in `TODO.md`, add a line at the top of its "Recent changes" list, in
  local time, and drop the oldest line past 20. Format:
  `- YYYY-MM-DD HH:MM · **<top-level TODO item>**: <what was done> (<short commit sha>)`, with
  the sha in backticks.
- Follow-ups, polish and edge cases found while building go to `TODO.md`'s Ideas section under
  the item they grew from, never to Later (Later holds only triaged milestone work).
- Keep files under ~400 lines; split by responsibility rather than growing a file.
- Tests: core logic uses fixture repos (see the `scratch_repo` helper); behavior uses headless
  `update` tests; views use `egui_kittest` snapshots.
- egui 0.31 APIs (e.g. `CornerRadius`, `Modal`, `ui.close_menu()`). The bundled fonts lack some
  glyphs (▾ ✎ ↻), so paint those as shapes (see `widgets::paint_icon`).
- Git operations that should honour user config, hooks or credentials go through the git CLI
  (`gitcli`); everything else uses libgit2.
- Never copy code, text, icons or other assets from other Git clients. Implement features from
  their behaviour, in Trellis's own words and visuals.
- Don't drive the GUI with synthetic mouse input on the user's desktop; use snapshot tests.
