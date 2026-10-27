# Glowfly roadmap

What Glowfly is for and how its work is planned. The plans themselves are on GitHub: see
[How planning works](#how-planning-works).

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

- **GitHub is the source of truth.** The [Glowfly Project](https://github.com/users/bg9m9r/projects/2)
  and the [GitHub milestones](https://github.com/bg9m9r/glowfly/milestones) hold what is
  planned, its priority, its status and each milestone's exit criteria. This file holds only
  the goal, what is out of scope, and how planning works; when they disagree, GitHub is right.
- A **milestone** is a state of the project with exit criteria that can be checked, written in
  its GitHub milestone's description. It is done when every criterion is met, not when its
  issues are closed. Criteria marked *(you)* need a person: hands-on use, an account, or a
  release decision. The rest are checked by automated tests or CI.
- **Issues** hold the items, each in a GitHub milestone when a milestone needs it. The
  Project's columns are Inbox (not triaged), Backlog (triaged and scheduled, not next), Ready
  (the focused set to build next, refilled from Backlog by milestone, then Priority), In progress,
  In review and Done, and its Priority field (P0 to P3) orders Backlog and Ready. Issues labelled `needs-decision` wait on the owner.
- **Follow-ups go to [docs/ideas](docs/ideas/README.md)**, which is not planned work. Finishing an
  item tends to suggest more follow-ups than it closes, so nothing there is built until a triage
  promotes it to an issue. Triage when a milestone closes.

## Milestones

- [Feature build-out](https://github.com/bg9m9r/glowfly/milestone/3)
- [Milestone 1: first release](https://github.com/bg9m9r/glowfly/milestone/1)
- [Milestone 2: daily-driver polish](https://github.com/bg9m9r/glowfly/milestone/2)
- [1.0](https://github.com/bg9m9r/glowfly/milestone/4)
