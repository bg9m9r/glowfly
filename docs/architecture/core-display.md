# glowfly-core: graph, diff rows, search, previews and watching (`crates/glowfly-core/src/`)

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here.

## Commit graph and branch colours

- `graph.rs`, `graph/`: lane layout for the commit graph, with WIP rows for this worktree and for other worktrees with uncommitted changes.
- `graph/lanes.rs`: the lane layout itself (`layout`), kept with where the lanes stand below the last row so `Layout::extend` lays more commits out under it.
- `owners.rs`: which branch owns each node and lane, naming a merge's other parents from its message: `merge_names.rs`. `Owners::extend` goes on from the last row's lane owners for more rows.
- `Snapshot::lay_out_graph` (`repo/branch_colours.rs`) lays the graph out with its owners and colours, at load and when the work tree turns clean or dirty; `Snapshot::layout_job` hands the same work to a worker (`LayoutJob`, `LayoutFit`) for a large history.
- `branch_colour.rs`: branch colours: a branch's key (its name, without the remote), its hue slot from an FNV-1a hash of the key, and a person's slot from their email (`person_slot`).
- `resolve` gives main the accent, holds pinned branches and keeps side-by-side branches on different hues.
- `Snapshot::set_colouring` recolours a loaded graph after a pin or preset change.

## Diff, whole-file and conflict rows

- `diff_rows.rs`, `whole_file.rs`, `whole_file/`, `conflict_rows.rs`: row layout of diffs, the whole-file view and the conflict view, with the conflict chunks safe to pick without asking: `trivial_pick`.
- `conflict_folds.rs`: the long clean runs of the conflict view folded into one row.
- `diff_rows/line_row.rs`: the row of a file line.
- `diff_rows/gaps.rs`: the hunk view's gaps between hunks, keyed like the whole-file folds of the same lines, and their filled-in rows.
- `intraline.rs`: word-level changes.
- `line_pairs.rs`: which removed line sits beside which added line.
- `tabs.rs`: tab stops.
- `wrap.rs`: where wrapped lines break.
- `conflict_pick.rs`: what each conflict chunk resolves to (whole sides, single lines in tick order, or text typed by hand) and the file that makes, with conflict markers left for the chunks not picked yet and the clean runs between them as edited (`render`, `render_picks`).
- `conflict_edit.rs`: the conflict view's output as editable text with each chunk's and clean run's range, and which one an edit of it falls in (`locate_edit`).
- `conflict_restore.rs`: the picks a saved file holds, read back against the stages.

## Syntax highlighting

- `highlight.rs`, `highlight/`: syntax highlighting of diff lines (syntect), as token kinds; whole files too (`highlight_file`, for blame).
- `memo.rs`: parse checkpoints a reload of the same file resumes from: content-defined chunks with their start and end states.
- `highlight/syntaxes.rs`: the syntax definitions, loaded once (immutable, see Rules).

## Previews

- `preview_image.rs`: decoding a PNG, JPEG, GIF, WebP or BMP (an animation's first frame), shared by the binary diff's preview and the file history's File view.
- `preview_image.rs` bounds dimensions and allocation, reads the format from the content, and scales down to at most 1024 px a side for the diff (`decode`) or 2048 for the File view (`decode_sized`, which says why an image can't be shown); it also says which paths count as images.
- `markdown.rs`: Markdown parsed with pulldown-cmark into blocks and styled runs, for pull request and issue bodies and the file history's File view preview: HTML dropped, images as alt-text links, bare https links, nesting capped, nothing over 512 KB. Pure.
- `preview.rs`: what the File view previews, by extension (`kind`: Markdown or an image). Pure.

## Search

- `fuzzy.rs`, `matching.rs`: matching for the command palette, filters and search (`contains_lower` looks for an ASCII needle in place, without lowercasing, even in text that isn't all ASCII).
- `commit_query.rs`: commit search queries: author and path terms, matching rows, the author list.
- `path_query.rs`: path terms of the commit search: folder, file name or glob, matched ignoring case and Unicode form.
- `commit_index/`: the whole-history index for search: every commit the graph's walk would visit, in its order, with what search matches (the summary and the whole flattened body, held once). Tests: `tests.rs`, `build_tests.rs`, `store_tests.rs`, `deep_match_tests.rs` (a match past the preview, found with and without the index; an old-format file rebuilt).
- `commit_index/build.rs`: build and incremental update off the UI thread.
- `commit_index/store.rs`: the saved index per repository in the cache folder: used as it is or as a base, pruned by age and size.
- `commit_index/store/format.rs`: the saved file's layout: header, sections, CRC trailer, and the checks on read.
- `commit_index/` matches through `CommitQuery`'s own text and author tests.
- `estimate.rs`: the commit-graph estimate of the commit count.
- The walk's order is shared with the graph through `repo/walk_order.rs` (see [core-repo.md](core-repo.md)).

## Watching for changes

- `watch.rs`: which file changes in a repository matter, and the work-tree paths a change names (`Change::Paths`, at most `MAX_WATCH_PATHS` per burst).
- The git dir's index is `Change::Index`, which keeps a burst's paths.
- A submodule's HEAD or refs moving (in `<git dir>/modules/…` or a nested `.git` folder) reads the parent's whole status; their index, logs and objects don't count.
- `config` and `config.worktree` reload in full, and so do the files they include, wherever they are (`WatchPaths::set_config_files`), as do a linked worktree's shared refs, packed-refs, HEAD and config in the common git dir.
- `Ignores` caches ignored folders but never drops a path the index holds (a force-added file, or one committed before its pattern), nor a folder above one when it comes or goes.
- `watch_sweep.rs`: the Windows safety poll's sweep of a watched tree: a baseline of every non-ignored folder's listing, then each sweep stats them, lists again the changed, racy and a rotating tenth, and names the paths (and the git dir's refs, index, exclude and attributes, and the config files with those they include) that may have changed.
- `watch_sweep/nested.rs`: the submodules and nested repositories the sweep walks into, each by its own ignore rules, with their HEAD and refs compared every sweep.
- `config_includes.rs`: the files git's `include.path` and `includeIf` pull in, resolved as git does (relative to the including file, `~/`, `gitdir:`, `gitdir/i:`, `onbranch:`), bounded in depth and count and never following a cycle.
- `config_includes/parse.rs`: the include entries of one config file's text; `config_includes/wildmatch.rs`: git's pattern matching for the conditions.
