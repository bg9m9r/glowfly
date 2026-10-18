# Trellis roadmap

What Trellis is for, the milestones it is working towards, and where it stands. This file is the
source of truth for direction and status; [TODO.md](TODO.md) is the backlog of items that get it
there. When a milestone's scope, exit criteria or status changes, update this file in the same
commit.

## Goal

A fast, native Git client that someone can use as their only Git GUI every day, on Windows,
macOS and Linux: the commit graph, staging and committing, branch and history work, conflict
resolution, and pull requests and issues on the common hosting services, without a vendor
account.

## How planning works

- A **milestone** is a state of the project with exit criteria that can be checked. It is done
  when every criterion is met, not when its items are ticked.
- **TODO.md** holds the items. The items a milestone needs sit in that milestone's section at the
  top of TODO.md, so they are built first; everything else is in the priority sections and
  [Later](TODO.md#later).
- Criteria marked *(you)* need a person: hands-on use, an account, or a release decision. The
  rest are checked by automated tests or CI.

## Milestones

### Feature build-out: done

The daily-driver, power-user, integration and polish items of TODO.md (P0 to P3 and Preferences
parity) are built, with tests on all three CI systems. Open leftovers: issues on Bitbucket, Azure
DevOps and Trello (Create and view issues), the views Language hasn't translated yet, and Default
group color (waits for repository groups).

### Milestone 1: first release (in progress)

The first build people are asked to install. release-please picks the version (0.2.0 from the
`feat:` commits since v0.1.0).

Exit criteria:

1. **Whole-app journeys pass headless** on Windows, macOS and Linux CI: clone, stage and commit,
   push and pull against a local remote, branch and merge, rebase with a conflict resolved in the
   merge tool, stash, and undo, each driven through the UI on a real fixture repository.
2. **Real input works**: smoke tests send real OS mouse and keyboard events to the release build
   on CI machines (Linux under Xvfb, Windows on the runner's desktop), including the first click
   after the window gains focus. This closes Input reliability.
3. **Installers work**: CI installs the MSI and DEB it built, starts Trellis from them, and
   checks the update path from an older build against the published checksums.
4. **No known crash or data-loss bug** is open.
5. **A few days of daily use** on real repositories *(you)*; what turns up goes into TODO.md
   under this milestone.
6. **The release pull request is merged** *(you)*.

### Milestone 2: daily-driver polish

The [Later](TODO.md#later) queue and the feedback from Milestone 1's users, plus hands-on use on
macOS and Linux *(you or a volunteer)*. Exit criteria to be set when Milestone 1 ships.

### 1.0

Settings and file formats that later versions keep loading, signed builds on Windows and macOS,
and user documentation. To be defined.

## Forecast

As of 2026-10-18. Recompute from git history when a milestone closes.

- **Pace so far.** The project started on 2026-10-13. In 4½ days it has 434 commits and about
  170,000 lines of Rust, and 71 of 75 top-level items are done. A five-slot campaign lands about
  2.5 to 4 items an hour while it runs.
- **Milestone 1.** The five items in its TODO.md section, about a day of campaign time. They add
  new CI jobs, which take a few CI round trips each. With a few days of daily use *(you)*,
  Milestone 1 is about 3 to 5 days away.
- **The Later queue does not converge yet.** It holds 692 bullets in 62 sections. Over the last
  36 landings, each one closed about 1.4 Later bullets and added about 9 follow-ups (50 closed,
  320 added). The landings that worked on Later itself closed 5 to 8 bullets and added 7 to 16.
  Frozen at its current size, Later would take about 35 to 50 hours of campaign time (5 to 8
  days). While follow-ups keep arriving at this rate it has no end date. Milestone 2 needs a
  triage and a rule for follow-ups before it can be estimated.
- **The largest Later sections** are Diff viewer upgrade (51), Command palette (28), In-app pull
  request view (27), File actions (25), Git LFS and Submodules (20 each), and Built-in terminal
  (18).

## Status

- 2026-10-18: Feature build-out done. Milestone 1 started: its test and release items are at the
  top of TODO.md.
