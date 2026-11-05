# Ideas

The ideas (untriaged polish, edge cases and follow-ups that are not planned work) now live on
GitHub as [`idea`-labelled issues](https://github.com/bg9m9r/glowfly/issues?q=label%3Aidea) in the
Backlog of the [Glowfly Project](https://github.com/users/bg9m9r/projects/2): one issue per
feature, each a checklist with one line per idea. Nothing in them is built until it is promoted.

- **Adding**: a follow-up, polish item or edge case found while building becomes a `- [ ]` line
  in the body of the matching `idea` issue. Read the body just before editing, since others may
  edit it too: `gh issue view <n> --json body -q .body > body.md`, add the line, then
  `gh issue edit <n> --body-file body.md`. If no issue fits, open a new `idea` issue titled
  `Ideas: <feature>` and put it in Backlog (`pwsh scripts/track.ps1 <n> Backlog`).
- **Promoting**: open a real issue for it (it goes to Inbox for triage), then tick the line in
  the `idea` issue and note the new issue's number on it. Several lines can become one issue.
- **Dropping**: delete the line; the issue's edit history keeps it.
- **A substantial new feature** is still its own issue in Inbox, not an idea line.
