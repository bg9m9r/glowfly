# Saved exploration cases

Each `.txt` file here is an exploration session that once broke a rule, shrunk to the few actions
that still broke it. `explore_cases` replays every one of them on every build, in name order,
with all six rules checked after each step, and fails if any rule breaks again. A case is how a
bug the explorer found stays fixed.

A case's text is what a failing run prints (and writes to `glowfly-explore-<tag>-<process id>-<seed>.txt` in the
temporary folder): `#` comment lines, a `seed <n>` line (the seed builds the same repositories
again), then one action per line, as `action.rs` writes and parses them. Actions that no longer
apply when replayed are skipped, not errors.

To keep one: fix the bug, copy the printed case into `<short-name>.txt` here (name it after what
broke, for example `undo-after-pop.txt`), add a comment line saying what it caught, and check that
`cargo test -p glowfly explore_cases` passes. Only passing cases belong here: a bug that isn't
fixed yet goes in its issue, with its case.

The cases:

- `resolve-theirs-after-outside-edit.txt`: taking theirs for a conflicted file the user had
  rewritten outside the app lost the rewrite (the regression test for #821).

Long runs with fresh seeds, to find new cases:

```
GLOWFLY_EXPLORE_SEEDS=50 GLOWFLY_EXPLORE_STEPS=500 cargo test -p glowfly explore_soak -- --ignored --nocapture
```

`GLOWFLY_EXPLORE_START` picks the first seed (1 by default).
