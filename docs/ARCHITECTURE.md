# Trellis architecture

Trellis is a native Git client (Rust, egui, libgit2). This document describes its structure and
the rules that keep it maintainable. Each module's `//!` comment says what it holds in more
detail. `docs/PLATFORMS.md` holds the rules for OS and file-system differences (paths, files,
processes, the environment, git).

## Goals

- **Small, focused files** (aim for < 400 lines) so a change, by a person or an AI, needs only
  the files for one area.
- **Layers enforced by the compiler and a test**: the UI cannot call git or HTTP directly, and
  core logic cannot depend on egui.
- **Testable without a window**: behavior lives in a pure `update` function; views get
  snapshot tests.
- **One way to do background work**: an `Effect`, run by the runner.

## Layout

Two crates. `trellis-core` is a library with no egui and no app state. `trellis-app` is the
binary (package `trellis`).

### trellis-core (`crates/trellis-core/src/`)

| Module | Holds |
| --- | --- |
| `repo/` | The repository model on libgit2: loading history, refs and status (re-reading only some work-tree paths: `status_paths.rs`, merged by `status_merge.rs`; what speeds git's status up and the slow-read note's commands: `status_speed.rs`); diffs (a commit against its parent, another commit, or the working tree: `compare_workdir`; the `linguist-language` override read from the commit's own `.gitattributes` for commit and comparison diffs: `attrs.rs`; the unchanged lines between a diff's hunks, read for the hunk view: `diff_gaps.rs`; the lines added and removed per file of a commit or of the working tree's staged and unstaged files, libgit2 only and capped at 2,000 files: `line_counts.rs`); file and folder history (following a file's renames backwards and forwards: `history.rs`, `history_forward.rs`; a folder's moves backwards: `folder_history.rs`) and a file's text at a commit for the File view (the version before a deleting commit; a Git LFS file's content through `git cat-file --filters` when git may run unasked: `file_at.rs`); staging, line staging, revert (a hunk, lines or a whole file of a commit diff, merged three ways with later edits: `revert_file.rs`, `revert_merge.rs`; a hunk or lines shown with whitespace hidden staged, discarded or reverted as shown, with the unchanged lines' exact bytes from the unfiltered diff: `shown_ws.rs`), restore; conflicts and their resolution (keeping git's resolve-undo record; the resolved files, the stage ids of one record, Mark resolved and Mark conflicted: `resolve_undo.rs`; conflict markers left in working files: `markers.rs`; the picks a file saved with markers holds: `conflict_saved.rs`; a merge's MERGE_MSG without its comment lines, read with libgit2 from the worktree's own git dir to prefill the commit draft: `merge_msg.rs`); blame through `git blame --porcelain` (each run's origin: the commit's path and the parent to step back to; whitespace, move and skipped-commit options); working-tree files; submodules (their state, including a missing folder and one added but not committed, the .gitmodules branch and the URL git clones from, a relative one resolved; and whether a new one can go in a folder: the index, the disk, .gitignore, a leftover stored module), worktrees (other worktrees' changes, counted within the load's limits or in the background: `worktree_counts.rs`), remotes, shallow-clone boundaries. Rows whose stored content is a Git LFS pointer get `FileEntry::lfs`, and a viewer diff with a pointer side carries its `LfsChange` (`lfs_marks.rs`). The commits one branch has and another lacks, walked in the loaded graph and marked incomplete when it runs off its end (`divergence.rs`), and when the repository last fetched, from FETCH_HEAD's time (`fetch_time.rs`). A binary diff's two sides for its preview (`binary_sides.rs`): each side's size (a blob's from the object header, a working file's from its metadata) and, when small enough and not an LFS pointer, its decoded image. |
| `gitcli.rs` | The git CLI, for commands that honour hooks, credentials and signing; reports every command's timing to the observer the app sets. |
| `git_version.rs`, `env_path.rs` | Which git runs (found on the PATH children get) and the features its version lacks; that PATH, which on macOS merges in the login shell's. |
| `git_program.rs` | The git program every git child runs (chosen in Preferences > Git, else `git` from PATH), checking a program before it is used, and finding the installed gits. |
| `safe_directory.rs` | Repositories git refuses because another user owns them: the typed error, and listing a folder in `safe.directory` in git's spelling. |
| `native_path.rs`, `volume.rs` | The spelling of paths that go to the OS or the screen (never libgit2's `C:/x/y/`); whether a folder is on a network or virtual file system. |
| `askpass/` | Credential prompts: classifying git's and ssh's questions (`host_key.rs` reads ssh's unknown-host question into fields), the helper protocol, the app's listener and the helper's client. |
| `git_errors.rs` | Why a failed git command failed, from its untranslated output (cause plus listed files and names), for the failure toast; `-c apply.whitespace=nowarn` on merge, pull and stash so git's internal patch warnings stay out of job logs (pure). |
| `git_credential.rs`, `auth_failure.rs`, `ssh_version.rs` | Saving and dropping HTTPS sign-ins typed in the prompt with git's credential helper (`git credential approve` / `reject`, setting up the platform's store when git has none); reading git's output for fetches that need a sign-in, rejected credentials and the remote they belong to (pure); OpenSSH's version, and the legacy askpass set-up (a `DISPLAY` placeholder and a new session per git child) before 8.4 on Unix. |
| `instance/` | One window for links: the running app's loopback listener and its private instance file (address, token, process id in the runtime folder), and a second process handing its `trellis://` link over (letting the window take the foreground on Windows). |
| `link.rs`, `link/find.rs` | `trellis://` links: the format (building, parsing and every refusal), remote URL identity (`url_key`, `same_url`), the root commit, and checking a folder for the linked repository and its target (libgit2 reads only). |
| `url_scheme/` | Registering the `trellis` scheme for this user: the registry class key on Windows (`windows.rs`), a desktop entry and `xdg-mime` on XDG desktops (`xdg.rs`), and the pure decisions both read (the Linux package's own `trellis-links.desktop` in `XDG_DATA_DIRS` counts too); macOS waits for Apple event handling in Trellis.app. |
| `gpg_prompt/` | Signing passphrases: Trellis as git's gpg.program, running the user's gpg with loopback pinentry and asking through the askpass listener. |
| `branch_ops.rs`, `tag_ops.rs`, `remote_ops.rs`, `clone_ops.rs`, `shallow_ops.rs`, `worktree_ops.rs`, `stash_ops.rs`, `sparse_ops.rs`, `patch_ops.rs`, `submodule_ops.rs` | Command lines and input checks for branches, tags, remotes, cloning (with the sparse clone job: the clone, then the folders to check out), fetching more of a shallow clone's history, worktrees (moving one: `worktree_ops/moving.rs`), submodules (the Add dialog's fields; removing one: deinit, `git rm`, then its stored module, with what would be lost; `set-url` / `set-branch` for editing one; batch `submodule update` command lines; resolving relative URLs; which commands move the work tree, after which submodules may be updated) and stashes (renaming a stash list entry by dropping and storing the top of `refs/stash` again, and stash push command lines: all, staged only, chosen paths; auto-stashes in `stash_ops/auto.rs`: which single commands the guard covers, their fixed English stash messages, and putting a stash back with `git stash pop` once nothing is in progress, telling restored, conflicted, failed and gone apart); sparse checkout state, paths and command lines; patch files (`patch_ops/`): format-patch export of commits or one file of a commit, uncommitted changes through git diff (all, staged or unstaged, chosen paths, untracked files), clipboard text, reading what a patch file holds without git, git apply command lines for each option (index, three-way, `.rej` files, reverse) and their errors, and git am series with where a stopped one is. |
| `pick_ops.rs` | Command lines for cherry-picking and reverting commits from the graph (several at once, a merge against a chosen mainline parent, skipping a stopped commit), run through gitcli so hooks, config and signing apply. |
| `init_ops.rs`, `init_ops/` | New repository files and the init job: `.gitignore` templates, license texts, what the chosen folder already holds, and init, write (never overwriting) and commit. |
| `preview_image.rs` | Decoding a PNG or JPEG for the binary diff's preview: bounded dimensions and allocation, scaled down to at most 1024 px a side, and which paths count as images. |
| `lfs.rs`, `lfs/` | Git LFS: install and setup state, tracked patterns, LFS command lines, pointer files (`lfs/pointer.rs`: parsed from bytes, read from a blob without inflating large ones or from a working file, sizes for the screen with `size_text`, and a diff's two sides as `LfsChange`). |
| `gitflow.rs`, `gitflow/` | Git Flow with plain git: config, checks, start and finish plans, versions, branch groups by kind. |
| `rebase/` | Interactive rebase: plan (from a commit, onto a branch or onto a commit), todo (commit rows, exec and break steps, reword and combined-message amends), ranges with merges recreated through `--rebase-merges` (`merges.rs`: label, reset and merge steps), run (with `--autostash`, and Continue / Skip), quick edits chosen in the graph (reword, squash, drop), progress (the step and the stopped commit of a rebase in progress, and a failed exec's command), the remaining steps of a rebase in progress and `--edit-todo` (`remaining.rs`). |
| `undo.rs`, `undo/` | Undo and redo of Trellis's own operations: recognising their command lines (`kinds.rs`), the recorded actions and their labels (`action.rs`), the state before and after (`record.rs`), and planning the reversal (`planning.rs`) of branches and tags (`refs.rs`), merges, pulls, cherry-picks, reverts and rebases (`moved.rs`), stashes, pops, applies and drops (`stash.rs`), resets (`reset.rs`), file, hunk and line discards and file restores (`saved.rs`), reverts of part of a commit in the working tree (`reverted.rs`), saved conflict resolutions (`resolved.rs`), and deletes and Unstage and delete (`deleted.rs`); interactive rebases, graph edits and rebases continued from the banner are recorded around their run (`rebased.rs`), and a merge, cherry-pick or revert finished after conflicts (Continue, Skip or a commit) is recorded as one move from the HEAD it started at (`resumed.rs`). Aborting a merge, single cherry-pick or single revert with every conflict resolved and no edits outside its files stashes its changes first (`abort_keeping_commands`), recorded as a stash push so Undo pops them back. |
| `forge/` | Hosting services behind the `Forge` trait (GitHub, GitLab, Bitbucket Cloud and Data Center, Azure DevOps, Jira, Trello), each described once in a `ProviderInfo`, mapped to one common model (`model.rs`: pull requests with their reviewers and CI checks, issues); CI checks on pull requests and commits (`checks.rs`, `*_checks.rs`), per-PR checks and votes read after the list (`details.rs`), issue key detection (`refs.rs`), repository web pages (`web.rs`), forks (`forks/`), the signed-in user's own repositories (`user_repos/`), SSH public keys on the user's account (`ssh_keys.rs`), pull request conversations, review threads, reviews and merging (`review/`), issue details and creating issues (`issue_details.rs`, `github_issues.rs`, `gitlab_issues.rs`, `jira_issues.rs`, `jira_adf.rs` for Jira's rich text, `bitbucket_issues.rs`, `azure_issues.rs` with `azure_work_items.rs` for Azure's work item lookups and queries and `html_text.rs` for Azure's HTML as plain text, `trello_issues.rs`), Trellis's own newest release and its public files (`releases.rs`, unauthenticated, https only), PR and issue filters (`filters.rs`, `filters/`): conditions, matching loaded pull requests, issue searches per service, and the dashboard's reading of local repositories' remotes with libgit2 and its bounded calls to several services at once (`dashboard.rs`). |
| `terminal/` | The built-in terminal: screen model and its history (scrolling, line numbers that selections follow, reading the text between two points, clearing; `history.rs`), key encoding, PTY sessions, which shell to start from Preferences > Terminal (`shell.rs`), whether the shell is running a program (`activity.rs`), and the window title a program sets with what of it names a running command (`title.rs`). |
| `graph.rs`, `graph/` | Lane layout for the commit graph, with WIP rows for this worktree and for other worktrees with uncommitted changes; which branch owns each node and lane (`owners.rs`, naming a merge's other parents from its message: `merge_names.rs`). `Snapshot::lay_out_graph` (`repo/branch_colours.rs`) lays the graph out with its owners and colours, at load and when the work tree turns clean or dirty. |
| `branch_colour.rs` | Branch colours: a branch's key (its name, without the remote), its hue slot from an FNV-1a hash of the key, `resolve`, which gives main the accent, holds pinned branches and keeps side-by-side branches on different hues, and a person's slot from their email (`person_slot`). `Snapshot::set_colouring` recolours a loaded graph after a pin or preset change. |
| `diff_rows.rs`, `whole_file.rs`, `whole_file/`, `conflict_rows.rs`, `conflict_folds.rs`, `intraline.rs`, `line_pairs.rs`, `tabs.rs`, `wrap.rs` | Row layout of diffs, the whole-file view and the conflict view (with the conflict chunks safe to pick without asking: `trivial_pick`; the long clean runs folded into one row: `conflict_folds.rs`), and the row of a file line (`diff_rows/line_row.rs`); the hunk view's gaps between hunks, keyed like the whole-file folds of the same lines, and their filled-in rows (`diff_rows/gaps.rs`); word-level changes; which removed line sits beside which added line; tab stops; where wrapped lines break. |
| `conflict_pick.rs`, `conflict_edit.rs`, `conflict_restore.rs` | What each conflict chunk resolves to (whole sides, single lines in tick order, or text typed by hand) and the file that makes, with conflict markers left for the chunks not picked yet and the clean runs between them as edited (`render`, `render_picks`); the conflict view's output as editable text with each chunk's and clean run's range, and which one an edit of it falls in (`locate_edit`); the picks a saved file holds, read back against the stages (`conflict_restore.rs`). |
| `highlight.rs`, `highlight/` | Syntax highlighting of diff lines (syntect), as token kinds; parse checkpoints a reload of the same file resumes from (`memo.rs`: content-defined chunks with their start and end states); whole files too (`highlight_file`, for blame). |
| `repo_scan.rs` | Finding the Git repositories under project folders (the Find repositories dialog): a bounded breadth-first walk that reads only folder listings and `.git` entries, never follows links or junctions, and doesn't descend into a repository it found. |
| `repo_sync.rs` | Fetching or pulling (fast-forward only) one listed repository that may not be open, never prompting: skipped when it has no remotes or upstream, or when its config or hooks run programs and it isn't trusted; tells "updated" from "up to date" by comparing its references before and after. |
| `watch.rs` | Which file changes in a repository matter, and the work-tree paths a change names (`Change::Paths`, at most `MAX_WATCH_PATHS` per burst). A submodule's HEAD or refs moving (in `<git dir>/modules/…` or a nested `.git` folder) reads the parent's whole status; their index, logs and objects don't count. `config` and `config.worktree` reload in full, as do a linked worktree's shared refs, packed-refs, HEAD and config in the common git dir. |
| `trust.rs` | Repository trust: config keys and hooks, the repository's own and its checked-out submodules', that make git run programs (read with libgit2 only, at every full load). |
| `fuzzy.rs`, `matching.rs` | Matching for the command palette, filters and search. |
| `commit_query.rs` | Commit search queries: author and path terms, matching rows, the author list. |
| `path_query.rs` | Path terms of the commit search: folder, file name or glob, matched ignoring case and Unicode form. |
| `settings.rs`, `settings/` | Preferences, theme choices and theme files (`themes.rs` parses them, aliases and loops included; `themes_io.rs` reads the themes folder and writes new files), which graph columns show (`graph_columns.rs`, which maps an old file's separate author and SHA toggles onto the COMMIT column) and their widths, per-repository graph filters, the branch colour preset and per-repository pinned branch colours (`branch_colours.rs`), per-repository PR and issue filters (`forge_filters.rs`), repository trust decisions and named profiles with their identity (seeding, syncing with git, name checks), saved connected accounts and app preferences (`profiles_prefs.rs`: what switches with a profile, and every setting classified as per-profile or global), whether the first-run welcome was seen (`welcome_done`, true for any file written before it existed), the update check and the release notes seen (`updates.rs`), the Downloads folder updates are saved to (`downloads.rs`), and how dates are shown (`date_fmt.rs`: the four patterns (Date, Long date, Date and time, Full date and time), blank following the date locale, relative ages and date hover text). |
| `identity.rs` | The author identity (user.name, user.email, user.signingkey) and the one path that writes a git config value; reading the global identity; writing an identity to the global config in order. |
| `persist.rs` | Crash-safe writes of Trellis's own files and of files saved for the user (patches); appends to logs. |
| `logs.rs`, `logs/` | Trellis's log files: the activity log's file format and trimming (`activity.rs`), the size-rotating writer of the error and performance logs (`rotate.rs`), reading a log's end (`tail.rs`), the text of performance and git command lines (`lines.rs`), and reading a job's output: its lines as a terminal shows them, their kinds and the last meaningful one (`output.rs`). |
| `i18n.rs`, `i18n/`, `locales/` | The UI's translations: languages, the bundled catalogs (English base, per-language JSON), lookups with {name} placeholders and English fallback, the OS language read once at startup. |
| `quoted.rs` | Decoding git's C-quoted paths (`"caf\303\251.txt"`), for blame's porcelain and sparse checkout lists. |
| `dates.rs`, `dates/locale_patterns.rs` | Dates: moment.js-style patterns, localized month and weekday words, relative ages, and each language's usual patterns for the date locale (ISO 8601 otherwise), keyed by a normalised locale tag with the OS locale read once per run. |
| `spell.rs`, `signing.rs`, `avatars.rs`, `text.rs`, `error.rs` | Spell checking, signing keys, avatar fetching, safe display of outside text, typed errors. |
| `ssh_keys.rs`, `ssh_keys/` | SSH key pairs in `~/.ssh` and the ones added from elsewhere (Use existing): listing them with `ssh-keygen` fingerprints, reading a picked pair and the key names ssh tries by default (`listing.rs`), finding ssh's programs (`tools.rs`), generating Ed25519 keys with `ssh-keygen`, written under a temporary name and then moved into place (`generate.rs`), choosing the key git uses in `core.sshCommand` (`command.rs`), and finding the SSH agent and adding keys with `ssh-add` (`agent.rs`). |
| `external_tools.rs`, `external_tools/` | External programs (Preferences > External Tools): argument templates and each tool's placeholders (`template.rs`), finding the program on PATH, starting it detached or waiting for it to exit (`run_to_exit`), the private folder for the copies diff and merge tools open (`temp.rs`: one session folder per opening, swept after 24 hours), and writing those copies for a diff or a conflict (`compare.rs`, from `repo/tool_sides.rs`). |
| `app_update.rs`, `app_update/` | Trellis's own updates: semantic versions (`version.rs`), the release and its files with unsafe links and names dropped (`release.rs`), this platform, how this copy was installed and which file to fetch (`assets.rs`), `SHA256SUMS` files (`sums.rs`), and downloading a file into the Downloads folder only after its SHA-256 matches (`download.rs`). |
| `changelog.rs` | Release notes in release-please's Markdown (the bundled `CHANGELOG.md`, a release's body): releases, sections and styled items. |
| `conflict_check.rs` | Predicting merge conflicts with `git merge-tree` (nothing in the working tree, index or refs changes), and choosing the branch to compare the checked-out branch with. |
| `signature_check.rs` | Checking commit signatures through git (`%G?`), so the user's signing programs, keyrings and allowed signers file apply. |

### trellis-app (`crates/trellis-app/src/`)

| Module | Holds |
| --- | --- |
| `main.rs` | Starts the window, or hands a `trellis://` argument to the window already running; also runs as git's sequence editor (`--write-todo`), gpg.program wrapper (`TRELLIS_GPG_WRAPPER`) and askpass helper. |
| `logging.rs` | The tracing setup, after the helper modes exit: warnings, errors and panics to `errors.log` (rotated; debug details with extended logging, switched at runtime), `RUST_LOG` to stderr, and the panic hook. |
| `msg/` | `Msg`: user intents and background results, one variant per area carrying that area's enum (`RepoMsg`, `DiffMsg`, `StageMsg`, `InitMsg` in `init.rs`, `FilterMsg` in `forge_filters.rs`, `IssueMsg` in `issue.rs` for the issue panel and the Create issue dialog, …, one file per few areas; the first-run welcome and the tour carry `OnboardingMsg` from `onboarding.rs`; a dialog's backdrop click, Esc or close button and the discard question carry `DialogMsg` from `dialog.rs`), plus a few cross-cutting messages (`Tick`, `Scoped`, `Confirm`, `ToastAction`, `Logged`, …). |
| `effect/` | `Effect`: everything slow or outside the process (`mod.rs`), the data some effects carry (`repo.rs` with `LoadKind`, `forge.rs`, `forge_filters.rs` for PR and issue filter calls, `issue.rs` for the issue panel's and the Create issue dialog's tracker calls, `blame.rs`, `notify.rs` for desktop notifications), grouped effects of the Clone and Init dialogs, the SUBMODULES section, interactive rebase, connected accounts and the stash list (`clone.rs`, `init.rs`, `submodule.rs`, `rebase.rs`, `account.rs`, `stash.rs`), the effects the UI shell carries out (`ui.rs`), `Effect::name` (`names.rs`) and `Effect::scope` (`scope.rs`). |
| `build.rs` | Bundles `CHANGELOG.md` into the program (`OUT_DIR`) and, for Windows, embeds the icon and version details in `trellis.exe`. |
| `lfs.rs`, `git_env.rs`, `themes.rs`, `gitflow.rs`, `terminal.rs`, `submodule.rs`, `remote.rs`, `tag.rs`, `worktree.rs`, `ssh.rs`, `profile.rs`, `file_op.rs`, `patch.rs`, `tools.rs`, `conflict_check.rs`, `pull_request.rs`, `logs.rs`, `onboarding.rs`, `deep_link.rs` | Per-area message and effect types of areas with their own data types (one `Msg` variant each). |
| `state/` | `AppState`: plain data, one file per area. Pure rules (why an action is blocked, form checks, labels) live next to their data (the Clone dialog in `clone.rs` and `clone_service.rs`, the Init dialog in `init.rs`, the words of the issue panel and Create dialog per tracker in `issue_words.rs`, the rebase panel's merge rows in `rebase_merges.rs` and rebasing onto a graph commit in `rebase_onto.rs`; whether applying, popping or deleting a stash asks first, for every entry point, in `stash_confirm.rs`; the auto-stash prompt, its texts and the stashes waiting to go back in `auto_stash.rs`; the palette's page and end keys in `palette_nav.rs`; the operation in progress: its banner commands in `op_actions.rs`, the strip's text in `op_strip.rs`, and the danger confirmations of Abort, Skip and Reset bisect, with when an abort keeps the resolved changes, in `op_confirm.rs`; the toolbar's rules: why each button is off, the ahead/behind and last-fetched lines and the Push, Pull, Stash and Pop previews, in `toolbar_tips.rs`, what each left-panel section header's + does (its name, why it is off and the message it sends) in `section_actions.rs`, the rich tooltip's data in `tip.rs`, and what Undo and Redo would change in `undo_preview.rs`). |
| `update/` | `update(state, msg, now) -> Vec<Effect>`: one file per area (the strip's Show work in progress and the banner's confirmed Abort in `op_progress.rs`; pinned branch colours and the preset's hue count brought into the snapshot at load, on Preferences edits and on a tab's return in `branch_colour.rs`), plus headless tests (`*_tests.rs`, declared in `update/test_mods.rs`). |
| `runner/` | Runs effects on worker threads and reports back with `Msg`s (`mod.rs`; `execute.rs` maps each effect to its work), one file per kind of work; `logs.rs` is the one ordered worker for the log files. `sandbox.rs` (tests only) keeps the journeys' runner inside the test: saves go to the test's folder, effects that reach the network, the OS shell, the desktop, file pickers, the credential store or the global git config are dropped by name, and it counts the effects in flight. |
| `journey/` | Tests only: whole-app journeys. The real app (`TrellisApp::for_test`) in an egui_kittest harness on fixture repositories, driven by clicking and typing on widgets found by accessible name (`find.rs`: a region of the window, the open command palette among them, and a pick among equal names), waiting until the app is idle after each step (`harness.rs`), and checking the screen and the repository on disk (`fixture.rs`). In the crate because the package has no library target `tests/` could reach. |
| `glyph_check/` | Tests only: every non-ASCII character in the app's and core's string literals and in the translation files has a glyph in the fonts egui draws with (`scan.rs` finds the characters, skipping comments and test code; `mod.rs` asks the fonts; `NEVER_DRAWN` lists the few that are never shown). It installs the app's bundled fonts through `ui::theme::fonts::install`, as the app does. |
| `watcher.rs` | The file watcher for the active tab's repository, owned by the runner. |
| `platform.rs` | OS integration: recent repositories and which listed folders are missing, open tabs, terminals, the file manager, and the window icon (`platform/icon.rs`). |
| `msg/updates.rs`, `effect/updates.rs`, `runner/updates.rs`, `state/updates.rs`, `state/release_notes.rs`, `update/updates.rs`, `update/release_notes.rs` | Trellis's own updates (`UpdateMsg`, `UpdateEffect`): the check's and download's status, opening the release notes, and the runner's check, download and install. Drawn by `ui/release_notes.rs` (the dialog), `ui/update_notice.rs` (the status bar chip) and `ui/prefs/updates_row.rs` (Preferences > General > Updates). |
| `keymap/` | The binding table (`table.rs`), chords and their text ("Cmd" and "Option" on macOS, through `KeyLabels`) and file forms (`chord.rs`), the effective keymap with the user's overrides (`active.rs`) and the rebinding rules (`rebind.rs`); shared by matching (`ui/shortcuts/global.rs`), the sheet, the palette, tooltips and Preferences > Keyboard. |
| `ui/` | egui views, one file or folder per region (the Clone dialog in `clone_dialog.rs` and `clone_service.rs`, the Init dialog in `init_dialog.rs`, the blame view in `blame/`: its header, gutter and line menu, and its body (`body.rs`: wrapped rows, or sideways scrolling under a fixed gutter, with line widths and wrapped heights measured once per blame in `rows.rs`); find and go to line, shared by the blame header and the file history's File view: `find_bar.rs`; the File view's measured line widths and wrapped heights: `history_text_cache.rs`; the graph row's right-click menu in `graph/ref_menu.rs`: a ref chip's own menu or the commit menu; what a double-click on a branch chip checks out in `graph/ref_checkout.rs` (a remote label split into remote and branch by `state/remote_branch.rs`); a remote branch's menu, shared by the REMOTE rows and the graph's remote labels, in `remote_branch_menu.rs`, with its reasons in `state/remote_branch.rs`), the first-run welcome (`welcome.rs`, `welcome_steps.rs`) and the tour of the main window (`tour.rs`), the themes' names in the language for View > Theme and Preferences' Theme row (`theme_names.rs`), the shared painted controls (`widgets/`: the controls in `mod.rs`, the `Icon` set in `icon.rs`, the tab bars' painted close × and new-tab + in `tab_glyph.rs` (one stroke and hover box for both), the three button styles in `buttons.rs` (primary, secondary and danger, with one disabled look: a neutral fill and weak text, never an outline or egui's fade, and a `why` shown on hover); list rows in `list_row.rs`, whose dim right-hand parts paint ahead/behind arrows; one selection fill with a left accent bar and lifted hover (`widgets/row_fill.rs`), shared by the graph, sidebar rows, file rows and the file history list; single-line text cut with an ellipsis and its whole text on hover (`widgets/truncate.rs`); the left panel's section headers with their hover + (the section's create action) and ⋯ (its menu), and the empty state line under an empty section (`widgets/section_header.rs`), each + described by `state/section_actions.rs`; `widgets/reason.rs` draws disabled reasons and setup suffixes: a weak line beside or under a disabled button, a dialog footer's reason (in the row before Cancel, or above it when the row is full; laid out by `dialog_footer.rs`), and a menu item's short reason or "Set up…" after its label, with the short forms from `state/blocked.rs`), the history table's column layout (`graph/layout.rs`: each column a cell, divider to divider, whose stored width is the whole cell, with its content 12 px in from a divider and 16 px in from the table's edges), the graph row's text cells (`graph/cells.rs`), the selected commit's details, title first (`details_commit.rs`: the title as git splits it, the author's row, the body block, the files), with its row of quiet chips for the SHA, parents and committer (`details_chips.rs`, drawn by `widgets/quiet_chip.rs`), and the changed-file rows' status chip (`file_status.rs`), this tab's WIP row with its label, change chips and line total, spanning the columns it leaves empty and keeping the label whole longest (`graph/wip_row.rs`), line counts drawn one way everywhere (`line_counts.rs`: a file row's `+N −M` and proportional bar, a total, the WIP row's chips; added, removed and modified share `common::added_color`, `removed_color` and `modified_color` with the status letters), its ref chips with their marks, fitting and "+N" chip (`graph/labels.rs`), its nodes (`graph/nodes.rs`), the painted local, remote and tag marks (`graph/marks.rs`) and its background (`graph/row_look.rs`: the lane band and the highlight; the selection and hover come from `widgets/row_fill.rs`), the WIP lists' headers with their more-actions menu (`details_wip_header.rs`), the diff's file header (`diff/header/`, one 48 px row built from `state/diff_header.rs`'s model: the file's icon, folder, name and chips in `identity.rs`; hunk and file steps with the counter in `nav.rs`; the View button and its menu in `view_menu.rs`; the file actions, their More menu, the primary Stage or Unstage and Close in `actions.rs`; the ghost buttons and dividers in `controls.rs`, re-exported from `widgets/ghost.rs`, which the terminal panel's header shares; and which groups fit, in narrowing order, in `fit.rs`), a binary diff's body (`diff/preview.rs`: the image's Before and After over a checkerboard, Added and Deleted placeholders, a card for an image that can't be decoded, or both sizes for any other binary), a segmented control whose segments can be disabled with a reason on hover (`widgets/segmented.rs`), the Diff | File | Blame switch the file history and blame headers share (`file_view_switch.rs`), date labels with their full date on hover (`date_tip.rs`), the strip under the toolbar while an operation is in progress (`op_strip.rs`), one style for every empty, loading, failed and banner state: an empty pane's icon, title, hint and buttons (`empty_state.rs`, its painted icons in `empty_state_icons.rs`; the graph's in `graph_empty.rs`, the file history list's and its footer's in `history_footer.rs`), a pane that is loading or failed with Try again and Copy details (`pane_status.rs`, which shows an error without its kind prefix, `error::plain_message`), the banner in four kinds, Info, Warning, Error and Progress, with a mark and a coloured stripe (`widgets/banner.rs`), the error, warning and information marks (`widgets/severity.rs`), and a form's messages (`widgets/form_message.rs`: under a field once it was edited or a submit was tried, `state/touched.rs`, or for the whole form in a tinted box; `tests/message_styles.rs` checks banners and every dialog's messages), the frame every dialog is drawn in (`dialog_frame.rs`: a header with the title, an optional icon and subtitle and a painted close button that acts as Esc (`dialog_header.rs`); a size class, Small 400–480, Medium 480–560 or Wide 640–760 px, following a long title up to its most and capped by the window (`dialog_size.rs`); a body that scrolls or resizes; for a less familiar operation a tinted side pane that explains it in numbered steps under "How it works", beside the form or, in a narrow window, stacked above it (`dialog_guide.rs`, the steps in `dialog_guides.rs`); and a footer in one order, extras then Cancel or Close then the actions with the primary last; Enter runs the default button and the default gets the focus on the first frame), a dialog's text fields (`widgets/text_field.rs`: 36 px with the text centred, tinted with `color::field_fill`, a Browse… button beside one with `with_button`, and `entered` for a field's Enter) inside a label-field-help group (`widgets/field_group.rs`: the label above, a weak Optional marker, then one line under the field, its message or else its help), the confirmation dialog and its danger form (`confirm_dialog.rs`), the themed form controls (`theme/controls.rs`: text fields with a border and focus ring, and checkboxes and radio buttons drawn as visible boxes and circles; views use these, which `tests/controls.rs` checks), `theme::modal` (every modal: the themed backdrop and dialog border), contrast maths and the graph's ref chip colours (`theme/contrast.rs`), the token families (`theme/families.rs`: status, branch, repository, attention, and the terminal's 16 basic colours, one table per built-in in `theme/ansi.rs`), the branch hues in two presets, standard and colour-blind safe, put into a palette by `applied` (`theme/branch_palette.rs`, their names in the language in `theme/branch_names.rs`) and a branch swatch's line and chip tones with a person's initials colours (`theme/branch_colours.rs`), read by the graph's per-row colours (`graph/lane_colours.rs`) and the branch dots (`branch_dot.rs`), the bundled fonts (`theme/fonts.rs`: Noto Sans, its semibold family and JetBrains Mono ahead of egui's fallbacks, from `assets/fonts/`; the one font setup, installed by `TrellisApp::new`, `theme::apply_style` and the glyph check), the type scale (`theme/type_scale.rs`: five steps, caption 11, small 12, body 14, subhead 16 and title 20 px, with nothing under the 11 px floor; one section-header style, Noto Sans SemiBold at body size, and SemiBold titles; `theme::apply_style` installs the steps as egui's text styles, and `common::with_code` sets prose with an inline monospace SHA), the spacing grid and panel padding (`theme/spacing.rs`), the terminal panel's 40 px header (`terminal/header.rs`: pill tabs in `terminal/pill.rs` named by `state/terminal_names.rs`, + and the shell picker in `terminal/tabs.rs`, and quiet icon buttons painted in `icons_panel.rs`), drawn under the central area or, maximised, in its place (`terminal::show_in_main`; renaming and maximising are handled in `update/terminal_header.rs`), the terminal grid's cell size, the monospace advance rounded to whole device pixels where epaint places each glyph, so text, cursor and mouse share one grid (`terminal/metrics.rs`), the side panels' row heights and insets (`list_row.rs`, with the file lists' fixed-height `file_row` and `file_row_with_end`, whose right-hand part keeps its width while the path is cut; the file rows are one click target each, with the selection painted across the row and into the panel padding, and `row_text` for their unnamed path), the conflict view's header with its more-actions menu (`conflict_header.rs`), the object menus (commit, multi-commit, local and remote branch, tag, remote: `menus.rs`, `branch_menu.rs`, `remote_branch_menu.rs`, `tags.rs`, `remotes/mod.rs`; the branch menus' Color submenu in `branch_colour_menu.rs`), each headed once by its object (`common::menu_header`, `common::commit_menu_header`) with its items in intent groups and its copy items in one Copy submenu (the commit menus' wording in `state/commit_menu.rs`), plus snapshot tests (`snapshot_tests*.rs`; `menu_testing.rs` opens a submenu and finds its entries). `ui/app.rs` is the shell: it owns the runner and dispatches messages (`ui/app_testing.rs` builds it for the journeys, without eframe and with a sandboxed runner); `ui/access.rs` gives painted buttons and hint-only text fields their accessible names (a toolbar button's name describes it and differs from the palette's rows); `central.rs` holds the central-area routing (which view fills it: the pull request panel and the issue panel (`issue/`) share one place) and the Esc chain. The window's panels are drawn in order by `frame_layout.rs`, the side panels' rails by `panel_rails.rs` and the toasts by `toasts.rs`. The toolbar (`toolbar.rs`) draws its buttons in groups, each on a quiet surface (`widgets::tool_group`), with their menus (`toolbar_menus.rs`), the repository and branch pickers sized to their names (`toolbar_pickers.rs`) and each button's tooltip from `toolbar_tips.rs`; the repository tabs are laid out by `tabs.rs` (the system frame's tab bar, or the strip a drawn title bar holds), painted by `tab_paint.rs` and dragged by `tab_drag.rs`; a menu that drops below a custom-drawn button (the title bar's menu button, the pickers) is opened through `menu_anchor.rs`; every rich tooltip shares one frame, timing and layout (`widgets/tooltip.rs`: title, keycaps or a state line, body, an inset preview and a footer) and its fade and rise (`widgets/tooltip_motion.rs`, off with Reduce motion). |

### Tests outside `src/`

| Path | Checks |
| --- | --- |
| `crates/trellis-app/tests/layering.rs` | No I/O in `ui/`, `update/` or `state/` (see Rules). |
| `crates/trellis-app/tests/controls.rs` | Views under `ui/` draw text fields, checkboxes, radio buttons and modals with `theme/controls.rs` and `theme::modal`, not egui's own. Dialogs draw their title and buttons through `dialog_frame` (no headings, egui buttons or English Cancel / Close) and their text fields as `widgets::TextField` (no `controls::field(` or `TextEdit::`); the dialogs for less familiar operations each have a guide side pane, and no other dialog has one; preference pages and panels use `widgets::buttons` with translated Cancel and Close; nothing wraps a styled button in `add_enabled`. |
| `crates/trellis-app/tests/type_scale.rs` | View text stays on the type scale: no pixel sizes in `FontId`, `.size(` or font constants, no egui small text or `RichText::strong()` (headers use `type_scale::section`, emphasis `type_scale::strong`), and no link coloured by hand; `ALLOWED` names the exceptions. |
| `crates/trellis-app/tests/sentence_case.rs` | English labels are in sentence case: `en.json`'s label values (not tips, help or other prose) and the first literal at the views' label calls, with proper nouns and acronyms keeping their capitals. |
| `crates/trellis-app/tests/message_styles.rs` | Only `widgets/banner.rs` fills with the banner colour (the diff's Git LFS note keeps it as a box), and the forms converted to `widgets/form_message.rs` don't colour problem lines red or amber themselves. Shares the source reading with `controls.rs` (`tests/support/views.rs`). |
| `crates/trellis-app/tests/i18n_keys.rs` | Every translation key the code looks up exists in English, and every English key is used. |
| `crates/trellis-app/tests/sequence_editor.rs` | A rebase with the built executable as the sequence editor. |
| `crates/trellis-app/tests/askpass.rs` | The built executable as the askpass helper. |
| `crates/trellis-app/tests/gpg_prompt.rs` | The built executable as gpg.program: the loop guard, and (Unix) `git commit -S` through a fake gpg. |
| `crates/trellis-app/tests/app_icon.rs` | The app icon: draws it (`app_icon/draw.rs`) and checks every PNG in `assets/icon/` and inside `trellis.ico` and `trellis.icns` (`app_icon/containers.rs`); `UPDATE_ICONS=1` rewrites them. |
| `crates/trellis-app/tests/snapshots/` | Approved UI images. |

### Data flow (Elm-style)

```
 view(&AppState) ──Msg──▶ update(&mut AppState, Msg) ──Vec<Effect>──▶ runner
       ▲                                                                   │
       └─────────────── Msg::…Done(result) ◀── worker thread ◀─────────────┘
```

- **Views** draw `AppState` and push `Msg`s. They change only text buffers and `view` state
  (scroll, hover, open sections, a drag's preview until its release sends a `Msg`), never domain
  state such as the log, the selection or a form's checks. They never touch disk, network,
  processes or the clock. They may use core types and pure helpers. Avatars, for example, are
  requested by the views, fetched through `Effect::FetchAvatars`, and turned into textures by the
  shell.
- **`update`** is the only place domain state changes. It never blocks and does no I/O. Anything
  slow or outside the process (git, HTTP, the file system) becomes an `Effect`.
- **The runner** (`runner/mod.rs`) starts one worker thread per effect and sends the result back
  as a `Msg`, waking the UI.
- **Unchanged lines in a diff**: `ViewState::diff_expanded` holds the opened runs by key (a
  run's first new line, the same in the whole-file view and the hunk view) and, for the hunk
  view, the text of the gaps between hunks, read once per shown diff (`Effect::LoadDiffGaps`,
  `update/diff_gaps.rs`). E and Shift+E open the next closed run from the top or fold back the
  one there (`update/fold_keys.rs`).
- **Next/previous change across files**: a step's file is `ViewState::diff_landing` until it
  loads; a refresh while it loads asks for the step's file again (`update/diff.rs::renew_step`)
  instead of reloading the old one. Once it lands, `ViewState::file_reveal` makes the file list
  it came from scroll to it once (`ui/file_reveal.rs`, guarded by the landing's generation).
- **Tabs**: the active tab's repository data lives directly in `AppState`; switching tabs parks
  it in `state::tabs::Parked` and restores the other tab's. Results of repository-scoped effects
  (`Effect::scope`) come back as `Msg::Scoped`. If their tab is no longer active, only what the
  area's `on_inactive` returns is applied (a log line, an undo entry for that tab, a dialog's
  result); the rest is dropped. Each tab's activity log lives outside `Parked`, in `Tab::log`
  (the active one's in `AppState::log`): `switch` and `activate` swap it, so a restored tab that
  hasn't loaded still gets lines.
  Paths are resolved on disk once, when a repository loads (`Snapshot::real_workdir`, with its
  identity `Snapshot::folder`). `update` compares paths with the pure `paths::same_spelling` plus
  the identities worker threads found (`AppState::same_folder`, `state/known_folders.rs`); it
  never folds case, which is the volume's business. Settings saved per repository are keyed by
  `real_workdir` through `settings::repo_keys`.
  Worktree tab groups: each `Tab::group` is its repository's common git dir (`Snapshot::common`,
  or for restored tabs `Effect::ReadTabGroups`, read at startup without loading them); the pure
  rules for blocks, drops and regrouping are in `state/tab_groups.rs`, applied by
  `update/tab_groups.rs` and drawn by `ui/tabs.rs`.
  Worktrees and branches: a local branch another worktree has checked out is found by its exact
  name and `WorktreeInfo::current` (`state/worktree_branches.rs`), never by comparing folder
  spellings; every UI checkout of a local branch goes through `BranchMsg::Checkout`, which
  refuses it. Moving a worktree (`update/worktree_move.rs`) points the tabs, the recent list,
  the closed tabs, favorites and `KnownFolders` at the new folder; removing one closes the other
  tabs showing it first (`update/worktree_manage.rs`).
  Each tab has a `TabId` for as long as it is open: what a request waiting for an answer names
  it by, since its index changes when tabs move or close. Closing a tab that holds work only in
  memory (a commit draft, conflict choices) asks first (`state/tab_losses.rs`), and the
  confirmation carries a `CloseRequest` by id (`update/tabs_close.rs`). A tab's label (Rename
  tab, `update/tab_label.rs`) is saved with the open tabs in tabs.txt and kept on the recently
  closed list.
- **Job follow-ups**: a job can carry what to do once it succeeds (`JobSpec`/`RunningJob`
  `then_push`, `then_open`, `then_pr`, `then_link`), handled in `update/ops.rs::finish_job`. Those that act
  on the repository run only while the job's tab is active and are logged otherwise. A follow-up that needs the refreshed snapshot waits for the
  reload: a push for a pull request sets `ForgeState::open_after_load`, and the next snapshot
  load opens the create dialog (`update/push_pr.rs::loaded`), never replacing another dialog.
- **Paced second passes**: work too slow to wait for rides behind the result that needs it.
  On services that read checks and votes one pull request at a time, the list carries them for
  the first `CHECKED_PRS`; `update/pr_details.rs` then queues the rest and sends one
  `ForgeCall::PrDetails` chunk at a time, each reply filling its pull requests in and sending
  the next. What was read is cached by head commit and age (`state/pr_details.rs`), so a list
  refresh re-reads only what changed or aged, and a failed chunk stops the pass without touching
  the host's status.
- **Dashboard**: global state (`state/dashboard.rs`), never parked; Scan then one Fetch effect,
  credentials resolved outside any repository; opening a PR waits for that tab's list
  (`pending_pr`). Which repositories and calls a refresh plans is in `state/dashboard_plan.rs`,
  the rows the view draws in `state/dashboard_rows.rs`.
- **Find repositories**: global state (`state/repo_scan.rs`, `st.repo_scan`), never parked; the
  project folders are a setting, each scan is one `Effect::Repos` (runner/repos.rs) whose
  results carry each repository's `FolderId`, learned into `KnownFolders` so the "open" tags
  match a tab that shows the same folder under another spelling.
- **Fetch or pull across repositories**: global state (`state/repo_sync.rs`, `st.repo_sync`)
  with a row per favorite or listed repository. `update/repo_sync.rs` starts at most
  `MAX_RUNNING` `ReposEffect::Sync`s and starts the next as each `ReposMsg::Synced` comes back
  (stale generations are ignored); a pull queues behind the repository's other jobs. A saved
  "don't trust" skips a repository without running anything. Closing the dialog leaves the run
  going; its end is then told in a toast that opens it again.
- **Dialogs**: `Modal::kind` (`state/dialog_kind.rs`) sorts each dialog into a form,
  information or confirm dialog. `ui/modals.rs` reports a backdrop click, Esc or the header's close button as
  `DialogMsg::Dismiss`; the close button acts as Esc. `update/dialog.rs` closes information and confirm dialogs, ignores the
  backdrop for forms, and on Esc compares the form's `Modal::draft` with the one `DialogGuard`
  recorded when it opened (after every update). When they differ it asks "Discard your
  changes?" (`ui/dialog_guard.rs`), whose default button is Keep editing.
  Enter runs a dialog's default button: its primary, or Cancel or Close when it has none or
  its tone is danger, so Enter never presses a danger button. Enter in a single-line field
  runs the primary on the first press: the frame sees the field give up the focus, so a field
  that takes the focus back when nothing has it skips the frame it lost it. `Modal::ConfirmDanger`
  (`state/confirm.rs`: `Modal::danger`, `with_ack`, `confirm_ready`) is the confirmation for a
  step that loses work; with an acknowledgement its danger button stays disabled until the box
  is ticked, and `update` refuses to run it before then.
- **Toasts** (`state/toasts.rs`) can carry one action button with a stored `Msg`. A click
  sends `Msg::ToastAction(id)`; `update/notify.rs` removes the toast and runs the message only
  while the toast's repository is the active tab. A failed job's toast says git's cause in
  plain words and can carry a fix (`update/job_errors.rs`, from `git_errors::classify`), with
  Details beside it in `Toast::details` (`Msg::ToastDetails`, which works from any tab).
  `toast_replacing` updates a toast still on screen instead of stacking another (the zoom toast
  keeps its id in `view.zoom_toast`).
- **Incremental status**: the watcher (`watcher.rs`) collects the work-tree paths of a burst
  when nothing else changed (no index, `info/exclude`, `.gitignore`, `.gitattributes`, HEAD or
  ref change, no lost events, at most `watch::MAX_WATCH_PATHS`) and sends them in
  `WatchMsg::RepoChanged { paths }`. `update/refresh_paths.rs` asks for
  `Effect::LoadStatus { paths: Some }`; the runner (`runner/status.rs`) reads only the unstaged
  rows under them (`repo::status_of_paths`, widened to a submodule's or nested repository's root)
  or, when they can't be read on their own, the whole status in the same effect. `update` merges
  the rows into the lists on screen (`repo::merge_unstaged`) and keeps the staged list, which only
  an index change (always a whole read) can change. Trellis's own refreshes and the polls read
  everything; reads asked for while one runs merge (`state::refresh::Wanted`). A whole read slower
  than `repo::SLOW_STATUS` shows a note once per repository (`update/status_tip.rs`) whose
  button turns on git's untracked cache in a job.
- **Other worktrees' changes** (their WIP rows) never hold up a load: `repo::worktree_counts`
  counts them during the load only while their index is small (`LOAD_COUNT_INDEX` entries, read
  from the index header) and the load's budget (`LOAD_COUNT_BUDGET`) lasts, and marks the rest
  `ChangesRead::Pending`. Right after the load `update/worktree_counts.rs` asks for one
  `Effect::CountWorktreeChanges` (scoped; one per tab at a time, `RefreshState::counting`, kept
  across unchanged refreshes). The runner reads each through git with `COUNT_TIMEOUT` when git
  may run there, otherwise through libgit2; `WorktreeMsg::ChangesCounted` fills in the counts for
  the same `refs_key` and lays the graph out again. A read that times out, or one libgit2 would
  have to make of a huge index, is `TooLarge` and shown as not counted until the next full load.
- **Submodules after a job**: `update/submodule_batch.rs` keeps submodules updated after a
  checkout, pull, merge, rebase or reset (Preferences > General, overridden per repository).
  `start_job` and the rebase launches mark the job in `st.submodules.after_job` when
  `submodule_ops::moves_work_tree` says it can change the recorded commits; `finish_job` then
  reads the submodules again (`SubmoduleEffect::CheckAll`, scoped) when that job succeeded in the
  tab that is still active, and the update leaves out any submodule where it would strand
  commits. The mark is matched by label and workdir, so one left by a job the trust prompt held
  never fires for another. `opened` offers (once per session) to initialize a newly opened
  repository's submodules.
- **Auto-stash** (`update/auto_stash.rs`, `state/auto_stash.rs`, `ui/auto_stash_dialog.rs`): one
  guard for checkout, pull, merge, plain rebase, cherry-pick, revert and hard reset. Each entry
  point passes its single command and a retry message to `gate`; on a dirty tree with nothing in
  progress it opens `Modal::AutoStash` holding the retry (Always stash skips it, except for a
  hard reset), and the answer reruns the retry with `st.auto_stash.approved` set for just that
  call. `wrap` puts `git stash push -m <message>` before the command in the same job (same label,
  undo recorded as the original command) and marks it (`making`, dropped with a held job). A
  guarded command the gate lets run unstashed (the snapshot showed nothing to stash) is recorded
  with its retry (`unstashed`, matched by label and workdir when the job ends); if git then says
  changes were in the way, the failure toast's Stash and <op> sends `AutoStashMsg::Retry`, which
  reruns it with the stash approved.
  `finish_job` turns the mark into a pending entry armed for the next load generation; `seen`
  runs after each snapshot of that tab, finds the stash, and once the operation is over (merge
  committed or aborted, rebase continued) and no job runs, puts it back as the quiet job
  `StashEffect::PutBack`; its result becomes a toast with Show stash when it conflicted or
  failed, and the stash then stays in the list (marked auto in STASHES). Interactive rebases and
  graph edits keep their own `--autostash` and skip their stash question with Always stash on.
- **Zoom and panels** are settings, not view state: `update/layout.rs` changes and saves them
  (`LayoutMsg`), and the shell applies `settings.zoom_percent` to egui every frame
  (`ui/layout.rs::apply_zoom`, like the theme), so a profile switch or a Preferences edit takes
  effect on the next frame. egui's built-in zoom keys are turned off at start-up so the keymap
  owns them. The panels are drawn in order by `ui/frame_layout.rs` (status bar, top chrome,
  side panels, central view, tour; the top chrome starts with the drawn title bar on Windows
  and macOS, and with the menu bar and the tab bar under the system frame);
  `show_left_panel`, `show_details_panel` and `show_toolbar` are checked there, and
  `show_menu_bar` by `ui/menu_bar::show` (system frame only). Hiding the menu bar through
  `LayoutMsg` toasts a Show menu bar button. `state::layout::details_shape` decides whether the
  commit details panel is drawn Full, as a 44 px Rail or Hidden: it is a rail while a tool view
  (`state::layout::tool_view`: conflict, blame, file or folder history, issue, Git Flow, in the
  central area's order) fills the central area, unless `view.details_expanded_for` names that
  view. `LayoutMsg::ExpandDetails` and Go to the commit message set it, and `update` clears it
  (`drop_stale_expansion`) as soon as the tool view changes or closes; the saved setting never
  changes. A hidden left or details panel leaves a 16 px edge strip that shows it again
  (`ui/panel_rails.rs`, with the rail). Toasts are anchored to `ToastBounds`, where the top
  chrome and the status bar end, measured while `frame_layout` draws them (`ui/toasts.rs`), all
  one width. The toolbar's repository and branch pickers (`ui/toolbar_pickers.rs`) are sized
  to their names up to a cap, and in a window too narrow for everything the pure
  `name_widths` shrinks them (the longer first, then both, down to a floor) against the rest
  of the bar measured last frame. Every group sits on its own surface (`widgets::tool_group`),
  the middle groups follow the pickers, the pull request group shows only with a hosting
  service, and the right-hand group is anchored to the window's right edge.
- **Window chrome**: `state/window_chrome.rs` decides who draws the frame (`Chrome::for_os`:
  Trellis on Windows, Trellis under the traffic lights on macOS, the system on Linux; set once
  by the real app, `System` in tests unless they pick another) and holds the pure pieces: the
  window actions (`WindowCmd`), what the shell read about the window (`WindowInfo`) and the Alt
  tap that opens the menu (`AltTap`). `ui/title_bar.rs` draws the 44 px bar: the menu button
  (the menus of `ui/menu_bar::menus` as submenus), the tabs (`tabs::strip`) and empty space
  that moves or maximizes the window. `ui/window_frame.rs` builds the viewport for each chrome
  (`viewport`, used by `main.rs`) and draws an `Order::Tooltip` overlay above every dialog on
  Windows: the window buttons (`ui/window_buttons.rs`), the resize edges and corners (none
  while maximized), a 1 px border, and a drag zone over the bar while a dialog blocks it.
  Views push `AppMsg::Window`; `update/app.rs` returns `UiEffect::Window`, and the shell's
  `dispatch` sends the viewport command (`window_frame::viewport_cmd`), the only
  `send_viewport_cmd` outside the shell's own effects (`tests/layering.rs`). On Windows the
  shell also installs `platform/native_window/` (`TrellisApp::new`): a window procedure in
  front of winit's (`win_proc.rs`, its state in atomics) that answers the hit test with
  `HTMAXBUTTON` over the drawn Maximize button for Snap Layouts, opens the system menu on
  Alt+Space or on `WindowCmd::SystemMenu` (`win_system.rs`, which also sets the Mica backdrop
  and reads transparency and high contrast). Each frame `ui/app_native.rs` polls it into view
  state (`WindowInfo::max_button`, `ViewState::backdrop`, a click as `SetMaximized`, a shown
  menu cancelling the Alt tap) and hands back `window_frame::caption_geometry`; the pure parts
  are in `state/window_native.rs`. Only `ui/app.rs` and `ui/app_native.rs` may name
  `platform::native_window`, and no view, update or state code names `windows::Win32`
  (`tests/layering.rs`). `ui/backdrop.rs` picks the palette's Mica tints (`mica_tint`, cached in
  `ThemeCache`) and gives the title bar, toolbar and sidebar their fill (`backdrop::fill`);
  `TrellisApp::clear_color` clears to transparent only while Mica shows.
- **Menu bar** (`ui/menu_bar/`, one file per menu; a bar under the system frame only, the
  drawn title bar's menu button otherwise): every item pushes the message the toolbar,
  palette or key already sends, shows its key from the keymap (`Keymap::hint`, never a literal)
  and explains a disabled state on hover; the texts and reasons are pure helpers in
  `state/menu_bar.rs` and `UndoHistory::undo_entry`. Its own window actions are `AppMsg`
  (`update/app.rs`): Quit returns `UiEffect::CloseWindow`, which the shell's `dispatch` turns
  into a viewport close, and About opens `Modal::About`, which lists the third-party licenses
  `trellis_core::notices` parses from THIRD-PARTY-NOTICES.md (embedded at build time).
- **Desktop notifications**: `update/notify.rs::desktop` returns `Effect::Notify` (a
  `DesktopNote`, `effect/notify.rs`) only while `settings.desktop_notifications` is on and
  `st.view.focused` is false; the shell sets `view.focused` from egui at the start of each frame,
  before runner results are handled, so `update` never asks egui. Finished non-quiet jobs
  (`update/ops.rs::finish_job`) and behind alerts use it; turning the setting on sends one
  confirmation regardless of focus. `runner/notify.rs` shows the note through notify-rust (the
  only caller, `tests/layering.rs`) and reports a refusal as
  `PrefsMsg::DesktopNotificationFailed`.
- **Credential prompts** (`askpass/`): every git child that may prompt gets Trellis as its
  `GIT_ASKPASS` / `SSH_ASKPASS`; the helper process asks the app's loopback listener, which
  reports `CredentialMsg::CredentialPrompt`, and `Effect::AnswerPrompt` hands the dialog's
  `Reply` back. `gitcli::NO_PROMPTS` (reads) turns all of it off; background jobs use
  `gitcli::BACKGROUND`, which also keeps Git Credential Manager from opening a window. HTTPS
  sign-ins typed with "Remember this sign-in" are kept in `CredentialPrompts::typed` until the
  job ends; `update/askpass_store.rs` then saves them (`CredentialEffect::Remember`) or drops
  the rejected ones (`Forget`).
  An unknown SSH host whose question parses (`PromptKind::HostKey`) is shown as fields
  (`ui/askpass_host_key.rs`); any other yes/no question shows ssh's text as it is.
- **Auto-fetch sign-ins** (`state/sign_in.rs`, `update/sign_in.rs`, `ui/sign_in_banner.rs`):
  `jobs::start_job` notes which remotes a fetch covers (`SignInNotices::fetching`); an
  auto-fetch that fails for want of a sign-in (`auth_failure::fetch_failures`) pauses those
  remotes (global, in memory, by workdir and remote), auto-fetch leaves paused remotes out, the
  banner offers a fetch that can prompt, and any fetch of the remote that works resumes it.
- **Git LFS notices** (`state/lfs.rs`'s `LfsSession`, `state/lfs_notes.rs`, `ui/lfs_banner.rs`,
  `ui/lfs_badge.rs`, `ui/diff/lfs_note.rs`): the root .gitattributes' LFS patterns ride on every
  whole status read (`LoadMsg::StatusLoaded` and `RepoUnchanged` carry them;
  `update::lfs::patterns_read` applies them), so hand edits reach the Track in Git LFS entries,
  the banner and the auto-fetch LFS download without a full reload. The banner shows while
  git-lfs is known missing (the startup `Effect::DetectLfs`) in a repository with patterns, until
  dismissed for that workdir this session. File rows marked `lfs` get a badge, and a diff whose
  `RepoMsg::DiffLoaded` brings an `LfsChange` shows a summary instead of the pointer text
  (`LfsDiffNote`, keyed by target, parked with its tab) until "Show pointer text".
- **Signing passphrases** (`gpg_prompt/`): gpg-agent's pinentry is fixed when the long-running
  agent starts and is shared by all its clients, so there is no Trellis pinentry; swapping it
  would mean editing gpg-agent.conf and restarting the agent. GnuPG's per-call alternative,
  `--pinentry-mode loopback`, is used instead: for commands that may prompt (not
  `NO_PROMPTS`), `gitcli` adds `GIT_CONFIG_COUNT`/`KEY_0`/`VALUE_0` setting `gpg.program` to
  the Trellis executable (command scope beats every config file) plus `TRELLIS_GPG_WRAPPER=1`.
  The wrapper runs the user's own gpg.program (or `gpg`) with `--pinentry-mode error` first, so
  a passphrase the agent has cached (or a key without one) signs without a prompt; on "No
  pinentry" it sends a structured `GpgAsk` through the askpass listener and signs with loopback,
  the passphrase on fd 0, asking again after a bad passphrase (three tries). When
  `settings.gpg_prompts_in_app` is off, `update/askpass.rs` answers `Reply::UseSystem` without
  showing anything; that, an unreachable app, or a GnuPG without loopback runs gpg exactly as
  git asked, so GnuPG's own pinentry appears.
- **Logs**: each tab has its own activity log: the active tab's is `AppState::log`, the others'
  `Tab::log`. `AppState::log` / `log_quiet` add to the active tab's; `log_in` / `log_quiet_in`
  route a line to the tab showing a repository (a scoped result, a finished job, a git command).
  Each line is also queued, tagged with its tab's repository, in the outbox
  `LogsState::pending` (`state/logs.rs`), which the end of `update` flushes as
  `Effect::Logs(LogsEffect::Append)` once the activity file has loaded; `activity.jsonl` lines
  carry that `repo` (`paths::encode`). `LogsMsg::Start` at startup reads the file, and its
  earlier entries are handed to the tabs showing their repositories, in front of this session's
  lines; those for a repository no tab shows wait in `LogsState::unclaimed` until a tab
  activates or opens it (`update/logs_tabs.rs`). Nothing opens the panel by itself: a failure
  logged with `log` / `log_in` is announced at the end of `update` as a failure toast with
  Details (`logs_tabs::announce`), or gives Details to a failure toast pushed in that update;
  Details selects the line's tab and shows its activity log, opening the failed run's row.
  An entry is a note or a run: a job that ends in `ops::finish_job` is logged by
  `update/logs_runs.rs` (`job_ended`) with a `RunInfo` (its label, a one-line summary from
  `logs::last_message` or git's explained cause, its duration from `LogsState::job_started`,
  which `logs_runs::stamp` sets at the end of `update`), persisted as the line's optional
  `run` object. A loud failure's row starts open (`LogsState::open`), and its fix is kept once
  in `LogsState::fixes`: the failure toast's button is a `LogsMsg::Fix(id)` proxy, so the
  panel and the toast run it once. `state::activity_rows::rows` groups the view's rows
  (session headers, runs and notes, consecutive successful auto-fetches as one routine row);
  `cap_routine` keeps such a streak to `ROUTINE_KEEP` entries, the first counting the rest.
  One ordered
  log worker (`runner/logs.rs`) owns the activity file and the performance log and stamps
  times, so `update` reads no clock. `Runner::run` times every effect that doesn't wait on the
  user (`Effect::timed`) and gitcli's run observer times every git command; those taking
  `logs::SLOW` or more go to `performance.log`, and with extended logging each git command
  also comes back as `LogsMsg::GitRan` for a quiet activity line.
- **Deep links** (`deep_link.rs`, `state/deep_link.rs`, `update/deep_link.rs`,
  `update/deep_link_open.rs`): `main.rs` first hands a `trellis://` argument to a running window
  (`trellis_core::instance`; `Runner::start_instance` listens, next to the askpass listener) and
  exits; otherwise the new window opens it after restoring its tabs. Either way it arrives as
  `DeepLinkMsg::Open`, which brings the window forward and, unless another dialog or Preferences
  is open (a toast offers to open it later), parses it and sends `DeepLinkEffect::Find` over the
  active tab, the other tabs, favorites and recent repositories. The runner checks each with
  `link::find::check` (root commit, remote URL); `state::deep_link::best` keeps the top rank
  (root and URL, root, URL), merges worktrees of one repository, and prefers the one open in a
  tab. One match opens through `tabs::open`; several ask (Choose), none offers Locate or Clone.
  The pending link (`DeepLinkState::pending`, numbered by `seq` so a newer link wins) waits in
  `Awaiting::Ready` until its repository is the active tab: `deep_link_open::ready` runs at the
  end of every snapshot load (and right after `tabs::open` for an open or parked tab) and selects
  the target, or offers Fetch in a toast when it is missing. The fetch is an ordinary job with
  `then_link`, after which `ops::finish_job` has the repository checked again. A clone from the
  Not found dialog is recognised by its remote URL when it loads. Nothing runs git without a
  click. Copying a link goes the other way: menus send `DeepLinkMsg::Copy`, update picks the
  remote URL, and `DeepLinkEffect::Build` adds the root commit.
- **Releases and updates**: the Release workflow packages each build (`scripts/package-*`,
  `packaging/`; see `docs/RELEASING.md`) and publishes `SHA256SUMS`. In the app, `UpdateMsg::Startup`
  arms the automatic check; `update/updates.rs::tick` starts it (at most once a day, after
  `STARTUP_DELAY`, an hour after a failure) as `UpdateEffect::Check`, which asks
  `forge::latest_release` and reports how this copy was installed. `app_update::offer` picks the
  file for this platform; Download runs `app_update::download`, which writes nothing until the
  SHA-256 matches. The release notes dialog reads the `CHANGELOG.md` that `build.rs` bundled, and
  `startup_notes` opens it once per new version (waiting in `notes_pending` while another dialog
  is open).
- **`AppState`** holds no egui types; textures and fonts live in a `UiCache` owned by the UI
  layer.

## Rules

1. `trellis-core` must not depend on `eframe`/`egui`.
2. `ui/`, `update/` and `state/` do no I/O. `tests/layering.rs` denies whole families (the file
   system, processes, network, threads, environment, clock, libgit2 access) and the core
   functions that do I/O, and names the file and line of each use. The few justified exceptions
   (the shell's startup in `ui/app.rs`) are listed in its `ALLOWED` table
   (`tests/layering/rules.rs`) with a reason.
3. New behavior gets an `update` test; new git or API logic gets a `trellis-core` test. A flow that
   crosses views, the runner and git gets a journey step (`src/journey/`).
4. New views get an `egui_kittest` snapshot when practical. Views read time from
   `st.view.now` / `st.view.utc_offset` (never the system clock) so images are reproducible.
5. No global mutable state; pass values in. The one exception is the active palette
   (`ui::theme::color`): a thread-local copy of a `Palette`, set by reference
   (`color::set_palette`) at the start of each frame, so renders on different threads (parallel
   snapshot tests) can't affect each other. Palettes built from theme files are values owned by
   the shell's `ThemeCache`, worked out again only when the theme, the theme files or the OS
   setting change. The colour families, what each colour is for, the reserved hue bands and the
   contrast floors are in [COLOUR.md](COLOUR.md). Immutable
   data loaded once (the syntax definitions in `highlight/syntaxes.rs`) is not mutable state,
   and neither are the askpass environment in `gitcli` (`enable_askpass`) and gitcli's run
   observer (`observe_runs`, which hands timings to the log worker): each set once by the app
   shell at startup, never changed afterwards and never set in tests. The PATH children get
   (`env_path`) is likewise computed once, on first use, and never changed. The git program
   (`git_program`) is the one setting that changes while Trellis runs: the shell sets it from
   the saved setting at startup, and afterwards only the runner sets it, after checking the new
   program (`GitEnvEffect::Use`); tests set it only to the git already on PATH. The counter
   behind `TabId::next` only hands out distinct ids; nothing depends on their values.
6. Each hosting provider implements `Forge`; the UI never matches on provider-specific types.
7. `scripts/check.ps1` (fmt, clippy -D warnings, tests) must pass before committing.
8. `trellis-core` returns `trellis_core::error::Error` (typed, via `thiserror`); decide behaviour
   from the variant (`is_auth()`, `is_not_found()`), never from message text.
9. Code that touches paths, files, processes, the environment or git follows
   `docs/PLATFORMS.md`, and uses the helpers it names (`paths`, `persist`, `names`, `repo/rpath.rs`,
   `env_path`, …) rather than writing new ones.
10. Views and update build user-visible text with `st.settings.lang()` and `tr` / `trf` / `trn`
    (keys in `crates/trellis-core/locales/`, see its README). Translated text is never matched
    against git output or used as an egui id; ids use fixed `from_id_salt` strings.

## Recipe: adding a feature

1. **Message.** Add a variant for the user's intent to the area's message enum (`RepoMsg` in
   `msg/repo.rs`, `LfsMsg` in `lfs.rs`, …). A new area gets its own enum, one `Msg` variant
   carrying it, and an arm in `Msg::on_inactive`; only messages no area owns go in `Msg`
   itself.
2. **Update.** Handle it in the area's handler in `update/<area>.rs`, which matches its enum
   exhaustively (no `_` arm, so nothing is dropped silently); `update/mod.rs` only routes each
   `Msg` variant to its handler. Change state there. Keep pure rules (checks, labels, why
   something is blocked) in `state/<area>.rs`.
3. **Effect.** If the work is slow or leaves the process, add an `Effect` variant in
   `effect/mod.rs`, give it a name in `effect/names.rs`, decide in `Effect::scope` and the
   runner's `queue::repo_of` whether it is scoped to a tab and whether it changes the
   repository (both list every effect), and return it from `update`. An effect that runs git
   as the single active job starts through `start_job` or `begin_job` (`update/jobs.rs`),
   which checks that no job is running and holds it for the trust prompt while git is held in
   the repository (`update/trust.rs`), so check their `Started` result rather than `st.job`.
4. **Runner.** Execute it in `runner/mod.rs`, or in `runner/<area>.rs` for more than a line.
   The real work (git, files, HTTP) goes in `trellis-core`.
5. **Result.** The runner sends a result message back. Handle it in the same `update` file.
   Drop stale results: compare a generation number, or the request with what is still open.
   Say in the enum's `on_inactive` what the result still needs if its tab is no longer active.
6. **View.** Draw it in `ui/<region>.rs`: read state, push `Msg`s.
7. **Tests.** Write an `update` test (`update/<area>_tests.rs`). Add a core test on a fixture
   repository (see `scratch_repo`) for new git logic, and an `egui_kittest` snapshot for a new
   view. A flow that crosses views, the runner and git gets a journey step (`src/journey/`):
   give a painted widget or a hint-only field an accessible name (`ui/access.rs`) so the
   journey can find it.
8. **Settings.** For a new preference, add a field with a default in `settings.rs`
   (`#[serde(default)]`) and a control on the matching Preferences page (`ui/prefs/`), built
   from `ui/prefs/form.rs` (rows, sections, labelled checkboxes). App settings apply at once.
   Values a page writes to git or repository config wait for the Save / Revert bar instead:
   `state/prefs_save.rs` says whether the open page has unsaved edits, `update/prefs_save.rs`
   saves and reverts them, `update/prefs_leave.rs` asks "Save your changes?" before leaving
   such a page, and `ui/prefs/save_bar.rs` draws the bar and the "Saved" tick.

## Migration status

- [x] 1. Baseline commit, `CLAUDE.md`, rustfmt config, check script, CI
- [x] 2. Extract `trellis-core` (git, graph, forge, settings, dates, spell, avatar fetching); `Forge` trait
- [x] 3. Single task runner replaces the per-feature thread/channel pairs
- [x] 4. Split `AppState` / `Msg` / `update` out of the `TrellisApp` struct, area by area
- [x] 5. Move views into `ui/` modules; remove global atomics (the palette flag is per thread)
- [x] 6. Headless `update` tests, `egui_kittest` snapshots, CI enforcing clippy
- [ ] 7. Resume `TODO.md`, one feature per commit with tests
