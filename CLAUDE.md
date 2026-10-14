# Trellis: notes for AI assistants

Native Git client in Rust (egui + libgit2). Read `docs/ARCHITECTURE.md`
before structural changes. `TODO.md` is the feature backlog.

## Commands

- `pwsh scripts/check.ps1`: fmt check, clippy (`-D warnings`), all tests. Run before every commit.
- `cargo run --release -- <repo path>`: run the app. With no path it reopens the last repo.
- `UPDATE_SNAPSHOTS=1 cargo test -p trellis snapshot`: re-render UI snapshots into
  `crates/trellis-app/tests/snapshots/`. Open the PNGs and check them before committing.
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
- New behaviour: add a `Msg` variant, handle it in `update/<area>.rs`, return an `Effect` for
  anything slow (add the effect to `effect.rs` and execute it in `runner.rs`), and have the
  runner report back with a result `Msg`. Test it in `update/tests.rs`.
- Drawing: `ui/<region>.rs`. Views take `&mut AppState` only for text buffers and `view`
  state, plus `UiCache` for textures, and push `Msg`s for everything else. No I/O in views;
  `tests/layering.rs` checks.
- Settings: add a field with a default in `settings.rs` (`#[serde(default)]` keeps old files
  loading), then a control on the matching Preferences page.

## Conventions

- Commit messages use Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`,
  `ci:`, `chore:`); release-please derives versions and the CHANGELOG from them. Don't edit
  the version in `Cargo.toml`, `version.txt` or `CHANGELOG.md` by hand.

- When you tick an item in `TODO.md`, add a line at the top of its "Recent changes" list:
  `- YYYY-MM-DD HH:MM · **<top-level TODO item>**: <what was done> (\`<commit>\`)`, in local
  time, and drop the oldest line past 20.
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
