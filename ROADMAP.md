# Glowfly roadmap

What Glowfly is for, the milestones it is working towards, and where it stands. This file is the
source of truth for direction and status. The work that gets it there is tracked as
[issues](https://github.com/bg9m9r/glowfly/issues) on the
[Glowfly Project](https://github.com/users/bg9m9r/projects/2). When a milestone's scope, exit criteria or status changes, update this file in the same
commit.

## Goal

A fast, native Git client that someone can use as their only Git GUI every day, on Windows,
macOS and Linux: the commit graph, staging and committing, branch and history work, conflict
resolution, and pull requests and issues on the common hosting services, without a vendor
account.

## Out of scope

These need a vendor account or cloud service rather than git:

- Sharing patches and code suggestions through a cloud service (local `.patch` files are
  supported)
- Cloud workspaces, teams and organizations
- Accounts, trials, licensing, surveys and analytics
- Status hooks for coding agents
- **Possible later:** AI features, such as generating commit messages and PR descriptions,
  explaining a commit or branch, and composing commits. These could be built on the Claude API
  as an opt-in.

## How planning works

- A **milestone** is a state of the project with exit criteria that can be checked. It is done
  when every criterion is met, not when its items are ticked.
- **Issues** hold the items, each in a [GitHub milestone](https://github.com/bg9m9r/glowfly/milestones)
  when a milestone needs it. The Project's columns are Inbox (not triaged), Ready, In progress,
  In review and Done, and its Priority field (P0 to P3) orders Ready. Issues labelled
  `needs-decision` wait on the owner.
- **Follow-ups go to [docs/ideas](docs/ideas/README.md)**, which is not planned work. Finishing an
  item tends to suggest more follow-ups than it closes, so nothing there is built until a triage
  promotes it to an issue. Triage when a milestone closes.
- Criteria marked *(you)* need a person: hands-on use, an account, or a release decision. The
  rest are checked by automated tests or CI.

## Milestones

### Feature build-out: done

The daily-driver, power-user, integration and polish items (P0 to P3 and Preferences parity;
the [Feature build-out](https://github.com/bg9m9r/glowfly/milestone/3) milestone) are built, with tests on all three CI systems. Open leftovers: issues on Bitbucket, Azure
DevOps and Trello (Create and view issues) and Default group color (#3, which waits for repository groups, #239).

### Milestone 1: first release (in progress)

[Its issues](https://github.com/bg9m9r/glowfly/milestone/1).
The first build people are asked to install. release-please picks the version (0.2.0 from the
`feat:` commits since v0.1.0).

Exit criteria:

1. **Whole-app journeys pass headless** on Windows, macOS and Linux CI: clone, stage and commit,
   push and pull against a local remote, branch and merge, rebase with a conflict resolved in the
   merge tool, stash, and undo, each driven through the UI on a real fixture repository.
2. **Real input works**: smoke tests send real OS mouse and keyboard events to the release build
   on CI machines (Linux under Xvfb, Windows on the runner's desktop), including the first click
   after the window gains focus. This closes Input reliability.
3. **Installers work**: CI installs the MSI and DEB it built, starts Glowfly from them, and
   checks the update path from an older build against the published checksums.
4. **No known crash or data-loss bug** is open.
5. **The UI review's blockers and major findings are fixed**: the
   [2026-10-18 review](docs/reviews/ui-2026-10-18.md)'s items in the Milestone 1 milestone
   (operations that stay visible and ask before losing work, dialogs that keep typed text,
   consistent dialogs, buttons and controls, readable text and contrast in every theme,
   truncation with ellipses, complete translations).
6. **A few days of daily use** on real repositories *(you)*; what turns up becomes issues
   in this milestone.
7. **The release pull request is merged** *(you)*.

### Milestone 2: daily-driver polish

[Its issues](https://github.com/bg9m9r/glowfly/milestone/2). Most of the 124 follow-ups kept by
the 2026-10-18 triage were built during Milestone 1. The 2026-10-25 triage of docs/ideas then
promoted 54 issues on the bar of what a daily user would miss: data safety and correctness first
(P1), then gaps in common workflows and platform behaviour (P2), and some polish (P3). The
milestone also takes the feedback from Milestone 1's users and hands-on use on macOS and Linux
*(you or a volunteer)*.

Exit criteria:

1. The milestone has no open issues: every one built, or closed with its idea moved to
   docs/ideas and a reason.
2. Every bug reported from Milestone 1's daily use is fixed or deliberately deferred.
3. A day of hands-on use on macOS and on Linux *(you or a volunteer)*, with what turns up fixed.

### 1.0

Settings and file formats that later versions keep loading, signed builds on Windows and macOS,
and user documentation. To be defined.

## Forecast

As of 2026-10-18. Recompute from git history when a milestone closes.

- **Pace so far.** The project started on 2026-10-13. In 4½ days it has 434 commits and about
  170,000 lines of Rust, and 71 of 75 top-level items are done. A five-slot campaign lands about
  2.5 to 4 items an hour while it runs.
- **Milestone 1.** Four test and release items, plus 16 from the UI review. Most review items
  are app-wide (one shared widget, then every screen moved onto it), so each is one or two
  landings. At the last batch's 1.6 landings an hour that is about 15 to 20 hours of campaign
  time; the CI items take a few CI round trips each. With a few days of daily use *(you)*,
  Milestone 1 is about 4 to 6 days away.
- **Why follow-ups go to Ideas.** Before the triage, Later held 692 bullets. Over the 36 landings
  before it, each landing closed about 1.4 Later bullets and added about 9 (50 closed, 320 added),
  so the queue had no end date. The triage kept 124 for Milestone 2 and moved 567 to Ideas (1 was
  already built), and new follow-ups now go to Ideas.
- **Milestone 2** (updated 2026-10-25). About 54 issues, sized S to L. At the recent pace of
  roughly one landing per slot every 1.5 to 2 hours, that is about 80 to 110 slot-hours (about 16 to
  22 hours of campaign time with five slots), plus the fixes from Milestone 1's daily use. A rough
  guess until a few have landed; L issues may take more than one landing.
- **Ideas** (567 bullets) has no estimate on purpose. The largest groups came from Diff viewer
  upgrade, Command palette, In-app pull request view and File actions.

## Status

- 2026-10-25: Milestone 2 organised: a triage of docs/ideas promoted 54 issues (12 P1, 37 P2, 5
  P3); repository groups became #239 and Default group color (#3) left Milestone 1 to wait for it.
- 2026-10-22: Work moved from TODO.md to GitHub issues on the
  [Glowfly Project](https://github.com/users/bg9m9r/projects/2) and GitHub milestones; the
  1,610 untriaged ideas moved to [docs/ideas](docs/ideas/README.md).
- 2026-10-18: UI review of every snapshot (visual design and UX): 16 Milestone 1 items from its
  blockers and major findings, and its minor findings in Later. A batch of 17 landings
  (whole-app journey tests and Later work). The MSI's install pages built and checked by hand.
- 2026-10-18: Feature build-out done. Milestone 1 started: its test and release items are at the
  top of TODO.md. Later triaged: 124 bullets kept for Milestone 2, 567 moved to Ideas.
