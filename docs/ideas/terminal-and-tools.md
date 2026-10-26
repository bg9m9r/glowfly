# Ideas: Terminal and external tools

The built-in terminal, custom commands, external tools and a command-line companion. Untriaged; see [README.md](README.md).

### Custom commands

- Optional toolbar slot for chosen custom commands (after the Design system toolbar rework)
- Commands in the remote branch, tag, stash and multi-commit menus, with a {shas} placeholder
  for a selection of commits
- A Windows cmd or PowerShell kind, or per-OS template variants (template_windows /
  template_unix)
- Run a command in the built-in terminal instead of as a job, for interactive programs and live
  output
- Stop a running custom command, and stream its output into the Activity row while it runs
- Ask for text before running ({input:Prompt} placeholders filled from a palette step)
- More placeholders: {short} sha, {upstream}, {remote}, {tag}, the file's folder
- Preferences page listing loaded commands, their files and problems, with Open file, Reload
  and Create user file (with a commented example)
- List custom command keys in the shortcut sheet and Preferences > Keyboard, and let them be
  rebound there
- Remember custom commands among the palette's recently run commands
- A setting to ignore repository commands files entirely
- Watch the user commands file for changes (today it reloads on each repository load, focus
  refresh or Reload)

### Terminal keeps the prompt's space

- Diff, blame, file history and conflict views measure columns with the unrounded 'M' advance;
  check long lines for the same drift against their highlights and wrap widths
- Log the raw PTY bytes at trace level (RUST_LOG) so a shell's output can be captured for
  regression tests

### Terminal colour themes

- Manage imported schemes in Preferences: open the terminal schemes folder, rename and delete
- Swatches of the chosen scheme's colours beside the Colors picker, and a live preview while
  the menu is open
- Import more formats: Alacritty (TOML), kitty, Ghostty, Xresources and base16 schemes; read
  binary iTerm2 property lists too
- Drop a scheme file on Preferences (or the terminal) to import it
- A "Terminal colors: <scheme>" palette entry
- An option to lift hard-to-read colours to the readability floor automatically, as some
  terminals do, instead of only warning
- Selection text colour and cursor text colour keys (Windows Terminal's selection and cursor
  text), and a selection opacity

### Built-in terminal

- Mouse reporting (xterm mouse modes) and the window or tab title from the shell (OSC 0/2)
- Tab strip overflow: scroll-left/right buttons and a list of all tabs when they don't fit
- Keys for new terminal tab, close tab and next/previous tab while the terminal has focus
- Rename a terminal tab
- A preference for the history length (fixed at 5,000 rows)
- Font family and line height preferences for the terminal; draw the dim (faint) text attribute
- Honour a program's cursor shape requests (DECSCUSR) and a blinking cursor
- Browse… for the custom shell program, and WSL distributions as shell choices
- Find in the terminal's history
- Clear for cmd in step with Windows' pseudoconsole (today it only clears locally)
- A palette entry for New terminal tab
- Show the running program's name in the close confirmation on macOS
- Note in the shortcut sheet and Preferences > Keyboard which keys the focused terminal keeps
- An option to send Shift+Home/End to the shell (PSReadLine selects with them) instead of
  scrolling the history

### External tools

- Open at the selected diff line: pass {line} from the diff or whole-file view
- Presets for well-known editors and terminals (fill in the arguments when one is picked), and
  detecting installed ones
- Name the editor in labels ("Open in <editor>"), and an Open in external editor button in the
  diff and file history headers
- A keyboard shortcut for opening the selected file in the external editor
- Report an editor that exits at once with an error
- Open several selected files or repositories at once, asking first when there are many
- Show / hide arguments toggle that collapses the argument fields

### Command-line companion

- History, diff and status subcommands

### Terminal panel

- Grey out shells in the caret's menu that aren't installed (probe PATH on a runner thread when the menu opens)
- Follow the shell's working folder from OSC 7 for the "program · folder" name
- Strip the elevated console's localised "Administrator: " prefix from window titles
- Remember custom tab names and the maximised state across restarts
- Keyboard shortcuts for maximise, rename and a new terminal with a chosen shell
- Drag pills to reorder terminal tabs
- Distinct painted icons per shell (PowerShell, cmd, bash, custom)
- Theme-file keys for the 16 terminal colours (custom themes take their base theme's table)
- Right-click menu on a pill: Rename, Restart, Close, Close others
- Bell or new-output indicator on inactive terminal tabs
- Restart terminal in the More menu when the active shell has ended
