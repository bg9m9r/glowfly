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
Windows; other platforms may build but are not tested yet.

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

## Pull requests

- Keep each pull request focused on one change, with tests.
- Write commit messages that say what changed and why.
- By submitting a contribution you agree that it is licensed under the project's terms: MIT OR
  Apache-2.0, as described in the [README](README.md#license).
