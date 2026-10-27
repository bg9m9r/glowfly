# Releasing Glowfly

Releases are cut by [release-please](https://github.com/googleapis/release-please) and built by
`.github/workflows/release.yml`. Nobody edits a version by hand.

## The flow

1. Every push to `main` updates a "chore(main): release x.y.z" pull request. Its version and its
   `CHANGELOG.md` entry come from the Conventional Commit messages since the last release.
2. Merging that pull request tags `vX.Y.Z` and creates the GitHub release.
3. The **build** job runs once per target. It builds `glowfly` in release mode, then the
   packaging script for its OS (`scripts/package-windows.ps1`, `scripts/package-macos.sh`,
   `scripts/package-linux.sh`) writes the files below into `dist/` and uploads them.
   The **flatpak** job builds the Flatpak bundle (`scripts/package-flatpak.sh`, see
   [Building the Flatpak locally](#building-the-flatpak-locally)) and uploads it too.
4. The **checksums** job downloads every `glowfly-*` file of the release and publishes
   `SHA256SUMS` over them.
5. The **manifests** job fills in the Homebrew cask and the winget manifests from `SHA256SUMS`
   and attaches them to the release (see [Homebrew and winget](#homebrew-and-winget)).

The build matrix has six targets: Windows x64 and ARM64 (both on `windows-latest`; ARM64 is
cross-compiled with the image's ARM64 build tools, and LLVM's clang is put on `PATH` for ring),
Linux x86_64 (`ubuntu-22.04`) and aarch64 (`ubuntu-22.04-arm`, built natively), and macOS on
Apple silicon and Intel (`macos-14`).

To rebuild an existing release's files, run the workflow by hand with its tag.

`CHANGELOG.md` is also bundled into the program at build time (`crates/glowfly-app/build.rs`):
it is what the "View release notes" dialog shows, and the dialog opens by itself the first time
a new version starts.

## What a release contains

| File | What it is |
| --- | --- |
| `glowfly-vX.Y.Z-x86_64-pc-windows-msvc.msi` | Windows installer (WiX 5, `packaging/windows/glowfly.wxs`, pages in `pages.wxs`, artwork in `assets/installer/` drawn by the app icon test): a per-user install into `%LOCALAPPDATA%\Programs\Glowfly`, no administrator rights; Welcome, License, Options, Ready and Finish pages, with "Launch Glowfly" at the end; Start menu shortcut; on the Options page, a desktop shortcut, "Open in Glowfly" on folders and folder backgrounds, and the `glowfly://` scheme in `HKEY_CURRENT_USER`, each remembered for upgrades; upgrades replace older (and same-version) builds and a downgrade is refused. `scripts/package-windows.ps1` fetches the WiX UI and Util extensions into `.wix/` |
| `glowfly-vX.Y.Z-x86_64-pc-windows-msvc.zip` | The Windows program on its own |
| `glowfly-vX.Y.Z-aarch64-pc-windows-msvc.msi`, `.zip` | The same for Windows on Arm (`wix build -arch arm64`). The MSI keeps the x64 one's UpgradeCode and folder, so it upgrades an x64 per-user install in place; no CI runner installs it |
| `glowfly-vX.Y.Z-aarch64-apple-darwin.dmg`, `glowfly-vX.Y.Z-x86_64-apple-darwin.dmg` | `Glowfly.app` (`packaging/macos/Info.plist`, the icns) and a link to `/Applications` |
| `glowfly-vX.Y.Z-<mac target>.tar.gz` | The macOS program on its own |
| `glowfly-vX.Y.Z-<linux target>.deb` | For `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`: `/usr/bin/glowfly`, `glowfly.desktop` (folders open with Glowfly), the hidden `glowfly-links.desktop` for `glowfly://` links, hicolor icons from 16 to 512 px, the licences and README in `/usr/share/doc/glowfly`, and `/usr/share/glowfly/package-format` reading `deb`; depends on git and on the windowing libraries the program loads at run time, which `$auto` can't see (xkbcommon and its X11 part, Xlib and its xcb bridge, Xcursor, Xi, xcb, the Wayland client and its EGL bridge, EGL and GL) |
| `glowfly-vX.Y.Z-<linux target>.rpm` | The same files (cargo-generate-rpm, `[package.metadata.generate-rpm]` in `crates/glowfly-app/Cargo.toml`), with `package-format` reading `rpm`; requires git, the libraries the program links, and the same run-time libraries as the .deb, named by soname (`libxkbcommon-x11.so.0()(64bit)`) so Fedora and openSUSE both resolve them. A pre-release's version uses `~` for `-` (`0.3.0~rc.1`), so it sorts before the release |
| `glowfly-vX.Y.Z-<linux target>.AppImage` | The program, its desktop entry and icons in one file that runs from anywhere (type-2 runtime; appimagetool and the runtime are pinned by version and SHA-256 in `scripts/package-linux.sh`). It bundles no libraries: it needs glibc 2.35 or newer (it is built on Ubuntu 22.04), GTK 3, git and the .deb's run-time windowing libraries on the system, which every X11 or Wayland desktop has; without FUSE, run it with `--appimage-extract-and-run` |
| `glowfly-vX.Y.Z-x86_64-unknown-linux-gnu.flatpak` | A Flatpak bundle with the app id `io.github.bg9m9r.glowfly` (`packaging/flatpak/`): the program, its own git (`/app/bin/git`), the desktop entries and icons named for the app id, and AppStream metadata, on the `org.freedesktop.Platform` 25.08 runtime, which installs from Flathub. Built by the `flatpak` job in the Flathub builder image; x86_64 only |
| `glowfly-vX.Y.Z-<linux target>.tar.gz` | The Linux program on its own |
| `<file>.sha256` | The SHA-256 of each file above |
| `SHA256SUMS` | Every file's SHA-256, in `sha256sum` format |
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
build failed), the job fails loudly; the release itself is unaffected. Submitting is by hand:

- **Homebrew**: put `glowfly.rb` in a tap repository as `Casks/glowfly.rb` (for example
  `<owner>/homebrew-glowfly`; then `brew install --cask <owner>/glowfly/glowfly`). The main
  `homebrew/cask` repository expects notarized apps, so a pull request there waits for the Apple
  secrets below. The cask says `auto_updates true`: Glowfly updates itself.
- **winget**: put the three yaml files in one folder and check them with
  `winget validate --manifest <folder>`, then submit them with
  `wingetcreate submit --token <PAT> <folder>`, or as a pull request to `microsoft/winget-pkgs`
  under `manifests/g/Glowfly/Glowfly/<version>/`. The manifests list the x64 and arm64 MSIs,
  install per user, and name the MSI's UpgradeCode so winget recognises an existing install.

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

(`shasum -a 256 -c SHA256SUMS --ignore-missing` on macOS; on Windows compare
`Get-FileHash <file>` with the file's line.) Signed builds can also be checked with
`signtool verify /pa glowfly.exe` and `codesign --verify --deep --strict Glowfly.app` or
`spctl -a -vv Glowfly.app`.

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
  `$RUNNER_TEMP`, never `/tmp`, which the sandbox has its own of.

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
