# Ideas: Colour, themes and type

Themes, brand and branch colour, contrast, type scale and control styling. Untriaged; see [README.md](README.md).

### Custom themes

- Reload theme files automatically when they change on disk (watch the themes folder)
- A theme file based on another theme file (inheritance, with loops detected across files)
- Edit colors in Preferences with a color picker and live preview, saved to the theme file
- Suggest the closest color name for an unknown key ("acent" -> "accent")
- Warn when a theme file's text or syntax colors fall below the contrast rules
- Translucent colors (#rrggbbaa) in theme files
- Theme the graph lane colors and avatar initials colors from theme files
- Import or share a theme file from Preferences (copy it into the themes folder)
- A toast with an Open Preferences button when the selected theme is missing or broken at startup
- The removed-theme toast names the old theme in English ("Ember", "Garden Dark") in every
  language, as its names left the locales; keep translated names for it if people miss them
- A per-repository theme

### No missing glyphs

- Give the semibold family the system's bold CJK face (Microsoft YaHei Bold, Yu Gothic Bold) instead of the regular one
- A monospaced CJK face (MS Gothic, Noto Sans Mono CJK) for monospace text, so CJK lines up in diffs and the terminal
- On Linux, ask fontconfig (`fc-match :lang=ja`) when none of the known CJK files is found
- On macOS releases that keep PingFang under `/System/Library/AssetsV2`, find it there
- System fallbacks for other scripts the bundled fonts lack (Thai, Devanagari, Arabic, Hebrew), which the debug warning now names
- Name the CJK locales in their own script in Preferences (日本語, 中文, 한국어) when the system has the fonts
- Put the ahead/behind counts into the LOCAL row's tooltip and accessible description ("2 ahead,
  3 behind origin/search"), so the painted arrows have words for screen readers
- Use the painted ahead/behind arrows wherever else counts appear (remote and Git Flow rows, tab
  tooltips, the dashboard)

### Readable controls in every theme

- Combo boxes get the field border and focus ring (they share egui's button visuals today)
- Menus, tooltips and popups get the stronger dialog_border edge (window_stroke still uses border)
- Dialogs in dark themes get a surface one step lighter than panel and a stronger shadow
- DragValue editors and other egui-internal text edits get the field border
- The selected row stays clearly different from a hovered row in the light themes
- Other chips reach 4.5:1: issue and PR state chips, PR thread chips, SSH key chips, the keyboard
  pill, the update and trust chips, the LFS badge, the graph's worktree and stash label chips,
  release-note chips
- The text cursor takes its colour from the palette
- Graph lane colours checked for contrast against bg in the light themes

### Comfortable look

- An opt-in compact density in Preferences > Appearance (Comfortable | Compact: about
  28 px rows on a 4 px grid), never the default
- 16 px padding and grid spacing for the bottom panels (activity log, terminal, conflict
  output), the banners and the Git Flow panel
- Preferences navigation and integrations side panels on the 16 px padding with 36 px rows
- Menu and popup margins on the 8 px grid, once dialogs no longer share egui's popup frame
  (menu_margin)
- A soft shadow or edge under the lifted side panels in light themes, beyond the palette step
- Retune the graph lane colours against the refreshed canvases (with the History table item)
- The graph filter banner (Showing only / hidden from the graph) on the sidebar's 16 px inset, its
  icon and text lined up with the rows' markers and names, without cutting its text

### Line counts in red and green

- Line counts on the multi-commit, compare-with-working-tree and pull request file lists
- A tooltip on a file row's counts and bar ("12 lines added, 3 removed")
- Compact large counts (12.3k) in narrow panels
- Other worktrees' WIP rows drawn with the same chips and total
- Counts and totals for commits and working trees past the 2,000-file cap
- Fill the graph's CHANGES total from a selected commit's per-file counts when they arrive first
- Counts for Git LFS files from their real content
- Sort a file list by the size of its change
- A preference to hide the per-file bars

### Toggles look like toggles

- Blame header switch: after "Blame before this commit" or Back, Diff and File could select the blamed commit in the history instead of its original selection
- Segmented control: an accessible radio-group node with a group name, and Left and Right arrow keys to move between segments
- Blame header row 2: show Ignore whitespace and Wrap lines as checkboxes, like the diff's View menu
- Blame header: hide or rename the row-2 File history button while the Diff | File | Blame switch shows, since both lead back to the history
- Hunk header: fold the hunk buttons into one menu when the visible width is too narrow for body-size labels
- Search bar: measure the Highlight | Filter width instead of the fixed CONTROLS_W

### Type scale and text styles

- WIP and conflict list headers: show the count as a muted trailing badge instead of "(n)" in the title
- Host-key prompt: explain it in words, with the host and the fingerprint as separate monospace rows
- Activity log entries: a time column, an error icon and a Details expander
- Check the Spanish labels for sentence case too (German capitalises its nouns)
- A text-size preference that scales the five steps, apart from zoom
- A semibold monospace face for status letters and emphasised code
- Check that long context menus still fit small windows at 14 px items; scroll or regroup where they don't
- Tune the Preferences label column for the longest translated labels at 14 px
- The commit details' "ISSUES" / "CARDS" heading and its German and Spanish values are still all caps

### Brand colour and tinted neutrals

- Retint the diff, hunk and banner backgrounds onto the tinted neutrals (the blue-grey hunk_bg
  sits on green-tinted panels)
- Status-coloured chips on the raised fill (issue state, a thread's Outdated, rebase step chips)
  use the status chip tint, so their text reaches 4.5:1 in light
- Give errors and failures their own attention colour instead of removed's red, and success its
  own token instead of added's green
- Theme files: note when a file uses an old colour name (green, red, amber, blue, purple) and
  offer the new one
- Theme files set the repository group colours
- Move the high-contrast tag chips onto sand too (they keep their amber-tinted tags)
- Retune the favourite star and other non-warning uses of warn (favourites, CI pending) once
  Vivid colour gives each job its own colour
- A slightly stronger modal backdrop in Dark: its darker panel now drops only about 8 L* under
  the backdrop

### Branch-led colour

- A tooltip naming the branch when hovering a lane line in the graph
- A pinned colour follows a branch rename, and pins of deleted branches are pruned at load
- A local branch whose upstream has a different name shares the upstream's colour
- A branch dot on WORKTREES rows (the worktree's checked-out branch) and in the command
  palette's checkout entries
- Remember resolved colours per repository so a branch keeps its colour when new neighbours
  appear
- Branch tones as theme-file keys
- Separate pins per palette preset (a pin keeps its slot number; 8 hues show it modulo 8)
- Share pins across a repository's worktrees (they are per working folder, like graph filters)
- Show on the Color submenu's Automatic row which hue it would give
- A preview strip of the chosen preset's hues on the Branch colors preference row
- Person colours in the blame gutter, the file history and the author picker
- Person colours that follow .mailmap (they follow the raw author email today)
- Pin a branch colour from the command palette
- Warn when a pin matches a side-by-side branch's pinned hue

### Vivid colour

- Theme files set the repository hues and the coral (`repo_*`, `needs_you`, `on_needs_you`
  keys, with an ink fallback like `on_primary`'s)
- A Color row for the open repository on a repository page of Preferences
- A worktree group's strip takes its worktrees' colour when they share one, and an option to
  share one colour across a repository's worktrees
- Repository swatches in the Find repositories dialog, the tab list dropdown, the Window menu's
  tab list, and a "Set repository color…" command palette entry
- Prune repository colours of folders removed with Forget or Remove missing
- Failed operations whose cause is conflicts (error panes, toasts, the activity log) use the
  coral needs-you mark
- A coral conflict chip on the graph's WIP row while files are conflicted (conflicted paths are
  counted under M today)
- The conflict view's ours/theirs line tints move off info/progress at 18% to their own
  diff-family pair
- Retune the favourite star (warn) in the repository list and finder with its own token now
  that warn has left the chrome
- The "Merge in progress" strip under the toolbar takes the coral NeedsYou kind while files are
  conflicted, like the WIP panel's banner
- Name the conflicted rows' coral badge for screen readers and on hover ("Conflicted"), now that
  the "!" letter is gone

### No brown

- A hygiene check that flags `gamma_multiply` on a status, accent or repository colour in `ui/`
  (they should go through `color::wash`)
- The light themes' amber text (#975600 on Dawn) is the most orange a 4.5:1 amber can be, still a
  burnt orange; split it like the branch amber's line and text tones: a clear amber (OKLCH about
  0.62) for status letters, dots and icons that need only 3:1, and the dark one (or neutral ink)
  for amber words
- High contrast light's amber text (`warn`, `modified`, `syn_type`, terminal yellows) is exempt
  from the text zone, as any amber at 7:1 is chocolate; consider dropping amber there for
  `modified` (a neutral letter with a mark)
- Mint (hue 176.5) sits 6.6 deg from Mist's sea-glass accent; they part by lightness and chroma,
  but judge the tab swatch beside Mist's accent
- The branch hue Olive (#9ea827) is vivid and passes the zone, but its name sits oddly with the
  no-olive rule; consider renaming it (Lime is taken; Chartreuse?)
