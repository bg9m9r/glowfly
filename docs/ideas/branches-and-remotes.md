# Ideas: Branches, tags and remotes

Branch and tag operations, remotes and Git Flow. Untriaged; see [README.md](README.md).

### Branch operations

- Pin a branch to the left (graph lane order, per-repo persistence, jump to the pinned branch)
- Choose a local branch as the upstream in the Set upstream dialog
- A shortcut for renaming the checked-out branch, and renaming any branch through a two-step
  palette entry
- Delete local + remote when the branch isn't merged into HEAD (force), in one step
- A "Commit 1a2b3c4" submenu in the graph's label menu, so a row with labels reaches the commit
  menu from the chips too
- Push the current branch to a chosen remote branch from its row
- Show the remote branch's name (not a commit) in the interactive rebase dialog started from a
  remote row
- Compare with the current branch from the local branch and tag menus too
- Push to all remotes: a log line per remote saying what it took (new, updated, up to date)
- Refuse force pushing a branch the code host protects, from its branch protection rules, as the
  default branch and Git Flow bases are refused today
- Force push with lease of a branch whose `branch.<name>.pushRemote` or `remote.pushDefault`
  (`PushRemotes`) names another remote than its upstream's: offer it toward that push remote too,
  leasing its tracking branch there, instead of only toward the upstream
- Merge from the menus names the short branch (so the merge message reads `Merge branch 'x'`), which git resolves to a same-named tag first; pass the full ref and write the usual message ourselves
- Undo plans (`undo/refs.rs`: `branch -D`, `tag -d`, recreate, `--set-upstream-to`), Git Flow init and finish, and the create-branch and create-tag commands still put names before options end; their names are typed and validated or already passed the load, but `--` (with the undo classifier taught it) would make that local

### Force push with lease from any branch, and push to all remotes

- Put the default-branch warning in its own paragraph of the confirmation
- Tests for the Git Flow base warnings, not only the default branch's
- Build the warning text when the confirmation opens, so it can't go stale
- Say which push URLs were read when a remote has several `pushurl`s and only some match
- Run Push to all remotes as a job, so it can be stopped and shows progress

### Check out a remote branch with its tracking branch fast-forwarded

- Offer Merge, Rebase or Reset to the remote for a diverged tracking branch, not only checking it
  out where it is
- Undo of a checkout that fast-forwarded the tracking branch also moves the branch back (only
  HEAD goes back now)
- Palette entries for remote branches a local branch tracks, checking out and fast-forwarding it
- The tracking New branch dialog: suggest a name, and a box to leave the upstream unset
- Say so when the checkout fails after the tracking branch was fast-forwarded, or move it back
- Explain a failing `merge-base` (unusual fetch refspecs) instead of a bare "Checkout failed"
- Wrap the long `//!` lines in `snapshot_tests_branch.rs` and `snapshot_tests_remote_branch.rs`

### Keep the remote in step on rename and remote delete

- Rename a branch whose upstream is gone: push the new name and make it the upstream, with
  nothing to delete
- Rename on the remote through the code host's branch rename API, so open pull requests follow
- Use the code host's default branch, when known, beside the remote's HEAD for the refusal
- Offer Pull, then rename, for a branch behind its upstream instead of only refusing
- Delete local + remote: unset the upstream of other local branches tracking the same branch
- Remember the "Also rename it on the remote" choice per repository

### One Delete for a local branch, with Delete anyway

- List the commits Delete anyway would leave on no branch (subjects, as the operation preview's
  lost commits), not only their count
- Fold Delete together with the remote branch into the same dialog (a box for the remote copy),
  so the menu has one delete row
- A palette entry for deleting a local branch that opens the same dialog
- Say in the dialog when a tag or stash still reaches the commits only the branch has

### Double-click a branch to check it out

- Double-click a tag (graph chip and TAGS row) to check it out detached, through the existing
  tag checkout and its detached-HEAD warning
- Double-click the "+N" chip to choose which hidden branch to check out
- Double-clicking a branch that another worktree has checked out opens that worktree's tab
  instead of logging the refusal
- Branch chip and row tooltips mention that a double-click checks the branch out
- Double-clicking the checked-out branch does nothing instead of logging that it is already
  checked out
- Enter on a selected graph row whose commit carries one branch checks that branch out

### Git Flow

- Finish options: rebase on develop first, squash, and push after finishing
- Finish options: skip the tag, sign it with a chosen key, delete the remote branch,
  force-delete
- Branch-menu and graph-menu entries for flow branches (finish this feature, start a hotfix
  from here)
- Bugfix and support branch types on the page (their default prefixes are already written)
- Fetch before Start, and publish or track flow branches on the remote
- Read the newer per-branch config layout (gitflow.branch.<name>.type / parent) and
  gitflow.origin
- Show how far each flow branch is ahead of or behind its base in the panel and the GIT FLOW
  section
- Bugfix and support branch groups in the panel and the GIT FLOW section
- List flow branches that exist only on a remote, with a track and checkout action
- Link a branch started from an issue back to the issue (mention it in the merge or tag
  message, move the issue to in progress)
- Publish, pull and rebase actions for flow branches in the panel and the section
- Order release and hotfix rows by version (1.10.0 after 1.9.0)
- Hide or show the GIT FLOW section from a left-panel context menu
- Show which issue a flow branch was started from in the Start dialog
- Remember a stopped Finish across restarts (stored in the repository's git dir), not only for
  the session's tab
- Continue a Finish that stopped for a reason other than a conflict (a refused checkout, a
  branch -d that refused, a tag that failed)
- Offer Continue finish from the Finish dialog of a branch whose Finish stopped, instead of
  planning all the steps again
- Recognise the committed merge when the target's history isn't in the loaded graph (hidden
  refs, smart branch mode), e.g. with an ancestry check in the runner
- Warn in the pull request merge's "delete the source branch" option when the source is the
  production or development branch
- Ask before renaming the production or development branch, and offer to update the Git Flow
  config to the new name
- Offer to pick a new production or development branch in Preferences > Git Flow right after
  one is deleted
- Translate the rest of Git Flow: Finish step texts, job headlines and refusals, the page, the
  Start and Finish dialogs and the panel (English today, shown inside the translated sentences)
- Show the base-branch warning in the delete confirmation in the warning style (a form
  message) instead of a plain paragraph, if the shared danger dialog gains a slot for one

### Remote management

- Publish to GitLab, Bitbucket and Azure DevOps
- Add an existing remote URL from the Publish dialog
- Undo removing or renaming a remote (re-add it with its URLs and fetch, or rename it back)
- Rewrite the saved graph filter's hidden or shown-only refs when a remote is renamed or removed
- Edit remotes defined outside the repository's config, and every URL of a remote with several
  fetch or push URLs
- Per-remote options: fetch refspecs, tag fetching, prune on fetch, the default branch
- Offer Add remote… when fetch, pull or push finds no remote
- Fork on Bitbucket Cloud and Bitbucket Data Center (find the user's fork and create one)
- List forks and fork on Azure DevOps
- Fork into an organization or group, under another name, or with only the default branch
- Show the user's own fork first in the Add a fork list, with a one-click Add
- View on service for self-hosted servers whose web pages use another port than their SSH address
- After forking, offer to make the fork the push remote of the current branch, or to rename
  origin to upstream
- View a remote branch or commit on its hosting service from the branch and commit menus
- Search a service's forks on the server, beyond the newest 1000
- Accept Unicode IDN hosts with combining marks (virama, ZWJ/ZWNJ), which the host-character allowlist (`host_char`) now refuses
- Accept IPv6 literal hosts with a zone (`[fe80::1%25eth0]`), which are refused now
- Push to a URL set as `branch.<b>.pushRemote` as git does, instead of refusing it
- A remote, upstream, branch or tag left out of the snapshot because its name reads as an option just vanishes; say so (a notice in the REMOTE section or the trust prompt) so the user can rename it
- Push-remote values (`branch.<name>.pushRemote`, `remote.pushDefault`) in `repo/push_config.rs` aren't filtered at load; they are safe today only because `pr_push_target` pushes to names it finds in `snap.remotes`

### Tag management

- After deleting a tag locally, offer to delete it from the remotes too
- Offer Annotate… from the graph's tag labels and from the command palette (two steps: pick a
  lightweight tag, then the message)
- Edit the message of an already annotated tag (re-annotate with the old message prefilled)
- Push the annotated tag over a remote's lightweight copy (force-push one tag, after a
  confirmation)
- Honour tag.forceSignAnnotated too (git applies it only without -a), and say in the Annotate
  dialog when the tag will be signed
- Show annotated tags' tagger date on tag rows (needs the tag time in the snapshot)

### One label for a branch and its copies on every remote

- Honour `checkout.defaultRemote` (and `remote.pushDefault`) when picking the lead copy of a
  remote-only chip
- Also fold copies named like the upstream's branch when it differs from the local name (main
  tracking origin/trunk, with fork/trunk on the commit)
- A small count or distinct mark on a chip that folds copies on more than one remote
- Group a folded chip's refs under one submenu in the right-click menu, with actions such as
  delete on every remote
- Map remote-tracking refs to remotes through each remote's fetch refspec instead of the name
  prefix
- Say in the tooltip when a local chip's own upstream is at another commit while a copy on
  another remote is level with it
