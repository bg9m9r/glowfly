<!-- glowfly-release-header:start -->
**These builds are not code-signed yet.** None of them is signed with a code-signing certificate,
so Windows and macOS may warn that the publisher is unknown or that the app can't be checked.
To confirm that a file is the one this release published, check it against `SHA256SUMS`. The
checksums show that a download matches what this release published, not who published it.

**Check a download.** Download `SHA256SUMS` into the same folder as the file, then run this in
that folder. Every file you downloaded should show OK.

Linux:

```sh
sha256sum -c SHA256SUMS --ignore-missing
```

macOS:

```sh
shasum -a 256 -c SHA256SUMS --ignore-missing
```

Windows (PowerShell):

```powershell
Get-Content SHA256SUMS | ForEach-Object { $h, $f = $_ -split '\s+\*?', 2; if (Test-Path -LiteralPath $f) { "${f}: $(if ((Get-FileHash -LiteralPath $f).Hash -eq $h) { 'OK' } else { 'FAILED' })" } }
```

**Check where it was built.** Every file listed in `SHA256SUMS`, and `SHA256SUMS` itself, also has
a build provenance attestation, which shows that this repository's Release workflow built it from
the release's source. With the GitHub CLI installed, run this in the folder with the file:

```sh
gh attestation verify <file> --repo bg9m9r/glowfly
```

**Open it anyway.** Because the builds aren't signed, Windows and macOS ask before the first
launch. Check the download against `SHA256SUMS` or its attestation first (above), so you know you
are opening the file this release published, then:

**Windows.** If SmartScreen says "Windows protected your PC", click "More info", then "Run
anyway". Unblock the `.zip` before you unpack it, so the program inside isn't marked as
downloaded: right-click it, choose Properties, tick "Unblock" and click OK, or run
`Unblock-File <file>` in PowerShell.

**macOS.** On macOS 15 and later, right-click > Open no longer gets past the warning. Open
Glowfly once and close the message, then go to System Settings > Privacy & Security, click
"Open Anyway" next to the note about Glowfly and confirm; after that it opens normally. Or run
`xattr -dr com.apple.quarantine /Applications/Glowfly.app` in Terminal: it removes the
"downloaded from the internet" mark that makes macOS check the app, from the app and everything
in it. For the `.tar.gz`, use the folder you unpacked instead of the app's path. On macOS 14
and earlier, Control-click the app and choose Open.

**Linux.** Make the AppImage executable before you run it: `chmod +x <file>.AppImage`, or tick
"Allow executing file as program" in its Properties. The other packages need nothing extra.

Installing through winget, Scoop, Homebrew and Flathub is coming (#557) and will be the smoother
way on every system.

**No warranty.** Glowfly is licensed under the MIT License or the Apache License, Version 2.0,
at your option (`LICENSE-MIT` and `LICENSE-APACHE`, in every package). It is provided "as is",
without warranty of any kind, express or implied. `THIRD-PARTY-NOTICES.md` lists the licences
of the code it includes. When a release has a `.flatpak` file, that bundle also includes git,
which is under the GNU General Public License, version 2: its notice is `GIT-NOTICE.md` in the
bundle, and the git source archive it was built from is attached to the release.

**Privacy.** Glowfly sends no telemetry. It checks GitHub for updates once a day, and looks up
author pictures only if you turn that on. [PRIVACY.md](https://github.com/bg9m9r/glowfly/blob/main/PRIVACY.md)
lists everything it sends and where.
<!-- glowfly-release-header:end -->
