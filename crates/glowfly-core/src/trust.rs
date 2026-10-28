//! What in a repository's own config and hooks would make git run programs (repository trust).
//!
//! A repository from elsewhere can name programs in its `.git/config` (an ssh command, a
//! credential helper, filters, an fsmonitor hook) or carry hooks, and git runs them as soon as it
//! touches the repository. `scan` lists them with libgit2 only, so reading them runs nothing.

use git2::{Config, ConfigLevel, Repository};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod digest;
mod print;
pub use print::{TrustPrint, allows};

/// A setting from the repository's own config that makes git run a program.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConfigCommand {
    /// The key in its usual spelling, e.g. `core.sshCommand`.
    pub key: String,
    pub value: String,
}

/// What `scan` found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrustScan {
    /// Settings from the repository's own config files (local and worktree level, with what they
    /// include), in file order.
    pub config: Vec<ConfigCommand>,
    /// Hook names that git would run, sorted.
    pub hooks: Vec<String>,
    /// The digest of each of `hooks`' script contents, by name (`digest::of_file`): an edited
    /// script asks again. A hook missing here counts as unreadable.
    pub hook_digests: BTreeMap<String, String>,
    /// The folder `hooks` were found in; None when hooks come from the user's own (global or
    /// system) core.hooksPath, or from a path that can't be resolved without the shell.
    pub hooks_dir: Option<PathBuf>,
    /// Checked-out submodules, nested ones included, whose own config or hooks run programs:
    /// git's status and fetch run git inside each of them. Their own `submodules` are empty.
    pub submodules: Vec<SubmoduleScan>,
    /// The digest of the repository's own commands file (`.glowfly/commands.toml`), when it has
    /// one: in the print, so a changed file asks again, but not a reason to ask by itself (its
    /// commands run only when the user picks one). None in a submodule.
    pub commands: Option<String>,
}

/// What `scan` found in one submodule.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SubmoduleScan {
    /// Where it is checked out, relative to the top repository's working directory.
    pub path: String,
    pub scan: TrustScan,
}

impl TrustScan {
    /// Nothing in the repository makes git run a program.
    pub fn is_empty(&self) -> bool {
        self.config.is_empty() && self.hooks.is_empty() && self.submodules.is_empty()
    }
}

/// The hook names git runs (`git help hooks`). Anything else in the folder, such as the
/// `*.sample` files `git init` writes, is never run.
pub const HOOK_NAMES: &[&str] = &[
    "applypatch-msg",
    "pre-applypatch",
    "post-applypatch",
    "pre-commit",
    "pre-merge-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "pre-rebase",
    "post-checkout",
    "post-merge",
    "pre-push",
    "pre-receive",
    "update",
    "proc-receive",
    "post-receive",
    "post-update",
    "reference-transaction",
    "push-to-checkout",
    "pre-auto-gc",
    "post-rewrite",
    "sendemail-validate",
    "fsmonitor-watchman",
    "p4-changelist",
    "p4-prepare-changelist",
    "p4-post-changelist",
    "p4-pre-submit",
    "post-index-change",
];

/// Single keys that name a program, as libgit2 returns them (lowercase), with their usual spelling.
const COMMAND_KEYS: &[(&str, &str)] = &[
    ("core.sshcommand", "core.sshCommand"),
    ("core.hookspath", "core.hooksPath"),
    ("core.editor", "core.editor"),
    ("core.askpass", "core.askPass"),
    ("core.gitproxy", "core.gitProxy"),
    ("sequence.editor", "sequence.editor"),
    ("diff.external", "diff.external"),
    ("credential.helper", "credential.helper"),
    ("gpg.program", "gpg.program"),
    ("gpg.openpgp.program", "gpg.openpgp.program"),
    ("gpg.x509.program", "gpg.x509.program"),
    ("gpg.ssh.program", "gpg.ssh.program"),
    ("gpg.ssh.defaultkeycommand", "gpg.ssh.defaultKeyCommand"),
];

/// The git commands Glowfly runs that aren't built into git, so a repository's alias of the
/// same name runs in their place when the program is missing (git looks for `git-<name>`
/// before aliases, but after its built-in commands, which an alias never replaces).
const ALIASED_COMMANDS: &[&str] = &["lfs", "mergetool"];

/// How deep `scan` follows submodules inside submodules.
const MAX_SUBMODULE_DEPTH: usize = 8;

/// Lists what in `repo`'s own config and hooks, and in those of its checked-out submodules, would
/// make git run programs. Settings from the user's own config (global, XDG, system) are left
/// out: the user put them there.
pub fn scan(repo: &Repository) -> TrustScan {
    let mut out = scan_top(repo);
    scan_submodules(repo, "", 0, &mut out.submodules);
    out
}

/// What a refresh re-checks: `repo`'s own settings, hooks (with their contents) and commands
/// file, without its submodules (a full load scans those). Cheap: the config files, a listing
/// of the hooks folder and a read of each hook git would run.
pub fn scan_top(repo: &Repository) -> TrustScan {
    let mut out = scan_own(repo);
    out.commands = repo.workdir().and_then(commands_digest);
    out
}

/// The digest of the commands file of the working folder `workdir`, if there is one.
fn commands_digest(workdir: &Path) -> Option<String> {
    let path = crate::custom_commands::repo_file(workdir);
    // `symlink_metadata`: a link counts as there (the commands reader refuses it).
    std::fs::symlink_metadata(&path).is_ok().then(|| digest::of_file(&path))
}

/// `scan` of `repo` alone, without its submodules.
fn scan_own(repo: &Repository) -> TrustScan {
    let mut out = TrustScan::default();
    let config = repo.config().ok();
    let mut local_hooks_path = None;
    if let Some(entries) = config.as_ref().and_then(|c| c.entries(None).ok()) {
        let _ = entries.for_each(|e| {
            if !matches!(e.level(), ConfigLevel::Local | ConfigLevel::Worktree) {
                return;
            }
            let (Some(name), Some(value)) = (e.name(), e.value()) else { return };
            if let Some(key) = command_key(name, value) {
                if name == "core.hookspath" {
                    local_hooks_path = Some(value.to_owned());
                }
                out.config.push(ConfigCommand { key, value: value.to_owned() });
            }
        });
    }
    out.hooks_dir = hooks_dir(repo, config.as_ref(), local_hooks_path);
    if let Some(dir) = &out.hooks_dir {
        for (name, path) in active_hooks(dir) {
            out.hook_digests.insert(name.clone(), digest::of_file(&path));
            out.hooks.push(name);
        }
    }
    out
}

/// Adds the submodules of `repo` (at `prefix` in the top repository) that run programs. Like
/// git's status, this takes every gitlink in the index whose folder holds a repository, listed
/// in .gitmodules or not.
fn scan_submodules(repo: &Repository, prefix: &str, depth: usize, out: &mut Vec<SubmoduleScan>) {
    let (Some(workdir), Ok(index)) = (repo.workdir(), repo.index()) else { return };
    if depth >= MAX_SUBMODULE_DEPTH {
        return;
    }
    let mut last: Option<Vec<u8>> = None;
    for e in index.iter() {
        // A conflicted gitlink has an entry per side.
        if e.mode != 0o160000 || last.as_ref() == Some(&e.path) {
            continue;
        }
        last = Some(e.path.clone());
        let Ok(rel) = std::str::from_utf8(&e.path) else { continue };
        // Doesn't search upward, so a folder that isn't checked out finds nothing.
        let Ok(sub) = Repository::open(workdir.join(rel)) else { continue };
        let path = if prefix.is_empty() { rel.to_owned() } else { format!("{prefix}/{rel}") };
        let own = scan_own(&sub);
        if !own.is_empty() {
            out.push(SubmoduleScan { path: path.clone(), scan: own });
        }
        scan_submodules(&sub, &path, depth + 1, out);
    }
}

/// `scan` of the repository at `path`.
pub fn scan_at(path: &Path) -> crate::error::Result<TrustScan> {
    Ok(scan(&Repository::open(path)?))
}

/// Whether `git init` output says it made a new repository (not "Reinitialized existing").
pub fn init_created_new(output: &str) -> bool {
    output.lines().any(|l| l.contains("Initialized empty Git repository"))
}

/// The display key when the setting `name` = `value` makes git run a program.
fn command_key(name: &str, value: &str) -> Option<String> {
    // An empty value resets a list (credential.helper) or turns the setting off.
    if value.is_empty() {
        return None;
    }
    if let Some((_, shown)) = COMMAND_KEYS.iter().find(|(k, _)| *k == name) {
        return Some((*shown).to_owned());
    }
    // git runs an alias for a command it can't find (alias.lfs when git-lfs isn't installed),
    // in any letter case of the name.
    if let Some(cmd) = name.strip_prefix("alias.").map(|rest| rest.strip_suffix(".command").unwrap_or(rest)) {
        let shadows = ALIASED_COMMANDS.iter().any(|c| c.eq_ignore_ascii_case(cmd));
        return shadows.then(|| format!("alias.{}", cmd.to_ascii_lowercase()));
    }
    if name == "protocol.allow" {
        return value.eq_ignore_ascii_case("always").then(|| name.to_owned());
    }
    if name == "core.fsmonitor" {
        // A boolean means git's built-in daemon; anything else is a hook's path.
        return Config::parse_bool(value).is_err().then(|| name.to_owned());
    }
    let (first, last) = (name.find('.')?, name.rfind('.')?);
    if first == last {
        return None;
    }
    let (section, sub, key) = (&name[..first], &name[first + 1..last], &name[last + 1..]);
    let runs = match section {
        "credential" => key == "helper",
        "diff" => matches!(key, "command" | "textconv"),
        "merge" => key == "driver",
        "filter" => matches!(key, "clean" | "smudge" | "process"),
        // git difftool / mergetool start a tool's own command, or a built-in tool from `.path`.
        "difftool" | "mergetool" => matches!(key, "cmd" | "path"),
        "remote" => matches!(key, "uploadpack" | "receivepack") || (matches!(key, "url" | "pushurl") && runs_transport(value)),
        // A branch's remote can be a URL.
        "branch" => matches!(key, "remote" | "pushremote") && runs_transport(value),
        // Rewrites matching URLs to the subsection.
        "url" => matches!(key, "insteadof" | "pushinsteadof") && runs_transport(sub),
        // Lets ext:: run, including for submodule URLs from .gitmodules.
        "protocol" => sub == "ext" && key == "allow" && value != "never",
        // libgit2 reads a conditional include only for the conditions it knows; git may read
        // others (hasconfig:), so what such a file sets is unseen.
        "includeif" => key == "path" && !["gitdir:", "gitdir/i:", "onbranch:"].iter().any(|c| sub.starts_with(c)),
        _ => false,
    };
    if !runs {
        return None;
    }
    Some(match section {
        "includeif" => format!("includeIf.{sub}.path"),
        "url" => format!("url.{sub}.{}", if key == "insteadof" { "insteadOf" } else { "pushInsteadOf" }),
        _ => name.to_owned(),
    })
}

/// Whether git reaching `url` runs a command: the ext:: transport (any case, since a Windows
/// file system finds its helper under any spelling).
fn runs_transport(url: &str) -> bool {
    url.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("ext::"))
}

/// The folder git takes hooks from. `local` is the repository's own core.hooksPath, if set.
fn hooks_dir(repo: &Repository, config: Option<&Config>, local: Option<String>) -> Option<PathBuf> {
    if let Some(v) = local {
        // A home-relative path needs the shell's idea of home: report the key alone.
        if v.starts_with('~') {
            return None;
        }
        let p = PathBuf::from(v);
        if p.is_absolute() {
            return Some(p);
        }
        return Some(repo.workdir().unwrap_or_else(|| repo.path()).join(p));
    }
    // Set in the user's own config: their hooks, not the repository's.
    if config.is_some_and(|c| c.get_path("core.hooksPath").is_ok()) {
        return None;
    }
    // A linked worktree runs its main repository's hooks.
    Some(repo.commondir().join("hooks"))
}

/// The hooks in `dir` that git would run, sorted. A missing folder has none.
///
/// A security check, so it overmatches: a listed file counts when its name is a hook name under
/// any case rule a file system might use (see [`is_hook_name`]), and each hook name is also
/// looked up directly, so a file system that matches names in some other way (a Linux FAT or
/// casefold mount, a Windows short name) can't hide one.
fn active_hooks(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let runs = |p: &Path| std::fs::metadata(p).is_ok_and(|m| m.is_file() && runnable(&m));
    let mut hooks: Vec<(String, PathBuf)> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            // Follows links: a link to a script runs the script.
            (is_hook_name(&name) && runs(&e.path())).then(|| (name, e.path()))
        })
        .collect();
    let exe = if cfg!(windows) { ".exe" } else { "" };
    for name in HOOK_NAMES {
        let found = [name.to_string(), format!("{name}{exe}")].into_iter().map(|n| dir.join(n)).find(|p| runs(p));
        if let Some(path) = found
            && !hooks.iter().any(|(h, _)| hook_base(h) == hook_base(name))
        {
            hooks.push(((*name).to_owned(), path));
        }
    }
    hooks.sort();
    hooks
}

/// Whether git may run a file called `file` in a hooks folder: a hook name in any letter case,
/// on every OS (Windows and macOS ignore case by default, and so do FAT, exFAT, CIFS and
/// casefold folders on Linux), with the code points HFS+ ignores left out. Git for Windows also
/// runs `<hook>.exe`.
pub(crate) fn is_hook_name(file: &str) -> bool {
    HOOK_NAMES.iter().any(|h| hook_base(file) == hook_base(h))
}

/// `name` as a case-insensitive file system may compare it: HFS+ ignorable code points
/// dropped, uppercased character by character as NTFS does (so "ı" and "ſ" become "I" and "S"),
/// then lowercased (so the Kelvin sign is "k"), and `.exe` dropped on Windows.
pub(crate) fn hook_base(name: &str) -> String {
    let kept: String = name.chars().filter(|c| !crate::names::is_hfs_ignorable(*c)).collect();
    let upper: String = kept.chars().flat_map(char::to_uppercase).collect();
    let key = upper.to_lowercase();
    match key.strip_suffix(".exe") {
        Some(base) if cfg!(windows) => base.to_owned(),
        _ => key,
    }
}

/// Git skips hooks without an execute bit on Unix; on Windows it runs any file.
#[cfg(unix)]
fn runnable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn runnable(_meta: &std::fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod reask_tests;
#[cfg(test)]
mod tests;
