# Exploration tests (layer 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A test that runs long seeded random sessions of real user and outside actions on the whole-app journey harness, checks six always-true rules after every step, and shrinks any failure to a short replayable case.

**Architecture:** A new module `crates/glowfly-app/src/journey/explore/` on top of `journey/harness.rs` and `journey/fixture.rs`. A pure core (seeded generator, action text, shrinker, marker reachability) is unit-tested without the app. The session loop drives `Journey` and runs the rules. Three small test-only seams extend what the journeys already use: runner panics recorded by the sandbox tally, pinned git dates per fixture root, and extra repositories opened from `TestStart`.

**Tech Stack:** Rust, egui 0.31 + egui_kittest (existing harness), git2, the git CLI through `glowfly_core::testkit::git_env`. No new crates.

**Spec:** `docs/superpowers/specs/2026-11-04-explore-tests-design.md`. Read it before each task; this plan argues from it.

## Global Constraints

- Files under ~400 lines, each with a `//!` doc; each new file gets its line in `docs/architecture/app-layers.md` (the explore files) or the matching area doc (seam files).
- No new crate dependency. The generator is our own splitmix64.
- Nothing that decides a choice may read the clock, iterate a `HashMap`/`HashSet`, or depend on screen order. Use `BTreeMap`/`BTreeSet` and fixed vocabulary order.
- Action text never contains an absolute path, a process id or a commit id. Paths in it are repository-relative with `/`.
- Commit dates are fixed. Fixture git uses `fixture::DATE`. App-made commits use the pinned clock (Task 2).
- Seams must not change release behaviour. Everything they add is behind `#[cfg(any(test, feature = "testkit"))]` (core) or `feature = "test-support"` (runner), or lives in `app_testing.rs`.
- Follow `docs/PLATFORMS.md`. Use temp folders from `glowfly_core::paths::temp_dir()` (already used by `Fixture::new`). Never assume case-insensitive paths.
- `tests/source_checks/hygiene.rs` allowlists and counts only shrink.
- Commit subjects are Conventional Commits, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Run `pwsh scripts/check.ps1` before each commit (targeted `cargo test -p glowfly explore` while working).
- Product fixes for real bugs the explorer finds land as their own `fix:` commit, with the saved case in `explore/cases/`.

## Review Focus

1. **A modal left open** (confirm delete, "Stash and check out", the commit dialog): only the modal's own buttons may apply while it is open. Otherwise every other pick panics in `click` with "disabled" or "no widget". Pinned in Task 6 (`modal_open_limits_choices`).
2. **Relative dates and ids on screen** ("2 minutes ago", "WIP on main: 1a2b3c"): choices must not depend on label text that changes between runs. Pinned in Task 9 (`same_seed_same_session`, run twice in one process with different fixture roots).
3. **CRLF files and markers:** a marker in a CRLF file must still be found by rule 3, and an outside append must keep the file's line endings. Pinned in Task 4 (`crlf_marker_found`).
4. **Windows file locks:** an outside delete or write can fail while the app or git holds the file. The explorer retries once after `settle`, then records the outside action as skipped. It must never panic as a rule 1 failure. Pinned in Task 7 (`outside_write_failure_is_skipped`).
5. **Replays where an action no longer applies:** the shrinker removes the action that made a branch, so a later `checkout "b3"` has no widget. The replay skips it and goes on. Pinned in Task 9 (`replay_skips_inapplicable`).

---

## File Structure

New, under `crates/glowfly-app/src/journey/explore/`:

| File | Responsibility |
|---|---|
| `mod.rs` | `//!` overview; the PR test, the case-replay test, the `#[ignore]` soak test |
| `rng.rs` | `Rng`: splitmix64, `below`, `weighted` |
| `action.rs` | `Action` / `Outside` vocabulary, `Display` and `parse` (the case text) |
| `scene.rs` | building the two fixture repositories and the bare remote from a seed |
| `shadow.rs` | `Shadow`: live markers and typed summaries |
| `reach.rs` | which markers and summaries are reachable in a repository |
| `perform.rs` | performing an `Action` through `Journey` (UI) or on disk (outside) |
| `probe.rs` | `Probe`: before/after readings and whether an action took effect |
| `choose.rs` | which actions apply on the current screen, with weights |
| `rules.rs` | `Rule`, `Broken`, the six checks |
| `session.rs` | `Step`, `Fault`, `Plan`, `Outcome`, the session loop, the failure report |
| `shrink.rs` | delta-debugging shrink over a predicate, capped by a deadline |
| `cases/README.md` | the case format, and how to save a case |

Modified:

- `crates/glowfly-runner/src/sandbox.rs`, `testing.rs`: record effects that panicked (Task 1).
- `crates/glowfly-app/src/shell/app_testing.rs`: `panicked_effects()` (Task 1); `TestStart::also_open` (Task 3).
- `crates/glowfly-core/src/gitcli.rs` and new `crates/glowfly-core/src/testkit/clock.rs`: pinned dates (Task 2). Also `undo/kept.rs:148` and `undo/aborted.rs:300` signatures.
- `crates/glowfly-app/src/journey/harness.rs` (+ `harness/actions.rs` if over 400 lines): `toasts_since_action`, `errors_logged_since`, `log_mark`, `labels`, `start_many` (Tasks 3, 8).
- `crates/glowfly-app/src/journey/find.rs`: `Region::Tabs` (Task 3).
- `crates/glowfly-app/src/journey/fixture.rs`: `Fixture::pin_dates`, and `DATE` made `pub(super)` (Task 2).
- `crates/glowfly-app/src/journey/mod.rs`: `mod explore;` and its line in the list.
- `CLAUDE.md` Commands: the soak command (Task 10). `docs/architecture/*.md`: file lines. `docs/ideas/testing.md` (or the matching ideas file): widgets left out.

---

### Task 1: The sandbox records effects that panicked

**Files:**
- Modify: `crates/glowfly-runner/src/sandbox.rs` (`Tally`, `Busy::drop`)
- Modify: `crates/glowfly-runner/src/testing.rs` (`Runner::panicked`)
- Modify: `crates/glowfly-app/src/shell/app_testing.rs` (`GlowflyApp::panicked_effects`)
- Test: `crates/glowfly-runner/src/sandbox_tests.rs` (or the sandbox's existing test file)

**Interfaces:**
- Produces: `Runner::panicked(&self) -> Vec<&'static str>` and `GlowflyApp::panicked_effects(&self) -> Vec<&'static str>`: the names of effects whose worker unwound from a panic, in order.

- [ ] **Step 1: Write the failing test** `busy_dropped_while_panicking_is_recorded`: make a `Tally`, spawn a thread that does `let _b = tally.begin("boom"); panic!()`, join it (expect Err). Assert `lock(&tally.panicked).as_slice() == ["boom"]` and `in_flight` is empty. A second `begin("fine")` dropped normally adds nothing.
- [ ] **Step 2: Run** `cargo test -p glowfly-runner --features test-support busy_dropped` → FAIL (no field `panicked`).
- [ ] **Step 3: Implement.** Add `pub(super) panicked: Mutex<Vec<&'static str>>` to `Tally`. In `Busy::drop`, push `self.name` when `std::thread::panicking()`. Add `Runner::panicked` in `testing.rs` beside `in_flight_names`, and `GlowflyApp::panicked_effects` beside `in_flight_names` in `app_testing.rs`.
- [ ] **Step 4: Run** the test → PASS.
- [ ] **Step 5: Commit** `test: record effects that panicked in the journey sandbox`.

### Task 2: Pinned git dates for a fixture root

**Files:**
- Create: `crates/glowfly-core/src/testkit/clock.rs`
- Modify: `crates/glowfly-core/src/testkit.rs` (or `testkit/mod.rs`): `pub mod clock;`
- Modify: `crates/glowfly-core/src/gitcli.rs` `base_command`
- Modify: `crates/glowfly-core/src/undo/kept.rs:148`, `undo/aborted.rs:300`
- Modify: `crates/glowfly-app/src/journey/fixture.rs`: `pub fn pin_dates(&self)`, unpinned in `Drop`
- Test: `crates/glowfly-core/src/testkit/clock_tests.rs`

**Interfaces:**
- Produces, in `glowfly_core::testkit::clock`:
  - `pub fn pin(root: &Path, start: i64)`: registers `root`. Every git command later started by gitcli with a `cwd` inside `root` gets `GIT_AUTHOR_DATE` and `GIT_COMMITTER_DATE` = `"@<t> +0000"`, where `t` starts at `start` and goes up by 60 for each command. This applies only when the caller's `env` doesn't already set them.
  - `pub fn unpin(root: &Path)`.
  - `pub(crate) fn next_for(path: &Path) -> Option<i64>`: the next tick when `path` is inside a pinned root.
  - `pub(crate) fn signature(path: &Path, name: &str, email: &str) -> Result<git2::Signature<'static>, git2::Error>`: `Signature::new` at `next_for(path)` when pinned, else `Signature::now`.
- The registry is a `static Mutex<BTreeMap<PathBuf, i64>>`. Match with `Path::starts_with` on the paths as given; the fixture passes its `root` exactly as the app's repository paths start.
- In `gitcli.rs`: one `#[cfg(any(test, feature = "testkit"))]` block in `base_command` that asks `next_for(cwd)` and sets the two env vars. Release builds compile none of it.
- In `undo/kept.rs` and `undo/aborted.rs`: replace `Signature::now("Glowfly", "glowfly@localhost")` with a crate helper `undo::glowfly_signature(path)`. In test and testkit builds it calls `testkit::clock::signature`; otherwise it is `Signature::now`.
- Consumed by Task 4: `Fixture::pin_dates(&self)` calls `clock::pin(&self.root, 1_768_039_200)` (2026-01-10T10:00Z, the fixture `DATE`). `Drop for Fixture` calls `clock::unpin(&self.root)`.

- [ ] **Step 1: Failing tests** in `clock_tests.rs`:
  - `pinned_root_gets_increasing_dates`: `pin(tmp, 1_000_000)`; init a repo inside `tmp` with `testkit` helpers. Run `gitcli::output` for `commit --allow-empty -m a`, then again with `-m b`. The author and committer times are `1_000_000` for a and `1_000_060` for b.
  - `unpinned_root_uses_the_clock`: no pin; the commit time is within 600 s of `SystemTime::now()`.
  - `caller_dates_win`: pinned, and the caller passes `GIT_COMMITTER_DATE`; that value is used.
  - `two_roots_count_separately`: pin A and B; commits in B don't advance A's tick.
- [ ] **Step 2: Run** `cargo test -p glowfly-core clock` → FAIL.
- [ ] **Step 3: Implement** as above. Keep `clock.rs` under 100 lines.
- [ ] **Step 4: Run** → PASS. Then confirm the release build leaves it out: `cargo tree -p glowfly -e normal,features -i glowfly-core` lists no `testkit` feature.
- [ ] **Step 5: Commit** `test: pin the dates of git commands run inside a test fixture`.

### Task 3: More than one repository at test start, and the tab strip as a region

**Files:**
- Modify: `crates/glowfly-app/src/shell/app_testing.rs` (`TestStart::also_open`)
- Modify: `crates/glowfly-app/src/journey/harness.rs` (`settings`/`start_with` callers fill `also_open: Vec::new()`; new `Journey::start_many`)
- Modify: `crates/glowfly-app/src/journey/find.rs` (`Region::Tabs`, from the `"tabs"` panel)
- Test: `crates/glowfly-app/src/journey/harness_tests.rs`

**Interfaces:**
- Produces: `TestStart { also_open: Vec<PathBuf>, .. }`. `for_test` dispatches `RepoMsg::Open` for each path in `also_open` first, then for `open`, so `open` ends up as the active tab.
- Produces: `Journey::start_many(fx: Fixture, open: PathBuf, also: Vec<PathBuf>, trusted: Vec<PathBuf>) -> Journey`.
- Produces: `Region::Tabs`.

- [ ] **Step 1: Failing test** `two_repositories_open_as_tabs`: build `app` and `lib` repositories in a `Fixture`. `start_many` with `open = app`, `also = [lib]`. Assert both names show in `Region::Tabs` (`j.see("app", Region::Tabs)` and the same for `lib`, using whatever the tab's accessible name is; if a tab has no name, give it one: its repository name). Assert `j.state()` reports `app` as the active tab. Click `lib` in `Region::Tabs`; then `disk::head_branch` is unchanged, and `see("main, checked out", Region::Sidebar)` refers to lib's sidebar.
- [ ] **Step 2: Run** `cargo test -p glowfly two_repositories_open_as_tabs` → FAIL.
- [ ] **Step 3: Implement.** If the tab close button has no accessible name, name it `"Close <repo name>"` (a new `t.tr` key in all three locales; the access_names source check lists the rule). The fixed-order list of the tab bar's labels goes in Task 6.
- [ ] **Step 4: Run** → PASS, and every existing journey still passes (`cargo test -p glowfly journey`).
- [ ] **Step 5: Commit** `test: open several repositories in a journey and find the tab strip`.

### Task 4: Seeded generator, case text, scene, shadow and reachability (the pure core)

**Files:**
- Create: `explore/mod.rs` (the `//!` doc and module list only, for now), `rng.rs`, `action.rs`, `scene.rs`, `shadow.rs`, `reach.rs`
- Modify: `journey/mod.rs`: `mod explore;` and its bullet
- Test: `#[cfg(test)] mod tests` at the bottom of each file, or `<file>_tests.rs` beside it when the file would pass 400 lines

**Interfaces (produced, used by every later task):**

```rust
// rng.rs
pub(super) struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self;
    pub fn next_u64(&mut self) -> u64;               // splitmix64, body below
    pub fn below(&mut self, n: usize) -> usize;      // n > 0; next_u64() % n is fine
    pub fn weighted<'a, T>(&mut self, items: &'a [(T, u32)]) -> Option<&'a T>; // None if empty or all weights 0
}

// action.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Action {
    SelectWip, Stage(String), Unstage(String), StageAll, UnstageAll, StageHunk(String),
    Commit(String), Amend, CreateBranch(String), Checkout(String), DeleteBranch(String),
    Merge(String), Rebase(String), Stash, Unstash, DropStash, Undo, Redo, Discard(String),
    ResolveOurs(String), ResolveTheirs(String), Continue, Abort, Refresh,
    SwitchTab(String), CloseTab(String),
    /// A modal's button, by its label (Cancel, "Stash and check out", Delete...).
    Confirm(String),
    Outside(Outside),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Outside {
    Write { path: String, lines: u8 }, Append { path: String, lines: u8 },
    Create { path: String, lines: u8 }, Delete { path: String },
    GitCommit(String), GitCheckout(String), GitBranch(String),
}
impl std::fmt::Display for Action { .. }            // one line, e.g. `commit "msg 7"`, `outside append "src/a.txt" +3`
pub(super) fn parse(line: &str) -> Result<Action, String>;

// scene.rs
pub(super) struct Scene { pub fx: Fixture, pub app: PathBuf, pub lib: PathBuf, pub bare: PathBuf }
pub(super) fn build(tag: &str, seed: u64) -> Scene;  // Fixture::new(&format!("explore-{tag}-{seed}")), pin_dates()
pub(super) const PATHS: [&str; N];                   // the repository-relative paths outside actions may touch

// shadow.rs
pub(super) struct Shadow { seed: u64, next: u64, live: BTreeMap<String /*repo name*/, BTreeSet<String>>, summaries: BTreeMap<String, BTreeSet<String>> }
impl Shadow {
    pub fn new(seed: u64) -> Self;
    pub fn marker(&mut self) -> String;                       // "edit-<seed>-<n>", n counting from 1
    pub fn add(&mut self, repo: &str, marker: String);
    pub fn forget_in(&mut self, repo: &str, text: &str);       // drops every live marker that occurs in `text`
    pub fn summary(&mut self, repo: &str, s: &str);
    pub fn live(&self, repo: &str) -> &BTreeSet<String>;
    pub fn summaries(&self, repo: &str) -> &BTreeSet<String>;
}

// reach.rs
/// The markers (searched in blob text, CR removed) and summaries (searched in commit
/// messages) found nowhere reachable: the work tree, the index, any commit reachable from any
/// ref (refs/glowfly/* and refs/stash included) or from any reflog entry.
pub(super) fn missing(repo: &Path, markers: &BTreeSet<String>, summaries: &BTreeSet<String>) -> Vec<String>;
```

splitmix64 (an algorithm, so its body is pinned here):

```rust
pub fn next_u64(&mut self) -> u64 {
    self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = self.0;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
```

Case text: one action per line. Strings are double-quoted, with `\"` and `\\` escapes. Keywords are kebab-case: `select-wip`, `stage "a.txt"`, `stage-hunk "a.txt"`, `create-branch "b2"`, `confirm "Stash and check out"`, `outside write "src/a.txt" +3`, `outside git-commit "msg"`. `parse` rejects unknown keywords with the line in the message. In a case file, `#` starts a comment line. The first non-comment line is `seed <u64>`.

Scene from the seed (the fixture helpers only, git dates fixed):
- `remote/app.git`, seeded with `a.txt`, `shared.txt` (10 lines), `crlf.txt` (CRLF line endings, committed with `core.autocrlf=false`), `src/b.txt`. `work/app` is cloned with `clone_cli`.
- On `work/app`: branches `topic` and `side`, each with 1–3 commits (count from `rng.below(3)+1`). Both change `shared.txt` lines that `main` also changes on its own extra commit, so merging or rebasing them conflicts.
- On Linux only (`cfg(target_os = "linux")`): a committed file whose name is not UTF-8 (`OsStr::from_bytes(b"caf\xe9.txt")`). It is not in `PATHS`; the explorer never names it, it only has to survive.
- `work/lib`: a plain repository (`init`) with `lib.txt` and one branch `dev`.

- [ ] **Step 1: Failing tests:**
  - rng: `same_seed_same_numbers` (two `Rng::new(42)` give the same first 100 values); `known_first_value` (`Rng::new(0).next_u64() == 0xE220_A839_7B1D_CDAF`); `weighted_never_picks_zero` (10 000 picks of `[("a",0),("b",1)]` are all `"b"`); `weighted_empty_is_none`.
  - action: `round_trips` (every variant, including strings with `"`, `\`, spaces and non-ASCII, gives `parse(&a.to_string()) == Ok(a)`); `rejects_unknown` (`parse("fly away")` is `Err` naming the line); `text_has_no_root` (no `Display` output contains `:\` or starts with `/`).
  - scene: `same_seed_same_scene` (`build("t1", 7)` and `build("t2", 7)` give equal `head_oid`s for `app`'s `main`, `topic` and `side`); `branches_conflict` (`git merge-tree --write-tree main topic` exits non-zero in `app`).
  - shadow: `markers_count_up` (`edit-9-1`, `edit-9-2`); `forget_in_drops_only_present` (live `{edit-9-1, edit-9-2}`, `forget_in("x\nedit-9-2\n")` leaves `{edit-9-1}`).
  - reach: `found_in_work_tree_index_commit_reflog_stash`, five small cases on a `testkit` scratch repository. A marker written only in the work tree is found. One that is staged and then deleted from the work tree is found. One committed is found. For a commit then `reset --hard HEAD~1`, the marker is found through the reflog. For a stashed change, the marker is found. `found_in_glowfly_refs`: a blob reachable only from `refs/glowfly/discarded/x` is found. `gone_is_missing`: a marker whose only commit is dropped (`reflog expire --expire=now --all` plus `gc --prune=now`) is reported. `crlf_marker_found`: a marker on a CRLF line is found. `summary_found_in_message`: a summary is found from the reflog after `reset --hard`.
- [ ] **Step 2: Run** `cargo test -p glowfly explore::` → FAIL to compile, then FAIL.
- [ ] **Step 3: Implement.** For reach, use libgit2: a `Revwalk` with `push_glob("*")` plus `push_head`. For every ref's reflog (`repo.references()`, then `repo.reflog(name)`), push each entry's `id_new` and `id_old` when non-zero. Walk the trees with a `BTreeSet<Oid>` of blobs already seen. Also read the index's stage 0–3 blobs, and the work-tree files (skipping `.git`). Stop early once nothing is left to find.
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `test: the exploration tests' generator, case text, scene and reachability`.

### Task 5: Probes

**Files:**
- Create: `explore/probe.rs`
- Test: `explore/probe.rs` `mod tests` (disk-only, with the `fixture` helpers)

**Interfaces:**
- Produces:

```rust
/// What the probes and rules read from a repository, before and after a step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Reading {
    pub head: Option<Oid>, pub branch: Option<String>,
    pub branches: BTreeMap<String, Oid>, pub stashes: usize,
    pub state: git2::RepositoryState, pub conflicted: BTreeSet<String>,
    pub changed: BTreeSet<String>,   // repository-relative paths with staged, unstaged or untracked changes
    pub staged: BTreeSet<String>,
}
pub(super) fn read(repo: &Path) -> Reading;
/// Whether `action` took effect, from the readings around it. None for actions with nothing to probe (SelectWip, Refresh, SwitchTab, Confirm "Cancel").
pub(super) fn took_effect(action: &Action, before: &Reading, after: &Reading) -> Option<bool>;
```

- Probe per action. Commit: head changed and its summary is the typed text. Amend: head changed, and the new head has the same parent as the old one. CreateBranch(n): `n` is in `branches`. Checkout(n): `branch == Some(n)`. DeleteBranch(n): `n` is not in `branches`. Merge/Rebase: head changed, or `state` is a merge/rebase with conflicts. Stash: `stashes` +1. Unstash: `stashes` -1, or the state shows conflicts with the stash kept. DropStash: -1. Stage(p): `p` in `staged`. Unstage(p): `p` not in `staged`. StageAll: `staged == changed`. Discard(p): `p` not in `changed`. ResolveOurs/Theirs(p): `p` not in `conflicted`. Continue: `state` is `Clean`, or the next conflict is shown with fewer steps left. Abort: `state` is `Clean`. Undo/Redo: head or a branch changed. Outside actions: None (the explorer did them itself).
- [ ] **Step 1: Failing tests:** `commit_probe` (true after a CLI commit with the summary, false when the head is unchanged), `stash_probe`, `checkout_probe`, `discard_probe`, `merge_conflict_counts_as_effect` (a conflicting CLI merge gives true).
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** with the `fixture` helpers and `git2::Repository::statuses`.
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `test: probes that tell whether an exploration step took effect`.

### Task 6: Choosing actions from the screen

**Files:**
- Create: `explore/choose.rs`
- Modify: `journey/harness.rs`: `pub fn labels(&self, region: Region) -> Vec<String>` (wraps `find::labels_in`), and `pub fn modal_open(&self) -> bool` (the app state's open dialog or confirm, the one the journeys' Cancel buttons close; name the field you find in `AppState`)
- Test: `explore/choose.rs` `mod tests` (journey-level, on a `scene::build`)

**Interfaces:**
- Consumes: `Journey`, `Scene`, `Reading`, `Rng`, `Action`.
- Produces: `pub(super) fn applicable(j: &Journey, scene: &Scene, active: &str, r: &Reading, rng: &mut Rng, shadow: &mut Shadow) -> Vec<(Action, u32)>`, in a fixed vocabulary order. The arguments an action needs (a branch name, a path, a summary `"msg <n>"`, a new branch `"b<n>"`) come from `r` (sorted `BTreeMap`/`BTreeSet`) and `rng`, never from screen order.
- Applicability is read from the screen: an action applies when its widget is present and enabled (`count`/`enabled`-style checks, without panicking). Exception: actions whose widget only appears after a right-click (branch menus). Those apply when the branch row is present, and Task 7's `perform` treats a missing or disabled menu item as "not applicable now": skipped, not a failure.
- While `modal_open()`, only `Confirm(label)` for the modal's buttons applies.
- Starting weights (to tune in Task 10, then fix): Commit 8, Stage/StageAll/StageHunk 6 each, outside Write/Append/Create 6 each, Checkout 4, CreateBranch 3, Merge 3, Rebase 2, Stash 3, Unstash 3, Undo 4, Redo 2, Refresh 2, SelectWip 2, Unstage/UnstageAll 2, ResolveOurs/Theirs 4 (only while conflicted), Continue 4 (only when nothing is left conflicted), SwitchTab 1, outside git commands 1 each, Amend 1, DeleteBranch 1, DropStash 1, Discard 1, Abort 1, CloseTab 1, outside Delete 1, Confirm: Cancel 1, the action's own button 5.
- [ ] **Step 1: Failing tests:**
  - `clean_main_offers_basics`: on a fresh scene, `applicable` includes `CreateBranch(_)`, `Checkout("topic")` and outside writes, and no `Commit` (nothing staged) and no `Continue`.
  - `conflict_offers_resolution`: after an outside `git merge topic` that conflicts, followed by `refresh`, `ResolveOurs("shared.txt")` and `Abort` apply and `Commit` doesn't.
  - `modal_open_limits_choices` (Review Focus 1): make a local edit, refresh, and open a branch's Checkout so "Stash and check out" shows. Every item is a `Confirm(_)`.
  - `choices_ignore_screen_order`: calling `applicable` twice with the same `Rng` seed gives the same list.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `test: choose exploration actions from what the screen offers`.

### Task 7: Performing actions

**Files:**
- Create: `explore/perform.rs` (split into `perform_ui.rs` and `perform_outside.rs` if it passes 400 lines)
- Test: `explore/perform.rs` `mod tests`

**Interfaces:**
- Consumes: `Journey` helpers (`click`, `click_containing`, `branch_menu`, `right_click`, `menu`, `type_into`, `refresh`, `select_wip`, `run_command`), `Shadow`, `Scene`.
- Produces:

```rust
pub(super) enum Done { Performed, Skipped(String) }   // Skipped: the widget or menu item wasn't there or was disabled, or an outside write failed twice
pub(super) fn perform(j: &mut Journey, scene: &Scene, active: &str, shadow: &mut Shadow, action: &Action) -> Done;
```

- UI actions go through the harness only, with the labels the journeys already use: "Stage all", "Stage" in `InRowOf(path)`, "Stage hunk", "Commit summary" plus the counted commit button (as `commit_all` does), "Create a branch" → "Branch name" → "Create branch", `branch_menu(n, "Checkout" | "Merge into main" | "Rebase <cur> onto this branch" | "Delete…")`, "Stash all changes", "Unstash the latest stash", "Undo the last action", "Redo the undone action", "View conflicts", "Take every conflict from theirs" / ours, "Mark resolved", "Continue", tab labels in `Region::Tabs`. Find the labels that are missing (Amend, Discard, Abort, Drop stash, ours) with `j.labels(..)`. A widget with no accessible name gets one if that is a one-line change; otherwise drop that action from Task 6's vocabulary and add one line to `docs/ideas/`.
- Before calling a harness helper that panics on a missing widget, check that it is present (`count`) and enabled, so a missing widget is `Skipped`, never a panic.
- Commit records the summary in `shadow.summary`. Outside Write and Create first `forget_in` the old file text. Then they write `lines` new lines, each carrying a fresh `shadow.marker()`, and `add` them. Append adds lines with markers and keeps the file's line ending (CRLF in `crlf.txt`). Delete forgets the file's markers. Every outside action ends with `j.refresh()`. Outside git commands run through `fixture::git` and are allowed to fail (use a non-asserting variant: add `fixture::git_try(dir, args) -> bool`). An outside commit records its summary.
- On Windows a write or delete can fail with a sharing violation. Retry once after `j.settle()`, then return `Skipped`.
- [ ] **Step 1: Failing tests:**
  - `scripted_tour`: on a scene, perform in order: outside write, StageAll, Commit("msg 1"), CreateBranch("b1"), Checkout("main"), Merge("topic") (it conflicts), ResolveTheirs("shared.txt"), Continue, Stash after an outside append, Unstash, Undo, Redo. After each step, assert `took_effect(..) == Some(true)` where a probe exists, and that `perform` returned `Performed`.
  - `missing_widget_is_skipped`: `perform(Checkout("nope"))` → `Skipped(_)`, with no panic.
  - `outside_write_failure_is_skipped` (Review Focus 4): an outside write to a path whose parent is a file (fails on every OS) → `Skipped`, with no panic.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `test: perform exploration actions through the app and on disk`.

### Task 8: The six rules

**Files:**
- Create: `explore/rules.rs` (put the tests in `rules_tests.rs`)
- Modify: `journey/harness.rs`: `pub fn toasts_since_action(&self) -> Vec<(bool, String)>`, `pub fn log_mark(&self) -> u64` (the largest `LogEntry::id` across the app log and the tab logs, as a number), and `pub fn errors_logged_since(&self, mark: u64) -> Vec<String>` (entries with `ok == false` and a larger id). If `EntryId` isn't ordered, use the count of entries per log instead.

**Interfaces:**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Rule { NoCrash, Sound, NoLostWork, ScreenAgrees, UndoReturns, ErrorsReported }
impl Rule { pub fn name(self) -> &'static str }   // "no-crash", "sound", "no-lost-work", "screen-agrees", "undo-returns", "errors-reported"
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Broken { pub rule: Rule, pub step: usize, pub detail: String }

pub(super) fn no_crash(j: &Journey) -> Result<(), String>;                 // j.app().panicked_effects() is empty (the harness panics are caught in Task 9)
pub(super) fn sound(repo: &Path) -> Result<(), String>;                   // `git fsck --no-progress` through fixture::git_try, with its output on failure
pub(super) fn no_lost_work(repo: &Path, name: &str, shadow: &Shadow) -> Result<(), String>;  // reach::missing is empty
pub(super) fn screen_agrees(j: &Journey, r: &Reading) -> Result<(), String>;
pub(super) fn undo_returns(before_undone: &Reading, after_undo: &Reading) -> Result<(), String>; // head, branch, branches and the changed-path set equal (the spec's "working tree content unchanged"; markers are rule 3's)
pub(super) fn errors_reported(took: Option<bool>, toasts: &[(bool, String)], errors: &[String]) -> Result<(), String>;
```

- `screen_agrees` reads the screen first: `"<branch>, checked out"` in Sidebar and `"Branch: <branch>"` in Toolbar for `r.branch`. Every name in `r.branches` must show in Sidebar, either as itself or as `", checked out"`. With `r.state` conflicted, "View conflicts" shows in Details once the working-copy row is selected (only check this when it is selected). The changed-file set and the stash count have no reliable names on screen, so compare them with the active tab's `Snapshot` in `j.state()` (the staged, unstaged/untracked and stash fields). Say so in a comment, as the spec asks.
- `errors_reported`: `Some(false)` needs a failure toast or a logged error. `Some(true)` must have no failure toast. `None` never fails.
- [ ] **Step 1: Failing tests** (each proves its rule fires, and stays quiet on correct behaviour):
  - `sound_passes_then_fails`: OK on a scene. Then delete one loose object file under `.git/objects` for a commit's tree → Err.
  - `lost_work_fires`: shadow `add("app", m)`; write `m` into `a.txt` → OK. Overwrite `a.txt` behind the shadow's back → Err naming `m`.
  - `screen_disagrees_after_unrefreshed_outside_checkout`: start a journey, run CLI `git checkout -q topic` without refreshing → Err. After `j.refresh()` → OK.
  - `undo_mismatch_fires`: two readings with different heads → Err; equal → OK.
  - `silent_failure_fires`: `errors_reported(Some(false), &[], &[])` → Err. With a failure toast → OK. `Some(true)` with a failure toast → Err.
  - `panicked_effect_fires`: hard to provoke without a planted bug; covered by Task 10's planted panic.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `test: the exploration tests' always-true rules`.

### Task 9: The session loop, the report and shrinking

**Files:**
- Create: `explore/session.rs`, `explore/shrink.rs`
- Test: `session.rs` and `shrink.rs` `mod tests`

**Interfaces:**

```rust
// session.rs
pub(super) enum Fault {}                       // layer 2 fills this in
pub(super) struct Step { pub action: Action, pub faults: Vec<Fault> }
pub(super) enum Plan { Random { steps: usize }, Replay(Vec<Action>) }
#[derive(Clone, Copy)] pub(super) struct Checks { pub slow_every: usize }   // 1 = fsck and reach after every step
pub(super) struct Outcome { pub seed: u64, pub steps: Vec<Step>, pub broke: Option<Broken>, pub state: String }
pub(super) fn run(tag: &str, seed: u64, plan: Plan, checks: Checks) -> Outcome;
pub(super) fn report(o: &Outcome, shrunk: &[Action]) -> String;   // rule, step, detail, seed, the action lines, o.state
pub(super) fn case_text(seed: u64, rule: Rule, actions: &[Action]) -> String;   // the cases/ format
pub(super) fn parse_case(text: &str) -> Result<(u64, Vec<Action>), String>;

// shrink.rs
/// The shortest sub-list of `actions` for which `breaks` still holds (ddmin: halves, then
/// quarters, down to single actions, keeping any shorter list that breaks), then each kept
/// action simplified by `simpler` (a shorter summary, fewer lines). Stops at `deadline`, returning the best so far.
pub(super) fn shrink(actions: Vec<Action>, breaks: impl FnMut(&[Action]) -> bool, deadline: Instant) -> Vec<Action>;
pub(super) fn simpler(a: &Action) -> Vec<Action>;
```

- `run`: build the scene with `scene::build(tag, seed)`, then `Journey::start_many(fx, app, vec![lib], vec![app, lib])`. The rng is `Rng::new(seed)`. Each step:
  1. Read `before`, the log mark and the active tab.
  2. Pick from `applicable` (Random), or take the next action (Replay; `Skipped` replays go on).
  3. `perform`, then read `after`.
  4. Run rules 1, 4, 5 (when this step is Undo and the previous performed step was an undoable app action: compare with the `before` of that previous step) and 6 every step. Run rules 2 and 3 when `step % slow_every == 0` and at the end.
  5. Record the `Step`.

  Wrap the whole session in `std::panic::catch_unwind(AssertUnwindSafe(..))`. A panic, including the harness's settle timeout, becomes `Broken { rule: NoCrash, detail: <panic message> }` at that step. The `Journey` is dropped before the `Scene`'s fixture, so a panicking session still cleans up.
- `state` in `Outcome`: the branch, `git status --short`, and `git log --oneline -5 --all` from the active repository, taken before the fixture drops.
- [ ] **Step 1: Failing tests:**
  - shrink, pure (no app): `shrinks_to_the_culprit` (`breaks = |l| l.contains(&Undo) && l.contains(&Stash)` over 40 mixed actions gives a 2-action list), `keeps_order` (the result is a subsequence of the input), `respects_deadline` (a deadline already passed returns the input unchanged), `simplifies_text` (`Commit("a very long message")` shrinks to a shorter summary that still breaks).
  - session: `case_round_trip` (`parse_case(&case_text(5, Rule::Sound, &acts)) == Ok((5, acts))`). `same_seed_same_session` (Review Focus 2): `run("d1", 11, Random{steps: 25}, Checks{slow_every: 5})` and `run("d2", 11, ..)` record equal action lists, and both have `broke == None`. `replay_reproduces`: replaying the first run's actions gives `broke == None`, with the same length. `replay_skips_inapplicable` (Review Focus 5): `Replay([Checkout("never-made"), Commit("x")])` doesn't break and records both steps.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `test: exploration sessions, their report and shrinking`.

### Task 10: The entry points, budgets, and proof that the rules fire

**Files:**
- Modify: `explore/mod.rs`: `explore_pr`, `explore_cases`, `explore_soak`
- Create: `explore/cases/README.md`
- Modify: `CLAUDE.md` (Commands), `docs/architecture/app-layers.md` (one line per new file), the area docs for the seam files, `docs/ideas/<area>.md` (actions left out)

**Interfaces:**
- Consumes: `session::{run, report, case_text, parse_case}`, `shrink::shrink`.
- `explore_pr`: for each seed in `const SEEDS: [u64; 3]` with `STEPS` steps and `Checks { slow_every: 10 }`. On a break: shrink with `deadline = now + 60 s`. The predicate replays with `slow_every: 1` and asks for the same `Rule`. Write `case_text` to `temp_dir()/glowfly-explore-<seed>.txt`, then `panic!` with `report(..)`.
- `explore_cases`: every `cases/*.txt` (read from `concat!(env!("CARGO_MANIFEST_DIR"), "/src/journey/explore/cases")`, sorted by name), replayed with `slow_every: 1`; `broke` must be `None`. No cases is fine (README only).
- `explore_soak` (`#[ignore]`): reads `GLOWFLY_EXPLORE_SEEDS` (default 20), `GLOWFLY_EXPLORE_STEPS` (default 500) and `GLOWFLY_EXPLORE_START` (default 1); runs seeds `START..START+SEEDS` with `slow_every: 1`; shrinks and reports as above, collecting every failure before panicking.
- CLAUDE.md Commands line: `` `GLOWFLY_EXPLORE_SEEDS=50 GLOWFLY_EXPLORE_STEPS=500 cargo test -p glowfly explore_soak -- --ignored --nocapture`: long exploration run with fresh seeds (`GLOWFLY_EXPLORE_START` picks the first seed); a failure prints its shrunk case for `src/journey/explore/cases/`. ``

- [ ] **Step 1: Measure.** Run `explore_pr` with 3 seeds × 150 steps in release-test mode on this machine: `cargo test -p glowfly explore_pr -- --nocapture` (print the elapsed time per seed). Set `SEEDS` and `STEPS` so the total is about 40 s locally (the Windows CI runner is slower; aim for ~1 minute there). Then tune the weights so that sessions reach conflicts, stashes and a second branch in most seeds: print a per-action count once, then delete the print.
- [ ] **Step 2: Fix or record what it finds.** Any break on the fixed seeds is a real bug (fix it in its own `fix:` commit, with the case saved in `cases/`) or an explorer bug (fix the explorer). Don't weaken a rule to make a seed pass. If a rule is wrong about correct behaviour, change the rule's text in the spec too and say why in the commit.
- [ ] **Step 3: Prove each kind of failure is caught (throwaway branch, nothing committed).** Plant each of these, one at a time, and confirm the named rule breaks and the case shrinks to ≤ 10 lines:
  - a panic in the stash effect's `Execute` → no-crash;
  - drop the stash without the keep ref (skip `undo::kept` in the drop path) → no-lost-work;
  - skip the branch-list reload after Create branch → screen-agrees;
  - make Checkout return its failure without a toast or log line → errors-reported.

  Revert each one. Paste the four shrunk cases in the PR description.
- [ ] **Step 4: Flake check.** Run `explore_pr` 20 times in a row (`for i in $(seq 20); do cargo test -p glowfly explore_pr -q || break; done`) → 20 passes. Run `pwsh scripts/check.ps1` → "All checks passed."
- [ ] **Step 5: Commit** `test: exploration tests on every build, with saved cases and a soak run`. The docs go in the same commit.

---

## Not in this plan

- The nightly GitHub Actions job. It is a CI change, so it goes to the owner separately, as its own workflow file for approval.
- Layer 2 fault injection. `Fault` and `Step::faults` are its only footprint here.
