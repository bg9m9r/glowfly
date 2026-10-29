# Ideas: Hosting integrations

Pull requests, issues, CI status, Trello, deep links and the cross-repo dashboard. Untriaged; see [README.md](README.md).

### CI / build status

- Name the permission build status needs in the GitHub token help, once it is confirmed which
  fine-grained permissions expose check runs and commit statuses
- Open the build page from the LOCAL and REMOTE branch menus and from graph ref labels
- A tooltip with the checks line on graph ref label markers
- GitLab: job counts for the pipeline (N of M jobs passed) and external commit statuses
- Azure Pipelines: partially succeeded builds as warnings, and optional build policies counted
  apart from required ones
- Bitbucket Data Center: one batched build-status call for many commits
- GitHub without GraphQL access: a REST fallback for branch-tip status
- Branch-tip status for branches on other remotes (forks) and their services
- Re-read pending pull request checks sooner than the 5-minute refresh
- A desktop notification when a build on the checked-out branch finishes
- A Preferences switch to turn off branch-tip status requests
- Re-run a failed check, job or pipeline from the pull request panel
- Show a check's log, annotations or failure output in the panel instead of only linking to it
- GitLab bridge (downstream pipeline) jobs in the checks list
- Bitbucket Data Center required builds (merge check) and Bitbucket Cloud required-builds merge
  checks as Required tags
- Group the checks list by workflow / stage with collapsible groups
- A "Show only failing" filter for long checks lists
- The same per-check list for a branch tip or a selected commit (commit details panel)
- Bitbucket Data Center build durations and build numbers from the newer builds API

### PR and issue filters

- Azure Boards iterations (the current or a named iteration) as a sprint condition for work items
- A typed filter query (author:, label:, base:, draft:, review:) with a syntax reference, as an
  alternative to the form
- Conditions on other people: author, assignee or reviewer by name, involves, reviewed by
- Pull request conditions on CI status, review outcome and created or updated dates
- Issue filters for closed or all states, and a choice of sort order
- Saved filters shared by every repository, and importing and exporting filters
- Palette commands and a shortcut for switching the shown pull request or issue filter
- Searching closed or merged pull requests on the service (filters only narrow the open list)
- Label suggestions on Jira Data Center; fix versions and sprints without a Jira project set
- Mentions on trackers that can't search by them (GitLab to-dos, Jira watchers or comments)
- A fuller search for Azure's Mentioning me, which covers only the last 30 days
- Count requests to grandparent and higher GitHub teams (walk parent chains past the direct parent)
- GitLab approval rules and Bitbucket default-reviewer groups that name the user's groups as eligible reviewers
- Read the user's teams again on Refresh, so a team joined or a scope granted mid-session counts without signing in again
- Say on a row which team or group a review request came through (e.g. "via acme/core")
- A choice to count only personal review requests in Needs my review
- A quiet hint when the token can't read teams or groups, naming the scope to add (read:org or Members for GitHub fine-grained tokens, Identity (Read) on Azure DevOps)
- Azure DevOps memberships past 200 groups, and Azure DevOps Server (on-premises) identity lookups

### Create and view issues

- Change an issue's state (Jira: its status, through a transition), assignees and labels from the panel
- Threaded replies with show and collapse (GitLab discussions, Jira comment threads)
- Post the comment with the commit shortcut (Ctrl+Enter, Cmd+Enter on macOS) from the comment box
- Show a GitHub issue's locked conversation in the panel and disable Comment before posting
- GitLab comments made only of quick actions (/close, /assign): show the summary GitLab answers with and reload the issue instead of a failed post
- Quote as a real block quote in Jira Cloud's rich text and Azure Boards' HTML (now plain "> " lines)
- Quote the description, or only the selected part of a comment
- Edit and delete your own comments from the issue panel
- Preview a Markdown comment before posting
- Suggest people to @-mention while typing a comment
- A palette command to comment on the open issue (focuses the comment box)
- Choose the base branch when creating a branch from an issue
- GitHub issue types and milestones when creating an issue
- GitLab milestones, epics and several assignees (paid tiers) when creating an issue
- Jira priority, components, fix versions, sprint, Epic Name and other required custom fields when creating an issue
- Open the tracker's own new-issue page in the browser when creating in Glowfly fails on required fields
- GitHub issue templates in the Create issue dialog
- Remember the "Create a branch for it afterwards" choice
- Open the issue panel from issue references in commit details and branch names
- Bitbucket Cloud components, versions and milestones when creating an issue
- Bitbucket Cloud issue attachments and the change history (state changes) in the panel
- Azure DevOps area path, iteration path, priority, severity and other required custom fields when creating a work item
- Azure DevOps child links and related work items in the panel; choosing a parent from any work item by search
- Show Azure DevOps inline images from descriptions and comments (signed-in attachment downloads)
- Trello due dates, checklists, attachments and cover images in the card panel
- Trello card comments past the first 1000 actions
- Choose a Trello card's position in its list (top or bottom) when creating
- Check a Trello token's write access before Create instead of after a refused create
- Azure DevOps work item priority and Jira priority in the panel's details
- Keep issue comment drafts across restarts (now kept per issue only while the tab is open)

### Limits found while building integrations

- Pace per-PR detail lookups by each service's rate-limit headers instead of a fixed max age
- Read per-PR details a few at a time in parallel once the test mock supports calls from other threads
- Let users add their own words to the list of Jira non-project prefixes
- Read the cached Jira project keys again when a key's project is missing (a project created after the first read)
- Skip 8-digit dates (`20241016-release`) like year-like branch names; they are still read as issues
- An explicit `ab#123` or `wi-123` branch marker for Azure Boards, like `gh-` and `issue-`
- Ask once, on the first year-like branch in a repository, instead of relying on the Issue tracker page to be found

### Trello

- Sign in through Trello's browser authorization flow (redirect back to Glowfly) instead of
  pasting the API key and token
- Show card labels as coloured chips, and filter the ISSUES list by list or label
- Recognise board-local card numbers (#12) and bare short ids in commit text, once a board is
  chosen
- The ISSUES error's Connect… button opens Integrations on the Trello page
- Wait out a rate limit and retry automatically instead of waiting for a manual refresh
- A token lifetime choice (1 day / 30 days / never) on the Allow a token link
- Add the commit or pull request link to the card as an attachment (needs a write-scope token)

### "Push and start a PR"

- On a LOCAL row, create a pull request from the checked-out branch into that branch (the REMOTE row has it)
- Choose source and target by selecting two branch rows
- Remember the last target used per branch and preselect it next time
- For a diverged branch, offer a force push with lease and then the pull request (today it is refused with the reason)
- A Preferences setting to create pull requests as drafts by default
- Push-and-create entries in the graph's commit and ref-label menus
- Find the upstream repository under any remote name by asking the service for a fork's parent, not only `upstream` or the code host's remote
- When the code host's remote is the user's fork, also list the upstream repository's pull requests, so a fork pull request shows on its branch
- Push to your fork… in the branch menus, the toolbar's Push dropdown and the palette, not only in the create dialog
- Keep the create dialog's typed title, description and linked issues across the Fork dialog detour
- On a fork remote's REMOTE row, create a pull request from the checked-out branch into it, and open the branch's existing pull request
- Push a stack's layers to a fork and open their pull requests against the upstream repository
- A remote that fetches the upstream repository but pushes to a fork (remote.<name>.pushurl) as the source repository
- An Allow edits by maintainers choice for fork pull requests on GitHub
- Remember the last target repository per repository and preselect it
- Say in the push item's tooltip when push config (pushRemote, pushDefault) chose the remote
- Read push.default and remote.<name>.push refspecs when naming the branch a pull request is pushed to

### In-app pull request view

- Azure DevOps threads, votes (approve / wait for author / reject) and completion with a merge
  type in the panel
- A note when the local target branch differs from the service's base (the base SHA is loaded now)
- Show the pull request from the graph's PR chip, a local branch's menu and the command palette
- Fetch more history from the no-common-commit note in a shallow clone
- Edit or delete your own comments, and reactions
- Re-request a review, and a participants list
- Mark a draft ready for review, and enable auto-merge
- Choose the commit email for a merge
- Open a single comment or review in the browser
- Code comment threads for GitLab diff discussions, Bitbucket inline comments and Azure DevOps
  file threads in the panel
- Switch straight between Approve and Request changes on Bitbucket (now: remove one vote, then
  cast the other)
- Bitbucket's other merge strategies: Cloud's squash_fast_forward and rebase_merge; Data
  Center's ff (fast-forward if possible), rebase-no-ff and squash-ff-only
- Name Data Center merge vetoes (and Cloud's failed merge checks) in the mergeability line
  instead of the general "rules block merging"
- Who may merge on Bitbucket Cloud, from the user's repository permission (unknown now)
- Delete the source branch through Bitbucket with the merge (Cloud's close_source_branch, Data
  Center's delete-after-merge) instead of a git push
- Word "deletes merged branches itself" for a Bitbucket Cloud pull request's own
  close-source-branch flag
- Merge with the server's default strategy when the Data Center merge settings can't be read
  (now: a merge commit)
- Data Center tasks (BLOCKER comments) shown and resolved as tasks
- Data Center review batches (8.x pending reviews): finish a started review in the panel
- Reload by itself later when Bitbucket Cloud is still merging after the status polls
- Say the summary is optional for Request changes on Bitbucket in the summary box's hint
- Comment counts on the Files changed rows
- Highlight the whole range of a multi-line comment, and markers on folded rows of the whole-file
  view
- Remember Show resolved / Show outdated per repository
- Load threads with more than 100 comments in full (a link opens the rest on the service now)
- Previous / next code comment thread in the diff
- Markdown bodies: show remote images (fetched in the background, size-capped, opt-in per
  repository) instead of their alt text as a link
- Markdown bodies: emoji shortcodes (`:robot:`), `#123` / `!12` references and `@mentions` as
  links to the service, and relative links resolved against the repository's web address
- Markdown bodies: a few safe HTML tags (`<details>`, `<br>`, `<sub>`, `<kbd>`) instead of
  dropping them, and select text across styled runs (each run selects on its own now)
- A Preview tab on the comment and reply boxes that renders the Markdown being written
- Remember the Checks section's collapsed state per repository
- Load the checks (and page) by itself once the repository is trusted, instead of on the next open
  or tab switch
- Up/Down keys to move through the Commits tab's list
- A commit's own file list inside the Commits tab (Files changed scoped to one commit or a range)
- Compare from the loaded page's head SHA when it is newer than the listed head
- Keep a fetched pull request head from being pruned by gc (a ref in a private namespace, removed
  when the panel closes)
- Fork pull requests on Azure DevOps and Bitbucket Cloud: fetch the source branch from the fork's
  URL (they publish no head ref)
- GitLab fork merge requests: read the source project's path by its id, so a fork remote here is
  matched as the source (the list names it only by id)
- Checking out a fork pull request whose fork isn't a remote here: offer to add the fork as a remote
- Keep fork pull requests' branches visible under Relevant branches by repository, not only by name
- Commits tab rows: avatars, signature badges, each commit's checks, and cherry-pick, create
  branch and compare in the row menu
- Command+Enter in the merge form to open its confirmation
- Hide the Commits tab's center hint when the pull request brings no commits
- Comment on a whole file (subject_type file) from the Files changed row or the pane header
- Draw the new comment box between the diff rows under its line instead of in the pane under the
  diff
- Start code comments from the Commits tab's per-commit diffs
- Keep several unposted line comments at once (one per line), and keep them across closing the
  panel or restarting
- Ask before another view replaces the pull request panel while a line comment is unposted
  (opening an issue, a diff outside the panel, the file history or blame still drops it)
- A key to start a comment on the line at the diff cursor, and one to extend it
- Show how many comments the pending review holds on Finish review, and discard a pending review
- Edit or delete a pending review comment before the review is submitted
- Suggestions that span deleted lines, and applying or batching suggestions from the panel
- Map a draft's lines onto the new head when the files are compared again, instead of asking to
  pick the line again
- Grow the Files changed comment pane to fit the new comment box when a draft opens it (it keeps
  its 220 px default and scrolls today)
- Start a code comment on a mapped line when the local branch differs from the service's head
  (post it against the head's line number)
- Map deleted-line comments across a changed merge base (a line map between the service's base
  and the local base)
- Map a multi-line comment's range when its first line changed locally (it shows as its last line
  now)
- Map outdated comments' original lines onto the local version when that commit is here
- Say on the Files changed rows which files have comments not on the local lines
- GitLab draft notes: write several comments and submit them at once as one review
- GitLab's Request changes reviewer state (newer servers) in the review form and the timeline
- Rebase the source branch on GitLab from the merge form when the project needs it (need_rebase)
- Say which GitLab rule blocks a merge (approvals left, unresolved threads, pipeline, merge time)
  instead of the generic blocked line
- Ask for the GitLab password when the project requires re-authentication to approve
- Merge when the pipeline succeeds (GitLab auto-merge) from the merge form
- Show approvals required and approvals left from GitLab's approvals endpoint in the details
- Say in the Squash help whether GitLab also adds a merge commit (the project's merge method)
- Log a GitLab Comment-choice review as "Commented on !N" rather than "Reviewed !N"
- GitLab code comment threads (discussions) and new code comments on diff lines in the panel
- Delete the local branch after a squash or rebase merge when its tip is the head that was
  merged (git can't tell those are merged, so the form disables it now)
- Delete the local branch of a fork's pull request (a branch checked out from the fork)
- Fast-forward the local target branch after a merge, in the same job
- Remember the "Delete … after merging" and "Delete local branch" choices per repository
- Undo of a merge's local branch delete: check the branch out again when it was checked out
  before, the working tree is clean and HEAD is still on the target (now it stays on the target)

### Deep links

- Offer to add the link's remote when the repository matched by root commit has no remote with that URL
- Copy link to a commit or tag on a chosen remote
- A palette step to paste and open a glowfly:// link
- Links to a file or line at a commit, and to a stash or pull request
- A progress toast while a link is resolved against long recent lists
- Linux fallback that edits mimeapps.list when xdg-mime isn't installed
- Show machine-wide or packaged registrations in the Preferences status on Windows
- An https redirect page so links can be shared where custom schemes aren't clickable
- Show which app opens glowfly:// links on macOS in Preferences (LaunchServices' default handler for the scheme)
- Hand a link that starts Glowfly.app over to an unbundled glowfly window already running, instead of opening a second window
- A folder macOS hands over while a dialog or Preferences is open waits behind a toast, as a link does
- Hand a path that isn't valid Unicode to the running window too (it opens its own window now; the request is JSON text)
- Open the repository holding a file dropped on the Dock icon (declare files too, as an Alternate viewer)
- An Open in Glowfly Finder Quick Action or Services menu entry for folders
- Start-up can wait about 7 s instead of 3 s to hand a path over; cap the connect and reply timeouts at the time left
- Any local process can stall the hand-over listener for 3 s per connection; serve each connection off the accept loop or drop it sooner

### Cross-repo dashboard

- Keep the last results on disk so they show at startup before the first refresh
- Reuse the pull request lists open tabs already loaded instead of asking those repositories again
- Read checks and votes past the first pull requests on services read one pull request at a time
- Refresh in the background while the dashboard stays open, and notify about new review requests
- A keyboard shortcut for opening the dashboard
- Remember the Mine / Review requested / Assigned choice across sessions
- Add repositories found by scanning a folder, beyond tabs, favorites and recent
- Treat service repository names that differ only in case as one repository when grouping clones
- Open issue rows in the in-app issue panel once issues can be viewed in Glowfly
- Apply saved PR and issue filters (labels, milestones) across repositories
- Check out a pull request's branch from the dashboard
- Created/updated dates on pull request and issue rows (the service models don't carry them yet)
- Show the requesting team on Review requested rows
- Say which list failed (pull requests or issues) on a repository's error line under Assigned
- List assignees in the pull request tooltip and mark rows assigned to someone besides the user
- Mark Assigned rows that are also the user's own or wait for the user's review
- Find assigned pull requests past the first page of the open list with the services' assignee search
