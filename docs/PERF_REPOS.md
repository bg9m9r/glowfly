# Large test repositories

`scripts/make-perf-repos.ps1` builds a reusable set of large repositories for trying Glowfly on
histories, trees and refs far bigger than the test fixtures. It works in PowerShell 7 on Windows,
macOS and Linux and needs only git (2.28 or newer).

```
pwsh scripts/make-perf-repos.ps1 -DryRun      # list what would be built and where; writes nothing
pwsh scripts/make-perf-repos.ps1              # build the synthetic repositories (a few minutes)
pwsh scripts/make-perf-repos.ps1 -Download    # also clone git/git and the Linux kernel
pwsh scripts/make-perf-repos.ps1 -Only linear,octopus
```

- **Where:** `-Out <folder>`, by default `glowfly-perf-repos` in the local cache folder
  (`%LOCALAPPDATA%`, `~/Library/Caches` or `$XDG_CACHE_HOME`, else `~/.cache`). It refuses a
  folder inside the Glowfly checkout. Never commit these repositories.
- **Downloads are opt-in:** the two clones are made only with `-Download`. They fetch about
  6 GB, nearly all of it the kernel.
- **Running again** builds only what is missing. A finished repository is never touched. An
  unfinished synthetic one is built again from the start, and an unfinished clone carries on
  fetching. Progress is kept in `<out>/.state/`.
- **Deleting:** the script deletes nothing it didn't make. Its git calls run hooks from an
  empty folder it makes in `<out>/.state`, never the user's. `-Rebuild <names>` builds finished
  synthetic repositories again; remove a clone's folder yourself to clone it again.
- **Smaller copies:** `-Scale 0.01` multiplies every count and size of the synthetic
  repositories, for a quick look. A dry run notes any repository built at another scale.
- **Mirrors:** `-GitUrl` and `-LinuxUrl` clone from elsewhere.

## The repositories

Sizes are rough, at scale 1. Point Glowfly or `GLOWFLY_PERF_REPO` at `<out>/<name>`, or at
`<out>/submodules/super` for the submodules; the script prints each path at the end.

- `git`: git/git's real history (~75k commits, ~300 MB).
- `linux`: the Linux kernel, a full clone, not shallow (~1.3M commits, ~90k files, ~7 GB with
  the checkout).
- `linear`: 1M commits in a line, each changing one of 2,000 files (~0.6 GB).
- `many-refs`: 100k tags (a tenth annotated) and 20k branches in 50 folders, on 100k commits
  (~0.1 GB).
- `wide-tree`: 500k files in 5,000 folders, with ~10k changed, ~2.5k deleted and ~5k untracked
  files, and the changes in one folder staged (~0.7 GB).
- `octopus`: 60 long-lived lanes merged into main by 100 octopus merges of 61 parents each, then
  2,000 topic branches forked 100 at a time and merged one by one (~10 MB).
- `big-files`: a 100 MB log, a 20 MB single-line file, a 50 MB binary in three revisions and
  200 binaries of 1 MB (~0.75 GB).
- `lfs-pointers`: 5,000 Git LFS pointer files over 200 commits, with no LFS objects behind them
  (~10 MB).
- `submodules`: a superproject with submodules three deep and one beside them; `libs/a` is
  checked out ahead of the commit it records and the deepest has an untracked file.

The synthetic repositories are made with `git fast-import` from deterministic streams
(`scripts/perf-repos/Streams.cs`): names, dates, messages and file contents follow from the
commit number, so two builds at one scale are alike. The authors include Ada Lovelace and the
messages "parser" and "über", which the queries in `perf_timing_tests` look for.

**Windows and macOS:** the kernel has file names Windows reserves (`aux.c`) and paths differing
only in case. The clone leaves those out of its checkout with a sparse checkout and lists them in
`<out>/.state/linux.skipped.txt`. On Windows it also turns off `core.protectNTFS` in that clone,
which git needs to keep a reserved name in the index. The setting stays off, because checking out
any other revision that holds the name fails with it on. To make up for it, the clone also leaves
out any `.git` or `git~1` path, the names that setting guards against.

## Timing tests

`perf_timing_tests` (glowfly-flows) times the whole-history work `update` does on the UI
thread: search keystrokes, relayout, branch hover and swapping snapshots.

```
GLOWFLY_PERF_REPO=<out>/linux cargo test -p glowfly-flows --release perf_timing -- --ignored --nocapture
```

In PowerShell, set `$env:GLOWFLY_PERF_REPO = '<out>/linux'` first. Run it against `git` and
`linux` at least, and against `linear` and `many-refs` when a change touches the history or refs.

## Manual checklist

Run a release build (`cargo run --release -- <repo>`) on each repository and time each step
with a stopwatch or from the activity log:

1. **Open:** from launch to the history showing, and to the graph and refs being complete.
2. **Scroll:** drag to the bottom of the history and back; note any stutter or blank rows.
3. **Search:** type a query (`parser`, `author:ada`), then one with no matches; time to results.
4. **Diff:** select commits with large diffs (`big-files`, a kernel merge) and a binary file.
5. **Status:** the working-tree list on `wide-tree` and `submodules`; refresh after a change.
6. **Branch switch:** check out another branch and back (`many-refs`, `octopus`, the kernel).

Record the results in a comment on the issue of the change being measured (the first baseline
went on #419): the commit, the machine (CPU, RAM, disk, OS), the repository, the
`perf_timing_tests` output and a line per checklist step.

## Checking the script

`pwsh scripts/make-perf-repos-test.ps1` runs the script on small local fixtures and downloads
nothing: the dry run, every synthetic repository at a thousandth of its size, both clones from a
local fixture (with a reserved name and a case pair), and a second run that rebuilds nothing,
resumes an unfinished clone and leaves a folder it didn't make alone. CI runs it on Windows,
macOS and Linux.
