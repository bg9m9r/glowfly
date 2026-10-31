//! Reading what a git command brings into the working tree from its arguments (`targets`).
//! Every subcommand is either one that brings in no other commit's files (`INERT`), one read
//! here, or Unmodeled: where hooks follow the working tree, a command Glowfly doesn't read
//! (an alias, `bisect`, `submodule update`, `worktree add`) is held, never run unchecked.

use crate::trust::incoming::{How, Target, Unchecked, sub_at};

/// Subcommands that write no other commit's files into this working tree: they read, change
/// refs, the index or other repositories, or run hooks from the working tree as it is.
const INERT: &[&str] = &[
    "add",
    "archive",
    "blame",
    "branch",
    "bundle",
    "cat-file",
    "check-attr",
    "check-ignore",
    "check-ref-format",
    "cherry",
    "clean",
    "clone",
    "commit",
    "commit-tree",
    "config",
    "count-objects",
    "credential",
    "describe",
    "diff",
    "diff-files",
    "diff-index",
    "diff-tree",
    "fetch",
    "for-each-ref",
    "format-patch",
    "fsck",
    "gc",
    "grep",
    "hash-object",
    "help",
    "init",
    "interpret-trailers",
    "log",
    "ls-files",
    "ls-remote",
    "ls-tree",
    "mailinfo",
    "mailsplit",
    "maintenance",
    "merge-base",
    "merge-tree",
    "mktree",
    "mv",
    "name-rev",
    "notes",
    "prune",
    "push",
    "range-diff",
    "reflog",
    "remote",
    "repack",
    "rev-list",
    "rev-parse",
    "rm",
    "shortlog",
    "show",
    "show-ref",
    "status",
    "stripspace",
    "symbolic-ref",
    "tag",
    "update-index",
    "update-ref",
    "var",
    "verify-commit",
    "verify-tag",
    "version",
    "write-tree",
];

/// The options of `sub` that take the next argument as their value.
fn valued(sub: &str, opt: &str) -> bool {
    let of: &[&str] = match sub {
        "checkout" | "switch" => &["-b", "-B", "-c", "-C", "--orphan", "--create", "--force-create", "--conflict", "--pathspec-from-file"],
        "restore" => &["-s", "--source", "--conflict", "--pathspec-from-file"],
        "merge" => &["-m", "-F", "--file", "-s", "-X", "--strategy", "--strategy-option", "--into-name", "--cleanup"],
        "rebase" => &["--onto", "-s", "-X", "-x", "--exec", "--strategy", "--strategy-option", "--whitespace", "--empty"],
        "cherry-pick" | "revert" => &["-m", "--mainline", "--strategy", "-X", "--strategy-option", "--cleanup"],
        "am" => &["--patch-format", "--exclude", "--include", "--directory", "--resolvemsg", "-C", "--quoted-cr", "--empty"],
        "apply" => &["--exclude", "--include", "--directory", "-C", "--whitespace", "--build-fake-ancestor"],
        "reset" => &["--pathspec-from-file"],
        _ => &[],
    };
    of.contains(&opt)
}

/// The rev-list options a cherry-pick or revert may pass on (any other is Unmodeled): those
/// that choose commits, never one that writes a file (`--output`).
fn walk_option(opt: &str) -> bool {
    const BARE: &[&str] = &[
        "--not",
        "--all",
        "--branches",
        "--tags",
        "--remotes",
        "--no-merges",
        "--merges",
        "--first-parent",
        "--ancestry-path",
        "--topo-order",
        "--date-order",
        "--author-date-order",
        "--cherry-pick",
        "--right-only",
        "--left-only",
        "--cherry",
        "--simplify-by-decoration",
        "--no-walk",
        "--do-walk",
        "--regexp-ignore-case",
        "-i",
        "--all-match",
        "--invert-grep",
    ];
    let valued = [
        "--branches=",
        "--tags=",
        "--remotes=",
        "--glob=",
        "--exclude=",
        "--max-count=",
        "--skip=",
        "--since=",
        "--after=",
        "--until=",
        "--before=",
        "--author=",
        "--committer=",
        "--grep=",
        "--min-parents=",
        "--max-parents=",
        "--no-walk=",
    ];
    BARE.contains(&opt) || valued.iter().any(|p| opt.starts_with(p))
}

/// Options of a cherry-pick or revert that aren't for its rev-list.
fn pick_own(opt: &str) -> bool {
    const OWN: &[&str] = &[
        "-e",
        "--edit",
        "--no-edit",
        "-n",
        "--no-commit",
        "-x",
        "-s",
        "--signoff",
        "--no-signoff",
        "--ff",
        "--allow-empty",
        "--allow-empty-message",
        "--keep-redundant-commits",
        "--no-gpg-sign",
        "--rerere-autoupdate",
        "--no-rerere-autoupdate",
        "--reference",
        "--no-reference",
    ];
    let prefixed = ["--empty=", "--gpg-sign", "-S", "--cleanup=", "--strategy=", "--strategy-option=", "-X", "-m", "--mainline="];
    OWN.contains(&opt) || prefixed.iter().any(|p| opt.starts_with(p))
}

/// An option's value from `--opt=value` or the next argument.
fn value_of(a: &str, long: &str, short: Option<&str>, next: &mut dyn Iterator<Item = &String>) -> Option<String> {
    if a == long || Some(a) == short {
        return next.next().cloned();
    }
    if let Some(v) = a.strip_prefix(long).and_then(|v| v.strip_prefix('=')) {
        return Some(v.to_owned());
    }
    short.and_then(|s| a.strip_prefix(s)).filter(|v| !v.is_empty() && !a.starts_with("--")).map(str::to_owned)
}

/// Git's own options before the subcommand that change where or how it runs: another
/// repository or working tree, or another place hooks run from.
fn global_unmodeled(cmd: &[String], at: usize) -> Option<String> {
    let globals = &cmd[..at];
    let mut it = globals.iter();
    while let Some(a) = it.next() {
        let config = match a.as_str() {
            "-c" | "--config-env" => it.next().map(|v| v.to_ascii_lowercase()),
            "-C" | "--git-dir" | "--work-tree" | "--namespace" => return Some(a.clone()),
            s if s.starts_with("--git-dir=") || s.starts_with("--work-tree=") || s.starts_with("--config-env=") => {
                return Some(s.to_owned());
            }
            _ => None,
        };
        if config.is_some_and(|c| c.starts_with("core.hookspath") || c.starts_with("core.worktree")) {
            return Some(format!("-c {}", a));
        }
    }
    None
}

/// What the git command `cmd` (its arguments, without `git`) brings into the working tree.
/// Empty for a command that brings in nothing (`checkout -b new`, `merge --abort`, `commit`)
/// and for a pull (pinned by pull.rs); Unmodeled for one Glowfly doesn't read.
pub fn targets(cmd: &[String]) -> Result<Vec<Target>, Unchecked> {
    let Some(at) = sub_at(cmd) else { return Ok(Vec::new()) };
    if let Some(g) = global_unmodeled(cmd, at) {
        return Err(Unchecked::Unmodeled(format!("git {g}")));
    }
    let (sub, args) = (cmd[at].as_str(), &cmd[at + 1..]);
    let unmodeled = |what: &str| Err(Unchecked::Unmodeled(format!("git {sub} {what}").trim_end().to_owned()));
    let first = args.first().map(String::as_str).unwrap_or_default();
    match sub {
        "pull" => return Ok(Vec::new()),
        _ if INERT.contains(&sub) => return Ok(Vec::new()),
        "stash" => {
            return match first {
                "list" | "show" | "drop" | "clear" | "store" | "create" => Ok(Vec::new()),
                // Stashing writes HEAD's version of the files (and with --keep-index, the
                // index's), and git runs hooks from it in the same command.
                "" | "push" | "save" => {
                    let keep = args.iter().any(|a| a == "--keep-index" || a == "-k");
                    let head = Target::Rev("HEAD".to_owned());
                    Ok(if keep { vec![head, Target::Index(vec![".".to_owned()])] } else { vec![head] })
                }
                "apply" | "pop" => {
                    let index = args.iter().any(|a| a == "--index");
                    let spec = args[1..].iter().find(|a| !a.starts_with('-')).cloned().unwrap_or_else(|| "stash@{0}".to_owned());
                    let spec = if spec.bytes().all(|b| b.is_ascii_digit()) { format!("stash@{{{spec}}}") } else { spec };
                    Ok(vec![Target::Stash { spec, index }])
                }
                _ => unmodeled(first),
            };
        }
        "worktree" => return if first == "add" { unmodeled("add") } else { Ok(Vec::new()) },
        "sparse-checkout" => return if first == "list" { Ok(Vec::new()) } else { unmodeled(first) },
        "submodule" => return if matches!(first, "status" | "summary") { Ok(Vec::new()) } else { unmodeled(first) },
        "lfs" => {
            let inert = [
                "fetch", "push", "track", "untrack", "install", "version", "ls-files", "locks", "lock", "unlock", "status", "env", "prune",
            ];
            return if inert.contains(&first) { Ok(Vec::new()) } else { unmodeled(first) };
        }
        "checkout" | "switch" | "restore" | "merge" | "rebase" | "cherry-pick" | "revert" | "am" | "apply" | "reset" => {}
        _ => return unmodeled(""),
    }
    let mut positional = Vec::new();
    let mut after_dashes: Option<Vec<String>> = None;
    let (mut onto, mut root, mut keep_base, mut sequencer, mut skip, mut creates) = (None, false, false, false, false, false);
    let (mut how, mut source, mut worktree, mut staged, mut cached, mut hard) = (How::default(), None, false, false, false, false);
    let (mut apply_opts, mut format, mut strategy, mut force) = (Vec::new(), None, None, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let a = a.as_str();
        if let Some(s) = value_of(a, "--strategy", matches!(sub, "merge" | "rebase").then_some("-s"), &mut it) {
            strategy = Some(s);
            continue;
        }
        if let Some(x) = value_of(a, "--strategy-option", Some("-X"), &mut it) {
            how.xopts.push(x);
            continue;
        }
        if matches!(sub, "cherry-pick" | "revert")
            && let Some(m) = value_of(a, "--mainline", Some("-m"), &mut it)
        {
            how.mainline = Some(m.parse().map_err(|_| Unchecked::Unresolved(format!("-m {m}")))?);
            continue;
        }
        if matches!(sub, "am" | "apply")
            && let Some(o) =
                ["--directory", "--exclude", "--include"].iter().find_map(|l| value_of(a, l, None, &mut it).map(|v| format!("{l}={v}")))
        {
            apply_opts.push(o);
            continue;
        }
        if sub == "restore"
            && let Some(s) = value_of(a, "--source", Some("-s"), &mut it)
        {
            source = Some(s);
            continue;
        }
        if sub == "am"
            && let Some(f) = value_of(a, "--patch-format", None, &mut it)
        {
            format = Some(f);
            continue;
        }
        match a {
            // After `--` a merge, rebase or pick still names revisions; elsewhere it names paths.
            "--" if matches!(sub, "merge" | "rebase" | "cherry-pick" | "revert") => {
                positional.extend(it.by_ref().cloned());
                break;
            }
            "--" => {
                after_dashes = Some(it.by_ref().cloned().collect());
                break;
            }
            "--abort" | "--quit" | "--show-current-patch" => return Ok(Vec::new()),
            "--skip" => (sequencer, skip) = (true, true),
            "--continue" | "--edit-todo" => sequencer = true,
            "-" => positional.push("@{-1}".to_owned()),
            "--onto" if sub == "rebase" => onto = it.next().cloned(),
            v if sub == "rebase" && v.starts_with("--onto=") => onto = Some(v["--onto=".len()..].to_owned()),
            "--root" => root = true,
            "--keep-base" => keep_base = true,
            "--allow-unrelated-histories" => how.unrelated = true,
            "--autostash" => how.autostash = true,
            "--fork-point" => how.fork_point = Some(true),
            "--no-fork-point" => how.fork_point = Some(false),
            "--reapply-cherry-picks" => how.reapply = true,
            "-W" | "--worktree" => worktree = true,
            "-S" | "--staged" if sub == "restore" => staged = true,
            "--cached" | "--index" if sub == "apply" => cached |= a == "--cached",
            "--hard" | "--merge" | "--keep" if sub == "reset" => hard = true,
            "-f" | "--force" if sub == "checkout" => force = true,
            "-3" | "--3way" if sub == "am" => return unmodeled("--3way"),
            "-i" | "--interactive" | "-r" | "--rebase-merges" | "--apply" | "--autosquash" if sub == "rebase" => return unmodeled(a),
            v if sub == "rebase" && (v.starts_with("-C") || v.starts_with("--whitespace") || v.starts_with("--rebase-merges")) => {
                return unmodeled(v);
            }
            "--stdin" => return unmodeled("--stdin"),
            "--check" | "--numstat" | "--stat" | "--summary" if sub == "apply" => cached = true,
            "-b" | "-B" | "-c" | "-C" | "--orphan" | "--create" | "--force-create" if matches!(sub, "checkout" | "switch") => {
                creates = true;
                it.next();
            }
            v if v.starts_with("-p") && matches!(sub, "am" | "apply") => apply_opts.push(v.to_owned()),
            v if valued(sub, v) => {
                it.next();
            }
            v if matches!(sub, "cherry-pick" | "revert") && v.starts_with('-') && !pick_own(v) && walk_option(v) => {
                how.walk.push(v.to_owned())
            }
            v if matches!(sub, "cherry-pick" | "revert") && v.starts_with('-') && !pick_own(v) => return unmodeled(v),
            v if v.starts_with('-') => {}
            v => positional.push(v.to_owned()),
        }
    }
    match strategy.as_deref() {
        None | Some("ort") | Some("recursive") => how.strategy = strategy.clone(),
        Some("ours") if sub == "merge" => how.strategy = strategy.clone(),
        Some(s) => return unmodeled(&format!("-s {s}")),
    }
    let rev = |s: &String| Target::Rev(s.clone());
    Ok(match sub {
        "rebase" | "cherry-pick" | "revert" | "am" if sequencer => vec![Target::Applying, Target::Sequencer { skip }],
        _ if sequencer => Vec::new(),
        "checkout" | "switch" if creates => positional.first().map(rev).into_iter().collect(),
        "switch" => positional.first().map(|n| Target::Switch { name: n.clone(), paths: false }).into_iter().collect(),
        "checkout" => match (positional.as_slice(), after_dashes) {
            ([], Some(paths)) => vec![Target::Index(paths)],
            // `checkout -f` puts back HEAD's version of every file.
            ([], None) if force => vec![rev(&"HEAD".to_owned())],
            ([], None) => Vec::new(),
            ([tree, ..], Some(paths)) if !paths.is_empty() => vec![Target::Paths { tree: tree.clone(), paths, maybe_paths: false }],
            ([name], Some(_)) => vec![Target::Switch { name: name.clone(), paths: false }],
            ([name], None) => vec![Target::Switch { name: name.clone(), paths: true }],
            ([tree, rest @ ..], _) => vec![Target::Paths { tree: tree.clone(), paths: rest.to_vec(), maybe_paths: true }],
        },
        "restore" => {
            let paths = [positional, after_dashes.unwrap_or_default()].concat();
            match source {
                _ if staged && !worktree => Vec::new(),
                Some(tree) => vec![Target::Paths { tree, paths, maybe_paths: false }],
                // Both the index and the files: from HEAD, as git restores them.
                None if staged => vec![Target::Paths { tree: "HEAD".to_owned(), paths, maybe_paths: false }],
                None => vec![Target::Index(paths)],
            }
        }
        // `reset <rev>` moves the index only; `--hard`, `--merge` and `--keep` write the files.
        "reset" if hard => vec![rev(positional.first().unwrap_or(&"HEAD".to_owned()))],
        "reset" => Vec::new(),
        "merge" if positional.is_empty() => vec![Target::Merged { revs: vec!["@{upstream}".to_owned()], how }],
        "merge" => vec![Target::Merged { revs: positional, how }],
        "am" | "apply" if format.as_deref().is_some_and(|f| f != "mbox") => {
            return unmodeled(&format!("--patch-format={}", format.unwrap_or_default()));
        }
        "am" | "apply" if positional.is_empty() && !cached => return unmodeled("(patches from standard input)"),
        "apply" if cached => Vec::new(),
        // One for each file: choosing to apply one that writes hooks covers that one only.
        "am" | "apply" => {
            positional.into_iter().map(|f| Target::Patches { files: vec![f], apply: apply_opts.clone(), mailbox: sub == "am" }).collect()
        }
        "rebase" => {
            let (upstream, branch) = (positional.first().cloned(), positional.get(1).cloned());
            vec![Target::Replayed { onto, upstream: if root { None } else { upstream }, branch, root, keep_base, how }]
        }
        "cherry-pick" | "revert" => vec![Target::Picked { revs: positional, revert: sub == "revert", how }],
        _ => Vec::new(),
    })
}
