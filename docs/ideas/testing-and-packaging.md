# Ideas: Testing and packaging

Journey tests, packaging, installers and updates. Untriaged; see [README.md](README.md).

### Packaging and updates

- Drop egui's Ubuntu Light and Hack fallbacks (eframe without `default_fonts`, with Noto Emoji and emoji-icon-font added by hand) so the notices carry two fewer font licences
- Check the AppImage notices' library versions (Alpine's musl, zstd, zlib, mimalloc) against the runtime's build log on each runtime bump; the test checks only the release number
- Add a copyright line from the crate's authors to MIT blocks whose crate file has none
- Skip this version: don't offer a release again until a newer one is out
- "Open in Glowfly" in Windows 11's modern context menu (an IExplorerCommand with a sparse package)
- "Open in Glowfly" on drives as well as folders
- Sign SHA256SUMS (minisign or GPG) and check that signature in the app before trusting the checksums
- Azure Trusted Signing as an alternative to a PFX certificate for Windows signing
- A beta channel that offers pre-releases to users who opt in
- A per-machine (all users) MSI
- Replace a macOS Glowfly.app in place from the disk image (hdiutil attach, ditto, detach) instead of opening it
- Install a .deb in place through pkexec and apt instead of opening the software installer
- A CI check of Install and restart: swap a portable copy from a local feed and run --finish-update on the runners
- Allow Install and restart when Windows Smart App Control or AppLocker blocks programs in the temp folder
- Move the signing secrets and RELEASE_PLEASE_TOKEN into the `release` environment (zizmor's auditor persona flags them as secrets outside an environment); the build jobs would then join the environment, adding one approval
- Installer checks for the ARM64 MSI and the aarch64 .deb and .rpm on Arm runners (windows-11-arm, ubuntu-22.04-arm)
- AppStream metainfo (usr/share/metainfo) for the AppImage and the Linux packages
- AppImage desktop integration: offer to add a menu entry and icon for an AppImage copy
- Embedded AppImage update information (zsync) for external AppImage updaters
- Build the Linux packages on an older glibc baseline so the AppImage and packages run on older distributions
- Sign the .rpm (rpm --addsign) and the AppImage, and publish apt and dnf repositories
- Debian version for pre-release tags: map '-' to '~' in --deb-version as the rpm does
- Automate the Homebrew tap and winget-pkgs submissions from the release job with a token
- Rename a swapped AppImage whose file name carries the old version to the new release's name
- An aarch64 Flatpak bundle (a native arm64 builder image, or a QEMU build if it fits the time)
- Publish Glowfly on Flathub (cargo-sources.json from the lock file, screenshots, submission) so flatpak update and software centres bring new releases
- Host a Flatpak repository (or use build-bundle --repo-url) so bundle installs update with flatpak update
- Run editors, diff tools, the terminal and custom commands on the host through flatpak-spawn --host, as an opt-in
- git-credential-libsecret and git-lfs in the Flatpak's bundled git
- Bring settings, recent repositories and tabs over from a host install (~/.config/glowfly) on a Flatpak copy's first start
- Turn a folder handed over as a /run/user/<uid>/doc path back into its host path through the Documents portal
- Screenshots and a fuller description in the AppStream metainfo
- Post desktop notifications through the notification portal instead of talking to org.freedesktop.Notifications
- Confirm gpg is in the Flatpak runtime for commit signing, and bundle it when it isn't
- A narrower Flatpak file-system choice (home only) for users who want a tighter sandbox
- Show GIT-NOTICE.md in Help > About when Glowfly runs inside the Flatpak, beside the third-party notices
- Link the privacy notice from the release notes header once PRIVACY.md exists
- Check the arm64 MSI's licence files in the Release workflow (an administrative install, msiexec /a), not only the installed x64 MSI in Installers

### Installer and update checks in CI

- Start the installed MSI copy's window with the smoke probe on the Windows runner once it has OpenGL 2 (Mesa llvmpipe)
- Run the installer checks from the Release workflow (workflow_call) against the files it uploads, before SHA256SUMS is published
- A macOS DMG check: mount the image, copy Glowfly.app to a temporary Applications folder, start it, and check that the update check picks the .dmg
- Attach to the parent console on Windows so `glowfly --version` and `--check-update` print in a terminal without redirection
- Show in Preferences > Updates when a local update feed (GLOWFLY_UPDATE_FEED) is in use
- Accept an https mirror URL in GLOWFLY_UPDATE_FEED besides a local folder
- Validate the MSI with `wix msi validate` (ICE checks) in the Installers workflow
- Check the MSI's Options page choices (no desktop shortcut, no folder menu, no scheme) with silent installs that set DESKTOPSHORTCUT, FOLDERMENU and LINKSCHEME to empty, and that an upgrade keeps them off
- Install the .deb through apt (`apt-get install ./file.deb`) to check dependency resolution on a minimal image
- Test the `<file>.sha256` single-checksum fallback end to end in the feed script (it is unit-tested)

### Whole-app journey tests

- Journey through an external merge tool, using a scripted fake tool that works on every OS
- Credential-prompt journey: the askpass server answering a password-protected local remote
- Journeys with the file watcher on, so outside edits show up without Refresh
- Interactive rebase journey, with the built executable as the sequence editor
- Multi-tab journeys: switching tabs while a job runs and checking each result reaches its tab
- Keyboard-driven journeys that press each OS's own shortcut keys
- Hosting-service journeys against a local fake API (pull requests, issues, build status)
- On a journey failure, save a rendered screenshot where wgpu is available, to make CI failures easier to read
- Accessible names for the remaining painted widgets (section headers, graph ref chips, sidebar ahead/behind counts), plus a test that every clickable widget has one

### Faster tests (the shared test kit)

- In-process forms of the next most common fixture commands still started as git (about 1,600 a
  run): `stash push`, `merge`, `reset --hard`, `mv`/`rm`, `commit -- <paths>`, `worktree add`
- A scheduled CI job that runs the suites with `GLOWFLY_TEST_GIT_CLI=1`, so a drift between the
  kit and the git CLI shows up as a difference between the two runs
- Build the app's `scratch_repo` on core's (one empty-repository builder for both crates), once
  the app's tests no longer depend on the machine's `core.autocrlf`
- cargo-nextest measured slower here (one process per test: 369 s against 159 s for the app's
  tests, as each test rebuilds the snapshot fixture and its wgpu device); revisit if tests stop
  sharing state within a process
- Merging the source-check test files into one binary saves only 1 to 2 s of linking a build
  (each links in 0.3 to 0.8 s); revisit if their number grows
- rust-lld on Windows measured no faster than the default linker for the test builds (11 s
  either way after an app change, 14 to 15 s after a core change); revisit if link times grow

### Snapshot strategy (text snapshots and pixel goldens)

- Move the pixel goldens out of git history (git LFS or a separate image store) and prune the old full-window images from the pack; history was left as it is
- Crop the text snapshots of menus, tooltips and popovers to the popup, as dialogs already are, so a toolbar label change doesn't touch them
- Find text snapshots no test writes any more (a `.txt` with no `.new.txt` after a full run) and delete them
- Give icon-only widgets and painted rows accessible names, so more of a view shows in its accessibility tree than in the painted-text list
- Run the text snapshots on the macOS and Linux CI runners too, with per-platform files where labels differ (shortcut names, title bar)

### Fuzzing the parsers of untrusted text

- Fuzz the other parsers the issue lists: git config and include chains (`profile_includes`, `external_tools/git_config.rs`), forge API JSON, `.gitattributes`, `.gitignore` and `.mailmap`, commit messages and trailers
- Fuzz the trust plumbing end to end: a todo resolved against a real fixture repository, and mailboxes through `git mailsplit` and `git mailinfo` (needs git in the fuzz job and a scratch repository per run)
- Minimize the cached corpus now and then (`cargo fuzz cmin`) so the scheduled run doesn't spend its minutes reloading near-duplicates
- Turn the fuzz checks' seeds into a dictionary per target (`-dict=`) for the marker and URL tokens the stable mutation test already splices in
