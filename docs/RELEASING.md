# Releasing Trellis

Releases are cut by [release-please](https://github.com/googleapis/release-please) and built by
`.github/workflows/release.yml`. Nobody edits a version by hand.

## The flow

1. Every push to `main` updates a "chore(main): release x.y.z" pull request. Its version and its
   `CHANGELOG.md` entry come from the Conventional Commit messages since the last release.
2. Merging that pull request tags `vX.Y.Z` and creates the GitHub release.
3. The **build** job runs once per target. It builds `trellis` in release mode, then the
   packaging script for its OS (`scripts/package-windows.ps1`, `scripts/package-macos.sh`,
   `scripts/package-linux.sh`) writes the files below into `dist/` and uploads them.
4. The **checksums** job downloads every `trellis-*` file of the release and publishes
   `SHA256SUMS` over them.

To rebuild an existing release's files, run the workflow by hand with its tag.

`CHANGELOG.md` is also bundled into the program at build time (`crates/trellis-app/build.rs`):
it is what the "View release notes" dialog shows, and the dialog opens by itself the first time
a new version starts.

## What a release contains

| File | What it is |
| --- | --- |
| `trellis-vX.Y.Z-x86_64-pc-windows-msvc.msi` | Windows installer (WiX 5, `packaging/windows/trellis.wxs`, pages in `pages.wxs`, artwork in `assets/installer/` drawn by the app icon test): a per-user install into `%LOCALAPPDATA%\Programs\Trellis`, no administrator rights; Welcome, License, Options, Ready and Finish pages, with "Launch Trellis" at the end; Start menu shortcut; on the Options page, a desktop shortcut, "Open in Trellis" on folders and folder backgrounds, and the `trellis://` scheme in `HKEY_CURRENT_USER`, each remembered for upgrades; upgrades replace older (and same-version) builds and a downgrade is refused. `scripts/package-windows.ps1` fetches the WiX UI and Util extensions into `.wix/` |
| `trellis-vX.Y.Z-x86_64-pc-windows-msvc.zip` | The Windows program on its own |
| `trellis-vX.Y.Z-aarch64-apple-darwin.dmg`, `trellis-vX.Y.Z-x86_64-apple-darwin.dmg` | `Trellis.app` (`packaging/macos/Info.plist`, the icns) and a link to `/Applications` |
| `trellis-vX.Y.Z-<mac target>.tar.gz` | The macOS program on its own |
| `trellis-vX.Y.Z-x86_64-unknown-linux-gnu.deb` | `/usr/bin/trellis`, `trellis.desktop` (folders open with Trellis), the hidden `trellis-links.desktop` for `trellis://` links, hicolor icons from 16 to 512 px, the licences and README in `/usr/share/doc/trellis`; depends on git |
| `trellis-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` | The Linux program on its own |
| `<file>.sha256` | The SHA-256 of each file above |
| `SHA256SUMS` | Every file's SHA-256, in `sha256sum` format |

The app icon is drawn by `crates/trellis-app/tests/app_icon.rs`; `UPDATE_ICONS=1 cargo test -p
trellis --test app_icon` redraws the files in `assets/icon/`. The Windows build embeds
`trellis.ico` in `trellis.exe` (`build.rs`, through winresource).

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

- **Windows**: `trellis.exe` before it goes into the zip and the MSI, then the MSI, with
  `signtool sign /fd sha256` and an RFC 3161 timestamp (`/tr`).
- **macOS**: `Trellis.app` with the hardened runtime and a timestamp
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
`signtool verify /pa trellis.exe` and `codesign --verify --deep --strict Trellis.app` or
`spctl -a -vv Trellis.app`.

## The update check

Once a day (and when you click Check now), Trellis makes one unauthenticated request to
`https://api.github.com/repos/<owner>/<repo>/releases/latest` for the repository in its
`Cargo.toml`. It sends no token and nothing about you or your repositories; GitHub sees the
request's address and a `Trellis/<version>` user agent. The first automatic check waits 15
seconds after startup; a failed one is retried after an hour and only logged.

When a newer release is out, Trellis offers its installer (for a copy installed by the MSI, the
dmg or the deb) or its archive (for an unpacked copy). A download happens only when you click
Download: it is fetched into your Downloads folder, checked against `SHA256SUMS` (or the file's
`.sha256`), and discarded if the checksum is missing or doesn't match. Trellis never replaces its
own program; Install opens the downloaded installer.

Turn the check off in Preferences > General > Updates ("Check for updates once a day"). The
release notes dialog keeps working without it.

## Checking the installers

The `Installers` workflow (`.github/workflows/installers.yml`) runs on every push and pull
request (except ones that only touch `docs/`, `TODO.md` or `ROADMAP.md`). It builds the release
program, packages it twice with the scripts above (once as `v0.0.1`, once as the current
version), and on a clean runner:

- **Windows** (`scripts/installer-check-windows.ps1`): installs the v0.0.1 MSI silently
  (`msiexec /i … /qn`), upgrades it to the current one, checks that exactly one Trellis at the
  current version is installed, its files, both shortcuts, the `HKEY_CURRENT_USER` keys, runs the
  installed program through each shortcut (`--version`) and through the update-feed check, then
  uninstalls it (`msiexec /x`) and checks that nothing is left.
- **Linux** (`scripts/installer-check-linux.sh`): the same with `dpkg -i` and
  `dpkg --purge`, plus `desktop-file-validate` on the installed entries and the window started
  from `trellis.desktop`'s `Exec` under its own Xvfb, seen drawing through the smoke probe.

MSI logs, every run's output and the probe are uploaded when a job fails. Both scripts refuse
to run outside GitHub Actions unless given `-AllowLocal` / `--allow-local`, and always refuse
when Trellis is already installed.

The update-feed check (`scripts/update-feed-check.ps1`) points `TRELLIS_UPDATE_FEED` at local
folders shaped like a release:

```
feed/
  release.json                                  {"tag_name": "v0.1.1", "assets": [{"name": …}, …]}
  trellis-v0.1.1-x86_64-pc-windows-msvc.msi     the current package, renamed to the feed's tag
  trellis-v0.1.1-x86_64-pc-windows-msvc.zip
  SHA256SUMS                                    "<sha256>  <name>" lines, LF, no BOM
```

and runs `trellis --check-update <folder>` for each case: the same version (exit 3), an empty
folder (3), a newer release (0, the installer or archive by install kind, byte for byte), a wrong
or missing checksum and a file changed after `SHA256SUMS` was written (1, nothing saved), and a
relative `TRELLIS_UPDATE_FEED` (2). It installs nothing and opens no window, so it runs locally
against a debug build, which counts as portable, with stand-in package files of any bytes:

```
cargo build -p trellis
pwsh -NoProfile -File scripts/update-feed-check.ps1 -Trellis target/debug/trellis.exe -Version 0.1.0 `
  -Installer <dir>/trellis-v0.1.0-x86_64-pc-windows-msvc.msi `
  -Archive <dir>/trellis-v0.1.0-x86_64-pc-windows-msvc.zip -Expect archive -Out <work folder>
```
