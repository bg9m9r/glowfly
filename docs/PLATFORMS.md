# Glowfly on every OS and file system

Glowfly runs on Windows (NTFS, FAT32 and exFAT, network shares, WSL through `\\wsl$`), macOS
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
| Window frame | Drawn by Glowfly: no system decorations; its own title bar, window buttons, resize edges and border, with Snap Layouts over Maximize, the system menu (Alt+Space or a right-click on the bar) and, on Windows 11 22H2 and later, Mica behind the title bar, toolbar and sidebar | The system frame with a full-size content view: the traffic lights sit over Glowfly's title bar | The system frame, with Glowfly's menu bar and tab bar below it |
| Menus | The drawn title bar's menu button | The system menu bar (the app menu, then File, Edit, View, Window and Help) and the title bar's menu button | The in-window menu bar, which can be hidden |
| Terminal keys and selection | Alt with a letter, digit or punctuation key sends ESC and the character; no PRIMARY selection | Option composes characters unless Option as Meta is on (Preferences > Terminal); no PRIMARY selection | Alt as on Windows; a mouse selection sets PRIMARY and a middle click pastes it (X11 and Wayland) |

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
   `submodule update` and `submodule sync` ignore the flag, so give them `:(literal)<path>`
   (`state/submodules.rs`, `submodule_ops/update.rs`); `submodule set-url` and `set-branch`
   take a plain path after `--`.
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
   The scan is kept per index file (`repo/collide_scan.rs`, by its length, time and checksum,
   trusted only once the file is `RACY_WINDOW` old), so reads of an unchanged index don't rescan.
10. **Turn libgit2's `C:/x/y/` into the OS spelling** before it reaches the OS, another program
    or the screen: `native_path::native` for arguments, `native_path::shown` for text. Keep the
    raw form only as a matching key.
11. **Report git's owner refusal as `Error::NotOwned`**, never "not a repository"
    (`safe_directory::open_error`). Offer `safe_directory::add` after confirmation, in git's
    spelling (`safe_directory::config_value`: forward slashes, `%(prefix)///server/share` for a
    share).
12. **Write commit messages in the repository's `i18n.commitEncoding`** (`commit_encoding`):
    git labels a message file's bytes with it and never re-encodes them. Hand git a typed commit
    message in a file, not with `-m`: Git for Windows reads its command line as UTF-8. Refuse one
    the encoding can't hold rather than letting it turn into `?`, judged by the label git
    records (`ISO-8859-1` has no `€`, though WHATWG reads it as windows-1252). Read commit
    text through `commit_text` and git's message files (MERGE_MSG) as bytes, never with
    libgit2's `Repository::message`, which panics on bytes that aren't UTF-8.

### Names Glowfly creates or accepts

1. **One rule set for every name Glowfly creates:** `names::win_name_problem` (and
   `names::path_problem`, `names::ref_name_problem`) for files, folders, renames, branches,
   tags, clone and worktree folders, patches and SSH keys. Typed names Windows can't use are
   errors on Windows and "won't check out on Windows" warnings elsewhere (`names::blocks_here`).
2. **Names Glowfly makes up go through `names::portable`** (`con` becomes `con-repo`, `aux`
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
   same fold (`trust::is_hook_name`) and are also looked up on disk. Each hook found (any file
   on Windows, an executable one on Unix, through links) is read for the trust print, up to
   1 MiB at a refresh and whole at a full load (`trust/digest.rs`); only a plain file is opened
   (a FIFO's open would block), and one that can't be read gets a fixed digest no contents have.
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
   matters; a crash must leave the old file or the new one. Append-only logs are the one
   exception: they grow through `persist::append` (or a handle from `persist::open_append`),
   and their readers skip a torn last line.
2. **Links:** Glowfly's own files follow a symlink to its real file and keep the link
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
   `problem` is a `TrashProblem`: `NoTrash`, `InUse`, `AccessDenied`, `Other` or
   `HomeTrashOnly`.
2. **Offer a permanent delete only for `NoTrash`** (and next to the home Trash for
   `HomeTrashOnly`). In-use, access-denied and other failures get Retry: a file held by a scanner
   is not a reason to destroy it.
3. **On Windows, read the bin's settings before moving** (`NukeOnDelete`, `MaxCapacity`,
   `NoRecycleFiles`; `repo/trash/win.rs`, decided by the pure `bin_refuses`). With no UI shown,
   the shell deletes for good without saying so. Read them for the volume's mount point
   (`GetVolumePathNameW`), not the drive letter: a volume mounted in a folder has its own.
4. **Remove links without following them,** and re-check parent folders before trashing.
5. **On Linux, check the mount's trash folders before calling the `trash` crate**
   (`repo/trash/xdg.rs`, decided by the pure `volume_trash_usable` in `home.rs`, which picks a
   folder as the crate does). When they can't be used, refuse with `HomeTrashOnly` and ask
   (Move to home Trash, Delete permanently) instead of letting the crate copy the file into the
   home Trash without a word. `repo::home_trash` does the move the user agreed to.
6. **A delete keeps what undo needs first:** the working copy's bytes as a blob (files up to
   64 MB, `undo::MAX_KEPT_BYTES`; not links) and, for Unstage and delete, the staged entry
   (`undo/deleted.rs`; also for Discard of an untracked file, and kept when the delete fails
   after the file went). A permanent delete whose confirmation promised Undo refuses a file over
   the limit (`Error::NoCopyKept`, decided when it runs, so a file that grew meanwhile counts)
   and asks again with an acknowledgement. Undo writes the file back with create-new semantics,
   so it never overwrites a file (or a case twin) that appeared at the path.
7. **Undo of a trashed file takes it back out of the trash through `repo::os_untrash`**
   (`repo/trash/untrash.rs`, called by `repo::untrash_file`), and only an item that was at that
   path and holds exactly the kept bytes (the newest such one; a candidate of another size is
   never read). The path is matched as given and with its parent folder resolved, since the trash
   records either. Windows lists the Recycle Bin with the `trash` crate, links the item's `$R`
   file back with `hard_link` and removes it and its `$I` record; the shell's own restore isn't
   used, since it runs without UI and so would replace a file saved there meanwhile. Linux and
   the BSDs read the crate's trash folders (inside Flatpak also the host's home Trash; the crate's
   list panics on some malformed `.trashinfo`, which is caught as a failure) and link the file
   back with `hard_link`, which never replaces anything, copying it into a new file when the
   trash is on another drive, then remove the trash's copy and its `.trashinfo`. macOS returns
   `Untrashed::Unsupported`. Whenever the trash can't give it back, Undo writes Glowfly's copy and
   says the trash's stays there; a file that came back with other bytes is left alone. The pure
   parts (`same_place`, `newest_ours`, `info_path`, `take_out`) are tested on every runner; the
   real round trip is an ignored test (`GLOWFLY_REAL_TRASH=1 cargo test -p glowfly-core
   real_trash_round_trip -- --ignored`).
8. **A lock file git left behind is deleted only through `lock_file::delete`**, which checks
   again just before it removes anything: the exact path git named (absolute, ending `.lock`, no
   `..`), inside the git dir or common dir once its folder is resolved, still a plain file (not a
   link, a folder or a Windows reparse point), and only while `git_procs::scan` finds no git
   process anywhere on the machine. Which repository a git works in can't be read reliably
   (`git -C`, `GIT_DIR`, a hook's child), so any git counts except `credential-cache--daemon`
   and `fsmonitor--daemon` when their command line can be read and names that subcommand
   (Windows: `NtQueryInformationProcess`; Linux: `/proc/<pid>/cmdline`; macOS:
   `KERN_PROCARGS2`), and every Unknown refuses with its
   reason: an unreadable process list or name (Windows: ToolHelp; Linux: `/proc/<pid>/comm`, a
   process that ended meanwhile is skipped; macOS: `proc_listallpids` and `proc_name`), `/proc`
   mounted with `hidepid`, the Flatpak sandbox (it sees only its own processes) and, on Windows,
   an open WSL session (`wsl.exe`, `wslhost.exe`), whose git isn't a Windows process.

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
   non-zero exit with the first stderr line. Starting a program is not proof it ran. `git
   difftool` gets 10 s (its scripts start slowly on Windows) and is then waited for
   (`watch_to_end`), so a failure that comes later is still reported.
5. **macOS `open -a` takes options only after `--args`, with `-n`;** a running app never sees
   `--args`.
6. **Search PATH like the OS does** (`external_tools.rs`): on Unix the file needs an exec bit;
   on Windows match names in any case by listing the folder and try `PATHEXT`. Never search the
   cwd.
7. **Copies for external tools go only under `external_tools::temp::root()`:** `glowfly-tools` in
   the temporary folder, used on Unix only when it is a real folder this user owns with mode
   0700, else `tool-files` in the cache folder. Copies are written through `persist`, marked
   read-only, and keep the file's name (`names::portable`) so tools can highlight it. They hold
   the stored bytes unless the repository is trusted, when `git cat-file --filters` gives the
   checkout form. A macOS application bundle that Glowfly waits for runs through `open -W`.
   The sessions a Glowfly process made (`<pid>-<hex>`) are removed when its window closes
   (`temp::remove_own_sessions`); a copy a tool still holds open on Windows stays for the
   24-hour sweep.
8. **A downloaded update is opened only after its SHA-256 matches the release's published
   checksum, and only on the user's click** (`app_update::download`, `runner/updates.rs`). The
   file's name comes from the server, so it is checked first (`app_update::asset_name_ok`: 1 to
   128 characters of `A-Za-z0-9._-`, no leading dot) and can't leave the downloads folder; the
   bytes are streamed into a temporary sibling (`persist::write_streamed`) and hashed on the way,
   and renamed to the final name only once the size and checksum match. Cancel, a mismatch or a
   failure removes the temporary file, so no half-written file has the final name. An
   existing file with other content is never replaced (`free_name`: `name (1).msi`). Install and
   restart (`app_update::restart`, `runner/update_restart.rs`) follows these rules:
   - The file is verified again (`read_verified`) before it is run, copied or unpacked.
   - Only a portable copy's program (the resolved `current_exe`) and the doc files already
     beside it are replaced, through `persist::move_aside` and `persist::write_file`, and a
     failure rolls back everything done so far. No target is built from a name in the archive
     (`app_update::archive` takes only fixed names).
   - An AppImage copy's swap target is the file `APPIMAGE` names, and only when the running
     program lies inside `APPDIR` (`app_update::appimage_file`; an `APPIMAGE` inherited by an
     unrelated program is ignored). The download must start like an AppImage (the ELF magic, then
     `AI` and 2 at byte 8) before it replaces that file, which is then started again from the
     same path; the helper is still a copy of the running program.
   - The helper is a copy of the running program in a private staging folder under
     `paths::temp_dir()` (`glowfly-update-<16 hex>`, created fresh with mode 0700; stale ones are
     removed at the next start). The copy is tried first (`app_update::launch::try_run`: `--version`
     must print this build's version and exit successfully within 5 s, else it is killed);
     where it can't run (a temp folder mounted noexec on Linux or macOS) the folder goes beside
     the program, then in the cache folder (`staging::create_runnable`). It is started detached (Windows: a new process group, out of
     the job when allowed; Unix: `setsid`) and waits for Glowfly to exit through its stdin: "go"
     once everything is prepared, then the end of input.
   - The MSI runs only after Glowfly has exited, as
     `%SystemRoot%\System32\msiexec.exe /i <msi> /passive /norestart /l*v <log>` (never searched
     for on PATH), and Glowfly is started again from the same path afterwards.
   - A .dmg, .deb, .rpm, or an MSI for a portable copy is only opened, and Glowfly quits.
   - The next start removes a `.bak` program the helper couldn't delete (a second window still
     ran it on Windows): only `<program>.bak` and `<program>.bak-<n>` beside the running program
     (or its AppImage) that the swap recorded (`<backup>.record`, written when it moved the
     program aside) and that still have the recorded size and SHA-256, so a `.bak` the user made
     is never removed; only regular files, never through a link, and only when the process list
     (`terminal::program_paths`) was read and no process runs it (`app_update::backups`). It also
     reads `update.log` and says when the installer failed or was cancelled, once.
   - The file offered is for the machine's real processor (`app_update::current_platform`):
     Windows asks `IsWow64Process2`, so an x64 build emulated on Arm counts as Arm, and macOS reads
     `sysctl.proc_translated`, 1 under Rosetta. Other systems, and a failed query, use the build's
     processor.
9. **Desktop notifications go only through `runner/notify.rs` (notify-rust):** WinRT toasts on
   Windows, Notification Center on macOS, freedesktop notifications over D-Bus (zbus, no
   libdbus) on Linux. A missing notification server is an error the confirmation reports;
   never call it from update or views (`tests/layering.rs`). A click comes back through the same
   worker thread, which waits for the answer (Linux notes declare the "default" action so a
   click on the body is reported). Glowfly's own app id (`app_identity::APP_ID`) is used on
   Windows only by the installed copy, whose shortcuts register it (an id nothing registers makes
   toasts vanish); on macOS `set_application` runs only inside Glowfly.app.
10. **The built-in terminal's shell comes from `terminal::shell::resolve`, on the runner thread
    only.** Automatic is PowerShell 7, else Windows PowerShell, else `COMSPEC` on Windows, and
    `$SHELL`, else `/bin/sh`, elsewhere. Windows PowerShell and Command Prompt are offered only on
    Windows; loaded from a settings file elsewhere they act as Automatic. Bash on Windows is Git
    Bash, found next to the git on PATH (`<Git>\bin\bash.exe` from `<Git>\cmd`, `<Git>\bin` or
    `<Git>\mingw64\bin`), else `bash.exe` on PATH. A chosen shell that can't be found is reported
    in the panel, never replaced by another. The panel's shell picker (the caret after +) offers
    `ShellChoice::offered` for this OS without probing PATH; a picked shell that is missing fails
    in its own tab rather than being hidden.
11. **Whether a terminal's shell is running a program comes only from
    `terminal::ShellProbe::activity`, off the UI thread** (the process lists are read in
    `terminal/procs.rs`). Closing a terminal tab, Kill, closing its repository's tab, Install
    and restart and quitting all ask it. On Windows it reads the shell's child processes from a
    ToolHelp snapshot, counting only those created at or after their parent (`GetProcessTimes`,
    read only for the pids whose parent pid matches; either time unreadable leaves it out):
    Windows keeps a dead parent's pid, which a new shell may reuse. Console hosts
    (`conhost.exe`, `OpenConsole.exe`) and WSL's `wslhost.exe` are ignored, and a single child
    with the shell's own executable name is a launcher (Git Bash's `bin` `bash.exe` starting the
    one in `usr/bin`, System32's `wsl.exe` starting WSL's) that is followed, up to four levels,
    unless `QueryFullProcessImageNameW` shows both run the same file (PowerShell's `Start-Job`:
    a background job). Any other child counts, background jobs included. A `wsl.exe` with no
    other child is Unknown: what runs inside the distribution isn't a Windows process. On Unix a
    parent pid is never stale (an orphan is reparented to init or a subreaper). It compares the
    PTY's foreground process group with the shell's pid, and at the prompt counts the shell's
    children as background jobs (Linux: from `/proc/<pid>/stat`, in the shell's session and not
    ended; macOS: libproc's `proc_listpids(PROC_PPID_ONLY)`). Linux names the program from
    `/proc/<pid>/comm`, macOS from `proc_name`. Anything that can't be read is Unknown, which
    never prompts.

### Environment and git

1. **Children get PATH only through `env_path::apply`** (or `Command::env`). Never mutate the
   process environment, and never build an env map keyed by `"PATH"`: Windows names ignore
   case, so `Path` and `PATH` would both reach the child.
2. **On macOS, merge in the login shell's PATH** (`env_path::read_login_path`, off the UI thread,
   `LOGIN_SHELL_TIMEOUT` of 3 s), falling back to `env_path::MACOS_FALLBACK`
   (`/opt/homebrew/bin:/usr/local/bin`).
3. **Run the git chosen in Preferences > Git, and check it at startup and on every switch**
   (`git_version::detect`). Every git child is started as `git_program::program()` (the chosen
   absolute path, else `git` from PATH); `GlowflyApp::new` puts the saved choice into effect
   before any git runs, and a switch is probed off the UI thread (`git_program::probe`:
   `--version` with a 10 s timeout, `NO_PROMPTS`, `CREATE_NO_WINDOW`, outside any repository)
   before `git_program::set`. Launchers (`git-bash`, `git-cmd`, `git-gui`, `gitk`) are refused
   by name without being run. The PATH is never changed for the choice: the built-in terminal
   and external tools keep the user's PATH git. Discovery lists every git on PATH plus
   well-known install folders, one row per file (`paths::resolve`) in the spelling found first;
   on macOS `/usr/bin/git` is only listed when the developer tools exist, since Apple's shim
   would open their install dialog. Show which git runs and its version (Preferences > General
   and > Git), gate features by `git_version::Feature` (`RangeDiff` 2.19, `Restore` 2.23, `ForceIfIncludes` 2.30,
   `SparseCheckout` 2.35, `MergeTree` 2.38), re-detect them when the git changes, and show
   `GitInfo::banner` with the reasons.
4. **Read git config booleans with `--type=bool`** (`runner/config.rs`): `yes`, `1` and `TRUE`
   are true. `core.autocrlf` is three-way (`true`, `input`, `false`), and a value that can't be
   read is never rewritten.
5. **For untranslated git messages set `LANGUAGE=C` and `LC_MESSAGES=C`, never `LC_ALL=C`**
   (`gitcli.rs`), which would also give hooks, gpg and pinentry an ASCII-only character set.
6. **On Unix, start every git (and ssh-add) child in a new session (`ssh_version::prepare`)**,
   so ssh has no terminal and asks the helper (`SSH_ASKPASS` with `SSH_ASKPASS_REQUIRE=force`).
   It also leads its own process group, which `process_tree.rs` stops as a whole. With OpenSSH
   before 8.4, which ignores `SSH_ASKPASS_REQUIRE`, also set `DISPLAY` when unset
   (`gitcli::askpass_vars_in`). Which ssh git runs follows git (`ssh_version::ssh_command_for`:
   `GIT_SSH_COMMAND`, then the repository's `core.sshCommand`, read from its config files, then
   `GIT_SSH`, then `ssh`). Run only a program the user's environment names (`GIT_SSH`, or `ssh`
   from PATH) with `-V` to read its version. **Never run a command line (`GIT_SSH_COMMAND`,
   `core.sshCommand`) to probe it**: a repository's config can name any program, and Glowfly
   reads untrusted repositories with git before the trust prompt. A command line gets the safe
   set-up, as an unknown version does. Never log the command.
7. **A path written into a git config value that a shell runs (`core.sshCommand`) goes through
   `native_path::native` and `rebase::shell_quote_for`** (`ssh_keys/command.rs`): no `\\?\`
   prefix, sh-quoted, and forward slashes on Windows. Git for Windows' bundled ssh doesn't read
   keys from the Windows OpenSSH agent service; only Windows' own `ssh.exe` does. Glowfly's value
   for that one (`ssh_keys/windows_ssh.rs`) is the program alone, resolved from `SystemRoot`
   (`paths::resolve`, so it has the on-disk spelling), written the same way
   (`'C:/Windows/System32/OpenSSH/ssh.exe'`), and recognised only by that exact spelling (a drive,
   then `System32/OpenSSH/ssh.exe`). Paths read from an ssh config are text that may have been
   written for another OS: decide whether one is a full path with `config_expand::absolute_for`
   for the OS ssh runs on, convert Git for Windows' MSYS drive form (`/c/Users/…`) only on Windows,
   and never use `Path::is_absolute` on such text.
8. **The gpg.program wrapper runs the gpg git would have run** (`gpg_prompt/`): the last
   `gpg.program` / `gpg.openpgp.program` from any scope (system, global, repository, worktree)
   that isn't Glowfly, else `gpg` on the PATH git passes on (on Windows that is Git for Windows'
   bundled gpg or a Gpg4win install, whichever git itself would find), read through the git in
   `GLOWFLY_GIT_PROGRAM` (the one that ran the wrapper). It keeps git's PATH (no
   `env_path::apply` in the wrapper), starts its gpg children with `CREATE_NO_WINDOW`, and feeds
   the passphrase on fd 0 (`--passphrase-fd 0`), never in argv, a file or the environment. It
   needs git 2.31 (`GIT_CONFIG_COUNT`) and GnuPG 2.1.12 (loopback allowed by default); with
   anything older, or an agent with `no-allow-loopback-pinentry`, GnuPG's own pinentry is used.
   On Windows a gpg.program in MSYS form (`/usr/bin/gpg`, `/c/…`, `/cygdrive/c/…`) is mapped
   into the install found from `GIT_EXEC_PATH` (git exports it to the wrapper; else
   `git --exec-path`): `<root>/<msystem>/libexec/git-core` or `<root>/usr/lib/git-core`, with
   `/bin` read as `/usr/bin` (`gpg_prompt/program.rs`). For `gpg.format=ssh`, git runs
   ssh-keygen directly; it asks through `SSH_ASKPASS`, and gitcli adds `GLOWFLY_SSH_SIGNING_KEY`
   (the user.signingKey file) for commands that may sign (`gpg_prompt/ssh_env.rs`).
9. **The UI language comes only from `i18n::detect_system_language`**, called once in `main.rs`
   before the window opens. On Windows and macOS it is sys-locale's ordered list (the user's
   preferred UI languages / the preferred languages). On Linux and other Unix it follows
   gettext's rule through `i18n::message_locales`: `LANGUAGE`'s list ahead of the first
   non-empty `LC_ALL`, `LC_MESSAGES`, `LANG`, with `LANGUAGE` ignored when that locale is
   `C`/`POSIX` (including `C.UTF-8`) or unset. The first supported language wins, and
   `C`/`POSIX` first or nothing supported means English. Nothing else reads these variables for
   it, and it is never read again while Glowfly runs. It is independent of the `LANGUAGE=C`
   that `gitcli` gives git children (rule 5).
10. **`GLOWFLY_UPDATE_FEED` replaces the release feed with a local folder** (`app_update::Feed`).
    Unset or empty means the hosting service over https, as always. Set, it must be an absolute
    path (`Path::is_absolute`, so `/feed` counts as relative on Windows, which needs a drive or
    UNC prefix); a relative value is an error, never resolved against the working folder. The
    folder holds `release.json` in the hosting service's release shape next to the files it
    lists. Listed files are read by their checked names (`asset_name_ok`), with their size from
    disk; a listed file that is absent or a folder is dropped. No `release.json` means no
    release yet; a missing folder or bad JSON is a failed check. Checksums still come from the
    folder's `SHA256SUMS` or `<file>.sha256` and are always enforced, and nothing reaches the
    network. The variable is read on every check, by the app's checks and downloads and by
    `glowfly --check-update`.
11. **Release builds on Windows are GUI-subsystem programs** (`windows_subsystem = "windows"`),
    so `glowfly --version` and `glowfly --check-update` print only to a caller that redirects
    their output (`Start-Process -RedirectStandardOutput … -Wait -PassThru`); a terminal shows
    nothing. Both modes run before logging, settings and the smoke probe, and write no file of
    their own.
12. **Stop a git child only through `gitcli::output_until`** (with a `gitcli::Stop`). It starts
    git so its whole process tree can be ended (`process_tree.rs`): on Unix in a new session,
    then SIGTERM to the process group, 3 s grace, then SIGKILL; on Windows in a job object that
    is terminated, since killing git.exe alone leaves its remote helpers and ssh running. Never
    `Child::kill` a git that may have started children. After a stop, remove only the lock and
    temporary files the run made (`stop_cleanup`), never ones that were there before it.
13. **Profile folder rules are `includeIf.gitdir` entries built only by
    `profile_includes::gitdir_pattern`**: forward slashes on Windows (a UNC share as
    `//server/share/`, and only there: on Unix a backslash is a name character and is escaped),
    the glob characters `* ? [ ]` escaped, a trailing `/` so the folder and everything in it
    match, and `gitdir/i` chosen by probing the folder in the other case (`check_folder`), never
    by OS. Folders are resolved (`paths::resolve`) and must be valid text without control
    characters. Linked worktrees and submodules match by their real git dir
    (`<main>/.git/worktrees/<name>`, `<super>/.git/modules/<name>`), so Use for this repository
    adds the common git dir. Which identity applies, and from where, comes from git
    (`repo_identity::read`, `--show-origin`), not from Glowfly's own matching.

### Key labels

1. **UI text names modifiers only through `Chord::text`/`Chord::parts` (`keymap::KeyLabels`),
   never with a literal "Ctrl" or "Alt":** macOS reads "Cmd" for the command modifier and
   "Option" for Alt ("Cmd+Option+1"); the real Ctrl key reads "Ctrl" everywhere. The choice is
   the pure `KeyLabels::native()`, and `text_in`/`parts_in` take a style, so a test checks both
   styles on every OS. Never paint the ⌘ ⌥ ⇧ glyphs: the bundled fonts lack them.

### The built-in terminal's keys and selection

1. **Alt is Meta through `TerminalPrefs::alt_is_meta`:** on Windows and Linux always, on macOS
   only with Option as Meta (`alt_is_meta_for` takes the system, so each case is tested
   everywhere). Alt with a letter, digit or punctuation key then sends ESC and a character
   (`ui/terminal/input.rs`): the one the press typed when its text comes in the same frame, so
   the keyboard layout decides Alt+_ or Alt+/, else the key's own with Shift as on a US keyboard
   (macOS, where Option types a composed character instead). Ctrl+Alt (AltGr on Windows) types
   its character, and Cmd chords stay the app's. On Windows a digit, +, - or / sends ESC only
   when its text comes (`Opts::alt_codes`): egui reports the numpad's keys as the main row's,
   and a numpad press while Alt is held is part of an Alt code (Alt+0233), which types no text
   until Alt is released.
2. **PRIMARY only through `platform::primary_selection`, from the shell:** a finished mouse
   selection, and Select all, become PRIMARY (`UiEffect::SetPrimary`, nothing where there is
   none), and a middle click (Linux, with Middle-click paste) asks for it
   (`UiEffect::PastePrimary`). A Wayland window uses the compositor's primary-selection protocol
   (smithay-clipboard on the window's own `wl_display`, let go in `on_exit` before the window
   closes), an X11 window the PRIMARY atom (arboard): the crates egui-winit already uses for the
   clipboard. When neither opens, a middle click pastes the clipboard (`middle_click_reads`).

### Links and the running window

1. **Register the `glowfly` scheme for this user only:** `HKEY_CURRENT_USER\Software\Classes\glowfly`
   on Windows, never HKLM; on XDG desktops `glowfly-links.desktop` in `$XDG_DATA_HOME/applications`
   (else `~/.local/share/applications`), made the default with `xdg-mime` (run with
   `env_path::apply`). The Windows command quotes the program and `"%1"` through
   `native_path::native` (`url_scheme::windows_command`: no `\\?\` prefix); a desktop entry's
   `Exec` argument is quoted by `url_scheme::desktop_exec_arg` (reserved characters, `%%`, and the
   desktop-file string escaping of `\`).
2. **The instance file is private:** it is written with `persist::WriteOptions { private: true }`
   (mode 0600, and 0700 for a folder the write creates, on Unix) under `settings::runtime_dir`
   (`$XDG_RUNTIME_DIR/glowfly` on Linux, `$XDG_RUNTIME_DIR/app/<FLATPAK_ID>/glowfly` inside
   Flatpak, else the cache folder). A file left by a window that
   crashed names a closed port, which the second process treats as no window. Only a process
   that can read the file has the token the listener asks for, so only this user is heard.
   Every request is one line of at most 16 KiB holding a `glowfly:` link or an absolute folder,
   which opens like a folder picked in the app; nothing in it is run.
3. **One window takes links and folders: the one holding the OS file lock on `instance.lock`**
   next to the instance file (`File::try_lock`: `LockFileEx` on Windows, `flock` on Unix), taken
   at start-up (`instance::claim`). The OS drops the lock with its process, so a crash never
   leaves a stale one; a later window waits for it on a thread and takes over. The lock file
   is never removed or replaced (a lock belongs to the inode on Unix, so a new file would let a
   second window lock too). A process that finds the lock held keeps trying the window for
   `START_WAIT` (it may still be starting), then opens its own window; so does one on a file
   system without locks after a single try.
4. **Windows refuses the foreground to a window its user didn't just use:** the process handing a
   link or folder over calls `AllowSetForegroundWindow` with the running window's process id first
   (`instance/foreground.rs`); the window then un-minimizes, focuses and, if that still fails,
   flashes its taskbar entry.
5. **A link names its repository by root commit and remote URL, never by the folder it is in
   here,** and opening one runs no git command until the user clicks Fetch or Clone; the
   repository is found with libgit2 reads only.
6. **macOS hands links and folders to Glowfly.app as Apple events,** never on the command line.
   packaging/macos/Info.plist declares both schemes in `CFBundleURLTypes` (Get URL events) and
   `public.folder` in `CFBundleDocumentTypes` as an Alternate-rank Viewer (Open Documents events:
   Finder's Open With, a Dock drop, `open -a Glowfly <folder>`), never with an Editor role or an
   `NSDocumentClass`, so Finder keeps opening folders itself and AppKit never makes an untitled
   document. `platform/macos_events.rs` installs its handlers before eframe starts and again at
   `NSApplicationWillFinishLaunchingNotification`, since AppKit installs its own Open Documents
   handler while launching and dispatches the launch event right after. Items wait in
   `platform/open_events.rs` until `Runner::start_open_events` attaches, then become the same
   messages as the command line (`StartArg::into_msg`); an event repeating the command line's
   item within 10 s of startup (`paths::same_spelling`, links as exact text) is dropped once. An
   Open Documents item is a `file:` URL, read with `paths::from_file_url`. A `-psn_...` argument
   (Finder's process serial number) is ignored on macOS. LaunchServices registers the bundle
   when it is copied into /Applications or first opened (`lsregister -f` registers a hand-built
   one); an unbundled binary has no Info.plist and gets no events.

### Flatpak sandbox

1. **Detect the sandbox only with `flatpak::in_flatpak`** (the pure `in_flatpak_with` in tests):
   `/.flatpak-info`, or a valid `FLATPAK_ID` with the program under `/app`. A `FLATPAK_ID` alone
   can be inherited by a host program, and one holding `/`, `..`, a leading `.` or other bad
   characters is ignored (`flatpak::app_id_ok`). It is never true outside Linux.
2. **Locations shared with the host go through `flatpak::host_var` or
   `flatpak::child_config_home`:** inside Flatpak the XDG variables point into
   `~/.var/app/<id>/`, so the home Trash (`repo/trash`) and git's XDG config, ignore and
   attributes files (children get `XDG_CONFIG_HOME` through `env_path::apply`; libgit2's XDG
   search path is set by `flatpak::init` at start) use the host's `HOST_XDG_*` values or the
   XDG default under `HOME`. Glowfly's own settings, cache and logs stay in the sandbox's folders,
   separate from a host install.
3. **The runtime folder is `$XDG_RUNTIME_DIR/app/<id>/glowfly`** (`settings::paths::runtime_dir_for`),
   the part every instance of the app shares, so a link opened in a second launch finds the
   running window; the rest of `/run/user/<uid>` is per instance.
4. **The window's identity comes from `app_identity::desktop_entry()`:** the Wayland app id, the
   X11 class, the notification icon and its desktop-entry hint are `io.github.bg9m9r.glowfly`
   inside Flatpak (the exported desktop file's name), else `glowfly`.
5. **Never self-register links inside Flatpak:** the exported
   `io.github.bg9m9r.glowfly.links.desktop` declares both schemes, so the scheme effects answer
   `SchemeStatus::Unsupported(url_scheme::FLATPAK_NOTE)` without touching `xdg-mime` or the
   applications folder (`runner/deep_link.rs`). Updates are only announced (`InstallKind::Flatpak`).
6. **Programs Glowfly starts run in the sandbox:** the bundled git (`/app/bin/git`, first on
   PATH) and the runtime's ssh, with the host's ssh-agent and gpg-agent sockets. `/tmp` is the
   sandbox's own, so a path handed to or from the host (an update feed, `--check-update`'s
   folder) must be elsewhere.

### Window frame

1. **The frame follows `Chrome::for_os`, tested for every OS:** Windows gets `Chrome::Windows`
   (Glowfly draws the title bar, window buttons, resize edges and border), macOS `Chrome::MacOs`
   (a full-size content view; the title bar leaves `TRAFFIC_LIGHTS_W` for the system's buttons
   and draws no window buttons or edges), and everything else `Chrome::System` (the system frame,
   the menu bar and the tab bar). Ask the `Chrome` value what to draw (`drawn`, `window_buttons`,
   `resize_edges`, `alt_opens_menu`, `leading_inset`), never `cfg!(target_os)` in a view.
2. **Views never send viewport commands:** a drag, double-click, window button or resize edge
   pushes `AppMsg::Window`, and only the shell (`ui/app.rs`) turns it into a
   `ViewportCommand`; `tests/layering.rs` checks.
3. **Resize edges are Windows-only and off while maximized or full screen,** like the 1 px
   border; on macOS the system frame resizes the window.
4. **The drawn Maximize button answers Windows' hit test (`WM_NCHITTEST`) with `HTMAXBUTTON`,**
   so Windows 11 shows Snap Layouts over it. Windows then tracks the button and egui sees no
   pointer there: its hover and press come from `st.view.window.max_button`, and its click
   reaches update from the shell (`ui/app_native.rs`) as `AppMsg::Window(SetMaximized)`. The
   shell hands the OS the button's rect each frame (`window_frame::caption_geometry`).
5. **Alt+Space and a right-click on the empty bar open the window's system menu** through
   `platform::native_window` (the only place with Win32 calls; `tests/layering.rs` checks).
   egui never sees Alt+Space, and the shell cancels the Alt tap once the menu has shown. The
   chosen command goes back as `WM_SYSCOMMAND`, so Windows carries it out.
6. **Mica only on build 22621 or later, with transparency effects on and high contrast off,**
   and only behind the title bar, toolbar and sidebar, painted through `ui::backdrop::fill`.
   Everything else stays opaque. The tints come from `backdrop::mica_tint`'s ladder, which keeps
   text readable over stand-in Mica colours; a surface that can't reach the floors at any step
   stays opaque, and high-contrast themes stay solid.
7. **Full screen is F11, except on macOS** (Ctrl+Cmd+F there, since F11 shows the desktop),
   chosen with `cfg!` in `keymap/table.rs` so both branches compile everywhere. It is the
   window's state, never saved: `LayoutMsg::ToggleFullscreen` sends `WindowCmd::SetFullscreen`
   from `st.view.window.fullscreen`, which the shell reads each frame, so full screen entered
   from the system (the green button) is seen too. While full screen, `WindowCmd::in_window`
   drops moves and resizes and turns Maximize or Restore into leaving full screen, the middle
   window button reads Restore, and `caption_geometry` leaves out the Maximize rect (no Snap
   Layouts). View > Full screen stays reachable on every OS.

### Menus

1. **macOS shows Glowfly's menus in the system menu bar** (`Chrome::native_menu`), planned by
   `state/menu_native.rs` from the same `state/menu_model.rs` the drawn menus come from, with
   winit's default app menu turned off (`native_menu::event_loop_hook`). No AppKit item ends the
   app without a close request: the app menu's Quit sends `AppMsg::Quit`, which asks first while
   a git operation or sync runs.
2. **The native menus show the keymap's keys but never act on a key.** Every menu's delegate
   answers `menuHasKeyEquivalent:forEvent:target:action:` with NO, so each press reaches the
   window and the keymap, whose context rules and text fields decide (Cmd+Z in the commit message
   undoes text; Preferences > Keyboard can record Cmd+Q). Only Cmd+H and Option+Cmd+H go to
   AppKit's Hide and Hide Others, so a Glowfly binding on those keys can't fire on macOS.
3. **A click is queued by tag** (`platform/native_menu.rs`, tag = generation << 32 | item index)
   and becomes the drawn item's action on the next frame. The menus are updated in place while
   only titles, states and keys change; a change of rows builds them anew under a new
   generation, and clicks on the old menus are dropped.
4. **The in-window menu bar and its Show menu bar setting exist only with the system frame**
   (`Chrome::drawn` hides them); a drawn title bar opens the same menus from its menu button.
5. **Known gap:** the Dock menu's Quit and logging out end the app through AppKit's `terminate:`
   without the quit question (winit's application delegate owns `applicationShouldTerminate:`).

### Watching and refresh

1. **Don't follow links** (`notify::Config::with_follow_symlinks(false)` in `watcher.rs`): a link
   to `$HOME` exhausts inotify's watch limit.
2. **For a linked worktree, also watch the common dir** (`watch::common_dir`): its `refs/`
   recursively, `HEAD` and `packed-refs`. Watch the folder (not recursively) of each file the
   config includes outside the watched folders (`config_includes`, `watcher/includes.rs`), and
   follow the includes again after every full reload.
3. **Assume the watcher can be silent.** Detect remote and virtual file systems with
   `volume::remote_kind` (statfs magic on Linux, `MNT_LOCAL` on macOS, `DRIVE_REMOTE` on
   Windows), tell the user once, and read the whole status every `REMOTE_POLL` (10 s) while
   focused. On Windows poll every `WINDOWS_POLL` (30 s) while focused (`state/watch.rs`,
   `poll_every`): a local repository's watcher sweeps its tree (`watch_sweep`: every folder
   stat'd, the changed and racy ones and a rotating tenth listed again, the git dir's refs, index,
   exclude and attributes files and the config files with those they include compared, and
   submodules' and nested repositories' work trees walked by their own ignore rules with their
   HEAD and refs compared) and feeds what changed through the usual
   classification; a failed watcher, a tree past `SWEEP_MAX_ENTRIES` and the first sweep read the
   whole status. Keep the refresh on focus and F5.
4. **Never trust size plus mtime alone.** Hash files up to 64 KiB, and treat a stamp taken
   within `refs_key::RACY_WINDOW` (2 s) of the modification time as untrustworthy
   (`refs_key::stamp_trustworthy`): FAT has 2 s timestamps and HFS+ 1 s.
5. **Match the watcher's work-tree paths the way git does** when only they are re-read
   (`repo::status_of_paths`, `repo::covers`, `repo::merge_unstaged`): ignoring case only under
   `core.ignorecase` (the watcher reports the name on disk, the index may spell it differently;
   git's pathspecs stay case-sensitive under it, so git is also given the index's spelling),
   NFC-normalized under `core.precomposeunicode` (as git composes its own arguments on macOS),
   and literally (`--literal-pathspecs`, libgit2's path list), never as patterns. A name that
   isn't UTF-8, or any sign of lost events (a notify error, a rescan flag), reads the whole
   status. Keep a git command line's paths under `CLI_PATH_BYTES` (16 KiB), well inside
   Windows' command-line limit.
6. **Git's built-in file system monitor runs only on Windows and macOS** (git 2.36 or newer,
   `repo::monitor_offer_for`). Set core.fsmonitor only to `true`, and only after
   `git fsmonitor--daemon status` says it is watching (`repo::turn_on_monitor`). Never set
   fsmonitor.allowRemote or fsmonitor.socketDir for the user, nor a hook path (the trust check
   flags one). On Linux, suggest Watchman with git's fsmonitor-watchman hook in text only.
   Offer it only to a git whose `git version --build-options` names `fsmonitor--daemon`
   (`repo::daemon_built`). On Windows the daemon holds its working directory open: stop it
   (`repo::stop_monitor`) before removing or moving a worktree. Turning it off unsets
   core.fsmonitor (every local value: `--unset-all`) before stopping the daemon, so no git
   started meanwhile starts it again.
7. **libgit2's index writes drop the extensions it doesn't know** (the untracked cache, the
   monitor's token). Don't copy them back into a rewritten index; a trusted whole status read
   lets git record them again (`repo::index_ext`), the token only where a daemon can give one
   (`repo::daemon_serves`), and once per index state (`repo::RecordMemo`), so a record that
   brings nothing back isn't repeated on every refresh.

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
   `settings::home_dir`, which ignores an empty or relative `HOME`. The search index store
   (`commit_index::Store`, `search-index`) is a cache too: private files written through
   `persist::write_streamed`, bounded by age and total size (`Store::prune`).
6. **Put logs in `settings::log_dir`** (`%LOCALAPPDATA%\Glowfly\logs`, `~/Library/Logs/Glowfly`,
   `$XDG_STATE_HOME/glowfly/logs`): local, never the roaming config or cache folder. Append
   through `persist::append` / `persist::open_append`, and keep every log bounded: trim it
   (the activity log, `logs::ActivityFile`) or rotate it by size (the error and performance
   logs, `logs::RotatingFile`). With no folder, logs stay in memory.
7. **Save downloaded updates to `settings::downloads_dir`:** the Downloads known folder on
   Windows (`SHGetKnownFolderPath`, wherever the user moved it; else `%USERPROFILE%\Downloads`),
   `~/Downloads` on macOS, and `XDG_DOWNLOAD_DIR` from `user-dirs.dirs` (else `~/Downloads`) on
   XDG desktops; when that isn't a folder, the cache folder's `updates`.
8. **Per-repository records go in the repository's own git dir**, never the config folder: the
   operation journal's notes are `<git dir>/glowfly/journal.jsonl` (`journal::notes::path`, a
   linked worktree's own git dir, since HEAD is per worktree), appended through
   `persist::append` and trimmed through `persist::atomic_write`, bounded like a log.
12. **Custom shell commands run under POSIX sh on every OS, found by `custom_commands::find_sh`
    on the runner thread only:** `/bin/sh` on Unix; on Windows the `sh.exe` beside the git in use
    (`<Git>\bin` or `<Git>\usr\bin`, from `<Git>\cmd`, `<Git>\bin` or `<Git>\mingw64\bin`),
    else `sh.exe` on PATH, else a clear error (never cmd.exe or PowerShell). The script goes in
    the `GLOWFLY_COMMAND` variable with argv `-c 'eval "$GLOWFLY_COMMAND" 2>&1'`, because the
    MSYS runtime re-parses a command line with its own quoting and globbing. Placeholder values
    are single-quoted (`shell_quote_for`), `{repo}` with forward slashes on Windows; git
    commands get each value as one argument. The child runs with `CREATE_NO_WINDOW`, stdin
    closed, `GIT_EDITOR` set to `true` and credential prompts sent to Glowfly's dialog, so
    nothing waits on a terminal. git config's diff and merge tool commands (`difftool.<name>.cmd`,
    `mergetool.<name>.cmd`) run the same way (`external_tools/shell_tool.rs`): `find_sh`, the
    command in `GLOWFLY_COMMAND` (argv `-c 'eval "$GLOWFLY_COMMAND"'`, stderr kept apart), and
    `LOCAL`, `REMOTE`, `BASE` and `MERGED` in the environment, with native paths for the copies
    and the repo-relative path for `MERGED`, in the repository's top folder. Which tool runs is
    read by `external_tools/git_config.rs` on the runner thread, through `gitcli` (`git config -z
    --get-regexp`, so values may hold newlines), with a built-in name checked against the
    `mergetools` folder under `git --exec-path`, never a hard-coded install path. A built-in
    merge tool runs as `git --literal-pathspecs mergetool -- <path>`, and a path holding `*`,
    `?`, `[` or `\` is refused, because git-mergetool's own sh script expands those as a pattern
    against the top folder's files and would merge another file.

## Testing on every OS

CI runs the same lint and tests on Windows, macOS and Linux. Most platform bugs are caught by
making the decision a pure function and testing it for every OS on every machine.

- **Pure `_for(..., os)` table tests.** Put each platform decision in a pure function that takes
  the OS, a `windows: bool`, error codes or volume facts as parameters
  (`check_new_path_for`, `check_branch_name_for`, `shell_quote_for`, `config_dir_for`,
  `opens_as_program` with `opens::Os`, `volume::linux_remote_kind`, `trash::bin_refuses`,
  `trash::volume_trash_usable`, `home::move_into` with an injected rename, `untrash::newest_ours`,
  `untrash::recycle_record` and `same_place` with `windows: bool`,
  `git_version::missing_for`, `poll_every`, `custom_commands::sh_for`, `repo::monitor_offer_for`,
  `app_update::install_method_for`, `swap::plan` with an injected `exists`) and test every OS's
  rows on every runner. The
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
- **Case-sensitive volumes.** Tests that need one read `GLOWFLY_TEST_CS_DIR` and skip with a
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
- **Installers** are tested by the `Installers` workflow (`.github/workflows/installers.yml`) on
  every push and pull request: the MSI on Windows (`scripts/installer-check-windows.ps1`) and the
  .deb on Linux (`scripts/installer-check-linux.sh`) are installed over an older package,
  checked (files, shortcuts or desktop entries, registry, icons), started, update-checked with
  `scripts/update-feed-check.ps1` against local feeds, and uninstalled. The install scripts
  refuse to run outside GitHub Actions unless given `-AllowLocal` / `--allow-local`, and always
  refuse when Glowfly is already installed, so they never replace or remove a developer's own
  copy. `update-feed-check.ps1` installs nothing and opens no window, so it runs anywhere.

## Known gaps and things only a real machine can confirm

Gaps that are accepted for now:

- **Names that aren't UTF-8 are refused, not carried.** Action APIs still take `&str`, so such
  rows are shown with `\xHH` and every action on them is refused, rather than passing their bytes
  through to git.
- **No Recycle Bin check without a volume GUID** (UNC paths, redirected folders): the bin
  settings can't be read there.
- **Replacing a file doesn't keep its owner, ACLs, xattrs or hard links;** only permissions and
  the read-only attribute carry over.
- **Undo can't take a file back out of the macOS Trash:** the Trash keeps no record of where a
  file came from that Glowfly can read, so Undo writes its own copy and the Trash keeps one too.
- **Taking a file out of the Recycle Bin leaves the bin's own index to Windows:** the `$R` file
  is linked back and it and its `$I` record removed directly, never through the shell (which
  would replace a file saved there meanwhile; without hard links, as on FAT, it is copied into a
  new file). An open Recycle Bin window may go on showing the item until it is refreshed.
- **Linux trash across devices is slow, but only when the user picks Move to home Trash:** the
  file is copied synchronously into the home Trash, which takes a while for large files on
  network mounts.
- **Windows file watching can drop events** without an error (`notify` sends no rescan on
  overflow); the 30 s poll's sweep finds files created, removed or renamed at once, but a
  content-only edit lost that way shows only within about 5 minutes (when its folder's turn in
  the rotation comes), or on F5.
- **Some desktop notifications still post under another app's identity:** a portable Windows
  copy's are attributed to PowerShell, and an unbundled macOS binary's (cargo run) to another
  app, through the deprecated NSUserNotification API. A Windows toast clicked in the
  notification centre after its popup closed does nothing, and on Wayland a click may only flash
  the window instead of raising it.
- **Glowfly started with `GIT_CONFIG_COUNT` already set keeps GnuPG's own passphrase window:**
  the user's command-scope config is left alone rather than appended to.
- **Windows 11's modern context menu has no "Open in Glowfly":** that menu needs an
  `IExplorerCommand` in a packaged (sparse) app; the MSI's entry is in the classic menu, under
  "Show more options".
- **Wayland compositors may refuse to raise the window** for a link from another process; it
  then only asks for attention.
- **`xdg-mime` can't unset a default:** Unregister removes the desktop entry, and desktops ignore
  a default whose file is gone.
- **Keychain-backed pinentries (pinentry-mac and the like) are bypassed** while "Ask for the
  key's passphrase in Glowfly" is on, since loopback skips the pinentry.
- **Detecting a terminal's running program is approximate:** a program running inside WSL (a
  `wsl.exe` shell) can't be seen from Windows, so it never asks; a disowned job or a prompt
  helper the shell keeps as a child (Unix) counts as a running program; macOS doesn't tell
  ended children from running ones until the shell reaps them; and on Windows the orphans of a
  process whose pid was reused for the shell could count.

- **The installed MSI copy's window isn't started on the Windows runner:** it has only Windows'
  software OpenGL 1.1, and the renderer needs OpenGL 2 or later. The installer check runs the
  installed program's `--version` and `--check-update` instead; Linux starts the window.
- **The drawn Windows frame has no DWM shadow or rounded corners.**
- **The system menu is drawn by Windows in its light style** under a dark theme.
- **Dark's sidebar and toolbar stay opaque on Mica:** the side surface's `faint` text can't reach
  4.5:1 over the lightest dark Mica stand-ins at any tint step (the title bar does sit on Mica).
- **macOS's traffic lights aren't centred on the title bar,** which is taller than the system's.
- **An Alt press and release within one frame isn't seen as a tap,** so it doesn't open the menu.
- **Option as Meta keeps Option's dead keys:** Option+E, U, I, N and ` still start a composition in
  macOS's input system, so the next letter can come out accented; and Option+Shift with
  punctuation follows a US keyboard's positions, since egui reports the key, not the layout's
  character without Option.
- **PRIMARY is read on the UI thread,** as egui-winit reads the clipboard. On X11 a selection
  owner that doesn't answer holds the frame until arboard's timeout (about 4 s); on Wayland
  smithay-clipboard's `load_primary` has no timeout, so an owner that never closes its pipe
  hangs the window until it does.
- **A Wayland compositor without the primary-selection protocol pastes nothing on a middle
  click,** instead of the clipboard: smithay-clipboard opens either way, so the fallback isn't
  taken.
- **Inside Flatpak, host programs are out of reach:** editors, diff and merge tools, terminal
  shells and hook interpreters that only the host has aren't found in the sandbox.
- **Inside Flatpak, a folder handed over by file forwarding may arrive as a
  `/run/user/<uid>/doc/...` path** (the Documents portal) rather than its host path.
- **Whether the Flatpak runtime has gpg** for commit signing is unconfirmed.

Unverified, needs a real machine:

- the terminal and Explorer for repositories on UNC shares and `\\wsl$`;
- `safe.directory` on a real share (the `%(prefix)///server/share` spelling);
- the PATH of an app started from the macOS Dock, and `open -n -a ... --args`;
- remote-volume detection on real NFS, SMB and 9P mounts;
- `DISPLAY` with OpenSSH before 8.4, and setsid with every OpenSSH;
- whether `\\wsl$\X` and `\\wsl.localhost\X` give one `FolderId`;
- trash for paths over 260 characters on Windows (expected: `NoTrash` and a permanent-delete
  offer);
- whether a minimized window handles results before it is restored, on each OS (if not, they
  land while it is focused and only toast);
- desktop notifications while Focus Assist or Do Not Disturb is on (expected: hidden, no
  error).
- clicking a desktop notification on each OS (expected: the window comes forward on the note's
  repository), and that the MSI install and Glowfly.app show Glowfly's name and icon on them.
- signing passphrases through Gpg4win and through Git for Windows' bundled gpg on a real
  machine (the codes `gpg_prompt/status.rs` reads come from GnuPG's documentation);
- smartcard PINs through loopback pinentry.
- the drawn Windows frame on Windows 10 and 11 at 100% and 150%: dragging the title bar,
  double-clicking it, the resize edges and Aero Snap;
- that the key after an Alt tap isn't swallowed by a system menu loop on Windows;
- Mica showing behind the title bar, toolbar and sidebar on Windows 11 22H2, 23H2 and 24H2 at
  100% and 150%, with no system caption buttons showing through, and the glow renderer's
  transparency on Intel, AMD and NVIDIA drivers;
- the Snap Layouts flyout over the drawn Maximize button, and its hover, press and click;
- where Alt+Space opens the system menu, and its keyboard Move and Size;
- the `MICA_DARK` and `MICA_LIGHT` stand-ins against a colour picker on the real backdrop over
  light, dark and busy wallpapers;
- turning transparency effects or high contrast on and off while Glowfly runs;
- on macOS, the system titlebar strip's own drag and double-click against Glowfly's.
- on macOS, a link that cold-starts Glowfly.app, Finder's Open With on a folder, a folder dropped
  on the Dock icon, and `open -a Glowfly <folder>`.
