# glowfly-core: git setup, the OS, settings and app services (`crates/glowfly-core/src/`)

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here.

## Which git, paths and errors

- `git_version.rs`: which git runs (found on the PATH children get) and the features its version lacks.
- `env_path.rs`: that PATH, which on macOS merges in the login shell's.
- `flatpak.rs`: running inside a Flatpak sandbox: detecting it (`/.flatpak-info`, or a valid `FLATPAK_ID` with the program under `/app`), the host's XDG folders (`host_var`, `child_config_home`) for what Glowfly shares with the host (git's XDG config, the home Trash), and pointing libgit2's XDG config search path at the host's at start (`init`).
- `git_program.rs`: the git program every git child runs (chosen in Preferences > Git, else `git` from PATH), checking a program before it is used, and finding the installed gits.
- `safe_directory.rs`: repositories git refuses because another user owns them: opening one git opens, the typed error for a real refusal, and listing a folder in `safe.directory` in git's spelling.
- `safe_directory/verdict.rs`: asking git whether it opens a folder libgit2 refused for its owner, and whom git names as the owner.
- `safe_directory/allowed.rs`: the folders git opens, allowed for libgit2 for this process only (a `safe.directory` list in a global config of its own that includes the user's `~/.gitconfig`).
- `native_path.rs`: the spelling of paths that go to the OS or the screen (never libgit2's `C:/x/y/`), and a folder's name and parent for list rows.
- `volume.rs`: whether a folder is on a network or virtual file system.
- `conflict_lines.rs`: the paths git's CONFLICT lines name (rename, delete and directory conflicts included), each once, for `git_errors`; a path that reads more than one way is counted but not named until `ordered` matches it to the status's unmerged paths (pure).
- `crlf_warnings.rs`: git's "LF will be replaced by CRLF" and "CRLF will be replaced by LF" warnings folded into one line for job logs (pure).
- `git_errors.rs`: why a failed git command failed, from its untranslated output (cause plus listed files and names, and the lock file's whole path), for the failure toast; `-c apply.whitespace=nowarn` on merge, pull and stash so git's internal patch warnings stay out of job logs (pure).
- `remote_failure.rs`: why git couldn't reach or read a remote (unreachable host, missing repository, no access, a server certificate it couldn't verify).
- `error.rs`: typed errors; `error/advice.rs` rewrites a hosting service's refusal as one translated sentence that says what to do.
- `error/read_failed.rs`: `ReadFailed`, why a graph or diff read failed: libgit2's or the system's words kept, Glowfly's own causes as variants; `Display` is the English form, and the model words it (`typed::read`).
- `quoted.rs`: decoding git's C-quoted paths (`"caf\303\251.txt"`), for blame's porcelain and sparse checkout lists.
- `paths.rs`: one folder, many spellings: a Windows short 8.3 name (`C:\Users\RUNNER~1`) and its long form, a link or junction and its target, macOS's `/var` and `/private/var`, `Repo` and `repo` on a volume that ignores case.
- `paths/encode.rs`: paths saved in Glowfly's own files (recent.txt, tabs.txt, settings keys), without losing any (a Unix name needn't be UTF-8, a Windows one may hold a lone UTF-16 surrogate) and one per line.
- `paths/file_url.rs`: the local path a `file:` URL names.
- `win_registry.rs`: reading single values from the Windows registry: a DWORD or a string, None when the key or value is missing (or has another type).

## Credentials and signing

- `askpass/`: credential prompts: classifying git's and ssh's questions (`host_key.rs` reads ssh's unknown-host question into fields), the helper protocol, the app's listener and the helper's client.
- `askpass/ssh_sign.rs`: ssh-keygen's signing passphrase question as a structured `SshSignAsk` (see [flow-shell.md](flow-shell.md)).
- `git_credential.rs`: saving and dropping HTTPS sign-ins typed in the prompt with git's credential helper (`git credential approve` / `reject`, setting up the platform's store when git has none, or git's in-memory `cache` on Linux without a keyring helper).
- `auth_failure.rs`: reading git's output for fetches that need a sign-in, rejected credentials and the remote they belong to (pure).
- `ssh_version.rs`: the ssh git runs in a repository (the environment, then `core.sshCommand`, read from config) and its OpenSSH version (a `DISPLAY` placeholder before 8.4; only an environment program is run with `-V`, a command line gets the placeholder unrun), and the new session every git child starts in on Unix.
- `gpg_prompt/`: signing passphrases: Glowfly as git's gpg.program, running the user's gpg with loopback pinentry and asking through the askpass listener; extending an expired key from Preferences the same way (`gpg_prompt/extend.rs`); for SSH-format keys, the env that marks ssh-keygen's prompt as a signing prompt (`gpg_prompt/ssh_env.rs`).
- `signing.rs`: signing keys.
- `askpass/client.rs`: the helper's side: sends one prompt to the app and waits for the answer.
- `askpass/prompt.rs`: what a git or ssh prompt asks for, decided from its text.
- `askpass/protocol.rs`: the line protocol between the helper process and the app's listener: one JSON request line from the helper, one JSON response line back.
- `askpass/server.rs`: the app's side: a loopback listener that turns each helper connection into an `Event::Asked` and waits for the app to `Server::answer` it.
- `gpg_prompt/args.rs`: reading git's arguments for gpg and building the wrapper's own.
- `gpg_prompt/env.rs`: the environment that makes git run Glowfly as its gpg.program (pure, `vars`), and the signing environment gitcli adds to each command (`for_command`).
- `gpg_prompt/flow.rs`: the wrapper's steps: probe, ask, sign with loopback, or fall back to the program's own pinentry.
- `gpg_prompt/program.rs`: which OpenPGP program the user configured, so the wrapper runs that one.
- `gpg_prompt/real.rs`: the real gpg runs and the real app: `Real` runs the user's program, `EnvAsker` asks through the askpass listener gitcli pointed the wrapper at.

## SSH keys (`ssh_keys.rs`, `ssh_keys/`)

- SSH key pairs in `~/.ssh` and the ones added from elsewhere (Use existing).
- `listing.rs`: listing them with `ssh-keygen` fingerprints, reading a picked pair and the key names ssh tries by default.
- `tools.rs`: finding ssh's programs.
- `generate.rs`: generating Ed25519 keys with `ssh-keygen`, written under a temporary name and then moved into place.
- `command.rs`: choosing the key git uses in `core.sshCommand`.
- `agent.rs`: finding the SSH agent, the keys it holds with their public lines (paired by a SHA256 fingerprint Glowfly computes) and adding keys with `ssh-add`.
- `config.rs`: the IdentityFile entries of `~/.ssh/config` and the files it Includes, split as ssh splits them.
- `config_expand.rs`: expanded with `~`, `%` tokens and `${VAR}` and judged as full paths by each OS's rules.
- `config_scan.rs`: read with Includes, wildcards and a 16-level limit.
- `windows_ssh.rs`: which ssh git runs on Windows, pointing it at Windows' OpenSSH through the global `core.sshCommand`.

## One window, links and the URL scheme

- `instance/`: one window for links and folders: the lock that picks it (`claim.rs`: one holder, a later window waits and takes over), its loopback listener and private instance file (address, token, process id in the runtime folder), and a second process handing its `glowfly://` link or folder over (letting the window take the foreground on Windows); the protocol's tests and the claim's (`tests.rs`, `tests_claim.rs`).
- `git_procs.rs`: whether any git process runs, for Delete lock file: None, the programs found, or Unknown and why (an unreadable list or name, `/proc` with `hidepid`, Flatpak, an open WSL session); the credential-cache and fsmonitor daemons are left out when their command line says so; and whether one pid runs (`running`, for the watcher's stale probes); tests in `git_procs_tests.rs`.
- `git_procs/os.rs`: its process lists and git processes' command lines (ToolHelp and `NtQueryInformationProcess`, `/proc`, libproc and `KERN_PROCARGS2`).
- `lock_file.rs`: Delete lock file's checks and delete: only the path git named, a plain file inside the git dir or common dir, never through a link, and only while no git runs; tests in `lock_file_tests.rs`.
- `link.rs`: `glowfly://` links: the format (building, parsing and every refusal), remote URL identity (`url_key`, `same_url`), the root commit.
- `link/find.rs`: checking a folder for the linked repository and its target (libgit2 reads only).
- `url_scheme/`: registering the `glowfly` scheme for this user, and the pure decisions both platforms read (the Linux package's own `glowfly-links.desktop` in `XDG_DATA_DIRS` counts too).
- `url_scheme/windows.rs`: the registry class key on Windows.
- `url_scheme/xdg.rs`: a desktop entry and `xdg-mime` on XDG desktops.
- macOS needs no registration: Glowfly.app's Info.plist declares the scheme, and links arrive as Get URL Apple events (`glowfly-app` `platform/macos_events.rs`).
- `instance/client.rs`: the second process's side: hands a link or folder to the running window.
- `instance/foreground.rs`: letting the running window come to the front.
- `instance/server.rs`: the running window's side: a loopback listener that reports each link or folder it is handed, started once this window holds the claim (`claim.rs`).
- `app_identity.rs`: Glowfly's identity with the operating system: the id its installers register and the Linux desktop entry's name.

## Built-in terminal (`terminal/`)

- The screen model and its history: scrolling, line numbers that selections follow, reading the text between two points, clearing (`history.rs`).
- `words.rs`: the word or the wrapped line around a cell, for the terminal's double- and triple-click.
- `edge.rs`: the cursor at the right edge: BS, CUB, EL, ECH and the other controls that start from the cursor act on the last cell right after a write into the last column, as in xterm.
- `replay.rs`: redrawing the screen and its history on a fresh parser (Clear history, recovery from a parser panic), keeping which rows wrapped.
- Key encoding and PTY sessions.
- `shell.rs`: which shell to start from Preferences > Terminal.
- `activity.rs`: whether the shell is running a program, in front or as a background job.
- `procs.rs`: the process lists `activity.rs` and `git_procs.rs` read: a Windows ToolHelp snapshot, image paths and creation times, Linux `/proc`, macOS libproc; and every process's program file (`program_paths`), for the update's `.bak` cleanup.
- `title.rs`: the window title a program sets, with what of it names a running command.
- `terminal/keys.rs`: what typing, named keys and pasting send to the program, as xterm encodes them.
- `terminal/pty.rs`: shell sessions on a pseudo-terminal: a running session with its output and exit reported through callbacks on their own threads (terminal/shell.rs picks the shell).
- `terminal/queries.rs`: queries a program sends the terminal and expects an answer to, found in its output.
- `terminal/screen.rs`: the terminal screen: a VT parser fed with a shell's output, read back as styled rows.

## Settings (`settings.rs`, `settings/`)

- `defaults.rs`: `Settings`' defaults, for a new file and for fields an older file lacks.
- `file.rs`: reading and writing `settings.json`. A read that fails is tried again after short waits (`READ_WAITS`), and one that worked only after a failure keeps a copy as `.bak` first; a damaged file is moved aside to `.bak`; a file that can't be read, copied or moved stays put and its `LoadProblem::locked` says nothing may be saved until `Settings::reload` reads it. Tests in `file_tests.rs`.
- Preferences, theme choices and theme files: `themes.rs` parses them, aliases and loops included; `themes_io.rs` reads the themes folder and writes new files.
- `theme_pair.rs`: the pair Match the system switches between (`SystemPair`, saved as ids and loaded leniently), each theme's twin, and what a theme card or the check does to the theme and the pair (`theme_pair_tests.rs`).
- `terminal_schemes.rs`: terminal color schemes: the `terminal_*` keys and the stored scheme files.
- `terminal_schemes/wt.rs` and `iterm.rs` read Windows Terminal JSON and iTerm2 `.itermcolors` files to import, and `io.rs` reads and writes the themes folder's `terminal` folder.
- `terminal.rs`: the terminal's Colors choice (`TerminalColours`).
- `graph_columns.rs`: which graph columns show (mapping an older file's combined COMMIT column onto AUTHOR and COMMIT).
- Per-repository graph filters.
- `colour_palette.rs`: the colour preset (standard or colour-blind safe; `branch_palette` in older files) and per-repository pinned branch colours.
- `repo_colours.rs`: repository colours per working folder, saved as stable names (a retired name loads as its replacement).
- `repo_groups.rs`: named groups of favorites with a colour each; every favorite stays in `favorite_repos`, so older versions still list them, and `new_group_colour` picks a new group's hue.
- `forge_filters.rs`: per-repository PR and issue filters.
- Repository trust decisions.
- Named profiles with their identity (seeding, syncing with git, name checks).
- `profile_rules.rs`: profile folder rules: adding and removing them, a folder belonging to one profile at most, and which rule git applies to a git dir, mirrored from git's `includeIf.gitdir` matching with the deepest folder winning.
- Saved connected accounts and app preferences.
- `profiles_prefs.rs`: what switches with a profile, and every setting classified as per-profile or global.
- `a11y.rs`: Preferences > Accessibility's own settings (cursor blink, how long toasts stay, the tooltip delay, the theme High contrast brings back), flat in the file and global.
- `file_lists.rs`: how the changed-file lists show their files (List or Tree).
- `ui_choices.rs`: the notification corner, descriptions, branch visibility, the history table's row density and the diff viewer's layout and mode; `tracker.rs`: where a repository's issues come from.
- `welcome_done`: whether the first-run welcome was seen (true for any file written before it existed).
- `hints`, `hints_seen`, `hints_ignored`: tips in context on or off, and the tips retired or ignored once, by id (strings, so a newer version's ids survive).
- `commit_limit.rs`: the least number of commits a load reads.
- `avatar_lookups`: author pictures looked up online, off unless chosen (it replaced `use_initials`, which a file no longer opts into).
- `updates.rs`: the update check and the release notes seen.
- `downloads.rs`: the Downloads folder updates are saved to.
- `date_fmt.rs`: how dates are shown: the four patterns (Date, Long date, Date and time, Full date and time), blank following the date locale, relative ages and date hover text.
- Settings saved per repository are keyed by `real_workdir` through `settings::repo_keys`.
- `settings/builtin_themes.rs`: the built-in themes: their ids (a theme file's `base`), names, kinds, the themes that took the place of the retired ones, and how the settings' `ThemeChoice` maps onto them.
- `settings/column_widths.rs`: widths of the graph's resizable columns (dragged from the header dividers), with the per-column defaults and limits.
- `settings/editor.rs`: diff text size and tab stops (Preferences > Diff viewer).
- `settings/graph_filter.rs`: refs hidden from or soloed in a repository's commit graph, saved per working directory.
- `settings/language.rs`: the UI language setting (Preferences > General): "system" or a language code.
- `settings/panel_sizes.rs`: the sizes the window's drawers were last dragged to.
- `settings/paths.rs`: where Glowfly keeps its own files (settings, recent repositories, tabs; the avatar cache is in `cache_dir`, the logs in `log_dir`).
- `settings/profiles.rs`: named profiles (Preferences > Profile): each keeps an author identity, and the current one's identity is the global git config's; other profiles also keep the connected accounts and app preferences saved when they were last current (`profiles_prefs`), and its folder rules (`profile_rules`).
- `settings/repo_keys.rs`: settings saved per repository (graph filters, pinned branch colours, pull request and issue filters, the issue tracker, trust decisions and more), keyed by the repository's resolved working directory (`Snapshot::real_workdir`) and saved losslessly (`paths::encode`).
- `settings/submodules.rs`: keeping submodules up to date: the Preferences > General switch and the three-way choice a repository's SUBMODULES menu makes over it (saved per repository in `Settings::repo_submodule_updates`, read through `repo_keys`).
- `settings/theme_choice.rs`: the theme chosen in Preferences > Appearance > Theme, as settings.json saves it, and how a retired theme's name loads: as the theme that took its place (`BuiltinTheme::successor`), noted for the one-time toast (`take_retired`).
- `settings/zoom.rs`: window zoom levels (keys, palette and Preferences > Appearance).

## Identity and profiles

- `identity.rs`: the author identity (user.name, user.email, user.signingkey) and the one path that writes a git config value; reading the global identity; writing an identity to the global config in order.
- `profile_includes.rs`, `profile_includes/`: profile folder rules in the global git config: an `[includeIf "gitdir:<folder>/"]` entry per rule pointing at the profile's include file in Glowfly's `git-profiles` folder (`[user]` only, no profile name).
- `pattern.rs`: the patterns and the include file's quoting.
- `profile_includes/` also holds the plan (entries shallow-first), the sync (files rewritten when changed; with `place`, Glowfly's own sections removed and added again at the end of the file, sections pointing elsewhere or shared with the user's keys left alone), and checking a picked folder (resolved, valid text, case folding probed).
- `repo_identity.rs`: a repository's effective identity and where git read it (`git config --show-origin`): a profile's include file, the repository's own config or elsewhere, plus what the rules expect; the identity variables set in the environment (`env_overrides`); removing the repository's own name and email.

## Files, logs, text and dates

- `persist.rs`: crash-safe writes of Glowfly's own files and of files saved for the user (patches), `write_streamed` for large files; appends to logs; backups beside a file (`move_aside`, `keep_copy`).
- `logs.rs`, `logs/`: Glowfly's log files.
- `activity.rs`: the activity log's file format and trimming.
- `rotate.rs`: the size-rotating writer of the error and performance logs.
- `tail.rs`: reading a log's end.
- `lines.rs`: the text of performance and git command lines.
- `output.rs`: reading a job's output: its lines as a terminal shows them, their kinds and the last meaningful one.
- `bundle.rs`: the bug-report bundle: every log file verbatim under a short header, written in one go.
- `i18n.rs`, `i18n/`, `locales/`: the UI's translations: languages, the bundled catalogs (English base, per-language JSON), lookups with {name} placeholders and English fallback, the OS language read once at startup (gettext's order and C rule on Linux).
- `i18n/toast_length_tests.rs`: every English text under a `toast` key is at most 140 characters (an allowlist that only shrinks).
- `dates.rs`: dates: moment.js-style patterns, localized month and weekday words, relative ages and the history table's short ones.
- `dates/locale_patterns.rs`: each language's usual patterns for the date locale (ISO 8601 otherwise), keyed by a normalised locale tag with the OS locale read once per run (Linux: LC_ALL, LC_TIME, LANG).
- `spell.rs`: spell checking, and the OS's spell-check language order (gettext's on Linux).
- `avatars.rs`: avatar fetching, and when it may happen (`lookups_allowed`: turned on, and a remote on github.com, gitlab.com, bitbucket.org or codeberg.org).
- `text.rs`: safe display of outside text (`printable` for one line, `printable_lines` for a description).
- `changelog.rs`: release notes in release-please's Markdown (the bundled `CHANGELOG.md`, a release's body): releases, sections and styled items.
- `notices.rs`: parses the licenses in THIRD-PARTY-NOTICES.md, section by marked section (fonts, egui's fonts, syntax definitions, encoding tables, libgit2, Rust crates, AppImage runtime); a block covering many crates gets a short name.
- `notices/fresh_tests.rs`: the notices are current: the syntax section matches two-face, and every registry crate in `Cargo.lock` is in the crate section or in `notices/not-shipped.txt` (no network); `UPDATE_NOTICES=1` regenerates both.
- `notices/generate.rs`: tests only: the generator: the crates compiled in for the release targets (`cargo tree -e normal`), their licence files from the registry (`cargo metadata`), standard texts from `notices/spdx/` for crates without one, grouped by identical text; libgit2's COPYING; egui's font licences.
- `notices/spdx.rs`: tests only: SPDX licence expressions, which option Glowfly takes, and which licence a file's text is.
- `advisory_ignores_tests.rs`: `deny.toml`'s advisory ignores and `osv-scanner.toml`'s `[[IgnoredVulns]]` list the same ids with the same reasons, each osv-scanner entry with an `ignoreUntil` date, and SECURITY.md's Known advisories table names the same ids.
- `glowfly_core::app_identity` holds Glowfly's identity with the OS (`APP_ID`, `DESKTOP_ENTRY`).
- `opens.rs`: whether "open in the default program" would run a working-tree file rather than show it.
- `persist/replace.rs`: the platform steps of a crash-safe write: permissions, the rename over the old file (with Windows' read-only attribute and brief locks), flushing the folder, and old temporary files.
- `i18n/catalog.rs`: the bundled catalogs: one flat JSON object of key → text per language, compiled in and parsed on first use.
- `i18n/system.rs`: the OS's UI language: read once at startup from its ordered list of preferred languages, and never again while Glowfly runs.

## External tools (`external_tools.rs`, `external_tools/`)

- External programs (Preferences > External Tools): argument templates and each tool's placeholders (`template.rs`).
- Finding the program on PATH, starting it detached or waiting for it to exit (`run_to_exit`), or watching it to its end to report a late failure (`watch_to_end`).
- `temp.rs`: the private folder for the copies diff and merge tools open: one session folder per opening, removed when Glowfly quits and swept after 24 hours.
- `compare.rs`: writing those copies for a diff or a conflict, from `repo/tool_sides.rs`.
- `git_config.rs`: when Preferences sets no diff or merge tool: git config's diff and merge tools, read through gitcli with git's fallbacks (the `guitool` keys first, as `--gui`, unless `guiDefault` is false), and which values are the repository's own.
- `shell_tool.rs`: running a configured tool command in sh with LOCAL/REMOTE/MERGED/BASE.

## Glowfly's own updates (`app_update.rs`, `app_update/`)

- `version.rs`: semantic versions.
- `release.rs`: the release and its files, with unsafe links and names dropped.
- `assets.rs`: this platform, how this copy was installed (installer, .rpm, AppImage or portable) and which file to fetch.
- `host.rs`: the machine's real processor under emulation.
- `appimage.rs`: whether this copy runs from an AppImage and which file that is.
- `sums.rs`: `SHA256SUMS` files.
- `download.rs`: downloading a file into the Downloads folder, streamed to a temporary file beside it and renamed into place only after its SHA-256 matches, with progress and Cancel; `check_saved` reads a saved file again.
- `download_tests.rs`: streamed downloads from a local feed (progress, Cancel and a mismatch removing the partial file).
- `feed.rs`: where releases are read from: the hosting service or a local folder named by `GLOWFLY_UPDATE_FEED`.
- `check.rs`: the headless check behind `glowfly --check-update`.
- `method.rs`: Install and restart: how a downloaded file is installed.
- `archive.rs`: the program and doc files of a portable copy's archive.
- `swap.rs`: swapping them (or an AppImage file) in place with rollback.
- `backups.rs`: recording each backup a swap makes (`<backup>.record`: size and SHA-256), and removing a recorded `.bak` program that still matches its record once no process runs it (regular files only, never through a link); tests in `backups_tests.rs`.
- `staging.rs`: the private staging folder, put in the first place the helper runs from (the temp folder, else beside the program or in the cache folder, for a noexec temp), and tidying it; tests in `staging_tests.rs`.
- `plan.rs`: the helper's plan.
- `restart.rs`: preparing and handing over; tests in `restart_tests.rs`.
- `launch.rs`: starting processes detached, and trying whether a copied helper runs (`try_run`: `--version` must print this build's version and exit successfully within 5 s); tests in `launch_tests.rs`.
- `finish.rs`: the helper itself, behind `glowfly --finish-update` (a backup's record is deleted only after the backup), and the fixed words it logs; tests in `finish_tests.rs`.
- `report.rs`: the last installer run's failure or cancel, read from `update.log` at the next start and marked shown; tests in `report_tests.rs`.
- `error.rs`: `UpdateError`, why a check, a download or Install and restart failed, as the update results carry it: the step, the paths as shown and the system's or the hosting service's own words; `Display` is the English `--check-update` prints, and the model words it (`state/error_words/typed.rs`).
