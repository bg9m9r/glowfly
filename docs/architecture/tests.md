# Tests outside `src/`

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
test file or script, update its line here. Tests inside `src/` (update tests, snapshots, journeys,
the glyph check) are in [app-layers.md](app-layers.md) and [ui-views.md](ui-views.md).

## Source checks (`crates/glowfly-app/tests/`)

- `crates/glowfly-app/tests/layering.rs`: no I/O in `ui/`, `update/` or `state/` (see Rules; its `ALLOWED` table is in `tests/layering/rules.rs`).
- `crates/glowfly-app/tests/controls.rs`: views under `ui/` draw text fields, checkboxes, radio buttons and modals with `theme/controls.rs` and `theme::modal`, not egui's own.
- `controls.rs`: dialogs (and the first-run welcome) draw their title and buttons through `dialog_frame` (no headings, egui buttons or English Cancel / Close) and their text fields as `widgets::TextField` (no `controls::field(` or `TextEdit::`).
- `controls.rs`: the dialogs for less familiar operations each have a guide side pane, and no other dialog has one.
- `controls.rs`: preference pages and panels use `widgets::buttons` with translated Cancel and Close; nothing wraps a styled button in `add_enabled`.
- `crates/glowfly-app/tests/type_scale.rs`: view text stays on the type scale: no pixel sizes in `FontId`, `.size(` or font constants, no egui small text or `RichText::strong()` (headers use `type_scale::section`, emphasis `type_scale::strong`), and no link coloured by hand; `ALLOWED` names the exceptions.
- `crates/glowfly-app/tests/tokens.rs`: views stay on the design tokens (`ui/theme/tokens.rs`): no number in `CornerRadius::same(`, `.corner_radius(`, a `CornerRadius { nw: … }` or a `*RADIUS*` constant, no hand-made `Shadow` or `popup_shadow` (outside `theme/mod.rs`), and no accent `rect_stroke` (focus rings go through `tokens::focus_ring`); the icon shapes are exempt and `ALLOWED` names the exceptions.
- `crates/glowfly-app/tests/sentence_case.rs`: English labels are in sentence case: `en.json`'s label values (not tips, help or other prose) and the first literal at the views' label calls, with proper nouns and acronyms keeping their capitals.
- `crates/glowfly-app/tests/menus.rs`: views open menus, submenus and menu popups only through `ui/widgets/menu.rs` (or `row_menu` / `menu_anchor`), so every menu gets the shared style.
- `crates/glowfly-app/tests/message_styles.rs`: only `widgets/banner.rs` fills with the banner colour (the diff's Git LFS note keeps it as a box), and the forms converted to `widgets/form_message.rs` don't colour problem lines red or amber themselves; shares the source reading with `controls.rs` (`tests/support/views.rs`).
- `crates/glowfly-app/tests/hygiene.rs`: ratchets across both crates: non-test files stay under 400 lines (`LONG_FILES` holds today's longer files at their length as a ceiling) and open with a `//!` doc.
- `hygiene.rs`: `update/` and `state/` add no log or toast call with English text (`ENGLISH_LOG_CALLS` only goes down).
- `hygiene.rs`: the fixture builders (`scratch_repo`, `long_repo`, `with_repo`, the kit's `git`, …) are not redefined or handed one folder name twice.
- `hygiene.rs`: view tests build their harnesses with `ui::snapshot_check::builder()` (`HARNESS_MAKERS` names the exceptions).
- `crates/glowfly-app/tests/i18n_keys.rs`: every translation key the code looks up exists in English, and every English key is used.

## The shared test kit (`crates/glowfly-core/src/testkit/`)

- `testkit/mod.rs`: what both crates' tests build fixture repositories with (the app's dev-dependency turns on core's `testkit` feature): `git`, `git_out`, `git_env`, `try_git_env`, `git_cli`, `set_config`, `set_identity`, `write`. `GLOWFLY_TEST_GIT_CLI=1` sends every fixture command to the git CLI.
- `testkit/fast.rs`: which fixture commands run in-process through libgit2 instead of starting git (`init`, `add`, `config`, `update-index -q --refresh`, `checkout -- <file>`, and the dispatch); `commit.rs`, `refs.rs` (`checkout`, `branch`, `tag`, `update-ref`, `remote`) and `push.rs` (to a bare repository in a local folder) hold the rest.
- `testkit/guard.rs`: when a command goes to the git CLI after all: hooks, signing, filters and working-tree encodings, submodules, sparse checkout, repository extensions, an operation in progress, environment that redirects git.
- `testkit/tests.rs`: one history built both ways (the CLI and in-process) gives the same commits, refs, reflogs, index, files and config.
- `crates/glowfly-core/src/testutil.rs`: core's own fixtures (`scratch_repo`, origins, submodules, partial clones, byte-level `raw_commit`s) on the kit.

## The built executable

- `crates/glowfly-app/tests/sequence_editor.rs`: a rebase with the built executable as the sequence editor.
- `crates/glowfly-app/tests/askpass.rs`: the built executable as the askpass helper.
- `crates/glowfly-app/tests/gpg_prompt.rs`: the built executable as gpg.program: the loop guard, (Unix) `git commit -S` through a fake gpg, and (Unix) `git commit -S` with a protected SSH key.

## Images

- `crates/glowfly-app/tests/app_icon.rs`: the app icon: draws it (`app_icon/draw.rs`) and checks every PNG in `assets/icon/` and inside `glowfly.ico` and `glowfly.icns` (`app_icon/containers.rs`); `UPDATE_ICONS=1` rewrites them.
- `crates/glowfly-app/tests/snapshots/`: approved view snapshots: a `<name>.txt` text snapshot for every view test, and a `<name>.png` for the pixel goldens (`src/ui/snapshot_check/goldens.rs`).
- `scripts/rerender-conflicted.ps1`: after a merge or rebase stops on snapshot conflicts, re-renders only the conflicted snapshots (the tests that name them) and stages them; `-DryRun` lists them.

## Installers

- `.github/workflows/installers.yml`: `scripts/installer-check-windows.ps1`, `scripts/installer-check-linux.sh`, `scripts/update-feed-check.ps1`: the packaged builds on clean runners.
- The MSI and the .deb are installed over a v0.0.1 package, and their files, shortcuts or desktop entries, registry and icons checked.
- The program is started (`--version`; on Linux also the window under its own Xvfb), update-checked against local feeds (up to date, newer, wrong or missing checksum, relative feed), and uninstalled with nothing left behind.

## Workflows

- Every action in `.github/workflows/` is pinned to a commit SHA with its version in a comment; `.github/dependabot.yml` proposes the updates (weekly for actions, skipping major versions; monthly for Cargo, inside the Cargo.toml requirements). Each workflow sets its token permissions: read-only except `release.yml`.
