# Contributing to Glowfly

Thanks for your interest! Bug reports, ideas and pull requests are all welcome.

## Before you start

- **Bugs**: open an issue with the steps to reproduce, what you expected, and what happened.
  The activity log (bottom panel) shows the exact git commands Glowfly ran, which helps a lot.
- **Features**: check the [issues](https://github.com/bg9m9r/glowfly/issues) and
  [docs/ideas](docs/ideas/README.md) first, then open an issue to agree on the approach before
  writing a large change.
- **Security problems**: don't open a public issue; see [SECURITY.md](SECURITY.md).

## Building and checking

You need a recent stable Rust toolchain, Git, and PowerShell 7 (`pwsh`). Glowfly is developed on
Windows; CI also builds it on macOS and Linux, where it is less tested. On Debian or Ubuntu,
`scripts/linux-deps.sh` installs the system libraries the build needs.

```
cargo run --release -- <repo path>     # run the app
pwsh scripts/check.ps1                 # fmt, clippy (-D warnings) and all tests
```

`scripts/check.ps1` must pass before a pull request is merged; CI runs the same script.
CodeQL (`.github/workflows/codeql.yml`) also scans each pull request's Rust code and workflows;
its config (`.github/codeql/codeql-config.yml`) leaves test code out. zizmor
(`.github/workflows/zizmor.yml`) lints the workflows and fails on findings of medium severity or
higher: when you change a workflow, pin every action to a full commit SHA with a `# vX.Y.Z`
comment, keep `persist-credentials: false` on checkouts, pass `${{ }}` values to `run` steps
through `env:` and give each job only the permissions it needs. A deliberate exception is
suppressed on its line with `# zizmor: ignore[<rule>]` and the reason. To run it locally:
`pip install zizmor`, then `zizmor .github/workflows`. OpenSSF Scorecard
(`.github/workflows/scorecard.yml`) scores the repository weekly. The parsers of untrusted text
are fuzzed nightly (`.github/workflows/fuzz.yml`); how to run a target yourself is under
Fuzzing in `docs/architecture/tests.md`.

UI changes are covered by snapshot tests. Every view test keeps a text snapshot (`<name>.txt`:
the accessibility tree and the painted text) in `crates/glowfly-app/tests/snapshots/`, and about a
hundred pixel goldens (the themes, graph, diffs, design sheets and a few dialogs, listed in
`src/ui/snapshot_check/goldens.rs`) also keep a PNG. Re-render them with
`UPDATE_SNAPSHOTS=1 cargo test -p glowfly snapshot`, read the changed text, look at the changed
PNGs, and commit them with your change. A mismatch writes `<name>.diff.txt` or `<name>.diff.png`
beside the snapshot.

When a merge or rebase stops on conflicted snapshots, resolve the code first, then run
`pwsh scripts/rerender-conflicted.ps1`: it re-renders only the conflicted snapshots and stages
them. To have git leave conflicted snapshots alone for the script, you can set up a local merge
driver that keeps your side (your own choice; the repository doesn't ship one):

```
git config merge.glowfly-snapshot.name "keep ours; re-render with scripts/rerender-conflicted.ps1"
git config merge.glowfly-snapshot.driver true
echo "crates/glowfly-app/tests/snapshots/* merge=glowfly-snapshot" >> .git/info/attributes
```

With that driver git reports no conflict, so run the snapshot tests after the merge instead.

## How the code is organized

Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before structural changes; what each file holds
is listed per area in [docs/architecture/](docs/architecture/). The short version:

- Git, CLI and network code lives in `glowfly-core`, which must not depend on egui.
- New behaviour is a `Msg` handled in `update/<area>.rs`. Anything slow returns an `Effect` that
  the runner (`runner/mod.rs`) executes in the background, reporting back with a result `Msg`.
  Test it with a headless `update` test. The architecture document has a step-by-step recipe.
- Views in `ui/` only draw and push `Msg`s. Views, `update/` and `state/` do no I/O
  (`tests/layering.rs` checks this).
- Keep files under about 400 lines; split by responsibility.
- Operations that should honour the user's git config, hooks or credentials go through the git
  CLI (`gitcli`); everything else uses libgit2.

## Original work only

Glowfly implements features that many Git clients share, but every line of code, text, icon and
image must be your own or come from a dependency with a compatible license. Don't copy code,
strings, screenshots or assets from other Git clients, including closed-source ones.

## Commit messages and releases

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/), because
releases are cut from them automatically:

```
feat: rebase onto a commit from the graph's context menu
fix(diff): keep the scroll position when a hunk is staged
docs: explain the snapshot tests
```

- `feat:` adds a feature and bumps the minor version (0.1.0 to 0.2.0).
- `fix:` and `perf:` bump the patch version (0.1.0 to 0.1.1).
- `feat!:` or a `BREAKING CHANGE:` footer marks a breaking change (a minor bump while the
  version is below 1.0).
- `docs:`, `refactor:`, `test:`, `build:`, `ci:` and `chore:` don't trigger a release.

After each push to `main`, [release-please](https://github.com/googleapis/release-please) keeps
a release pull request open with the next version and its CHANGELOG entry. Merging it tags the
release, and the Release workflow builds Windows, macOS and Linux downloads and attaches them.
Pull requests are squash-merged, so the pull request title is the commit message that counts.

## Pull requests

- Keep each pull request focused on one change, with tests.
- Give the pull request a Conventional Commit title, and say in the description what changed and
  why.
- By submitting a contribution you agree that it is licensed under the project's terms: MIT OR
  Apache-2.0, as described in the [README](README.md#license).
