# Glowfly: notes for AI assistants

Native Git client in Rust (egui + libgit2). Read `docs/ARCHITECTURE.md`
before structural changes; what each file holds is in `docs/architecture/<area>.md`, one line per
file. The [Glowfly Project](https://github.com/users/bg9m9r/projects/2) and its GitHub milestones
are the source of truth for milestones (with their exit criteria), priorities and status; the
work is its issues. `ROADMAP.md` holds only the goal, the scope and how planning works.

## Commands

- `pwsh scripts/check.ps1`: fmt check, clippy (`-D warnings`), all tests. Run before every commit.
- `cargo run --release -- <repo path>`: run the app. With no path it reopens the last repo.
- `UPDATE_SNAPSHOTS=1 cargo test -p glowfly snapshot`: re-render UI snapshots into
  `crates/glowfly-app/tests/snapshots/`: a `.txt` text snapshot per view test, and a PNG only for
  the pixel goldens listed in `ui/snapshot_check/goldens.rs`. Read the changed text and open
  the changed PNGs before committing.
- `pwsh scripts/rerender-conflicted.ps1`: after a merge or rebase stops on snapshot conflicts,
  re-render and stage only the conflicted snapshots.
- `UPDATE_ICONS=1 cargo test -p glowfly --test app_icon`: redraw the app icon into `assets/icon/`
  (PNGs, `glowfly.ico`, `glowfly.icns`) after changing `tests/app_icon/draw.rs`.
- `UPDATE_NOTICES=1 cargo test -p glowfly-core notice`: regenerate the syntax-definition licenses
  in `THIRD-PARTY-NOTICES.md` after updating the `two-face` crate.
- `RUST_LOG=glowfly=debug`: log background effects as they run.
- `GLOWFLY_LIVE_REMOTE=<url> cargo test live_remote -- --ignored --nocapture`: read-only check
  against a real hosting service.

## Worktrees

Make every change in a git worktree, never in the main checkout (`C:/projects/glowfly`):

1. `git worktree add ../glowfly-worktrees/<slug> -b <branch>` from the main checkout's HEAD.
2. Work, run `scripts/check.ps1` and commit there. Set `CARGO_TARGET_DIR=C:/projects/glowfly/target`
   to reuse the main build output (one build at a time).
3. When done, land it through a pull request:
   - `git push -u origin <branch>`, then `gh pr create --fill` (one PR per branch).
   - Have the `pr-reviewer` agent review it. It posts its findings on the PR and passes or fails
     the required `AI review` status. Fix a failure with new commits on the branch; each new head
     commit needs a new review.
   - Once CI and `AI review` pass: `gh pr merge <n> --rebase --delete-branch` (never `--admin`),
     then `git -C C:/projects/glowfly pull --ff-only`.
   - `git worktree remove ../glowfly-worktrees/<slug>` and `git branch -d <branch>`.
   Dependabot and release PRs get the same review before merging.
   Unfinished work: commit it as WIP on its branch, remove the worktree, keep the branch.

`git worktree list` should show only the main checkout when nothing is in progress.

## Tracking work

- Pick work from the Project's **Ready** column (the focused next set; Backlog holds the rest of
  the scheduled work and is refilled into Ready by the `project-manager` agent), highest Priority first (within a priority, the
  current milestone first). Never pick a `needs-decision` issue: those are the owner's to decide.
- One branch (and worktree) per issue, e.g. `issue/<n>-<slug>`. On starting, move its card:
  `pwsh scripts/track.ps1 <n> 'In progress'`. The Project doesn't move closed issues, so after
  the landing that closes the issue, move it: `pwsh scripts/track.ps1 <n> Done`.
- Commit subjects stay Conventional Commits; the landing commit's body says `Closes #<n>`
  (`Refs #<n>` for a partial landing, with a comment on the issue saying what is left).
- Follow-ups, polish and edge cases found while building: one line in the matching
  `docs/ideas/<area>.md` file, under the heading of the feature they grew from. Never open
  issues for them unless the owner asks. A substantial new feature becomes a new issue in
  Inbox: `gh issue create`, then `pwsh scripts/track.ps1 <n> Inbox`.

## Where code goes

- Git reads/writes and CLI calls: `glowfly-core` (`repo.rs`, `gitcli.rs`). Network APIs:
  `glowfly-core/src/forge/`. Nothing in `glowfly-core` may depend on egui.
- New behaviour: add a variant to the area's message enum (`msg/<area>.rs`, e.g. `RepoMsg`;
  its handler in `update/<area>.rs` matches it exhaustively, and its `on_inactive` decides
  what a result needs for an inactive tab; a new area is one row in `msg/areas.rs`), return an
  `Effect` for anything slow (a variant of the area's effect type in `effect/<area>.rs`, with
  its `EffectMeta`, run by its `Execute` impl in `runner/<area>.rs`; a new effect area is one
  row in `effect/areas.rs`), and have the runner report back with a result message. Test it
  in `update/tests/` (one `<area>_tests.rs` per area, listed in its `mod.rs`).
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
- Keep files under ~400 lines; split by responsibility rather than growing a file.
- Adding, moving or removing a source file: update its line in the matching
  `docs/architecture/<area>.md` (one line per file or topic, never long table cells).
- `tests/hygiene.rs` ratchets these: a new file over 400 lines, a file without a `//!` doc, a
  new English log/toast text in `update/` or `state/` (use `t.tr`) or a copied fixture helper
  fails it. Its allowlists and counts only shrink; lower them when you fix one, never raise them.
- Tests: core logic uses fixture repos (see the `scratch_repo` helper); behavior uses headless
  `update` tests; views use `egui_kittest` snapshots.
- egui 0.31 APIs (e.g. `CornerRadius`, `Modal`, `ui.close_menu()`). The bundled fonts lack some
  glyphs (▾ ✎ ↻), so paint those as shapes (see `widgets::paint_icon`).
- Git operations that should honour user config, hooks or credentials go through the git CLI
  (`gitcli`); everything else uses libgit2.
- Never copy code, text, icons or other assets from other Git clients. Implement features from
  their behaviour, in Glowfly's own words and visuals.
- Don't drive the GUI with synthetic mouse input on the user's desktop; use snapshot tests.
