//! The in-process path of the kit's `git`: which commands it takes, and `init`, `config`,
//! `add`, `update-index --refresh` and `checkout -- <file>` in their plain forms through
//! libgit2 (`commit` is in `commit.rs`; `checkout`, `branch`, `tag`, `update-ref` and `remote`
//! in `refs.rs`; `push` in `push.rs`). Each declines, having changed nothing a later CLI run
//! would notice, when the command isn't in one of those forms or the repository needs the CLI
//! (`guard.rs`).

use super::{commit, guard, push, refs};
use git2::build::CheckoutBuilder;
use git2::{DiffOptions, IndexAddOption, Repository, RepositoryInitOptions};
use std::path::Path;

/// Runs `a` in `dir` in-process when it can: Some(whether it succeeded, as git's exit status
/// would say), or None when the git CLI has to run it.
pub(super) fn run(dir: &Path, a: &[&str], env: &[(&str, &str)]) -> Option<bool> {
    if guard::env_needs_cli(env) {
        return None;
    }
    let (identity, a) = identity_overrides(a)?;
    let (&cmd, rest) = a.split_first()?;
    if !identity.is_empty() && cmd != "commit" {
        return None;
    }
    if cmd == "init" {
        return init(dir, rest).then_some(true);
    }
    let repo = guard::open(dir)?;
    if cmd == "config" {
        return config(&repo, rest).then_some(true);
    }
    if repo.is_bare() || guard::repo_needs_cli(&repo) {
        return None;
    }
    if cmd == "update-index" {
        return if rest == ["-q", "--refresh"] { refresh(&repo) } else { None };
    }
    let done = match cmd {
        "add" => add(&repo, rest),
        "commit" => commit::commit(&repo, rest, env, &identity),
        "checkout" => restore(&repo, rest) || refs::checkout(&repo, rest),
        "branch" => refs::branch(&repo, rest),
        "tag" => refs::tag(&repo, rest),
        "remote" => refs::remote(&repo, rest),
        "update-ref" => refs::update_ref(&repo, rest),
        "push" => push::push(&repo, rest),
        _ => false,
    };
    done.then_some(true)
}

/// `(key, value)` settings given on the command line.
type Settings<'a> = Vec<(&'a str, &'a str)>;

/// Splits leading `-c user.name=<n>` and `-c user.email=<e>` from `a`; None for any other `-c`.
fn identity_overrides<'a>(mut a: &'a [&'a str]) -> Option<(Settings<'a>, &'a [&'a str])> {
    let mut out = Vec::new();
    while let ["-c", setting, rest @ ..] = a {
        let (key, value) = setting.split_once('=')?;
        if key != "user.name" && key != "user.email" {
            return None;
        }
        out.push((key, value));
        a = rest;
    }
    Some((out, a))
}

/// `git init [-q] [--bare] [-b <name>]` in an existing folder that isn't a repository yet.
fn init(dir: &Path, a: &[&str]) -> bool {
    let (mut bare, mut head) = (false, None);
    let mut it = a.iter();
    while let Some(&arg) = it.next() {
        match arg {
            "-q" | "--quiet" => {}
            "--bare" => bare = true,
            "-b" | "--initial-branch" => match it.next() {
                Some(name) => head = Some(*name),
                None => return false,
            },
            _ => match arg.strip_prefix("--initial-branch=") {
                Some(name) => head = Some(name),
                None => return false,
            },
        }
    }
    if !dir.is_dir() || dir.join(".git").exists() || dir.join("HEAD").exists() || std::env::var_os("GIT_TEMPLATE_DIR").is_some() {
        return false;
    }
    let mut opts = RepositoryInitOptions::new();
    opts.bare(bare).no_reinit(true).mkdir(false);
    if let Some(h) = head {
        opts.initial_head(h);
    }
    let Ok(repo) = Repository::init_opts(dir, &opts) else { return false };
    git_core_order(&repo.path().join("config"));
    true
}

/// Puts the `[core]` keys of a new repository's config in the order `git init` writes them
/// (libgit2 writes the same keys in another order), for tests that read the file.
fn git_core_order(config: &Path) {
    const ORDER: [&str; 7] =
        ["repositoryformatversion", "filemode", "bare", "logallrefupdates", "symlinks", "ignorecase", "precomposeunicode"];
    let text = std::fs::read_to_string(config).expect("the new repository's config");
    let mut lines: Vec<&str> = text.lines().collect();
    let end = lines.iter().skip(1).position(|l| l.starts_with('[')).map_or(lines.len(), |p| p + 1);
    if lines.first() != Some(&"[core]") {
        return;
    }
    let rank = |l: &&str| ORDER.iter().position(|k| l.trim_start().starts_with(&format!("{k} "))).unwrap_or(ORDER.len());
    lines[1..end].sort_by_key(rank);
    std::fs::write(config, lines.join("\n") + "\n").expect("the new repository's config");
}

/// `git config <key> <value>`: sets the key in the repository's own config.
fn config(repo: &Repository, a: &[&str]) -> bool {
    let [key, value] = a else { return false };
    // libgit2 writes an empty value, or one with quotes, comment marks, backslashes or edge
    // spaces, differently from git (`key = ""` for `key = `); leave those to git.
    let plain = !value.is_empty() && value.trim() == *value && !value.contains(['"', '\\', '#', ';', '\n']);
    if key.starts_with('-') || !plain {
        return false;
    }
    let Ok(mut local) = repo.config().and_then(|c| c.open_level(git2::ConfigLevel::Local)) else { return false };
    local.set_str(key, value).is_ok()
}

/// `git add [-A | -u] [--] [<path>...]`: stages new, changed and (as git 2 does) deleted files
/// under the paths (`.` or none with `-A` meaning everything).
fn add(repo: &Repository, a: &[&str]) -> bool {
    let (mut tracked_only, mut all, mut paths) = (false, false, Vec::new());
    let mut options_done = false;
    for &arg in a {
        match arg {
            "-A" | "--all" if !options_done => all = true,
            "-u" | "--update" if !options_done => tracked_only = true,
            "--" if !options_done => options_done = true,
            _ if arg.starts_with('-') && !options_done => return false,
            "." => paths.push("*".to_owned()),
            _ => paths.push(arg.replace('\\', "/")),
        }
    }
    if paths.is_empty() {
        if !(all || tracked_only) {
            return false;
        }
        paths.push("*".to_owned());
    }
    let Ok(mut index) = repo.index() else { return false };
    if index.has_conflicts() {
        return false;
    }
    if !tracked_only && index.add_all(&paths, IndexAddOption::DEFAULT, None).is_err() {
        return false;
    }
    if index.update_all(&paths, None).is_err() || guard::index_needs_cli(repo, &index) {
        return false;
    }
    index.write().is_ok()
}

/// `git update-index -q --refresh`: re-reads the stat data of every file whose content still
/// matches the index (after a copy, say). With `-q` git succeeds even when some file changed.
fn refresh(repo: &Repository) -> Option<bool> {
    let mut index = repo.index().ok()?;
    let mut opts = DiffOptions::new();
    opts.update_index(true);
    repo.diff_index_to_workdir(Some(&index), Some(&mut opts)).ok()?;
    index.write().ok()?;
    Some(true)
}

/// `git checkout [-q] -- <file>...`: puts each file back as the index has it.
fn restore(repo: &Repository, a: &[&str]) -> bool {
    let files = match a {
        ["--", files @ ..] | ["-q", "--", files @ ..] if !files.is_empty() => files,
        _ => return false,
    };
    let plain = |f: &str| !f.contains('\\') && f.split('/').all(|part| !part.is_empty() && part != "." && part != "..");
    if !files.iter().all(|f| plain(f)) {
        return false;
    }
    let Ok(index) = repo.index() else { return false };
    if !files.iter().all(|f| index.get_path(Path::new(f), 0).is_some()) || guard::index_needs_cli(repo, &index) {
        return false;
    }
    let mut opts = CheckoutBuilder::new();
    opts.force().disable_pathspec_match(true);
    for f in files {
        opts.path(*f);
    }
    repo.checkout_index(None, Some(&mut opts)).is_ok()
}
