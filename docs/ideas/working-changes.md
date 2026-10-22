# Ideas: Working changes, staging and stashes

The working-changes panel, staging, commit box, file actions, stashes and local patches. Untriaged; see [README.md](README.md).

### Working-changes panel, step by step
- "Commit or amend first…" in step 3 of a rebase or pick reads like text; a disclosure row may
  read better
- The pinned commit box is measured the frame before, so a resize settles one frame late
- A key to open the next step or run step 3's primary (see Keyboard shortcuts for Continue)

### Stash and carry on

- Interactive rebase and graph edits: use the named auto-stash instead of git's `--autostash`
- A Show stash button on the toast when git's own autostash is kept after an interactive rebase
- Restore the staged state on put-back (`stash pop --index`, falling back to a plain pop)
- Guard multi-command jobs too: pull request checkout, Git Flow start and finish
- Don't count submodule-only changes as dirty in the guard
- An option to include untracked files in the auto-stash
- Remember pending auto-stashes across restarts (offer to put them back on reopen)
- Pull and Checkout tooltips say when the changes will be stashed first
- Offer Stash and carry on from the "would be overwritten" error toast for operations started
  outside the guard

### Line-level staging

- Per-line hover buttons (stage / unstage / discard this line)
- Selections spanning several hunks
- Keep the line selection across tab switches
- Auto-scroll the diff while a line drag is held past the viewport's top or bottom edge
- Drag selection in the whole-file view, once line operations work there
- Discarding every line of an untracked file deletes it instead of leaving an empty file
- After unstaging every line of a new file, follow the diff to its untracked view instead of
  closing it
- Ctrl/Cmd-drag over already selected lines removes them (toggle the range) rather than adding
- Changed-word fills on a selected line are a step weaker than the stronger selected tint in
  Dark; derive a selected-word fill from it
- Space toggles the line under the keyboard cursor (with the check box as its focus mark)

### Live working-tree updates

- Notice a watcher that started but receives no events (a probe after starting) and warn once,
  suggesting a restart
- Watch inactive tabs so their state updates in the background
- Status-bar indicator showing whether live updates are active for the repository
- Watch config files pulled in by include.path / includeIf (outside the git dir) so their edits
  reload too
- Ask about trust again when a trusted repository gains new command-running settings outside
  Glowfly

### Commit panel extras

- Repo-specific Commit preferences page: edit and save the template, skip hooks by default,
  push after commit by default

### File actions

- Create, rename and delete files from the command palette
- New file in this folder… on folder rows (folder rows wait for a tree view in file lists)
- Delete several selected files at once (needs multi-select in file lists)
- A button in the trash toast that opens the Recycle Bin / Trash or restores the file
- Send several selected files to the Recycle Bin / Trash at once (needs multi-select in file
  lists)
- Name a staged rename's source path in its Unstage and delete confirmation and toast
- Open the newly created file in the editor or default app after creating it
- Create a file from a template, or with initial contents
- Rename or delete folders and submodules
- Explain when a created path is inside a nested repository or submodule, so the parent's
  status won't list it
- Edit a file in place
- Restore several selected files from a commit at once (needs multi-select in file lists)
- Choose which side to restore from comparison rows for a file the newer commit modified
- Re-check which comparison and commit-detail paths are on disk after a status refresh (Open /
  Show can go stale until the commits are reselected)
- Explain when a restored or opened path is outside a sparse checkout
- Restore, Open and Show buttons in the diff header for two-commit comparisons (restoring the
  newer commit's version, as the comparison rows do)
- Make discarding an untracked file undoable the same way Delete is
- Make Unstage deleted file (a staged new file already gone from disk) undoable by keeping its
  staged blob
- Take the file back out of the Recycle Bin / Trash on undo instead of leaving its copy there
- Keep an undo entry when a delete fails after the file went (the trash reported an error, or
  Unstage and delete couldn't change the index)
- Keep a copy for undo of files over 64 MB (outside the object database, or by restoring from
  the trash)
- Read Recycle Bin settings for redirected known folders and subst drives, which have no volume
  GUID
- Find the mount's trash folder on unix systems without /proc/self/mounts or /etc/mtab (the
  BSDs) instead of leaving the choice to the trash crate
- Show progress for, and allow cancelling, the copy into the home Trash for large files
- Open and Show for the file history's current file in its top header, independent of the
  selected commit
- Re-check whether the file history's file is on disk after status refreshes and file watcher
  events (an untracked file deleted outside Glowfly keeps Open enabled)
- Open the commit's version of a file (a temporary copy) when it is no longer in the working
  tree
- Name the file in the show-in-folder tooltip and the open failure message
- Painted icons for Open and Show in the diff header to save room (split widgets.rs first)

### Stash improvements

- Hide stashes in the graph
- Edit the stash message inline from the commit details panel when a stash is selected, and
  show the stash list message there (the stash commit's own message keeps its old text)
- Keep the original reflog dates of the stash entries above the renamed one
- Move apply, pop and drop (`JobMsg::Stash`) into `StashMsg`, so all stash actions share one
  message enum
- Share one stash-action message builder between the sidebar and the palette, Rename… included
- Undo a stash rename (record the old message so Ctrl+Z restores it)
- Rank recently picked stashes first in the rename sub-list
- Stash ignored files too (`--all`) from the Stash dialog
- Stash chosen hunks or lines of a file, not only whole files
- Filter field above the Stash dialog's file checklist for large working trees
- Check the installed git version before offering "Only staged changes", instead of explaining after git refuses
- Right-click menu on the toolbar Stash button with the Stash dialog's choices
- "Stash…" entry in the STASHES section header menu that opens the Stash dialog
- Stash the files selected in the WIP lists once working-tree files can be multi-selected
- Show each stash's date in the sidebar and palette rows (needs the stash commit time in StashInfo)
- A palette entry that stashes while keeping the staged changes in one step
- Remember the Keep staged changes choice between openings of the Stash dialog
- Show a staged rename's old path in the Stash dialog's file list and the WIP lists ("from a.txt")
- Stash a staged rename whole when its old name isn't UTF-8 (now stashed as an added file, its deletion left staged)
- Stash a staged rename whole when a new file sits at its old path (now the rename splits and the file stays)
- Undo a keep-index stash of a file that had both staged and unstaged changes (now refused by the working-tree check)
- Stash a case-only staged rename whole on a volume that ignores case (now it splits like the other cases)
- Offer Undo for a stash whose put-back of a rename's old file failed after the stash was made (the stash stays, no undo is recorded)
- Translate the English job details and redo refusal for a blocked rename stash along with the other runner and undo texts

### Local patches

- Drop a `.patch` file on the window to apply it
- Say when a patch is already applied (a reverse check after a failed apply) instead of
  reporting that it doesn't apply
- Stash uncommitted changes before applying a patch and restore them afterwards
- Choose several files of a commit for one patch (only one file from the row menu now)
- Create a patch of one file from the rows comparing two commits
- Keep a renamed file's rename in a one-file commit patch (include its old path)
- git am options: keep carriage returns (`--keep-cr`) for CRLF content, Signed-off-by, and the
  committer date
- Remember the Apply dialog's last options
- Undo for Apply as commits
- Show the patch git am stopped on (`git am --show-current-patch`) from the banner
- Palette entries for Continue / Skip patch / Abort while applying patches, and Apply patch /
  Create patch in the menu bar's File menu
- Include more than 1000 untracked files in a changes patch (one diff through a temporary index)
- Copy a patch of uncommitted changes to the clipboard
