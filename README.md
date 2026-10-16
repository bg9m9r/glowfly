# Trellis

A fast, native Git GUI written in Rust (egui + libgit2): a central commit graph, a
branch/remote/tag/stash sidebar, and a right-hand panel for commit details or staging and
committing. It connects to GitHub, GitLab, Bitbucket, Azure DevOps, Jira and Trello for pull
requests and issues.

Trellis is an independent project. It is not affiliated with or endorsed by any other Git client
or hosting service; product names are used only to describe compatibility.

## Install

Builds for Windows, macOS (Apple silicon and Intel) and Linux are on the
[Releases](https://github.com/bg9m9r/trellis/releases) page. Git must be installed and on your
`PATH`.

- **Windows**: `trellis-vX.Y.Z-x86_64-pc-windows-msvc.msi` installs for your user only (no
  administrator rights) into `%LOCALAPPDATA%\Programs\Trellis`, with a Start menu shortcut,
  "Open in Trellis" on folders in Explorer (on Windows 11 under "Show more options") and
  `trellis://` links. Uninstall it from Settings > Apps.
- **macOS**: open `trellis-vX.Y.Z-<arch>-apple-darwin.dmg` and drag Trellis to Applications.
- **Debian and Ubuntu**: `sudo apt install ./trellis-vX.Y.Z-x86_64-unknown-linux-gnu.deb`. It
  adds Trellis to the applications menu, "Open with Trellis" on folders in the file manager, and
  `trellis://` links.
- **Anywhere**: the `.zip` (Windows) and `.tar.gz` (macOS, Linux) hold the program on its own;
  unpack it wherever you like.

Each download has a `.sha256` file, and `SHA256SUMS` lists them all: check a download with
`sha256sum -c SHA256SUMS --ignore-missing`. Builds are code-signed once the project's signing
certificates are set up (see [docs/RELEASING.md](docs/RELEASING.md)); until then Windows
SmartScreen and macOS Gatekeeper warn the first time you open them.

Trellis checks GitHub for a newer release once a day (Preferences > General > Updates, where you
can turn it off) and downloads it only when you ask, after checking it against the published
checksums. "View release notes" in the command palette shows what changed.

## Building from source

Trellis is developed and tested on Windows; macOS and Linux builds are less tested. You need a
recent stable Rust toolchain and Git on your `PATH` (on Debian or Ubuntu, also run
`scripts/linux-deps.sh`).

```
cargo run --release -- C:\path\to\repo
```

With no path it reopens the last repository. See [TODO.md](TODO.md) for the roadmap.

## What it does

- **Commit graph**: lane layout with merge/fork curves, branch, remote and tag badges, a
  `// WIP` node above HEAD when the tree is dirty, ↑/↓ keyboard navigation, and commit search
  (Enter jumps to the next match).
- **Sidebar**: local branches (ahead/behind counts), remotes, tags and stashes. Click jumps to the
  commit, double-click checks out, and right-click opens merge, rebase, delete, push-tag,
  apply/pop/drop.
- **Staging**: stage or unstage single files, whole lists, or individual hunks. You can discard a
  file or hunk. "Discard all" moves changes into a stash so they can be recovered. In a commit's
  diff, Revert Hunk undoes that hunk in your working tree.
- **Commit**: summary with a 72-char counter, description, and amend. Runs through `git commit`,
  so hooks and GPG signing apply.
- **Commit context menu**: create branch/tag here, checkout (detached), cherry-pick, revert,
  reset soft/mixed/hard.
- **Remote operations**: fetch, pull (ff / ff-only / rebase), push (sets upstream automatically,
  force-with-lease behind a confirmation), plus clone and init.
- **Merge/rebase/cherry-pick in progress**: a banner with Continue/Skip/Abort, and conflicted
  files are marked with `!`.
- **Activity log** with the exact git commands run and their output. Auto-refreshes when the
  window regains focus, or on F5.

## Hosting services and issue trackers

| Service | Pull/merge requests | Issues | Sign-in |
|---|---|---|---|
| GitHub / GitHub Enterprise Server | ✔ (with review states) | GitHub Issues | GCM, `GITHUB_TOKEN`/`GH_TOKEN`, PAT |
| GitLab / GitLab Self-Managed | ✔ merge requests (with approvals) | GitLab Issues | GCM, `GITLAB_TOKEN`, PAT |
| Bitbucket Cloud | ✔ | Bitbucket Issues | GCM, username + app password |
| Bitbucket Data Center | ✔ | (use Jira) | GCM, HTTP access token |
| Azure DevOps | ✔ | Azure Boards | GCM, `AZURE_DEVOPS_EXT_PAT`, PAT |
| Jira Cloud / Jira Data Center | — | ✔ | email + API token / PAT |
| Trello | — | ✔ cards | API key + token |

The code host is detected from the remote URL (`origin` preferred). Self-hosted servers are
recognized once connected in Preferences, or by common URL shapes (`gitlab.*`, `github.*`,
`/scm/` for Bitbucket); a server recognized only by its URL shape is not contacted until you
connect it. The `*_TOKEN` environment variables are used only for github.com, gitlab.com and
Azure DevOps, never for self-hosted servers. The issue tracker defaults to the host's own issues, and can be switched
to a connected Jira site (optionally one project) or a connected Trello account (optionally one
board) under Repo-Specific Preferences > Issue Tracker.

- **Pull requests**: a PULL REQUESTS (or MERGE REQUESTS) sidebar section with drafts, approval
  counts and a reviewer tooltip, plus badges on the head commit in the graph. You can open,
  check out, and create them from the toolbar **PR** button or a branch's context menu. The
  target defaults to the repo's default branch.
- **Issues**: the ISSUES section lists open issues assigned to you. "Create branch from issue"
  names the branch `123-title` / `PROJ-12-title` / `AbCd1234-title` (a Trello card id). Commits
  that mention `#123`, `AB#123`, `PROJ-123` or a Trello card link (`trello.com/c/AbCd1234`) show
  issue chips, and on an issue branch the commit panel offers to add the mention.
- **Linking**: a new PR links the issues mentioned on its branch. Azure Boards links through the
  API, GitHub and GitLab get `Closes #n`, Bitbucket gets `Resolves #n`, Jira gets the key, and
  Trello gets the card link.
- **Credentials**: an environment variable, then a token saved in the Windows Credential Manager,
  then Git Credential Manager (the same sign-in `git push` uses). Background refreshes (every 5
  minutes) never show a sign-in window.
- **Live check**: `TRELLIS_LIVE_REMOTE=<url> cargo test live_remote -- --ignored --nocapture` runs
  read-only API calls against a real service.

## Preferences

Open it with the gear button or Ctrl+, (Esc closes it): a navigation column, an Integrations
sub-list, and forms with right-aligned labels.

- **General**: auto-fetch interval (0–60 min) and auto-prune, default branch name for new repos,
  initial commits in the graph (minimum 500), reopen last repository, `core.longpaths`,
  `core.autocrlf`, and "Forget All" for connections. A second button also signs Git Credential
  Manager out of those hosts, after a confirmation.
- **Profile**: global `user.name` / `user.email`.
- **Integrations**: one page per service (GitHub, GitHub Enterprise Server, GitLab, GitLab
  Self-Managed, Bitbucket, Bitbucket Data Center, Azure DevOps, Jira Cloud, Jira Data Center,
  Trello).
  Each has a connection card with "Connect to …" (signs in through Git Credential Manager), or a
  host domain plus token. Credentials are verified before being saved.
- **UI Customization**: author/date columns and date format (ISO, long, relative).
- **Editor**: how the diff viewer draws text: font size, tab size (tab stops), line numbers,
  syntax highlighting and word-level highlights. Changes apply at once, without reloading.
- **Commit Signing**: `commit.gpgsign`, `gpg.format`, `user.signingkey`.
- **Repo-specific**: identity override (local `user.name` / `user.email`), `core.hooksPath`, and
  the issue tracker.

App settings live in `settings.json` in `%APPDATA%\Trellis` on Windows,
`~/Library/Application Support/Trellis` on macOS and `$XDG_CONFIG_HOME/trellis` (usually
`~/.config/trellis`) on Linux. Git settings are written straight to git
config. Tokens go to the Windows Credential Manager.

## Architecture

Two crates: `trellis-core` (git, hosting services, settings; no UI) and `trellis-app` (the egui
app, built as an Elm-style message/update loop with background effects).
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) describes the layout and the rules that keep it
tidy.

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) first; in short, run
`pwsh scripts/check.ps1` before sending a pull request. By taking part you agree to the
[Code of Conduct](CODE_OF_CONDUCT.md). Report security problems as described in
[SECURITY.md](SECURITY.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.

The binaries also embed third-party syntax definitions for diff highlighting; see
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) for their licenses.
