# Exploration tests: random user sessions with always-true checks

Status: design, layer 1 (with layer 2's hooks planned in). Date: 2026-11-04.

## Why

Glowfly's tests are broad but each one follows a path someone thought of: about 300 flow test
files drive `update` and the runner, view snapshots pin the drawing, and ten whole-app journeys
(`crates/glowfly-app/src/journey/`) run the real app, views and runner on fixture repositories,
clicking widgets by their accessible names. The bugs that still get through come from
combinations nobody wrote down: an undo after a stash after a failed rebase, an outside edit
between staging two hunks, a branch deleted from a terminal while the app still lists it.

The owner asked for testing that "really thoroughly" exercises the app's UI, logic and I/O.
This design adds a test that generates long random sessions of real user actions on the
existing journey harness. After every step it checks rules that must always hold, and when a
rule breaks it shrinks the session to the shortest one that still breaks it and saves it as a
test that can be replayed.

## Goals

- Find crashes, hangs, repository damage, lost user work, and screens that disagree with the
  repository, in sequences of actions no one scripted.
- Every failure is reproducible from a seed and a short, readable action list, and turns into a
  regression test with one file.
- Run on every PR in a fixed, small time budget with fixed seeds (deterministic: the same code
  gives the same result), and longer with fresh seeds on a schedule.
- Leave a clear seam for layer 2 (fault injection for file and git calls), without building it
  now.

## Non-goals

- Not a replacement for the flow tests, snapshots or hand-written journeys; those keep pinning
  exact behaviour and wording.
- No network, hosting services, credential prompts, OS shell or desktop: the journey sandbox
  keeps dropping those effects (`runner/sandbox.rs`). Hosting-service journeys stay in layer 3.
- No pixel checks, and no real window or real mouse (CLAUDE.md: no synthetic input on the
  user's desktop).
- No acting while the app is busy: every step waits for idle. Overlapping actions with running
  jobs needs its own determinism story and belongs to layer 2 or later (decided in the builder
  check: my call).
- No new property-testing framework dependency in layer 1 (see "Randomness and shrinking").

## Anti-goals (would make this a failure even if it works)

- Flaky: a PR run that fails without a code change. Any nondeterminism (timing, temp paths,
  hash order, wall-clock dates in commits) is a bug in the explorer.
- Slow PR CI: the PR run has a hard budget (see "Running it"), and the explorer must not make
  `check.ps1` noticeably slower locally.
- Failures nobody can read: a report that is a seed and a stack trace. A failure must print the
  shrunk action list in plain words and which rule broke, with the repository state.
- Rules so loose they never fire, or so strict they fire on correct behaviour (a refused action
  with a clear error is correct, not a failure).

## Where it lives

`crates/glowfly-app/src/journey/explore/`, next to the journeys, because the package has no
library target that `tests/` could reach (the reason `journey/mod.rs` gives). Files stay under
~400 lines with `//!` docs, and each gets its line in `docs/architecture/app-layers.md`.
Suggested split (the builder may adjust):

- `mod.rs`: the PR test entry points and the replay test.
- `actions.rs`: the action vocabulary and how each is performed through the harness.
- `choose.rs`: which actions apply now, and the weighted random pick.
- `rules.rs`: the always-true checks.
- `shadow.rs`: the record of the user's own content (for the lost-work rule).
- `shrink.rs`: shrinking a failing session.
- `cases/`: saved failing sessions (plain text, one per file), replayed by a test.

## The session

A session is: a seed, a fixture, and a list of actions. The runner:

1. Builds the fixture from the seed (see "Fixtures"), starts `Journey::start` on it.
2. For each step: lists the actions that apply now, picks one with the seeded random source,
   performs it, waits for the app to be idle (`settle`), then runs every rule.
3. Stops at the step budget, or at the first broken rule, a panic, or a `settle` timeout.

Steps are recorded as they are performed, in a text form that can be read back (`actions.rs`
owns both directions), for example `commit "msg 7"` or `outside-edit src/a.txt +3 lines`.

### Actions

Each action has two parts: how it is performed, and a **probe** that reads the repository
afterwards and says whether it took effect (Commit: HEAD moved to a new commit with that
summary; Stash: the stash count went up by one; Checkout: HEAD names that branch; and so on).
The probe is what rule 6 uses to tell success from failure, since a successful commit shows no
toast. Probes are short predicates on the repository, not full expected states.

Two kinds of action, both chosen by the same random source:

**User actions, performed only through the UI** (click, right-click menus, typing, keys, the
command palette), using the harness's existing helpers (`click`, `branch_menu`, `run_command`,
`type_into`, `press`, ...). Applicability comes from the screen: an action applies when its
widget is present and enabled (the harness already has `count` and `enabled`), so the explorer
also catches widgets that are enabled but shouldn't be. First set:

- Select the working-copy row; stage / unstage a file; stage all / unstage all; stage a hunk.
- Commit (summary from the seed); amend.
- Create a branch; check out a branch; delete a branch; merge a branch; rebase onto a branch.
- Stash all; unstash; drop a stash.
- Undo; redo.
- Discard a file's changes (records the discard in the shadow, see below).
- During a conflict: resolve a file with ours / theirs; continue; abort.
- Refresh.
- Open, switch and close repository tabs (two fixture repositories).

**Outside actions, done to the repository directly** (the file watcher is off in journeys, so
each is followed by Refresh, as the journeys do): write, append to, create or delete a file in
the working tree (every line an outside write adds carries a unique marker, see rule 3); and a few git CLI commands a user might run in a terminal next to the app
(commit, checkout, branch). These are what make the app's view go stale.

Weights favour the common actions and keep risky ones (abort, delete, discard) rare, so
sessions get deep instead of resetting. Exact weights are the builder's to tune, then fix.

### Fixtures

Built from the seed with the journey fixture helpers (`Fixture`, `init`, `commit_files`,
`seeded_remote`): two or three branches that diverge, files that both branches touch (so merges
and rebases can conflict), a local bare remote, and one file with CRLF or a non-UTF-8 name on
platforms where that is valid. Commit dates are fixed, never wall-clock. Two repositories, so
tab actions have something to switch to.

## The rules (checked after every step)

1. **No crash, no hang.** No panic in the app or runner; `settle` returns within the harness
   timeout (`journey/harness.rs`, 120 s per step).
2. **The repository is sound.** `git fsck --no-progress` exits 0 (run through the fixture's
   `git` helper with the test's isolated config).
3. **No lost work.** The unit is a line, not a file: every line an outside write adds carries a
   unique marker (for example `edit-<seed>-<n>`), and the shadow records the live markers, plus
   each commit summary the user typed. Lines survive merges, rebases and auto-stash-and-reapply
   even when the exact file blob does not, so correct actions don't trip the rule. After every
   check, every live marker must be findable in at least one of: the working tree, the index,
   any commit reachable from any ref (including `refs/glowfly/discarded/*`, where the app keeps
   discarded files and dropped stashes) or any reflog entry, or a stash. Because the app's own
   keep records count, a discard or a dropped stash removes nothing from the record, and the
   rule also checks that the keep mechanism works. A marker leaves the record only when a later
   outside action of the user's removes it (overwriting or deleting that line or its file).
   Conflict markers and resolving with "theirs" are fine: the user's side stays in its commit or
   stash. This rule is the heart of the design: it is what "never lose the user's work" means,
   in a form a test can check. (Line markers and counting the keep refs: my call, from the
   builder check.)
4. **The screen agrees with the repository.** Read from git (libgit2 through the fixture
   helpers, or the CLI) and compared with what the UI shows through its accessible names:
   the checked-out branch (`"<name>, checked out"` in the sidebar), the set of local branches,
   the set of changed files in the working-copy list, the conflict state, and the stash count.
   After an app action the comparison runs without refreshing, so a view the app forgot to
   update after its own action is caught; after an outside action it runs after the Refresh
   that action ends with. (My call, from the builder check.)
5. **Undo returns to before.** When the previous action was undoable and the next is Undo, HEAD
   and the refs it touched match their values from before that action, and the working tree
   content in the shadow is unchanged. The step that finishes a merge or rebase that stopped on
   conflicts (Continue, or committing the merge) records the whole operation, so its Undo
   returns to before the merge or rebase began, not to the operation in progress. "Undoable"
   means the action added exactly one entry to the app's undo history: one Undo reverses one
   entry, so after a step that added two (a checkout that stashed the changes and put them back
   records the put-back as well) the rule isn't checked, and nor is it after an operation during
   which other undoable actions were recorded.
6. **Errors are reported, never silent.** Each action's probe says whether it took effect. If
   it did not, the app must have shown an error toast or written an error log line (the harness
   keeps both). If it did, no error toast may claim it failed. A clearly worded refusal is
   correct and not a failure. An action whose widget was enabled but that the app then refused
   with no message is caught here too.

The rules are plain functions over the harness and the fixture paths, so layer 2 and future
journeys can reuse them.

## Randomness and shrinking

- A small seeded generator inside the explorer (for example a 64-bit xorshift or splitmix),
  not a new crate dependency: the session's only randomness, so a seed fully decides the
  session. Nothing may read the clock or iterate a hash map to make choices.
- Shrinking when a rule breaks: replay the recorded action list with chunks removed (halves,
  then quarters, down to single actions), keeping any shorter list that still breaks the same
  rule; then try simplifying single actions (shorter text, fewer lines). Actions that no longer
  apply during a replay are skipped, not errors. Each replay starts from a fresh fixture built
  from the same seed. Cap shrinking by time so a failing CI run still ends promptly.
- The report prints the rule that broke, the shrunk list as plain lines, the seed, and the
  repository state (branch, `git status --short`, last commits), and writes the list to the
  test's temp folder.
- Saving a case: copying that text into `explore/cases/<short-name>.txt` makes the replay test
  run it on every build. Fixing the bug and keeping the case is how it becomes a regression
  test.

## Layer 2 hooks (planned, not built now)

Fault injection will plug into the journey runner's sandbox (`glowfly-runner/src/sandbox.rs`,
rule enum `SandboxRule` in `glowfly-model/src/effect/meta.rs`): a fault plan, decided by the
same seed, that makes chosen effects fail or stall before they run (disk full, permission
denied, `index.lock` held, git killed mid-run, a file changed during a commit, a slow git).
Layer 1 must leave room for this:

- Actions and the session loop must not assume an action succeeds; rule 6 already covers
  failures.
- The session record has a place for "faults planned for this step", empty in layer 1.
- The rules must hold under faults too, which is why rule 3 is about reachability, not about
  each action's expected result.

## Running it

- **Check frequency:** rules 1, 4, 5 and 6 run after every step. Rules 2 and 3 (a `git fsck`
  process and a reachability scan) may run every N steps and at the end of a session in the PR
  run, and after every step in soak runs, so the report names the step where the rule broke.
  While shrinking, each replay runs them once, at its end: a broken repository or lost work stays
  so, and checking them after every step made each replay of a long session so slow that the
  minute of shrinking ended with dozens of actions left. Replays run a few at a time, side by
  side. (My call, from the builder check; the shrinking change measured while building.)
- **PR CI and local `check.ps1`:** a test that runs a fixed list of seeds within a fixed step
  budget (as a starting point: 3 seeds x 150 steps; the builder measures and sets numbers that
  keep it near one minute on the Windows CI runner), plus the replay test for `explore/cases/`.
  Fixed seeds keep it deterministic: a failure means the code changed.
- **Longer runs:** an `#[ignore]` test that reads the seed count, step count and starting seed
  from environment variables, for local soak runs (`cargo test -p glowfly explore_soak --
  --ignored`, with the variables documented in CLAUDE.md's Commands section).
- **Scheduled run in GitHub Actions:** a nightly job running the soak test with fresh seeds,
  which opens an issue (in Inbox) with the shrunk case when a rule breaks. Adding a workflow is
  a CI change, so this part waits for the owner's approval of the workflow file; layer 1 is
  complete without it.

## Constraints

- Follows CLAUDE.md and `docs/PLATFORMS.md`: runs on Windows, Linux and macOS CI; paths through
  the platform helpers, temp folders from `glowfly_core::paths::temp_dir()`; no Windows-only
  paths or case-insensitivity assumptions.
- No I/O in views. Product code changes only for two reasons: a real bug a rule found (its fix
  lands as its own commit, with the saved case as the test), or extending the existing
  test-only seams the journeys already use (`TestStart`, `GlowflyApp::for_test`, the runner's
  sandbox). Two such seams are expected (my call, from the builder check):
  - **Fixed commit time:** commits, amends, merges, rebases and stashes the app makes use the
    real clock, so their ids, and stash names like "WIP on main: <id>", change every run. In a
    sandboxed run the runner gives each app-made commit a time from a fixed, increasing
    test clock (for libgit2 signatures and as `GIT_AUTHOR_DATE`/`GIT_COMMITTER_DATE` on the
    git commands that runner starts, never set process-wide, since tests run in parallel).
    Outside the sandbox nothing changes.
  - **Second repository:** Open goes through the folder picker, which the sandbox drops, so
    `TestStart` gains a way to open a second repository (or a recent-repositories list) for
    the tab actions.
  Neither seam may change release behaviour; `source_checks` already keep test switches out of
  normal builds, and the builder extends those checks as needed.
- Accessible names: where an action needs a widget that has no accessible name today (the
  ideas file lists some, such as Preferences' navigation rows), adding the name is in scope;
  widgets out of reach are left out of the first action set and noted in `docs/ideas/`.
- Memory: one session at a time per test thread, and the harness is dropped between sessions
  (each holds a renderer).

## Left to the builder

- Exact file split, weights, step budgets and seed list (measured, then fixed).
- The text format of a saved case, as long as it is human-readable and stable.
- Which outside git commands to include beyond commit, checkout and branch.
- How to read the screen for rule 4 (accessible names or app state through `Journey::state()`;
  prefer what the user sees, fall back to state only where the screen has no name for it, and
  note it).

## Success

- The PR test runs in about a minute, passes on all CI systems, and has no flakes over 20
  repeated runs locally.
- A deliberately planted bug of each kind (a panic, a lost stash, a stale branch list, a silent
  failure) is caught and shrunk to a short readable case. The builder plants each one on a
  throwaway branch to prove the rules fire, and reverts it.
- Any real bugs it finds land as fixes with their saved cases.
