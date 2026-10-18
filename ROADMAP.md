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
  top of TODO.md, so they are built first; then come the priority sections and
  [Later](TODO.md#later), which holds Milestone 2's triaged follow-ups.
- **Follow-ups go to [Ideas](TODO.md#ideas)**, which is not planned work. Finishing an item tends
  to suggest more follow-ups than it closes, so nothing in Ideas is built until a triage moves it
  to Later or a milestone. Triage when a milestone closes.
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

The [Later](TODO.md#later) queue: 124 follow-ups kept by the 2026-10-18 triage because a daily
user would miss them (data safety, correctness, platform behaviour, and gaps in common
workflows), plus the feedback from Milestone 1's users and hands-on use on macOS and Linux *(you
or a volunteer)*.

Exit criteria:

1. Later is empty: every bullet built, or moved to Ideas with a reason.
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
- **Milestone 1.** The five items in its TODO.md section, about a day of campaign time. They add
  new CI jobs, which take a few CI round trips each. With a few days of daily use *(you)*,
  Milestone 1 is about 3 to 5 days away.
- **Why follow-ups go to Ideas.** Before the triage, Later held 692 bullets. Over the 36 landings
  before it, each landing closed about 1.4 Later bullets and added about 9 (50 closed, 320 added),
  so the queue had no end date. The triage kept 124 for Milestone 2 and moved 567 to Ideas (1 was
  already built), and new follow-ups now go to Ideas.
- **Milestone 2.** The 124 Later bullets in 46 sections. Landings on Later have closed 5 to 8
  bullets each, so about 20 landings, or 6 to 10 hours of campaign time, plus the fixes from
  Milestone 1's daily use. About 1 to 2 days of campaign time once Milestone 1 ships.
- **Ideas** (567 bullets) has no estimate on purpose. The largest groups came from Diff viewer
  upgrade, Command palette, In-app pull request view and File actions.

## Status

- 2026-10-18: Feature build-out done. Milestone 1 started: its test and release items are at the
  top of TODO.md. Later triaged: 124 bullets kept for Milestone 2, 567 moved to Ideas.
