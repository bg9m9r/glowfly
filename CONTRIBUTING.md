# Contributing to Trellis

Thanks for your interest! Bug reports, ideas and pull requests are all welcome.

## Before you start

- **Bugs**: open an issue with the steps to reproduce, what you expected, and what happened.
  The activity log (bottom panel) shows the exact git commands Trellis ran, which helps a lot.
- **Features**: check [TODO.md](TODO.md) first, then open an issue to agree on the approach
  before writing a large change.
- **Security problems**: don't open a public issue; see [SECURITY.md](SECURITY.md).

## Building and checking

You need a recent stable Rust toolchain, Git, and PowerShell 7 (`pwsh`). Trellis is developed on
Windows; CI also builds it on macOS and Linux, where it is less tested. On Debian or Ubuntu,
`scripts/linux-deps.sh` installs the system libraries the build needs.

```
cargo run --release -- <repo path>     # run the app
pwsh scripts/check.ps1                 # fmt, clippy (-D warnings) and all tests
```

`scripts/check.ps1` must pass before a pull request is merged; CI runs the same script.

UI changes are covered by snapshot tests. Re-render them with
`UPDATE_SNAPSHOTS=1 cargo test -p trellis snapshot`, look at the changed PNGs in
`crates/trellis-app/tests/snapshots/`, and commit them with your change.

## How the code is organized

Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before structural changes. The short version:

- Git, CLI and network code lives in `trellis-core`, which must not depend on egui.
- New behaviour is a `Msg` handled in `update/<area>.rs`. Anything slow returns an `Effect` that
  `runner.rs` executes in the background, reporting back with a result `Msg`. Test it with a
  headless `update` test.
- Views in `ui/` only draw and push `Msg`s; they do no I/O (`tests/layering.rs` checks this).
- Keep files under about 400 lines; split by responsibility.
- Operations that should honour the user's git config, hooks or credentials go through the git
  CLI (`gitcli`); everything else uses libgit2.

## Original work only

Trellis implements features that many Git clients share, but every line of code, text, icon and
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
