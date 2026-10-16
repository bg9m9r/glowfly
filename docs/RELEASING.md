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
| `trellis-vX.Y.Z-x86_64-pc-windows-msvc.msi` | Windows installer (WiX 5, `packaging/windows/trellis.wxs`): a per-user install into `%LOCALAPPDATA%\Programs\Trellis`, no administrator rights; Start menu shortcut; "Open in Trellis" on folders and folder backgrounds; the `trellis://` scheme in `HKEY_CURRENT_USER`; upgrades replace older versions and a downgrade is refused |
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
