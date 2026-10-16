# Trellis on every OS and file system

Trellis runs on Windows (NTFS, FAT32 and exFAT, network shares, WSL through `\\wsl$`), macOS
(APFS, including case-sensitive volumes, and HFS+) and Linux (ext4, btrfs, tmpfs, vfat,
NFS and SMB). These platforms differ in ways that are easy to miss on one developer's machine
and expensive when they bite: the wrong file deleted, a repository opened twice, a crash on a
file name. Every rule below comes from a bug that was found and fixed.

Follow this file for any change that touches paths, files, processes, the environment or git.
Each rule names the helper to use. If a helper doesn't fit, extend it rather than writing a
second one next to it.

## Platform differences at a glance

| | Windows | macOS | Linux |
| --- | --- | --- | --- |
| Case sensitivity | NTFS ignores case, but a folder can be made case-sensitive (`fsutil`, and WSL does it); FAT and exFAT ignore case | APFS and HFS+ ignore case by default; case-sensitive volumes exist | Case-sensitive, except vfat, exFAT, CIFS and ext4 casefold folders |
| Unicode forms | Kept apart (`é` composed and decomposed are two names) | APFS and HFS+ treat them as one name | Kept apart |
| Path encoding | UTF-16, possibly unpaired surrogates (stored as WTF-8 by Rust) | UTF-8 | Any bytes except NUL and `/` |
| Separators and prefixes | `\` and `/`; drive letters, UNC `\\server\share`, `\\?\` and `\\.\`, `\\wsl$` and `\\wsl.localhost`; libgit2 reports `C:/x/y/` | `/`; `/var` is a link to `/private/var` | `/` |
| Reserved names | `CON`, `PRN`, `AUX`, `NUL`, `CONIN$`, `CONOUT$`, `COM0`–`9`, `LPT0`–`9` (and `¹²³`) with any extension; `<>:"\|?*`; trailing dot or space; `:` streams; 8.3 short names (`git~1`) | HFS+ ignores some invisible code points (`.g\u{200C}it` is `.git`) | Only `/` and NUL |
| Links | Symlinks need a privilege or developer mode; junctions; 8.3 names are another spelling | Symlinks | Symlinks |
| Long paths | 260 characters unless long paths are on, and some shell APIs never take more | Long | Long |
| Locking | Open files can't be renamed or deleted; scanners, indexers and sync clients hold files briefly (errors 5, 32, 33) | Advisory only | Advisory only |
| Permissions | A read-only attribute blocks replace and delete; no exec bit | Mode bits; exec bit | Mode bits; exec bit; chmod fails on vfat and some shares |
| Line endings | Git for Windows sets `core.autocrlf=true` by default | LF, `autocrlf` usually unset | LF, `autocrlf` usually unset |
| File watching | `ReadDirectoryChangesW` can drop events silently | FSEvents reports overflows | inotify reports overflows, follows links unless told, has a watch limit; network and 9P mounts send nothing |
| Trash | Recycle Bin per volume; none on shares; can be turned off or sized so the shell deletes for good without a word | Trash; none on some volumes | freedesktop trash, copied across devices; may be missing |
| PATH and environment | Variable names ignore case; `PATHEXT`; cmd.exe can't start in a UNC folder | Apps started from the Dock or Finder get a minimal PATH (no Homebrew) | The desktop session's PATH |
| Timestamps | NTFS 100 ns; FAT 2 s | APFS ns; HFS+ 1 s | ns on ext4 and btrfs; network attribute caches lag |
| Notifications | WinRT toasts; Focus Assist can hide them | Notification Center; Do Not Disturb can hide them | freedesktop notifications over D-Bus; needs a notification server |
| git builds | Git for Windows (MinGW), often a newer version | Apple's `/usr/bin/git` (older) or Homebrew's | Whatever the distribution ships, sometimes old |

## Rules

### Path identity and comparison

1. **Decide "same folder" by asking the disk, never by `cfg!(windows)` or `cfg!(macos)`.** Use
   `paths::same_folder` on worker threads; it compares `paths::FolderId` (device and inode on
   Unix, volume serial and file id on Windows; an id of 0 counts as unknown). Case sensitivity
   belongs to the volume, and on Windows to the folder.
2. **Compare spellings only with `paths::same_spelling`** (and `paths::is_within`). It never
   folds case or Unicode form; it only equates separator style, `.` parts, trailing separators,
   drive-letter case, UNC server and share case, `\\?\C:` / `\\.\C:` / `C:`, and `\\wsl$` /
   `\\wsl.localhost`. Use it for paths that don't exist, or as the cheap first check.
3. **No disk work in `update` or views.** Use `AppState::same_folder`, `names_repo` and
   `tab_showing` (`state/known_folders.rs`), which read identities that workers cached in
   `Snapshot::folder`, `Tab::folder` and `KnownFolders`. `tests/layering.rs` flags
   `paths::{resolve, temp_dir, same_folder}` and `FolderId::of` there.
4. **Resolve outside paths before comparing them with git's.** Anything from argv, typed text,
   settings, the recent and tabs files or `%TEMP%` goes through `paths::resolve`, which follows
   links, junctions and 8.3 names and drops `\\?\` (via `paths::plain`).
5. **Call `canonicalize` only through `paths::resolve`.** The two other calls handle failure:
   `repo/workfile_disk.rs` (`Disk::canonicalize`, falling back to a component walk that refuses
   `..` and links, for RAM disks, VirtualBox shares and WinFsp volumes) and
   `watch::with_canonical` (best effort). Never show or pass on a `\\?\` path.
6. **Refuse typed relative paths.** Expand `~` with `paths::expand_home`, then require
   `paths::typed_is_absolute`; never resolve against the process cwd (on Windows that is often
   System32). Command-line paths are made absolute against the shell's cwd.
7. **Read the command line with `args_os`, never `args`** (which panics on a name that isn't
   UTF-8), and undo the Windows trailing-backslash quote (`"C:\My Repo\"` arrives as
   `C:\My Repo"`; see `start_path` in `main.rs`).
8. **Trust is shared only by the same spelling or the same identity**, never by a case fold, so
   trusting `/Volumes/cs/app` doesn't trust `/Volumes/cs/App`.
9. **Normalize for matching, never for acting.** Search and filters may compare NFC lowercase;
   anything that touches a file uses the exact bytes.

### Repo paths and git

1. **Repo paths are git's bytes:** `/`-separated, usually but not always UTF-8. Read them only
   through `path_bytes()` and the other `*_bytes` accessors, via `repo/rpath.rs`. Never call
   `DiffFile::path()`, `Submodule::path()` or any git2 accessor that returns `&Path`: they panic
   on Windows for names that aren't UTF-8.
2. **A name that isn't UTF-8 is shown, not acted on.** `rpath::shown` writes it with `\xHH` and
   the row carries `Hazard::NotUtf8`; `state::path_hazards::refusal`, checked at the top of
   `update`, refuses every action on it.
3. **Match repo paths byte for byte.** Never trim them, fold their case or
   `replace('\\', "/")` them; on Unix `\` is an ordinary character in a name.
4. **Never pass file paths to libgit2 pathspec APIs** (`reset_default`, `add_all`): they read
   globs (`[id].tsx` matches `i.tsx`). Edit the index exactly with `repo/exact.rs`:
   `exact::index` opens the index file on its own, because the repository's index folds case
   under `core.ignorecase`.
5. **Patches touch one path.** Filter by delta (`ApplyOptions::delta_callback`, with
   `rpath::touches`), and apply index patches with `exact::apply_to_index`.
6. **Give the git CLI literal paths.** Use `--literal-pathspecs` and `--` before paths.
   `submodule update` ignores the flag, so give it `:(literal)<path>`
   (`state/submodules.rs`).
7. **Parse only `-z` output, as bytes,** and strip the documented prefix exactly
   (`strip_prefix(b"? ")` in `repo/status_cli.rs`). Never trim a field: ` a.txt` is a real name.
8. **Ask the file system whether it folds names.** `repo::collide::probe` creates a
   `*.lock` scratch file in the git dir (the watcher ignores locks), looks it up by its
   case-flipped and NFD names, and caches the answer per git dir in `Snapshot::folding`. Warn
   when `core.ignorecase` disagrees (a repository copied from Linux to exFAT, say). Work-tree
   decisions use the probe; code that mirrors git's own matching uses `core.ignorecase`.
9. **Index paths equal under NFC plus lowercase are a collision** (`repo/collide.rs`). Always
   warn (the "File names" banner and row notes). Refuse discard, delete and restore only when
   the probe says they share one file here, and check again in core at the moment of acting.
10. **Turn libgit2's `C:/x/y/` into the OS spelling** before it reaches the OS, another program
    or the screen: `native_path::native` for arguments, `native_path::shown` for text. Keep the
    raw form only as a matching key.
11. **Report git's owner refusal as `Error::NotOwned`**, never "not a repository"
    (`safe_directory::open_error`). Offer `safe_directory::add` after confirmation, in git's
    spelling (`safe_directory::config_value`: forward slashes, `%(prefix)///server/share` for a
    share).

### Names Trellis creates or accepts

1. **One rule set for every name Trellis creates:** `names::win_name_problem` (and
   `names::path_problem`, `names::ref_name_problem`) for files, folders, renames, branches,
   tags, clone and worktree folders, patches and SSH keys. Typed names Windows can't use are
   errors on Windows and "won't check out on Windows" warnings elsewhere (`names::blocks_here`).
2. **Names Trellis makes up go through `names::portable`** (`con` becomes `con-repo`, `aux`
   becomes `aux-wt`), so they work everywhere without asking.
3. **Decide "same file" by identity, never by spelling or OS.** Use `Disk::same_file`
   (`same-file`, or device and inode) in `repo/workfile_disk.rs`, and check the parent's
   listing for the exact spelling (`listed_exactly`). Disk calls behind such decisions go
   through the injectable `Disk` so tests can stand in for another file system.
4. **Read folder spellings from disk.** An existing folder typed in another case or form is
   refused (`check_parent_spelling`), never silently reused.
5. **Security name checks overmatch on every OS.** Strip HFS+ ignorable code points
   (`names::is_hfs_ignorable`), then NFC, then case-fold (`names::fold`); also catch an NTFS
   trailing dot or space, `:` streams and `git~1` (`names::is_git_dir_name`). Hook names use the
   same fold (`trust::is_hook_name`) and are also looked up on disk.
6. **Compare ref names and ref folders by NFC plus lowercase on every OS** (`names::ref_clash`):
   loose ref files collide on Windows and macOS, so `Feature/bar` must not slip into
   `feature/`.
7. **Warn when a new path equals a tracked path or folder under NFC plus lowercase**
   (`workfile_disk::new_path_warnings`), even on Linux, where it would break other people's
   checkouts.
8. **Rename a change of case or Unicode form through a temporary name,** including `git mv`
   (`workfile_mv::git_mv_via_temp`), whose own case check is ASCII-only.

### Writing files

1. **Every write goes through `persist::write_file` or `persist::atomic_write`:** a sibling
   temp file, `sync_all`, rename, then a folder fsync on Unix. Never `fs::write` a file that
   matters; a crash must leave the old file or the new one.
2. **Links:** Trellis's own files follow a symlink to its real file and keep the link
   (`atomic_write`). Working-tree writes set `WriteOptions::refuse_links` and never write
   through a link, which can point anywhere.
3. **A replaced file keeps its permissions and read-only attribute.** Set the exec bit only
   through `WriteOptions::executable`, only when it changes (`persist::needs_chmod`), honour
   `core.fileMode`, and never fail a write because chmod failed (vfat, some shares).
4. **On Windows, clear read-only to replace or remove a file, then restore it, and retry errors
   5, 32 and 33 briefly** (`persist/replace.rs`; `persist::remove_file` for removal).
5. **When git has the bytes, let git write the working file** (`git checkout-index`, as conflict
   resolution does in `repo/resolve.rs`). Never rewrite line endings wholesale; build output
   from the stages' own bytes so each line keeps its ending.
6. **A multi-file replacement keeps the old set until the new set is in place.** Move the old
   files aside with `persist::move_aside`, install the new ones, restore on any failure, and
   delete the backups last (see `ssh_keys/generate.rs` and `settings/file.rs`).
7. **Saves from several threads go through `persist::LatestOnly`** so an older save can't land
   after a newer one.

### Deleting and trash

1. **Trash through `repo::trash::os_trash`, which returns a typed `TrashRefusal`** whose
   `problem` is a `TrashProblem`: `NoTrash`, `InUse`, `AccessDenied` or `Other`.
2. **Offer a permanent delete only for `NoTrash`.** In-use, access-denied and other failures get
   Retry: a file held by a scanner is not a reason to destroy it.
3. **On Windows, read the bin's settings before moving** (`NukeOnDelete`, `MaxCapacity`,
   `NoRecycleFiles`; `repo/trash/win.rs`, decided by the pure `bin_refuses`). With no UI shown,
   the shell deletes for good without saying so.
4. **Remove links without following them,** and re-check parent folders before trashing.

### Opening files and starting programs

1. **Never OS-open a repo file the system would run without asking.** Classify it with
   `opens::opens_as_program(path, facts, os, pathext)`, the pure per-OS table, plus the exec
   bit and the symlink target (`opens::FileFacts`). In a repository the user chose not to trust,
   refuse; otherwise confirm. Cloned files carry no Mark-of-the-Web or quarantine flag.
2. **Never give cmd.exe a UNC working directory.** Use `pushd` inside the command or PowerShell
   `-LiteralPath` (`platform/terminal.rs`).
3. **Quote raw command lines for cmd and PowerShell yourself,** and use `CREATE_NEW_CONSOLE` for
   terminals and `CREATE_NO_WINDOW` for helpers, so nothing flashes a console.
4. **Watch started programs for `external_tools::EARLY_EXIT` (1.5 s)** and report an early
   non-zero exit with the first stderr line. Starting a program is not proof it ran.
5. **macOS `open -a` takes options only after `--args`, with `-n`;** a running app never sees
   `--args`.
6. **Search PATH like the OS does** (`external_tools.rs`): on Unix the file needs an exec bit;
   on Windows match names in any case by listing the folder and try `PATHEXT`. Never search the
   cwd.
7. **Desktop notifications go only through `runner/notify.rs` (notify-rust):** WinRT toasts on
   Windows, Notification Center on macOS, freedesktop notifications over D-Bus (zbus, no
   libdbus) on Linux. A missing notification server is an error the confirmation reports;
   never call it from update or views (`tests/layering.rs`).

### Environment and git

1. **Children get PATH only through `env_path::apply`** (or `Command::env`). Never mutate the
   process environment, and never build an env map keyed by `"PATH"`: Windows names ignore
   case, so `Path` and `PATH` would both reach the child.
2. **On macOS, merge in the login shell's PATH** (`env_path::read_login_path`, off the UI thread,
   `LOGIN_SHELL_TIMEOUT` of 3 s), falling back to `env_path::MACOS_FALLBACK`
   (`/opt/homebrew/bin:/usr/local/bin`).
3. **Check git once at startup** (`git_version::detect`). Show which git runs and its version
   (Preferences > General), gate features by `git_version::Feature` (`Restore` 2.23,
   `ForceIfIncludes` 2.30, `SparseCheckout` 2.35, `MergeTree` 2.38), and show
   `GitInfo::banner` with the reasons.
4. **Read git config booleans with `--type=bool`** (`runner/config.rs`): `yes`, `1` and `TRUE`
   are true. `core.autocrlf` is three-way (`true`, `input`, `false`), and a value that can't be
   read is never rewritten.
5. **For untranslated git messages set `LANGUAGE=C` and `LC_MESSAGES=C`, never `LC_ALL=C`**
   (`gitcli.rs`), which would also give hooks, gpg and pinentry an ASCII-only character set.
6. **On Unix, set `DISPLAY` when it's unset** (`gitcli::display_placeholder`): OpenSSH before
   8.4 asks the askpass helper only when `DISPLAY` is set.
7. **A path written into a git config value that a shell runs (`core.sshCommand`) goes through
   `native_path::native` and `rebase::shell_quote_for`** (`ssh_keys/command.rs`): no `\\?\`
   prefix, sh-quoted, and forward slashes on Windows. Git for Windows' bundled ssh doesn't read
   keys from the Windows OpenSSH agent service; only Windows' own `ssh.exe` does.

### Watching and refresh

1. **Don't follow links** (`notify::Config::with_follow_symlinks(false)` in `watcher.rs`): a link
   to `$HOME` exhausts inotify's watch limit.
2. **For a linked worktree, also watch the common dir** (`watch::common_dir`): its `refs/`
   recursively, `HEAD` and `packed-refs`.
3. **Assume the watcher can be silent.** Detect remote and virtual file systems with
   `volume::remote_kind` (statfs magic on Linux, `MNT_LOCAL` on macOS, `DRIVE_REMOTE` on
   Windows), tell the user once, and poll every `REMOTE_POLL` (10 s) while focused. On Windows
   poll every `WINDOWS_POLL` (30 s) while focused (`state/watch.rs`, `poll_every`). Keep the
   refresh on focus and F5.
4. **Never trust size plus mtime alone.** Hash files up to 64 KiB, and treat a stamp taken
   within `refs_key::RACY_WINDOW` (2 s) of the modification time as untrustworthy
   (`refs_key::stamp_trustworthy`): FAT has 2 s timestamps and HFS+ 1 s.

### Persistence

1. **Persist paths with `paths::encode` and read them with `paths::decode`, never `display()`.**
   A path that isn't plain is written as `~enc:` plus percent-encoded Unix bytes or Windows
   WTF-8, so a newline or a name that isn't UTF-8 survives.
2. **Never persist a `FolderId`:** device numbers change when a volume is mounted again.
3. **Key settings saved per repository with `settings::repo_keys`** (`key`, `get`, `set`), by
   `real_workdir`. Saving replaces entries under other spellings, and `adopt` migrates old keys
   on load.
4. **No file-system calls on the UI thread at startup,** not even `exists()`: an unreachable
   share hangs the window. Restore tabs unchecked and let the background check mark missing
   ones.
5. **Put caches in the cache folder** (`%LOCALAPPDATA%`, `~/Library/Caches`, `$XDG_CACHE_HOME`;
   `settings/paths.rs`), not the roaming config folder, and take the home folder from
   `settings::home_dir`, which ignores an empty or relative `HOME`.

## Testing on every OS

CI runs the same lint and tests on Windows, macOS and Linux. Most platform bugs are caught by
making the decision a pure function and testing it for every OS on every machine.

- **Pure `_for(..., os)` table tests.** Put each platform decision in a pure function that takes
  the OS, a `windows: bool`, error codes or volume facts as parameters
  (`check_new_path_for`, `check_branch_name_for`, `shell_quote_for`, `config_dir_for`,
  `opens_as_program` with `opens::Os`, `volume::linux_remote_kind`, `trash::bin_refuses`,
  `git_version::missing_for`, `poll_every`) and test every OS's rows on every runner. The
  wrapper that passes `cfg!(windows)` or `Os::current()` stays one line.
- **Fixtures that no file system has to allow.** Build trees with names that aren't UTF-8,
  NFC/NFD pairs and case twins with `git mktree` or `git update-index --cacheinfo` /
  `--index-info`, so they run on all three OSes. Add real-file tests only where the file system
  takes the name.
- **Temp folders come from `paths::temp_dir()`,** never `std::env::temp_dir()`: on Windows CI
  that is an 8.3 spelling (`RUNNER~1`) and on macOS a `/var` link, so tests would compare one
  folder's two spellings. For alias cases add a fixture: a symlink, or a junction on Windows.
- **No hard-coded `C:/` paths in tests that resolve or touch them.** On Unix `C:/work` is a
  relative path, which resolves against the cwd; this broke CI twice. Build paths from
  `paths::temp_dir()`; a `C:/` string is fine only as a label a view shows.
- **Case-sensitive volumes.** Tests that need one read `TRELLIS_TEST_CS_DIR` and skip with a
  note when it's unset. CI provides it: a case-sensitive APFS image (`hdiutil create -fs
  "Case-sensitive APFS"`) on macOS, and `fsutil file setCaseSensitiveInfo <dir> enable` on
  Windows; both run the `case_twins` tests. Gate real-volume tests by a live probe, not by
  `cfg!`.
- **cfg-gated tests skip cleanly.** Tests that need real OS features (symlinks, which Windows may
  deny; `sudo chown`; read-only files; network file systems) check what they need and return
  with a note when it's missing, never fail.
- **`#[cfg(unix)]` code isn't compiled on the Windows dev machine,** so `scripts/check.ps1`
  doesn't lint it. Read it twice before committing (unused imports, a moved helper's name); the
  macOS and Linux CI runs catch the rest.
- **Snapshot images** are rendered on Windows; the other runners skip only the image comparison.

## Known gaps and things only a real machine can confirm

Gaps that are accepted for now:

- **Names that aren't UTF-8 are refused, not carried.** Action APIs still take `&str`, so such
  rows are shown with `\xHH` and every action on them is refused, rather than passing their bytes
  through to git.
- **No Recycle Bin check without a volume GUID** (UNC paths, redirected folders): the bin
  settings can't be read there.
- **Replacing a file doesn't keep its owner, ACLs, xattrs or hard links;** only permissions and
  the read-only attribute carry over.
- **Linux trash across devices is slow:** the `trash` crate copies the file synchronously, which
  takes a while for large files on network mounts.
- **Windows file watching can drop events** without an error (`notify` sends no rescan on
  overflow); the 30 s poll covers it.
- **Desktop notifications don't carry Trellis's name yet:** on Windows they are attributed to
  PowerShell's app id until Trellis registers its own AppUserModelID; on macOS an unbundled
  binary posts under another app's identity through the deprecated NSUserNotification API.

Unverified, needs a real machine:

- the terminal and Explorer for repositories on UNC shares and `\\wsl$`;
- `safe.directory` on a real share (the `%(prefix)///server/share` spelling);
- the PATH of an app started from the macOS Dock, and `open -n -a ... --args`;
- remote-volume detection on real NFS, SMB and 9P mounts;
- `DISPLAY` with OpenSSH before 8.4;
- whether `\\wsl$\X` and `\\wsl.localhost\X` give one `FolderId`;
- trash for paths over 260 characters on Windows (expected: `NoTrash` and a permanent-delete
  offer);
- whether a minimized window handles results before it is restored, on each OS (if not, they
  land while it is focused and only toast);
- desktop notifications while Focus Assist or Do Not Disturb is on (expected: hidden, no
  error).
