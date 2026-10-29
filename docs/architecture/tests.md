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
- `testkit/bin.rs`: a stand-in Recycle Bin / Trash folder that tests' `Trasher` and `Untrasher` move files into and take them back out of, so no test touches the real one; each process's folder is emptied on first use, since Windows reuses process ids.
- `testkit/tests.rs`: one history built both ways (the CLI and in-process) gives the same commits, refs, reflogs, index, files and config.
- `crates/glowfly-core/src/testutil.rs`: core's own fixtures (`scratch_repo`, origins, submodules, partial clones, byte-level `raw_commit`s) on the kit.

## The built executable

- `crates/glowfly-app/tests/sequence_editor.rs`: a rebase with the built executable as the sequence editor.
- `crates/glowfly-app/tests/askpass.rs`: the built executable as the askpass helper.
- `crates/glowfly-app/tests/gpg_prompt.rs`: the built executable as gpg.program: the loop guard, (Unix) `git commit -S` through a fake gpg, and (Unix) `git commit -S` with a protected SSH key.

## Fuzzing

The parsers in `glowfly-core` that read text from other people are fuzzed with cargo-fuzz (libFuzzer). Run a target for a while from the repository root, on Linux or macOS with nightly and `cargo install cargo-fuzz`:

```
cp Cargo.lock fuzz/Cargo.lock            # the app's dependency versions
cargo +nightly fuzz run trust_todo fuzz/corpus/trust_todo fuzz/seeds/trust_todo -- -max_total_time=60
cargo +nightly fuzz list                 # every target
```

A failing input lands in `fuzz/artifacts/<target>/`; `cargo +nightly fuzz run <target> <file>` replays it. Each one becomes a regression test beside the parser, with the fix. On Windows, or without nightly, `GLOWFLY_FUZZ_ROUNDS=100000 cargo test -p glowfly-core fuzzing` runs the same checks over mutated seeds instead (not coverage-guided).

- `fuzz/Cargo.toml`: the fuzz crate, a workspace of its own (the main one `exclude`s it, so the build, clippy and tests never need nightly); one `[[bin]]` per target.
- `fuzz/fuzz_targets/<target>.rs`: each target hands its bytes to the check of the same name in `glowfly-core`'s `fuzzing` module.
- `fuzz/seeds/<target>/`: the inputs each target starts from (byte-exact: `.gitattributes` marks them binary).
- `crates/glowfly-core/src/fuzzing.rs`: the checks, built only for tests and with core's `fuzzing` feature (which also reaches the crate-private parsers they need); `CHECKS` lists them by target.
- `fuzzing/trust.rs`: the trust plumbing's readers: a rebase or sequencer todo (every pick resolved and listed), FETCH_HEAD (every line for merging listed with its commit), and a patch's paths (`git apply --numstat`, am's mailbox detection, any spelling of a path under the hooks folder caught).
- `fuzzing/urls.rs`: remote URLs (a recognized remote's host is a whole authority with no `%` or other character a host isn't written with, and its names hold no line break) and `redact_credentials` (a URL's password never shows in what is logged).
- `fuzzing/diffs.rs`: a patch file's summary (files listed once), conflict marker regions and the marker size, and merged text with markers (parsed, written back and parsed again to the same chunks).
- `fuzzing/tests.rs`: every check over its seeds and their mutations in an ordinary `cargo test`, and every target in `fuzz/fuzz_targets/` having a check.

## Images

- `crates/glowfly-app/tests/app_icon.rs`: the app icon: draws it (`app_icon/draw.rs`) and checks every PNG in `assets/icon/` and inside `glowfly.ico` and `glowfly.icns` (`app_icon/containers.rs`); `UPDATE_ICONS=1` rewrites them.
- `crates/glowfly-app/tests/snapshots/`: approved view snapshots: a `<name>.txt` text snapshot for every view test, and a `<name>.png` for the pixel goldens (`src/ui/snapshot_check/goldens.rs`).
- `scripts/rerender-conflicted.ps1`: after a merge or rebase stops on snapshot conflicts, re-renders only the conflicted snapshots (the tests that name them) and stages them; `-DryRun` lists them.

## Installers

- `.github/workflows/installers.yml`: `scripts/installer-check-windows.ps1`, `scripts/installer-check-linux.sh`, `scripts/update-feed-check.ps1`: the packaged builds on clean runners.
- The MSI and the .deb are installed over a v0.0.1 package, and their files, shortcuts or desktop entries, registry and icons checked.
- The program is started (`--version`; on Linux also the window under its own Xvfb), update-checked against local feeds (up to date, newer, wrong or missing checksum, relative feed), and uninstalled with nothing left behind.
- `scripts/package-check-licences.sh`: every archive and installer (tar.gz, zip, .app, dmg, AppImage, .deb, .rpm) holds `LICENSE-MIT`, `LICENSE-APACHE` and `THIRD-PARTY-NOTICES.md`, the same as the repository's; run by the Release workflow after packaging and by Installers, whose macOS job packages the app only for this.
- `scripts/installer-check-windows.ps1` checks the zip and the installed MSI's licence files the same way; `scripts/installer-check-flatpak.sh` checks the installed bundle's, `GIT-NOTICE.md` and git's licences in `/app/share/doc/git/`.
- `scripts/release-check-git-source.sh`: a folder of release files with a `.flatpak` also has the git source archive the manifest names, with its sha256; run by the Release workflow's checksums job and by Installers.
- `crates/glowfly-app/tests/flatpak_package.rs`: the Flatpak manifest and its jobs, read line by line, including git's licences, `GIT-NOTICE.md` naming the manifest's git archive, and the release attaching it.

## Workflows

- Every action in `.github/workflows/` is pinned to a commit SHA with its version in a comment; `.github/dependabot.yml` proposes the updates (weekly for actions, skipping major versions; monthly for Cargo, inside the Cargo.toml requirements). Each workflow sets read-only token permissions at the top and each job adds only what it needs (`release.yml`'s publishing jobs `contents: write`, its checksums job also `id-token: write` and `attestations: write` for the build provenance, `codeql.yml` and `scorecard.yml` `security-events: write`); every checkout sets `persist-credentials: false`.
- `.github/workflows/release.yml`: release-please, then the prepare job in the `release` environment (the one owner approval) and the build, flatpak, checksums (SHA256SUMS and the build provenance attestation, attached as `glowfly-<tag>.intoto.jsonl`) and manifests jobs after it; release builds use no Rust cache.
- `.github/workflows/zizmor.yml`: zizmor lints the workflows on every push and pull request, failing on findings of medium severity or higher.
- `.github/workflows/fuzz.yml`: runs every fuzz target (see Fuzzing) nightly for four minutes each, and for a minute each on pull requests that change `glowfly-core`, `fuzz/` or `Cargo.lock`; keeps the corpus in the Actions cache and uploads a failing input as `fuzz-artifacts`. Not a required check.
- `.github/workflows/scorecard.yml`: OpenSSF Scorecard, weekly, on branch protection changes and pushes to `main`; results go to the Security tab and are published for the README badge.
- `deny.toml` and `osv-scanner.toml`: the advisory exceptions, once for cargo-deny (CI's deny job) and once for osv-scanner (Scorecard's Vulnerabilities check), kept identical (and named in SECURITY.md's Known advisories) by `glowfly-core`'s `advisory_ignores_tests.rs`.
- `.github/workflows/codeql.yml`: CodeQL code scanning of the Rust code and the workflows (`build-mode: none`), on pushes and pull requests to `main` and weekly; results go to the Security tab.
- `.github/codeql/codeql-config.yml`: its config: every default query on, test code (`*_tests.rs`, `tests/`, `tests.rs`, `snapshot_tests`, `testkit/`) left out.
