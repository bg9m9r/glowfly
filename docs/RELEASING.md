# Releasing Glowfly

Releases are cut by [release-please](https://github.com/googleapis/release-please) and built by
`.github/workflows/release.yml`. Nobody edits a version by hand.

## The flow

1. Every push to `main` updates a "chore(main): release x.y.z" pull request. Its version and its
   `CHANGELOG.md` entry come from the Conventional Commit messages since the last release.
2. Merging that pull request pushes the tag `vX.Y.Z` and creates the GitHub release as a
   **draft** (`release-please-config.json`: `draft` and `force-tag-creation`; see
   [Drafts and release immutability](#drafts-and-release-immutability)). The **prepare** job then
   waits for the owner's approval (see [Approving a release](#approving-a-release)), finds the
   draft (`scripts/release-draft.sh`) and puts the fixed header above its notes (see
   [The release notes header](#the-release-notes-header)). Every job below uploads to that draft.
3. The **build** job runs once per target. It builds `glowfly` in release mode, then the
   packaging script for its OS (`scripts/package-windows.ps1`, `scripts/package-macos.sh`,
   `scripts/package-linux.sh`) writes the files below into `dist/`, checks that each archive and
   installer holds the licence and notice files (`scripts/package-check-licences.sh`) and
   uploads them. The **flatpak** job builds the Flatpak bundle (`scripts/package-flatpak.sh`,
   see [Building the Flatpak locally](#building-the-flatpak-locally)) and uploads it with the
   git source archive it was built from (see [Licences and notices](#licences-and-notices)).
4. The **checksums** job downloads every `glowfly-*` file of the release and the git source
   archive, fails when a `.flatpak` is there without its git source
   (`scripts/release-check-git-source.sh`), and publishes `SHA256SUMS` over them. It then
   attests their build provenance with `actions/attest-build-provenance`: one signed SLSA
   provenance statement covering every file in `SHA256SUMS` and `SHA256SUMS` itself, which says
   they were built by this workflow from this repository at the tag's commit. The attestation is
   signed through Sigstore with the job's OIDC identity (no key to keep), stored by GitHub, and
   attached as `glowfly-vX.Y.Z.intoto.jsonl` (see [Verifying a download](#verifying-a-download)).
   That bundle isn't in `SHA256SUMS`: it is made after it, from the digests it lists, and is
   signed itself. The job is the only one with `id-token: write` and `attestations: write`.
5. The **manifests** job fills in the Homebrew cask and the winget manifests from `SHA256SUMS`
   and attaches them to the release (see [Homebrew and winget](#homebrew-and-winget)).
6. The **publish** job runs last, and only when every job above succeeded. It
   (`scripts/release-publish.sh`) checks that `SHA256SUMS`, the provenance bundle and every file
   `SHA256SUMS` lists are on the draft, each with the listed SHA-256 where GitHub reports the
   file's digest, and then publishes it: a `vX.Y.Z` tag as the latest release, any other tag
   (`v0.3.0-rc.1`) as a pre-release that is never the latest. Until then the release is a draft
   that only people with write access see.

The build matrix has six targets: Windows x64 and ARM64 (both on `windows-latest`; ARM64 is
cross-compiled with the image's ARM64 build tools, and LLVM's clang is put on `PATH` for ring),
Linux x86_64 (`ubuntu-22.04`) and aarch64 (`ubuntu-22.04-arm`, built natively), and macOS on
Apple silicon and Intel (`macos-14`).

When a job fails, the release stays a draft: fix the cause and choose **Re-run failed jobs**.
The re-run uploads to the same draft (`--clobber` replaces a file from the first try), the jobs
after it run again, and publish runs once all of them succeed.

To build an existing tag's files, run the workflow by hand with its tag. With no release for the
tag, prepare creates a draft (named `Glowfly <tag>`, with GitHub's generated notes) and the run
ends by publishing it, as above; an existing draft is reused. A release that is already published
is rebuilt in place and left published, which works only while it isn't immutable: prepare fails
for an immutable one, whose files can't change. Prepare also fails when the tag has more than
one release (delete the extra drafts), since every job finds the release by its tag. The build
jobs use the scripts at that tag, so a tag from before the licence checks existed fails them: its
packages lack those files and shouldn't be published again. A run by hand waits for the same
approval, and only a run started from `main` (or a `v*` tag) can get it.

The build jobs don't use a Rust cache: every release binary is built from a clean target
folder, so a cache entry written by another run can never end up in a download.

### Approving a release

Publishing runs in the `release` deployment environment (Settings > Environments), which has the
owner as its required reviewer and accepts only the `main` branch and `v*` tags. The **prepare**
job is the only job in that environment, and every job that uploads to, edits or publishes the
release (build, flatpak, checksums, manifests, publish) needs it, so a release takes one
approval: open the Release run in the Actions tab, choose **Review deployments**, tick `release`
and approve. Until then nothing is built, uploaded or published; rejecting it skips every
publishing job. The draft that release-please created stays an unpublished draft without
downloads until the run is approved, or until the workflow is run by hand for its tag; its tag
is public from the start. The **release-please** job itself isn't gated, so pushes to `main`
don't wait.

`CHANGELOG.md` is also bundled into the program at build time (`crates/glowfly-app/build.rs`):
it is what the "View release notes" dialog shows, and the dialog opens by itself the first time
a new version starts.

## Drafts and release immutability

With release immutability on (Settings > General > Releases > **Enable release immutability**),
a published release's files can't be added, replaced or deleted and its tag can't be moved or
deleted; its title and notes stay editable. A stolen token or a compromised workflow then can't
swap a download after the fact, and GitHub generates a release attestation for each release
it publishes. GitHub's advice for this is to create the release as a draft, attach every file, then
publish it, and that is the flow above:

- release-please creates the draft with `draft: true`. GitHub creates a draft's tag only when
  it is published, so `force-tag-creation: true` makes release-please push the tag first. The
  next release pull request finds the last release by that tag: release-please skips releases
  without a tagged commit, then falls back to looking up the tag the manifest's version names.
- Every upload goes to the draft (`gh` finds a draft by its tag), and nothing touches the
  release's files after the publish job.
- Pre-releases aren't marked by release-please (its `prerelease` option would mark every 0.x
  release too); the publish job marks every tag that isn't `vX.Y.Z`.

**One-time step for the owner**: after the first release that goes out this way has been
published with all its files, turn on **Enable release immutability** in Settings > General >
Releases. Nothing in the workflow depends on it being on or off. Once it is on, a published
release can't be rebuilt; publish a new version instead.

Nothing outside the repository's writers sees a draft: the update check reads
`/releases/latest`, which GitHub answers with the newest published release that isn't a
pre-release, and the Homebrew and winget manifests point at download links that exist only once
the release is published.

## What a release contains

| File | What it is |
| --- | --- |
| `glowfly-vX.Y.Z-x86_64-pc-windows-msvc.msi` | Windows installer (WiX 5, `packaging/windows/glowfly.wxs`, pages in `pages.wxs`, artwork in `assets/installer/` drawn by the app icon test): a per-user install into `%LOCALAPPDATA%\Programs\Glowfly`, no administrator rights; Welcome, License, Options, Ready and Finish pages, with "Launch Glowfly" at the end; Start menu shortcut; on the Options page, a desktop shortcut, "Open in Glowfly" on folders and folder backgrounds, and the `glowfly://` scheme in `HKEY_CURRENT_USER`, each remembered for upgrades; upgrades replace older (and same-version) builds and a downgrade is refused. `scripts/package-windows.ps1` fetches the WiX UI and Util extensions into `.wix/` |
| `glowfly-vX.Y.Z-x86_64-pc-windows-msvc.zip` | The Windows program on its own |
| `glowfly-vX.Y.Z-aarch64-pc-windows-msvc.msi`, `.zip` | The same for Windows on Arm (`wix build -arch arm64`). The MSI keeps the x64 one's UpgradeCode and folder, so it upgrades an x64 per-user install in place; no CI runner installs it |
| `glowfly-vX.Y.Z-aarch64-apple-darwin.dmg`, `glowfly-vX.Y.Z-x86_64-apple-darwin.dmg` | `Glowfly.app` (`packaging/macos/Info.plist`, the icns, and the licence and notice files in `Contents/Resources`) and a link to `/Applications` |
| `glowfly-vX.Y.Z-<mac target>.tar.gz` | The macOS program on its own |
| `glowfly-vX.Y.Z-<linux target>.deb` | For `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`: `/usr/bin/glowfly`, `glowfly.desktop` (folders open with Glowfly), the hidden `glowfly-links.desktop` for `glowfly://` links, hicolor icons from 16 to 512 px, the licences and README in `/usr/share/doc/glowfly`, and `/usr/share/glowfly/package-format` reading `deb`; depends on git and on the windowing libraries the program loads at run time, which `$auto` can't see (xkbcommon and its X11 part, Xlib and its xcb bridge, Xcursor, Xi, xcb, the Wayland client and its EGL bridge, EGL and GL) |
| `glowfly-vX.Y.Z-<linux target>.rpm` | The same files (cargo-generate-rpm, `[package.metadata.generate-rpm]` in `crates/glowfly-app/Cargo.toml`), with `package-format` reading `rpm`; requires git, the libraries the program links, and the same run-time libraries as the .deb, named by soname (`libxkbcommon-x11.so.0()(64bit)`) so Fedora and openSUSE both resolve them. A pre-release's version uses `~` for `-` (`0.3.0~rc.1`), so it sorts before the release |
| `glowfly-vX.Y.Z-<linux target>.AppImage` | The program, its desktop entry and icons in one file that runs from anywhere (type-2 runtime; appimagetool and the runtime are pinned by version and SHA-256 in `scripts/package-linux.sh`). It bundles no libraries: it needs glibc 2.35 or newer (it is built on Ubuntu 22.04), GTK 3, git and the .deb's run-time windowing libraries on the system, which every X11 or Wayland desktop has; without FUSE, run it with `--appimage-extract-and-run` |
| `glowfly-vX.Y.Z-x86_64-unknown-linux-gnu.flatpak` | A Flatpak bundle with the app id `io.github.bg9m9r.glowfly` (`packaging/flatpak/`): the program, its own git (`/app/bin/git`), the desktop entries and icons named for the app id, and AppStream metadata, on the `org.freedesktop.Platform` 25.08 runtime, which installs from Flathub. Built by the `flatpak` job in the Flathub builder image; x86_64 only. Its licence files are in `/app/share/doc/glowfly` with `GIT-NOTICE.md`, and git's in `/app/share/doc/git` |
| `git-<version>.tar.xz` | The git source the Flatpak's git was built from, byte for byte the archive the manifest names (its url and sha256); only in a release with a `.flatpak` |
| `glowfly-vX.Y.Z-<linux target>.tar.gz` | The Linux program on its own |
| `<file>.sha256` | The SHA-256 of each file above |
| `SHA256SUMS` | Every file's SHA-256, in `sha256sum` format |
| `glowfly-vX.Y.Z.intoto.jsonl` | The Sigstore bundle of the build provenance attestation over every file in `SHA256SUMS` and `SHA256SUMS` itself |
| `glowfly.rb`, `Glowfly.Glowfly.yaml`, `Glowfly.Glowfly.installer.yaml`, `Glowfly.Glowfly.locale.en-US.yaml` | The Homebrew cask and the winget manifests for this release (not for a pre-release) |

Install and restart reads the zip and tar.gz layout (`glowfly-<tag>-<target>/` holding the
program and the doc files) to swap a portable copy in place (`app_update/archive.rs`), so keep
it when the packaging scripts change.

The app icon is drawn by `crates/glowfly-app/tests/app_icon.rs`; `UPDATE_ICONS=1 cargo test -p
glowfly --test app_icon` redraws the files in `assets/icon/`. The Windows build embeds
`glowfly.ico` in `glowfly.exe` (`build.rs`, through winresource).

## Homebrew and winget

After the checksums, the **manifests** job runs `scripts/fill-manifests.sh <tag> SHA256SUMS
<out dir> <owner/repo>`. It fills the templates in `packaging/homebrew/glowfly.rb.in` and
`packaging/winget/*.yaml.in` with the version, the tag, the repository and the SHA-256 of both
dmgs and both MSIs, and attaches the results to the release. A pre-release tag (anything but
`vX.Y.Z`) gets a notice and no manifests. When `SHA256SUMS` lacks a file the manifests need (a
build failed), the job fails loudly and the release stays a draft, since publish needs every
job to succeed. Submit them by hand once the release is published (their download links work
only then):

- **Homebrew**: put `glowfly.rb` in a tap repository as `Casks/glowfly.rb` (for example
  `<owner>/homebrew-glowfly`; then `brew install --cask <owner>/glowfly/glowfly`). The main
  `homebrew/cask` repository expects notarized apps, so a pull request there waits for the Apple
  secrets below. The cask says `auto_updates true`: Glowfly updates itself.
- **winget**: put the three yaml files in one folder and check them with
  `winget validate --manifest <folder>`, then submit them with
  `wingetcreate submit --token <PAT> <folder>`, or as a pull request to `microsoft/winget-pkgs`
  under `manifests/g/Glowfly/Glowfly/<version>/`. The manifests list the x64 and arm64 MSIs,
  install per user, and name the MSI's UpgradeCode so winget recognises an existing install.

## Licences and notices

Every archive and installer carries `LICENSE-MIT`, `LICENSE-APACHE` and `THIRD-PARTY-NOTICES.md`:
beside the program in the zip and tar.gz, in the install folder of the MSI, in
`/usr/share/doc/glowfly` for the .deb, .rpm and AppImage, in `Glowfly.app/Contents/Resources` on
macOS, and in `/app/share/doc/glowfly` in the Flatpak. `scripts/package-check-licences.sh`
checks them in every package the Release workflow builds (the zip on Windows; the .app, dmg and
tar.gz on macOS; the tar.gz, .deb and AppImage on Linux), and the Installers workflow checks
them in the installed packages and the .rpm.

The Flatpak is the only package with its own git. git is under the GPL-2.0, so the bundle also
carries:

- git's licences in `/app/share/doc/git`: `COPYING` (the GPL-2.0), `LGPL-2.1`, and the licences
  of the two libraries it is built with, `reftable-LICENSE` (BSD-3-Clause) and
  `sha1dc-LICENSE.txt` (MIT);
- `packaging/flatpak/GIT-NOTICE.md`, installed as `/app/share/doc/glowfly/GIT-NOTICE.md`: which
  git it is, its licence, and where its source is.

The source itself goes out with the bundle: `package-flatpak.sh bundle` copies the archive
flatpak-builder downloaded (or downloads it again), checks it against the manifest's sha256 and
writes it to `dist/git-<version>.tar.xz`, and the flatpak job uploads it beside the `.flatpak`.
When git is updated, change its url and sha256 in the manifest and the version, url and sha256 in
`GIT-NOTICE.md` together; `tests/flatpak_package.rs` fails until they match.

## The release notes header

The **prepare** job runs `scripts/release-notes-header.sh <tag>`, which puts
`packaging/release-notes-header.md` above the release's notes: the builds aren't code-signed
yet, how to check a download against `SHA256SUMS` (a command for each system) and its build
provenance (`gh attestation verify`), and that Glowfly
is provided as is, without warranty, under MIT OR Apache-2.0, and a link to `PRIVACY.md`. The header sits between two HTML
comment markers, so a re-run replaces it instead of adding a second. The header has no `## `
heading and no list items, so the app's release notes dialog, which reads the release body,
skips it (`changelog_tests.rs` checks). Edit the file to change the text, for example once the
builds are signed; `bash scripts/release-notes-header.sh --print <notes file>` shows the result
without touching a release.

## Code signing

Signing happens only in CI, and only when the secrets below are set on the repository
(Settings > Secrets and variables > Actions). Without them each packaging script prints a
`::notice::` line and goes on, and the release still succeeds: the Windows files are unsigned and
the macOS app is ad-hoc signed (`codesign --sign -`).

| Secret | Used for |
| --- | --- |
| `WINDOWS_CERTIFICATE` | The code-signing certificate as a base64 PFX (`base64 -w0 cert.pfx`) |
| `WINDOWS_CERTIFICATE_PASSWORD` | Its password |
| `APPLE_CERTIFICATE` | A "Developer ID Application" certificate as a base64 .p12 |
| `APPLE_CERTIFICATE_PASSWORD` | Its password |
| `APPLE_SIGNING_IDENTITY` | The identity name, e.g. `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID` | The Apple ID that notarizes |
| `APPLE_TEAM_ID` | The team id |
| `APPLE_APP_PASSWORD` | An app-specific password for that Apple ID |

What gets signed:

- **Windows**: `glowfly.exe` before it goes into the zip and the MSI, then the MSI, with
  `signtool sign /fd sha256` and an RFC 3161 timestamp (`/tr`).
- **macOS**: `Glowfly.app` with the hardened runtime and a timestamp
  (`codesign --options runtime --timestamp`), then the dmg. With `APPLE_ID`, `APPLE_TEAM_ID`
  and `APPLE_APP_PASSWORD` as well, the dmg is notarized (`notarytool submit --wait`) and the
  ticket stapled (`stapler staple`).
- **Linux**: nothing; the checksums cover the files.

The temporary keychain and certificate files are removed at the end of each job, whether it
succeeded or not.

## Verifying a download

Download `SHA256SUMS` next to the file and run:

```
sha256sum -c SHA256SUMS --ignore-missing
```

(`shasum -a 256 -c SHA256SUMS --ignore-missing` on macOS; on Windows, in PowerShell, the
command in the release notes header, which prints OK or FAILED for each file listed that is in
the folder.) Signed builds can also be checked with
`signtool verify /pa glowfly.exe` and `codesign --verify --deep --strict Glowfly.app` or
`spctl -a -vv Glowfly.app`.

Every file in `SHA256SUMS`, and `SHA256SUMS` itself, also has a build provenance attestation,
which shows it was built by the Release workflow from this repository. With the GitHub CLI:

```
gh attestation verify <file> --repo bg9m9r/glowfly
```

It looks the attestation up on GitHub by the file's digest; `--bundle glowfly-vX.Y.Z.intoto.jsonl`
checks it against the release's bundle instead. The app's update check doesn't read the
provenance yet: it checks a download against `SHA256SUMS` only.

## The update check

Once a day (and when you click Check now), Glowfly makes one unauthenticated request to
`https://api.github.com/repos/<owner>/<repo>/releases/latest` for the repository in its
`Cargo.toml`. It sends no token and nothing about you or your repositories; GitHub sees the
request's address and a `Glowfly/<version>` user agent. The first automatic check waits 15
seconds after startup; a failed one is retried after an hour and only logged.

When a newer release is out, Glowfly offers the file that fits how this copy was installed,
falling back to the other one when the release lacks it:

| Copy | Offered first, then | Install |
| --- | --- | --- |
| Installed by the MSI | msi, zip | Install and restart: the MSI runs once Glowfly has quit |
| Portable on Windows | zip, msi | Install and restart: the program is swapped in place |
| `Glowfly.app` | dmg, tar.gz | Open installer and quit |
| Portable on macOS or Linux | tar.gz, then the system's installer | Install and restart: the program is swapped in place |
| The .deb (`/usr/bin/glowfly`, `package-format` reading `deb` or missing) | deb, tar.gz | Open installer and quit (the software installer) |
| The .rpm (`/usr/bin/glowfly`, `package-format` reading `rpm`) | rpm, tar.gz | Open installer and quit (the software installer) |
| An AppImage (`APPIMAGE` and `APPDIR` set, the program inside `APPDIR`) | AppImage, tar.gz | Install and restart: the AppImage file is swapped in place and started again from the same path |
| Flatpak (`/.flatpak-info`, or `FLATPAK_ID` with the program under `/app`) | nothing; says to update with Flatpak or the software centre | Nothing: Release notes and Open release page only |

The machine's real processor decides the build: an x64 Glowfly emulated on Windows on Arm, or an
x86_64 one under Rosetta, is offered the aarch64 file. When a release has no aarch64 file for
Windows or macOS, the x86_64 one is offered (both systems run it); Linux on Arm never falls back.
The format is chosen before the processor, so an installed copy on Windows on Arm takes an x64
MSI before an arm64 zip.

A download happens only when you click Download: it is fetched into your Downloads folder,
checked against `SHA256SUMS` (or the file's `.sha256`), and discarded if the checksum is missing
or doesn't match. Install checks it again before anything runs or is replaced; an AppImage swap
also checks that the file is an AppImage. A file the copy can't install (an archive for an
installed copy, for example) is only shown. No copy ever takes the `.flatpak` bundle: file
formats are matched by their exact extension.

Inside Flatpak, Glowfly still checks for releases and says when one is out, but never downloads
one: Flatpak (`flatpak update`, or the software centre) installs updates. A copy installed from a
downloaded `.flatpak` file updates by installing the next release's `.flatpak` the same way,
since the bundle comes from no hosted Flatpak repository.

Turn the check off in Preferences > General > Updates ("Check for updates once a day"). The
release notes dialog keeps working without it.

## Checking the installers

The `Installers` workflow (`.github/workflows/installers.yml`) runs on every push and pull
request (except ones that only touch `docs/`, `TODO.md` or `ROADMAP.md`). It builds the release
program, packages it twice with the scripts above (once as `v0.0.1`, once as the current
version), and on a clean runner:

- **Windows** (`scripts/installer-check-windows.ps1`): installs the v0.0.1 MSI silently
  (`msiexec /i … /qn`), upgrades it to the current one, checks that exactly one Glowfly at the
  current version is installed, its files, both shortcuts, the `HKEY_CURRENT_USER` keys, runs the
  installed program through each shortcut (`--version`) and through the update-feed check, then
  uninstalls it (`msiexec /x`) and checks that nothing is left.
- **Linux** (`scripts/installer-check-linux.sh`): the same with `apt-get install` of
  each .deb (so its dependencies come from the archive, not the runner's image) and
  `dpkg --purge`, plus `desktop-file-validate` on the installed entries, `package-format`, and
  the window started from `glowfly.desktop`'s `Exec` under its own Xvfb, seen drawing through
  the smoke probe.
- **The .rpm** (`scripts/installer-check-rpm.sh`), in a `fedora:43` container: `dnf install` of
  the v0.0.1 package and then the current one (dnf pulls git, GTK 3 and the run-time libraries; `requires.txt` must list them), the version, the files
  `rpm -ql` lists, `package-format`, the desktop entries, `--version`, `--check-update` against
  an empty feed folder (exit 3, `install rpm`), the window under Xvfb, then `dnf remove` and
  nothing left. PowerShell isn't in Fedora's repositories, so it doesn't run the update-feed
  check.
- **The AppImage**: run with `APPIMAGE_EXTRACT_AND_RUN=1` (no FUSE on the runner), it must print
  its version and, against an empty feed, exit 3 with `install appimage`, which shows the
  runtime's `APPIMAGE` and `APPDIR` reach the program.
- **The Flatpak** (`scripts/installer-check-flatpak.sh`, its own job on Ubuntu 22.04, since
  newer runners restrict the user namespaces bubblewrap needs): the bundle is built in the
  Flathub builder image, installed with `flatpak install --user` (the runtime comes from
  Flathub), and checked: the exported desktop entries (`desktop-file-validate`) and icon,
  `--version`, `--check-update` against an empty feed (exit 3, `install flatpak`) and against a
  newer release (exit 4, `update-with flatpak`, nothing downloaded), the bundled git and the
  runtime's ssh; then `flatpak uninstall` and no exported entry left. Its feed folders are under
  `$RUNNER_TEMP`, never `/tmp`, which the sandbox has its own of. Before installing, the job
  checks that the git source archive is in `dist/` beside the bundle
  (`scripts/release-check-git-source.sh`), and the install check reads the licence files,
  `GIT-NOTICE.md` and git's licences from the installed app.
- **The licence files** (`scripts/package-check-licences.sh`): the Linux job checks the tar.gz,
  .deb, .rpm and AppImage; the Windows check unpacks the zip and compares the installed MSI's
  files; the **macOS** job builds and packages the app (ad-hoc signed, no install) only to check
  the .app, the dmg and the tar.gz.

The Linux checks share `scripts/installer-lib-linux.sh`. MSI logs, every run's output and the
probe are uploaded when a job fails. The scripts refuse to run outside GitHub Actions unless
given `-AllowLocal` / `--allow-local`, and always refuse when Glowfly is already installed. The
ARM64 MSI and the aarch64 packages aren't installed by any check.

The update-feed check (`scripts/update-feed-check.ps1`) points `GLOWFLY_UPDATE_FEED` at local
folders shaped like a release:

```
feed/
  release.json                                  {"tag_name": "v0.1.1", "assets": [{"name": …}, …]}
  glowfly-v0.1.1-x86_64-pc-windows-msvc.msi     the current package, renamed to the feed's tag
  glowfly-v0.1.1-x86_64-pc-windows-msvc.zip
  SHA256SUMS                                    "<sha256>  <name>" lines, LF, no BOM
```

and runs `glowfly --check-update <folder>` for each case: the same version (exit 3), an empty
folder (3), a newer release (0, the installer or archive by install kind, byte for byte), a wrong
or missing checksum and a file changed after `SHA256SUMS` was written (1, nothing saved), and a
relative `GLOWFLY_UPDATE_FEED` (2). It installs nothing and opens no window, so it runs locally
against a debug build, which counts as portable, with stand-in package files of any bytes:

```
cargo build -p glowfly
pwsh -NoProfile -File scripts/update-feed-check.ps1 -Glowfly target/debug/glowfly.exe -Version 0.1.0 `
  -Installer <dir>/glowfly-v0.1.0-x86_64-pc-windows-msvc.msi `
  -Archive <dir>/glowfly-v0.1.0-x86_64-pc-windows-msvc.zip -Expect archive -Out <work folder>
```

The `--check-update` exit codes: 0 when a newer release's file was downloaded and verified, 3
when there is no newer release, 4 inside Flatpak when there is one (it prints `offer <version>`
and `update-with flatpak`, and writes nothing to the folder), 1 when the check or download
failed and 2 on a usage or configuration error. Inside Flatpak the feed and the folder must be
somewhere the sandbox sees: `/tmp` is the sandbox's own.

## Building the Flatpak locally

The Flatpak build has no network, so `scripts/package-flatpak.sh` works in two steps. On a
machine with cargo, `bash scripts/package-flatpak.sh vendor` copies every crate into `vendor/`
and writes the cargo config that points at it to `packaging/flatpak/generated/` (both ignored by
git). Then, where `flatpak` and `flatpak-builder` are installed (or in the
`ghcr.io/flathub-infra/flatpak-github-actions:freedesktop-25.08` image the workflows use, run
with `--privileged`), `bash scripts/package-flatpak.sh bundle v0.3.0` fills in the AppStream
metadata, builds `packaging/flatpak/io.github.bg9m9r.glowfly.yml` (git from its pinned tarball,
then Glowfly offline) and writes `dist/glowfly-v0.3.0-x86_64-unknown-linux-gnu.flatpak` and its
`.sha256`. Install it with `flatpak install --user ./dist/glowfly-…flatpak`.
