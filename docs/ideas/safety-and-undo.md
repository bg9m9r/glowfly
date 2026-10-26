# Ideas: Safety, undo and errors

Previews, the operation journal, undo, in-progress operations, disabled actions and error messages. Untriaged; see [README.md](README.md).

### Preview before you act

- Draw the preview on the real lane graph (both sides of a merge, branch lanes), with a
  before/after view side by side
- Replay rebase, cherry-pick and drop previews with git merge-tree --merge-base per step (git
  2.40+) so merge drivers, rerere and merge.renames apply as in the real run
- Preview a cherry-pick or revert of a merge commit with its mainline (from the mainline
  dialog), and preview Revert
- Preview the interactive rebase panel's list (reorder, squash, reword, exec) and the graph's
  reword and squash edits
- Preview a pull, a Git Flow finish and a stash pop
- Preview a rebase.rebaseMerges rebase and octopus merges in drop plans
- A Cancel button and a progress line for long previews; lift the 500-commit replay cap
- Command palette entries for the preview actions
- Offer Preview from the conflict check report and chip popover, and from the drop and hard
  reset confirmations
- Show what happens to uncommitted changes (the auto-stash and whether putting it back
  conflicts)
- Click a preview row to see its rewritten diff against the original commit
- A preference that makes the merge, rebase, reset, cherry-pick and drop items always preview
  first

### Operation journal

- Show the remote-tracking branches' and refs/stash reflogs in the timeline
- Filter the timeline by branch or HEAD, and search the labels and commit summaries
- Day headers in the list (Today, Yesterday, then dates)
- Up and Down to move between rows, Enter for Restore..., and a bindable shortcut to open the
  journal
- Expand a finished rebase's row to show the steps the timeline folds away
- Restore every branch a rebase --update-refs moved from its one note
- Notes for operations run in the terminal panel or outside Glowfly
- Notes for stash drops and Discard all, with stash states in the timeline
- Draw the preview's history with the real lane graph instead of one lane
- Load more than the newest 2,000 states
- Compare a state with now as a plain diff
- Say when git will expire a state (gc.reflogExpire, gc.reflogExpireUnreachable)
- Notes for jobs run in other repositories (repo sync pulls)
- An Undo or Redo note's operation name comes from core's English description, so it reads in
  English in other languages; key it to the language tables instead
- Stop a long git fsck from the Lost commits tab
- Throttle Lost commits reloads on watcher refreshes in very large repositories
- Show the files and diff a dropped stash holds in its preview
- Branch from a dropped stash (git stash branch <name> <commit>)
- Undo for Put back in stash list (drop it again)
- Count the commits under each lost tip and show the whole lost segment
- Say when git will prune a lost commit (gc.pruneExpire)
- A palette command and a View menu item that open the journal on Lost commits
- Show the lost count on the Lost commits tab label
- Filter or search the Lost commits list (stashes only, by author, by text)
- Tell Glowfly's own auto-stashes and rebase autostashes apart from other dropped stashes
- Say "lost" in an unreachable row's accessible name
- Recover dangling blobs (files staged but never committed)
- Jump from a lost tip to its marked row in the States tab

### Disabled actions say why

- Other setup-gated entries stay enabled and open their setting: pull request and issue items
  that need a connected account open Preferences > Integrations, and an issue tracker that isn't
  chosen opens Repo-Specific Preferences > Issue Tracker
- When Preferences opens from a setup-gated entry, focus and highlight the matching field (diff
  tool, merge tool or editor)
- Footer reasons for the form dialogs whose primary is disabled only by a field problem with no
  reason given (clone, init, tag, new and rename branch, worktree create and move, submodule add
  and edit, publish, create PR, Git Flow dialogs, credential prompt)
- Reasons for the remaining preference-page buttons with more than one blocker (the integrations
  token Connect: token or user missing)
- One shared BUSY constant: replace the copies of "Another operation is still running." in the
  patch, stash, mainline, submodule and worktree dialogs and `state/remotes.rs` with
  `state::blocked::BUSY`
- Draw the rebase panel's blocker with the shared reason helper so every disabled reason has one
  look (it is red today)
- The multi-commit selection panel's disabled Cherry-pick and Revert buttons say why only on
  hover; give them a reason line like the other panels

### Disabled menu items explain on hover

- Keep the toolbar dropdowns open when a blocked item is clicked (their popup closes on any click)
- Let a blocked menu-bar submenu (Open recent) still open with its entries greyed, instead of
  being a flat blocked item
- Show the rich preview tooltip (what Fetch all or a Pull variant would bring in) for blocked
  toolbar dropdown items as well as the reason
- Give the dialog lists that grey out rows without an info mark (the fork picker's "added as"
  rows, the issue picker when full) the same mark, focus and tooltip
- Check with NVDA and VoiceOver that a focused blocked item reads its label, "unavailable" and
  the description, and tune the role or description if not
- Up and Down arrow focus that stays inside an open context menu and wraps at its ends, so the
  keyboard walks the items, blocked ones included
- A preference to show the reason inline under a blocked item instead of in a tooltip, for
  people who don't hover
- The saved-filter rows' disabled Move up and Move down icon buttons in the filter popover say why
  only through egui's disabled look; give them the info tooltip on hover and focus

### Undo / Redo

- Undo across restarts by rebuilding actions from the HEAD, branch and stash reflogs
- Undo of Create tag and push: also delete the tag from the remote it was pushed to
- Undo a rebase whose branch can't be known (a detached HEAD rebase continued from the banner)
- Undo of branch rename and of the new branch made by `worktree add -b`
- Undo of a Git Flow start or finish as one step
- Roll back the remote-tracking refs a pull fetched when undoing it
- Restore a popped or dropped stash at its original position in the list instead of at the top
- Undo of a pop or apply onto a working tree that already had changes
- Name the picked or reverted commits in the undo label of a sequence finished after conflicts
  (it now says N commits)
- Undo of a merge, cherry-pick or revert continued in the terminal panel or outside Glowfly
- Undo of discarding an untracked file or a symbolic link (only regular files are saved for undo)
- Undo of a stash drop across restarts, by finding the dropped commit in the stash reflog or with
  fsck

### In-progress operations stay visible and safe

- Undo for aborting a rebase, a patch series or a multi-commit cherry-pick or revert
- Undo for Skip commit and Skip patch (the resolved files of the skipped step)
- Bisect in the strip: the good and bad counts, and Mark good / Mark bad
- Keyboard shortcuts for Continue and Commit merge
- A status-bar chip variant of the strip, and a preference to hide the strip
- Hover tips on the conflict header menu's items (what Save with markers and Mark resolved do)
- Translate the remaining English text in the conflict view body and the rebase and patch banner titles

### Clearer prompts and safer controls

- Stop in repo sync also kills the git processes already running (a cancellable gitcli run)
- Repo sync: Retry all failed rows at once, and Copy error from a failed row
- Host-key prompt: list the other names or addresses the key is known by as their own field
- Host-key prompt: a Copy fingerprint button and a hint linking to the service's published
  fingerprints
- Explain ssh's "host key has changed" refusal in the failed job's error, with the
  known_hosts line to check
- Sign-in dialog: a caps-lock warning on the secret field
- Terminal: Kill all names the programs running in each terminal before stopping them
- Commit more-actions menu: further per-commit options (sign-off trailer, sign this commit)
- Graph search: Esc in a non-empty field clears it before a second Esc closes the bar
- The commit button's label is cut with "…" in a narrow panel; show the whole label on hover
- The search bar's fixed count slot leaves the query field at its 160 px minimum in a
  740 px graph pane; shrink the slot or move the count under the field when narrow

### Errors say what to do

- A Fetch-then-fast-forward fix for a sidebar Fast-forward that was rejected as non-fast-forward
  on a non-HEAD branch
