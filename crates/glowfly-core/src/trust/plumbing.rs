//! git's own plumbing, asked what a command would do instead of modelling it: what a revision
//! names (`resolve`), the commits a rev-list names (`rev_list`), the tree a three-way merge
//! leaves (`merge_tree`, ort exactly, objects only), the paths the patches in a mailbox write
//! once decoded (`mailbox_paths`: mailsplit, mailinfo, `apply --numstat`). None of these runs a
//! hook, touches the index or the working tree, or reads the fsmonitor; whatever fails, or
//! needs a newer git, is Unchecked.

use super::incoming::Unchecked;
use crate::gitcli;
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The folder git runs in for `repo`.
fn cwd(repo: &Repository) -> &Path {
    repo.workdir().unwrap_or_else(|| repo.path())
}

/// `git <args>` with the fsmonitor off (plumbing never runs a hook).
fn cmd(args: &[&str]) -> Vec<String> {
    gitcli::args(&[&["-c", "core.fsmonitor=false"], args].concat())
}

/// Runs `cmd` in `repo` with `input`: whether git succeeded, and its standard output (or all
/// it said when it failed).
fn run(repo: &Repository, cmd: &[String], input: &str) -> (bool, String) {
    let out = gitcli::output(cwd(repo), cmd, input, gitcli::NO_PROMPTS, None);
    (out.ok, if out.ok { out.stdout } else { out.combined() })
}

/// The installed git's (major, minor) version; (0, 0) when it can't be read.
fn version() -> (u32, u32) {
    static V: OnceLock<(u32, u32)> = OnceLock::new();
    *V.get_or_init(|| {
        let out = gitcli::output(Path::new("."), &gitcli::args(&["version"]), "", gitcli::NO_PROMPTS, None);
        let num = out.stdout.split_whitespace().nth(2).unwrap_or_default().to_owned();
        let mut parts = num.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
        (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
    })
}

/// Unmodeled unless git is at least `major.minor` (`what` needs it).
pub(crate) fn needs(major: u32, minor: u32, what: &str) -> Result<(), Unchecked> {
    if version() >= (major, minor) { Ok(()) } else { Err(Unchecked::Unmodeled(format!("{what} (git {major}.{minor} or newer)"))) }
}

/// The object of type `peel` (commit, tree) that `spec` names: resolved by `git rev-parse`,
/// then peeled (a tag to what it tags).
pub(crate) fn resolve(repo: &Repository, spec: &str, peel: &str) -> Result<Oid, Unchecked> {
    let unresolved = || Unchecked::Unresolved(spec.to_owned());
    if spec.starts_with('-') {
        return Err(unresolved());
    }
    let oid = match run(repo, &cmd(&["rev-parse", "--verify", "--quiet", spec]), "") {
        (true, out) => Oid::from_str(out.trim()).map_err(|_| unresolved())?,
        (false, _) => return Err(unresolved()),
    };
    let object = repo.find_object(oid, None).map_err(|e| Unchecked::Unreadable(e.message().to_owned()))?;
    let peeled = if peel == "tree" { object.peel_to_tree().map(|t| t.id()) } else { object.peel_to_commit().map(|c| c.id()) };
    peeled.map_err(|_| unresolved())
}

/// The commits `git rev-list <args>` lists, in its order.
pub(crate) fn rev_list(repo: &Repository, args: &[String]) -> Result<Vec<Oid>, Unchecked> {
    let full = [cmd(&["rev-list"]), args.to_vec()].concat();
    match run(repo, &full, "") {
        (true, out) => out
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| Oid::from_str(l.trim()).map_err(|_| Unchecked::Unreadable(l.to_owned())))
            .collect(),
        (false, why) => Err(Unchecked::Unresolved(format!("{}: {}", args.join(" "), why.trim()))),
    }
}

/// `git merge-base --fork-point <upstream> <branch>`, when there is one.
pub(crate) fn fork_point(repo: &Repository, upstream: &str, branch: &str) -> Option<Oid> {
    match run(repo, &cmd(&["merge-base", "--fork-point", upstream, branch]), "") {
        (true, out) => Oid::from_str(out.trim()).ok(),
        _ => None,
    }
}

/// The value of the configuration `key` as git reads it (as type `kind`: bool, int), None
/// when it isn't set; Err with git's own refusal when the value isn't one of that type.
pub fn config(repo: &Repository, key: &str, kind: Option<&str>) -> Result<Option<String>, String> {
    let ty = kind.map(|k| format!("--type={k}"));
    let mut args = vec!["config"];
    args.extend(ty.as_deref());
    args.extend(["--get", key]);
    let out = gitcli::output(cwd(repo), &cmd(&args), "", gitcli::NO_PROMPTS, None);
    match out.code {
        Some(0) => Ok(Some(out.stdout.trim_end_matches(['\r', '\n']).to_owned())),
        Some(1) => Ok(None),
        _ => Err(out.combined()),
    }
}

/// Whether any configuration key matches `pattern` (`git config --get-regexp`).
fn configured(repo: &Repository, pattern: &str) -> Result<bool, Unchecked> {
    let out = gitcli::output(cwd(repo), &cmd(&["config", "--get-regexp", pattern]), "", gitcli::NO_PROMPTS, None);
    match out.code {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(Unchecked::Unreadable(out.combined())),
    }
}

/// What one three-way merge leaves: its tree (with conflict markers where it conflicts), and
/// the paths that conflicted (git stops there).
#[derive(Debug)]
pub(crate) struct Merged {
    pub tree: Oid,
    pub conflicts: Vec<String>,
}

/// `git merge-tree --write-tree` of `ours` and `theirs` (commits, or trees with `base`), from
/// `base` when given (else the merge bases git finds, criss-cross ones too), with the strategy
/// options `xopts`. A merge driver configured (a program the comparison would run) is
/// Unmodeled, as is renormalizing while filters are configured.
pub(crate) fn merge_tree(
    repo: &Repository,
    ours: Oid,
    theirs: Oid,
    base: Option<Oid>,
    xopts: &[String],
    unrelated: bool,
) -> Result<Merged, Unchecked> {
    needs(2, 38, "comparing a merge")?;
    if base.is_some() || !xopts.is_empty() {
        needs(2, 45, "comparing a pick, or a merge with -X")?;
    }
    if configured(repo, r"^merge\..*\.driver$")? {
        return Err(Unchecked::Unmodeled("a merge driver".to_owned()));
    }
    let renormalize = xopts.iter().any(|x| x == "renormalize")
        || config(repo, "merge.renormalize", Some("bool")).ok().flatten().as_deref() == Some("true");
    if renormalize && configured(repo, r"^filter\.")? {
        return Err(Unchecked::Unmodeled("renormalizing with filters".to_owned()));
    }
    let mut full = cmd(&["merge-tree", "--write-tree", "--no-messages", "--name-only", "-z"]);
    full.extend(xopts.iter().map(|x| format!("-X{x}")));
    if unrelated {
        full.push("--allow-unrelated-histories".to_owned());
    }
    full.extend(base.map(|b| format!("--merge-base={b}")));
    full.extend([ours.to_string(), theirs.to_string()]);
    let out = gitcli::output(cwd(repo), &full, "", gitcli::NO_PROMPTS, None);
    let mut parts = out.stdout.split('\0');
    let tree = parts.next().and_then(|l| Oid::from_str(l.trim()).ok());
    match (out.code, tree) {
        (Some(0), Some(tree)) => Ok(Merged { tree, conflicts: Vec::new() }),
        (Some(1), Some(tree)) => Ok(Merged { tree, conflicts: parts.take_while(|p| !p.is_empty()).map(str::to_owned).collect() }),
        _ => Err(Unchecked::Unreadable(format!("git merge-tree: {}", out.combined().trim()))),
    }
}

/// A temporary folder that is removed when dropped.
pub(crate) struct Scratch(pub PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn scratch() -> Result<Scratch, Unchecked> {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = crate::paths::temp_dir().join(format!("glowfly-mail-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| Unchecked::Unreadable(format!("{}: {e}", dir.display())))?;
    Ok(Scratch(dir))
}

/// The paths the patches in the mailbox `file` write, decoded as `git am` decodes them
/// (`git mailsplit`, then `git mailinfo` on each mail), read by `git apply --numstat`.
pub(crate) fn mailbox_paths(repo: &Repository, file: &Path, apply: &[String]) -> Result<Vec<String>, Unchecked> {
    let dir = scratch()?;
    let split = dir.0.join("split");
    std::fs::create_dir_all(&split).map_err(|e| Unchecked::Unreadable(e.to_string()))?;
    let full = [cmd(&["mailsplit", "-b"]), vec![format!("-o{}", split.display()), "--".to_owned(), file.display().to_string()]].concat();
    if let (false, why) = run(repo, &full, "") {
        return Err(Unchecked::Unreadable(format!("git mailsplit: {}", why.trim())));
    }
    let mut mails: Vec<PathBuf> =
        std::fs::read_dir(&split).map_err(|e| Unchecked::Unreadable(e.to_string()))?.flatten().map(|e| e.path()).collect();
    mails.sort();
    let mut out = Vec::new();
    for m in mails {
        let bytes = std::fs::read(&m).map_err(|e| Unchecked::Unreadable(e.to_string()))?;
        out.extend(mail_paths(repo, &bytes, &dir.0, apply)?);
    }
    Ok(out)
}

/// The paths one mail's patch writes (`git mailinfo`, then `git apply --numstat -z`).
pub(crate) fn mail_paths(repo: &Repository, mail: &[u8], scratch_dir: &Path, apply: &[String]) -> Result<Vec<String>, Unchecked> {
    // Given to git as text: a mail that isn't UTF-8 (8-bit in another charset) isn't read.
    let text = std::str::from_utf8(mail).map_err(|_| Unchecked::Unmodeled("a mail that isn't UTF-8".to_owned()))?;
    let (msg, patch) = (scratch_dir.join("msg"), scratch_dir.join("patch"));
    let full = [cmd(&["mailinfo", "-u"]), vec![msg.display().to_string(), patch.display().to_string()]].concat();
    if let (false, why) = run(repo, &full, text) {
        return Err(Unchecked::Unreadable(format!("git mailinfo: {}", why.trim())));
    }
    if std::fs::metadata(&patch).map(|m| m.len()).unwrap_or(0) == 0 {
        return Ok(Vec::new());
    }
    patch_paths(repo, &patch, apply)
}

/// The paths the patch file `patch` writes (`git apply --numstat -z`, with the `apply` options
/// that move or filter its paths).
pub(crate) fn patch_paths(repo: &Repository, patch: &Path, apply: &[String]) -> Result<Vec<String>, Unchecked> {
    let full = [cmd(&["apply", "--numstat", "-z"]), apply.to_vec(), vec!["--".to_owned(), patch.display().to_string()]].concat();
    match run(repo, &full, "") {
        (true, out) => Ok(numstat_paths(&out)),
        (false, why) => Err(Unchecked::Unreadable(format!("git apply --numstat: {}", why.trim()))),
    }
}

/// The paths in `git apply --numstat -z` output: `added\tdeleted\tpath\0`, or for a rename or
/// copy `added\tdeleted\t\0from\0to\0`.
pub(crate) fn numstat_paths(out: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut it = out.split('\0');
    while let Some(entry) = it.next() {
        let Some(path) = entry.splitn(3, '\t').nth(2) else { continue };
        if path.is_empty() {
            paths.extend(it.next().map(str::to_owned));
            paths.extend(it.next().map(str::to_owned));
        } else {
            paths.push(path.to_owned());
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numstat_lists_both_names_of_a_rename() {
        let out = "1\t0\ta.txt\0-\t-\t\0old/x\0.githooks/x\0";
        assert_eq!(numstat_paths(out), ["a.txt", "old/x", ".githooks/x"]);
    }
}
