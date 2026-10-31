# Data flow: dialogs, toasts, layout, window, menus, prompts, logs, links and updates

Part of [ARCHITECTURE.md](../ARCHITECTURE.md), which has the Msg / update / Effect / runner loop
these follow. One line per topic: when you change how a flow works, update its lines here. The
repository flows are in [flow-repo.md](flow-repo.md).

## Dialogs

- `Modal::kind` (`state/dialog_kind.rs`) sorts each dialog into a form, information or confirm dialog.
- `ui/modals.rs` reports a backdrop click, Esc or the header's close button as `DialogMsg::Dismiss`; the close button acts as Esc.
- `update/dialog.rs` closes information and confirm dialogs, ignores the backdrop for forms, and on Esc compares the form's `Modal::draft` with the one `DialogGuard` recorded when it opened (after every update).
- When they differ it asks "Discard your changes?" (`ui/dialog_guard.rs`), whose default button is Keep editing.
- A question that opens over an edited form (the quit question, a tab close's or a restart's confirmation, the trust prompt) goes through `state/dialog_guard.rs::open_over`, which keeps the form underneath (`ui/modals.rs` draws it beneath, dimmed and without the keyboard); `restore` (after every update) brings it back, still counted as edited, once the question closes.
- Enter runs a dialog's default button: its primary, or Cancel or Close when it has none or its tone is danger, so Enter never presses a danger button.
- Enter in a single-line field runs the primary on the first press: the frame sees the field give up the focus, so a field that takes the focus back when nothing has it skips the frame it lost it.
- `Modal::ConfirmDanger` (`state/confirm.rs`: `Modal::danger`, `with_ack`, `confirm_ready`) is the confirmation for a step that loses work.
- With an acknowledgement, its danger button stays disabled until the box is ticked, and `update` refuses to run it before then.
- `Modal::ConfirmDetails` asks in plain words and keeps the literal values (a command, a config value) under a collapsed Details (`ui/widgets/details.rs`, also under the Publish dialog's rewritten service errors).

## Toasts

- Toasts (`state/toasts.rs`) can carry one action button with a stored `Msg`.
- A click sends `Msg::ToastAction(id)`; `update/notify.rs` removes the toast and runs the message only while the toast's repository is the active tab.
- A failed job's toast says git's cause in plain words and can carry a fix (`update/job_errors.rs`, from `git_errors::classify`), with Details beside it in `Toast::details` (`Msg::ToastDetails`, which works from any tab), and a second fix between them in `Toast::alt` (`Msg::ToastAlt`): Delete lock file comes first with Try again beside it (`update/lock_file.rs`, `update/job_retry.rs`), and a push rejected as behind offers Pull (for the checked-out branch) with Force push with lease beside it, only when the job's push command and git's rejection line show it was that branch, pushed to its upstream (`state/branches/rejected_push.rs`). The buttons wrap inside the toast's width.
- Every cause's sentence comes from `state/error_words.rs` (`cause`, `explained`), which the auto-stash put-back, clone, submodule, Git Flow and worktree headlines and the Fetch/Pull repositories rows share; a toast that stopped on conflicts takes its count from the status read after the job (`update/conflict_count.rs`). The fallback "<label> failed: …" names the job in the UI language (`state/job_spec/labels.rs`).
- Failures reaching or reading a remote get their sentences from `state/error_words.rs`, which the Fetch/Pull repositories rows share; a missing or unreadable remote's toast offers Edit remote…. Hosting-service failures (fork, pull request, issue, comment, dashboard, status bar) go through its `hosting`, from `error::advice`.
- `toast_replacing` updates a toast still on screen instead of stacking another (the zoom toast keeps its id in `view.zoom_toast`).
- Every toast has an x (`NotifyMsg::CloseToast`) that removes it without running its action.
- The conflict view's Resolved and Saved-with-markers toasts carry Undo while the entry that did it is the newest (`update/conflict.rs::undo_button`, run as `JobMsg::UndoWith`, which refuses once the history moved).
- The pointer over a toast holds it: `ui/toasts.rs` sends `NotifyMsg::HoldToast` when the toast under the pointer changes, and a held toast's clock stops (`Toast::counted` and `held`), so it neither fades nor expires.
- Let go, a held toast has at least `HOLD_GRACE` left, and the toast limit drops the oldest toast that isn't held.
- Toasts fade and expire by `view.frame_at`, the shell's time stamp for the frame (None in snapshot tests, so they draw at full strength).

## Zoom and panels

- Zoom and panels are settings, not view state: `update/layout.rs` changes and saves them (`LayoutMsg`).
- The shell applies `settings.zoom_percent` to egui every frame (`ui/layout.rs::apply_zoom`, like the theme), so a profile switch or a Preferences edit takes effect on the next frame.
- egui's built-in zoom keys are turned off at start-up so the keymap owns them.
- The panels are drawn in order by `ui/frame_layout.rs`: status bar, top chrome, side panels, central view, tour.
- The top chrome starts with the drawn title bar on Windows and macOS, and with the menu bar and the tab bar under the system frame.
- `show_left_panel`, `show_details_panel` and `show_toolbar` are checked in `ui/frame_layout.rs`, and `show_menu_bar` by `ui/menu_bar::show` (system frame only).
- Hiding the menu bar through `LayoutMsg` toasts a Show menu bar button.
- `state::layout::details_shape` decides whether the commit details panel is drawn Full, as a 44 px Rail or Hidden.
- The details panel is a rail while a tool view (`state::layout::tool_view`: conflict, blame, file or folder history, issue, Git Flow, in the central area's order) fills the central area, unless `view.details_expanded_for` names that view.
- `LayoutMsg::ExpandDetails` and Go to the commit message set `view.details_expanded_for`, and `update` clears it (`drop_stale_expansion`) as soon as the tool view changes or closes; the saved setting never changes.
- A hidden left or details panel leaves a 16 px edge strip that shows it again (`ui/panel_rails.rs`, with the rail).
- Full screen is the window's own state, never saved: `LayoutMsg::ToggleFullscreen` asks the shell for `WindowCmd::SetFullscreen` from `view.window.fullscreen` (read each frame).
- `WindowCmd::in_window` narrows what the drawn bar's window actions do while full screen (docs/PLATFORMS.md, Window frame).
- Toasts are anchored to `ToastBounds`, where the top chrome and the status bar end, measured while `frame_layout` draws them (`ui/toasts.rs`), all one width.
- The toolbar's repository and branch pickers (`ui/toolbar_pickers.rs`) are sized to their names up to a cap.
- In a window too narrow for everything, the pure `name_widths` shrinks the pickers (the longer first, then both, down to a floor) against the rest of the bar measured last frame.
- Every toolbar group sits on its own surface (`widgets::tool_group`); the middle groups follow the pickers, the pull request group shows only with a hosting service, and the right-hand group is anchored to the window's right edge.

## Window chrome

- `state/window_chrome.rs` decides who draws the frame (`Chrome::for_os`: Glowfly on Windows, Glowfly under the traffic lights on macOS, the system on Linux; `Chrome::chosen` gives Windows the system frame when Preferences > UI asks for it; `main.rs` reads the settings before making the window, and the real app sets it once, `System` in tests unless they pick another).
- `state/window_chrome.rs` holds the pure pieces: the window actions (`WindowCmd`), what the shell read about the window (`WindowInfo`) and the Alt tap that opens the menu (`AltTap`).
- `ui/title_bar.rs` draws the 44 px bar: the menu button (the menus of `ui/menu_bar::menus` as submenus), the tabs (`tabs::strip`) and empty space that moves or maximizes the window.
- `ui/window_frame.rs` builds the viewport for each chrome (`viewport`, used by `main.rs`).
- On Windows `ui/window_frame.rs` draws an `Order::Tooltip` overlay above every dialog: the window buttons (`ui/window_buttons.rs`), the resize edges and corners (none while maximized), a 1 px border, and a drag zone over the bar while a dialog blocks it.
- Views push `AppMsg::Window`; `update/app.rs` returns `UiEffect::Window`, and the shell sends the viewport command (`shell/app_effects.rs`, `window_frame::viewport_cmd`), the only `send_viewport_cmd` outside the shell's own effects (`tests/source_checks/layering.rs`).
- On Windows the shell also installs `platform/native_window/` (`GlowflyApp::new`): a window procedure in front of winit's (`win_proc.rs`, its state in atomics).
- `win_proc.rs` answers the hit test (`CaptionGeometry::hit`) with `HTMAXBUTTON` over the drawn Maximize button for Snap Layouts, and with `HTTOP` over the resize strip along its top.
- `win_system.rs` opens the system menu on Alt+Space or on `WindowCmd::SystemMenu`, gives the frame DWM's shadow and Windows 11's rounded corners, sets the Mica backdrop and reads transparency and high contrast.
- Each frame `shell/app_native.rs` polls it into view state (`WindowInfo::max_button`, `ViewState::backdrop`, a click as `SetMaximized`, a shown menu cancelling the Alt tap) and hands back `window_frame::caption_geometry`.
- The pure parts of the native window are in `state/window_native.rs`.
- Only the shell (`src/shell/`) and `main.rs` may name `platform::native_window`, and no view, update or state code names `windows::Win32` (`tests/source_checks/layering.rs`).
- Only the shell may name `platform::primary_selection`: `shell/app.rs` opens it and `shell/app_effects.rs` runs `UiEffect::SetPrimary` and `UiEffect::PastePrimary` (`tests/source_checks/layering.rs`).
- `ui/backdrop.rs` picks the palette's Mica tints (`mica_tint`, cached in `ThemeCache`) and gives the title bar, toolbar and sidebar their fill (`backdrop::fill`).
- `GlowflyApp::clear_color` clears to transparent only while Mica shows.

## Menu bar

- `ui/menu_bar/`: a bar under the system frame only, the drawn title bar's menu button otherwise, and on macOS the system menu bar too.
- The five menus are one egui-free model, `state/menu_model.rs` (each menu's builder in `state/menu_model_items.rs`).
- Every menu item has its label, the binding whose first key it shows (never a literal), why it is disabled, its check or radio mark, and the action it runs: the message the toolbar, palette or key already sends (or the view-state change of Activity log and Show only matching commits).
- `ui/menu_bar/draw.rs` draws a model's entries with the menu widgets.
- The texts and reasons are pure helpers in `state/menu_bar.rs`, `state/repo_lists.rs` and `UndoHistory::undo_entry`.
- The themes' names in the language (View > Theme and Preferences' Theme row) are in `state/theme_names.rs`.
- On macOS (`Chrome::native_menu`) `state/menu_native.rs` plans the system menu bar from the same model: the app menu takes About, Preferences and Quit, beside Services, Hide, Hide Others and Show All; key equivalents; tags of generation and item index.
- `platform/macos_menu.rs` builds the system menu bar, and `shell/app_menu.rs` runs its picks before each frame's input and hands it the menus after the frame's messages (docs/PLATFORMS.md, Menus).
- The menu bar's own window actions are `AppMsg` (`update/app.rs`).
- About opens `Modal::About`, whose Privacy button opens PRIVACY.md on the website and whose Third-party notices… button opens `Modal::Notices` in its place (`ui/menu_bar/notices.rs`, with Back to About), which lists the licenses `glowfly_core::notices` parses from THIRD-PARTY-NOTICES.md (embedded at build time).
- Each menu is drawn from the model by `menu_bar::contents`, so the bar and the title bar's menu button share them.
- `bar.rs`: the titles as buttons, each menu opened below its title through `menu_anchor.rs`.
- The title bar's menu button shows the five titles as submenus, or one menu alone under a row of titles when opened from the keyboard.

## Menu keys

- Not on macOS (`Chrome::mnemonics`).
- `state/menu_keys.rs` (egui-free) picks each title's mnemonic from the translated title, skipping letters a plain Alt key of the keymap or a custom command uses.
- `state/menu_keys.rs` holds `ViewState::menu_keys` (`MenuKeys`: the menu an Alt+letter asked for this frame, whether a menu was opened from the keyboard, which item takes focus and which single menu the title bar shows).
- `ui/shortcuts/global.rs` takes the Alt+letter press, and the text it types, out of the input before the views run (`ui/menu_bar/keys.rs::take`, which also underlines the letters while Alt is held or a keyboard-opened menu shows); the bar or the title bar's menu button opens that menu.
- The bar's menus take the keyboard every menu has (`ui/menu_keys/`, see `ui-shell.md`): the first item takes focus, Up and Down wrap, typeahead, Right opens a submenu (Theme, Open recent).
- Focus that leaks out of an open menu is put back (`menu_keys::keep_inside`), and Left and Right switch menus while the focus is in the root menu, not a submenu (`menu_keys::may_step`).
- The shortcut sheet lists the menu keys as its Menus group (`ui/shortcuts/sheet_menus.rs`).
- Enter and Space click the focused item (egui's own), and Esc closes the menu before the central Esc chain (`central::escape_key` waits while `menu_bar::any_open`).

## Quitting

- Quit (the menu item, the macOS app menu's Quit Glowfly, or the rebindable Quit key, `BindingId::Quit`, Cmd+Q or Ctrl+Q, kept by a focused terminal on Windows and Linux) sends `AppMsg::Quit`.
- winit's default app menu, whose Quit would end the app without asking, is off: `platform::native_menu::event_loop_hook`.
- The window's close button reaches `update` as `AppMsg::CloseRequested` (the shell reads egui's `close_requested` in `input`).
- `update/app.rs::quit` asks first while work exists only in memory (any tab's commit draft, conflict choices or comments not posted, an edited dialog, unsaved Preferences edits: `state/tab_losses.rs::quit_lines`), a non-quiet job (a sparse clone among them), a repository sync or an update's download runs, or a terminal runs a program (`state/quit_confirm.rs` builds one danger confirmation naming each, with its tab; Cancel is focused), and cancels the OS close with `UiEffect::CancelClose`.
- Running shells are asked about first: `TerminalEffect::CheckBusy` probes them off the UI thread and answers with one `TerminalMsg::BusyChecked`, which `update/terminal_busy.rs` hands to the close waiting on it (quit, closing repository tabs, Install and restart); `state/terminal/busy.rs` names the busy terminals.
- Quit anyway sends `AppMsg::QuitConfirmed`, which sets `view.quitting` and returns `UiEffect::CloseWindow`, and the close that follows is let through. A job that can be stopped now (a fetch, push, clone or a pull's fetch) is stopped first (`job_stop::cancel`, so its lock files are removed) and the window closes once it has ended (`view.quit_after_stop`).
- While the question shows, `update/app.rs::settle` (after every message) closes the window once the work and terminals it named (`view.quit_ask`) have ended, or names only what is left; never while it names unsaved work, which doesn't end by itself.

## Desktop notifications

- `update/notify.rs::desktop` returns `Effect::Notify` (a `DesktopNote`, `effect/notify.rs`) only while `settings.desktop_notifications` is on and `st.view.focused` is false.
- The shell sets `view.focused` from egui at the start of each frame, before runner results are handled, so `update` never asks egui.
- Finished non-quiet jobs (`update/ops.rs::finish_job`) and behind alerts use it; turning the setting on sends one confirmation regardless of focus.
- `runner/notify.rs` shows the note through notify-rust (the only caller, `tests/source_checks/layering.rs`) and reports a refusal as `PrefsMsg::DesktopNotificationFailed`.
- A note carries the repository it is about (`DesktopNote::workdir`); the effect's worker thread waits for the user's answer (at most `MAX_WAITING` at once) and reports a click as `NotifyMsg::DesktopClicked`.
- A click focuses the window (`UiEffect::FocusWindow`) and shows that repository through `update::tabs::open`, or, under a dialog (`deep_link::busy`), offers Show in a toast.
- Glowfly's identity with the OS is `glowfly_core::app_identity` (`APP_ID`, `DESKTOP_ENTRY`): an installed Windows copy takes the app id before its window opens (`platform/app_id.rs`) and posts toasts under it, Glowfly.app posts under its bundle id, and Linux notes name the desktop entry.

## Credential prompts (`askpass/`)

- Every git child that may prompt gets Glowfly as its `GIT_ASKPASS` / `SSH_ASKPASS`.
- The helper process asks the app's loopback listener, which reports `CredentialMsg::CredentialPrompt`, and `Effect::AnswerPrompt` hands the dialog's `Reply` back.
- `gitcli::NO_PROMPTS` (reads) turns all of it off; background jobs use `gitcli::BACKGROUND`, which also keeps Git Credential Manager from opening a window.
- HTTPS sign-ins typed with "Remember this sign-in" are kept in `CredentialPrompts::typed` until the job ends; `update/askpass_store.rs` then saves them (`CredentialEffect::Remember`) or drops the rejected ones (`Forget`).
- An unknown SSH host whose question parses (`PromptKind::HostKey`) is shown as fields (`ui/askpass_host_key.rs`); any other yes/no question shows ssh's text as it is.

## Auto-fetch sign-ins

- Files: `state/sign_in.rs`, `update/sign_in.rs`, `ui/sign_in_banner.rs`.
- `jobs::start_job` notes which remotes a fetch covers (`SignInNotices::fetching`).
- An auto-fetch that fails for want of a sign-in (`auth_failure::fetch_failures`) pauses those remotes (global, in memory, by workdir and remote), and auto-fetch leaves paused remotes out.
- The banner offers a fetch that can prompt, and any fetch of the remote that works resumes it.

## Unreadable settings file

- Files: `state/settings_file.rs`, `update/settings_file.rs`, `ui/settings_banner.rs`; flow tests in `settings_file_tests.rs`.
- A startup load whose `LoadProblem` is `locked` sets `settings_file.unreadable` (`SettingsFile::hold`) before the first startup step (`shell/app_start.rs`), since some save, and `PrefsMsg::SettingsUnreadable` reports it after them: defaults are in use, and `update/prefs.rs::save_settings` returns no save while it is set, so they can't replace the user's file. Journey: `journey/settings_unreadable.rs`.
- The banner (under the toolbar and above Preferences) stays until Try again (`PrefsEffect::ReadSettings`, `Settings::reload`) reads the file; its settings then replace the defaults (a toast says when edits made meanwhile were dropped), the keymap, the terminal panel and live settings follow, the git they name is put into effect (`GitEnvEffect::Use`, after any check already running), the release notes seen at this start stay seen, and saving goes on. Open settings folder shows where the file is.

## Git LFS notices

- Files: `state/lfs.rs`'s `LfsSession`, `state/lfs_notes.rs`, `state/session.rs` (the open diff's note), `ui/lfs_banner.rs`, `ui/lfs_badge.rs`, `ui/diff/lfs_note.rs`.
- The LFS patterns of every attributes file (`lfs::repo_patterns`: root, nested, info/attributes) ride on every whole status read (`LoadMsg::StatusLoaded` and `RepoUnchanged` carry them; `update::lfs::patterns_read` applies them), so hand edits reach the Track in Git LFS entries, the banner and the auto-fetch LFS download without a full reload.
- The banner shows while git-lfs is known missing (the startup `LfsEffect::Detect`) in a repository with patterns, until dismissed for that workdir this session.
- File rows marked `lfs` get a badge.
- A diff whose `RepoMsg::DiffLoaded` brings an `LfsChange` shows a summary instead of the pointer text (`LfsDiffNote`, keyed by target, in `RepoSession::diff.lfs`, parked with its tab) until "Show pointer text".

## Signing passphrases (`gpg_prompt/`)

- gpg-agent's pinentry is fixed when the long-running agent starts and is shared by all its clients, so there is no Glowfly pinentry; swapping it would mean editing gpg-agent.conf and restarting the agent.
- GnuPG's per-call alternative, `--pinentry-mode loopback`, is used instead.
- For commands that may prompt (not `NO_PROMPTS`), `gitcli` adds `GIT_CONFIG_COUNT`/`KEY_0`/`VALUE_0` setting `gpg.program` to the Glowfly executable (command scope beats every config file) plus `GLOWFLY_GPG_WRAPPER=1`.
- The wrapper runs the user's own gpg.program (or `gpg`) with `--pinentry-mode error` first, so a passphrase the agent has cached (or a key without one) signs without a prompt.
- On "No pinentry" the wrapper sends a structured `GpgAsk` through the askpass listener and signs with loopback, the passphrase on fd 0, asking again after a bad passphrase (three tries).
- When `settings.gpg_prompts_in_app` is off, `update/askpass.rs` answers `Reply::UseSystem` without showing anything; that, an unreachable app, or a GnuPG without loopback runs gpg exactly as git asked, so GnuPG's own pinentry appears.
- On Windows the wrapper maps an MSYS-style gpg.program (`/usr/bin/gpg`, `/c/…`) into the git install found from `GIT_EXEC_PATH`.
- With `gpg.format=ssh` git runs ssh-keygen itself, which asks through `SSH_ASKPASS` like ssh.
- For commands that may sign (commit, merge, rebase, cherry-pick, revert, am, pull, tag, commit-tree), `gitcli` reads `gpg.format` and `user.signingKey` once (`gpg_prompt/ssh_env.rs`) and, for a key file (not a literal `key::` key, which is agent-only), sets `GLOWFLY_SSH_SIGNING_KEY`.
- The helper then sends ssh-keygen's wording (`Enter passphrase:` or `Enter passphrase for "<key>":`) as a structured `SshSignAsk` (`askpass/ssh_sign.rs`), shown as `PromptKind::SshSigningPassphrase`.
- The SSH signing prompt is always shown in Glowfly (the GnuPG-window preference doesn't apply) and never as a retry, since ssh-keygen stops at the first wrong passphrase.
- A job that then fails with ssh-keygen's "incorrect passphrase" gets the headline "<Label> failed: the passphrase didn't unlock the SSH signing key" (`CredentialPrompts::answered_ssh_signing`).

## Logs

- Each tab has its own activity log: the active tab's is `AppState::log`, the others' `Tab::log`.
- `AppState::log` / `log_quiet` add to the active tab's; `log_in` / `log_quiet_in` route a line to the tab showing a repository (a scoped result, a finished job, a git command).
- Each line is also queued, tagged with its tab's repository, in the outbox `LogsState::pending` (`state/logs.rs`), which the end of `update` flushes as `Effect::Logs(LogsEffect::Append)` once the activity file has loaded.
- `activity.jsonl` lines carry that `repo` (`paths::encode`).
- `LogsMsg::Start` at startup reads the file, and its earlier entries are handed to the tabs showing their repositories, in front of this session's lines.
- Entries for a repository no tab shows wait in `LogsState::unclaimed` until a tab activates or opens it (`update/logs_tabs.rs`).
- Nothing opens the panel by itself: a failure logged with `log` / `log_in` is announced at the end of `update` as a failure toast with Details (`logs_tabs::announce`), or gives Details to a failure toast pushed in that update.
- Details selects the line's tab and shows its activity log, opening the failed run's row.
- An entry is a note or a run: a job that ends in `ops::finish_job` is logged by `update/logs_runs.rs` (`job_ended`) with a `RunInfo`, persisted as the line's optional `run` object.
- A run's text has git's line-ending warnings folded into one line that says how many (`glowfly_core::crlf_warnings`).
- A `RunInfo` holds the job's label, a one-line summary from `logs::last_message` or git's explained cause, and its duration from `LogsState::job_started`, which `logs_runs::stamp` sets at the end of `update`.
- A loud failure's row starts open (`LogsState::open`), and its fix is kept once in `LogsState::fixes`: the failure toast's button is a `LogsMsg::Fix(id)` proxy, so the panel and the toast run it once.
- `state::activity_rows::rows` groups the view's rows (session headers, runs and notes, consecutive successful auto-fetches as one routine row); `cap_routine` keeps such a streak to `ROUTINE_KEEP` entries, the first counting the rest.
- One ordered log worker (`runner/logs.rs`) owns the activity file and the performance log and stamps times, so `update` reads no clock.
- `Runner::run` times every effect that doesn't wait on the user (`Effect::timed`) and gitcli's run observer times every git command; those taking `logs::SLOW` or more go to `performance.log`.
- With extended logging each git command also comes back as `LogsMsg::GitRan` for a quiet activity line.
- Copy entry and Copy log build plain text in `state/activity_copy.rs`.
- Export logs picks a file on a worker thread (`LogsEffect::PickExport`), then the log worker writes the bundle in order after the pending appends (`LogsEffect::Export`, answered by `LogsMsg::Exported`).

## Deep links

- Files: `deep_link.rs`, `state/deep_link.rs`, `update/deep_link.rs`, `update/deep_link_open.rs`.
- `main.rs` takes the claim on links and folders when no other window holds it, else hands its `glowfly://` or folder argument to the window that does (`glowfly_core::instance`; `Runner::start_instance` listens, next to the askpass listener, and reports `DeepLinkMsg::Open` or `OpenFolder`) and exits; otherwise the new window opens it after restoring its tabs.
- On macOS, LaunchServices sends links and folders to Glowfly.app as Apple events instead of argv; they arrive as the same `DeepLinkMsg::Open` / `RepoMsg::Open`, via `Runner::start_open_events`.
- Either way a link arrives as `DeepLinkMsg::Open`, which brings the window forward and, unless another dialog or Preferences is open (a toast offers to open it later), parses it and sends `DeepLinkEffect::Find` over the active tab, the other tabs, favorites and recent repositories.
- The runner checks each with `link::find::check` (root commit, remote URL).
- `state::deep_link::best` keeps the top rank (root and URL, root, URL), merges worktrees of one repository, and prefers the one open in a tab.
- One match opens through `tabs::open`; several ask (Choose), none offers Locate or Clone.
- The pending link (`DeepLinkState::pending`, numbered by `seq` so a newer link wins) waits in `Awaiting::Ready` until its repository is the active tab.
- `deep_link_open::ready` runs at the end of every snapshot load (and right after `tabs::open` for an open or parked tab) and selects the target, or offers Fetch in a toast when it is missing.
- The fetch is an ordinary job with `then_link`, after which `ops::finish_job` has the repository checked again.
- A clone from the Not found dialog is recognised by its remote URL when it loads.
- Nothing runs git without a click.
- Copying a link goes the other way: menus send `DeepLinkMsg::Copy`, update picks the remote URL, and `DeepLinkEffect::Build` adds the root commit.

## Releases and updates

- The Release workflow packages each build (`scripts/package-*`, `packaging/`; see `docs/RELEASING.md`) and publishes `SHA256SUMS`: it uploads everything to a draft release (`scripts/release-draft.sh`) and publishes the draft last (`scripts/release-publish.sh`), so release immutability can lock it.
- It puts a fixed header above each release's notes (`packaging/release-notes-header.md`, by `scripts/release-notes-header.sh`), which `changelog::parse_body` skips, and attaches the Flatpak's git source beside the bundle (`packaging/flatpak/GIT-NOTICE.md`).
- In the app, `UpdateMsg::Startup` arms the automatic check.
- `update/updates.rs::tick` starts the check (at most once a day, after `STARTUP_DELAY`, an hour after a failure) as `UpdateEffect::Check`, which asks the `app_update::Feed` (`forge::latest_release`, or the folder `GLOWFLY_UPDATE_FEED` names) and reports how this copy was installed.
- `app_update::offer` picks the file for this platform; Download runs `app_update::download_streamed`, which streams to a temporary file and renames it into place only once the SHA-256 matches, reporting `DownloadProgress` a few times a second; Cancel stops it between chunks and removes the partial file.
- The release notes dialog reads the `CHANGELOG.md` that `build.rs` bundled, and `startup_notes` opens it once per new version (waiting in `notes_pending` while another dialog is open).
