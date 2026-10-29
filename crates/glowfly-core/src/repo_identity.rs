//! Where a repository's author identity comes from, as git sees it: the effective user.name and
//! user.email with the file each was read from (`git config --show-origin`), told apart as a
//! profile's include file (a folder rule), the repository's own config, or anywhere else (the
//! global or system config). Also what the folder rules say should apply, the
//! `GIT_AUTHOR_*` / `GIT_COMMITTER_*` variables in Glowfly's environment that git takes before
//! any config (`env_overrides`), and removing the repository's own name and email so a
//! profile's rule can apply (asked first by the app).

use crate::gitcli::{self, args};
use crate::paths;
use crate::profile_includes;
use crate::settings::{FolderRule, ProfileId, profile_for_git_dir};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// The file the effective identity was read from.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum IdentitySource {
    /// A profile's include file: one of its folder rules applies.
    Profile(ProfileId),
    /// The repository's own config (`config` or `config.worktree`).
    Repository,
    /// The global or system config, or anything else.
    Elsewhere,
    /// No user.name or user.email anywhere.
    #[default]
    Unset,
}

/// A repository's effective identity.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RepoIdentity {
    pub name: String,
    pub email: String,
    pub source: IdentitySource,
    /// The values the repository's own config sets, if any.
    pub local_name: Option<String>,
    pub local_email: Option<String>,
    /// What the rules say should apply (`profile_for_git_dir` on the resolved git dir).
    pub expected: Option<ProfileId>,
    /// The identity variables set in Glowfly's environment, which git children inherit and use
    /// instead of the config (`env_overrides`); `read` leaves them empty.
    pub overrides: Vec<EnvOverride>,
}

/// The variables git reads for a commit's author and committer before any config, in this
/// order.
pub const ENV_VARS: [&str; 4] = ["GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL"];

/// One of `ENV_VARS` that is set (an empty value counts: git uses it too).
#[derive(Clone, Debug, PartialEq)]
pub struct EnvOverride {
    pub var: &'static str,
    pub value: String,
}

impl RepoIdentity {
    /// The value `var` (one of `ENV_VARS`) is set to, if it is.
    pub fn override_of(&self, var: &str) -> Option<&str> {
        self.overrides.iter().find(|o| o.var == var).map(|o| o.value.as_str())
    }
}

/// The `ENV_VARS` that `env` sets, in `ENV_VARS` order. Pure.
pub fn env_overrides_with(env: impl Fn(&str) -> Option<OsString>) -> Vec<EnvOverride> {
    ENV_VARS.iter().filter_map(|&var| env(var).map(|v| EnvOverride { var, value: v.to_string_lossy().into_owned() })).collect()
}

/// [`env_overrides_with`] for this process, whose environment every git child inherits.
pub fn env_overrides() -> Vec<EnvOverride> {
    env_overrides_with(crate::flatpak::host_var)
}

/// Reads the identity git gives commits in `workdir` (whose git dir is `git_dir`), with
/// `include_dir` holding the profiles' include files. Exit code 1 (nothing set) reads as Unset.
pub fn read(
    workdir: &Path,
    git_dir: &Path,
    include_dir: Option<&Path>,
    rules: &[(ProfileId, FolderRule)],
    env: &[(&str, &str)],
) -> Result<RepoIdentity, String> {
    let a = args(&["config", "-z", "--show-origin", "--get-regexp", r"^user\.(name|email)$"]);
    let (out, bytes) = gitcli::output_bytes(workdir, &a, env, Some(gitcli::READ_TIMEOUT));
    let mut id = match (out.ok, out.code) {
        (true, _) => parse(&bytes, workdir, git_dir, include_dir),
        (false, Some(1)) => RepoIdentity::default(),
        _ => return Err(format!("Could not read the repository's identity: {}", out.combined())),
    };
    id.expected = profile_for_git_dir(rules, &paths::resolve(git_dir));
    Ok(id)
}

/// The identity in `git config -z --show-origin --get-regexp` output (`<origin>\0<key>\n<value>\0`
/// per value), read in `cwd`: the last value of each key wins, and the source follows the
/// effective email (else the name). `expected` is left for `read`. Pure.
pub(crate) fn parse(bytes: &[u8], cwd: &Path, git_dir: &Path, include_dir: Option<&Path>) -> RepoIdentity {
    let mut id = RepoIdentity::default();
    let (mut name_from, mut email_from) = (None, None);
    let items: Vec<&[u8]> = bytes.split(|b| *b == 0).collect();
    for pair in items.chunks(2) {
        let [origin, entry] = pair else { continue };
        let entry = String::from_utf8_lossy(entry);
        let Some((key, value)) = entry.split_once('\n') else { continue };
        let source = source_of(&String::from_utf8_lossy(origin), cwd, git_dir, include_dir);
        let local = (source == IdentitySource::Repository).then(|| value.to_owned());
        match key {
            "user.name" => {
                id.name = value.to_owned();
                name_from = Some(source);
                id.local_name = local.or(id.local_name.take());
            }
            "user.email" => {
                id.email = value.to_owned();
                email_from = Some(source);
                id.local_email = local.or(id.local_email.take());
            }
            _ => {}
        }
    }
    id.source = email_from.or(name_from).unwrap_or_default();
    id
}

/// What kind of file a `--show-origin` origin names.
fn source_of(origin: &str, cwd: &Path, git_dir: &Path, include_dir: Option<&Path>) -> IdentitySource {
    let Some(file) = origin.strip_prefix("file:") else { return IdentitySource::Elsewhere };
    let file = lexical(&cwd.join(file));
    if let Some(id) = include_dir.and_then(|dir| profile_includes::profile_of_file(dir, &file)) {
        return IdentitySource::Profile(id);
    }
    let own = file.file_name().is_some_and(|n| n == "config" || n == "config.worktree");
    let git_dir = lexical(git_dir);
    let in_repo = file.parent().is_some_and(|p| paths::same_spelling(p, &git_dir) || paths::same_spelling(p, &common_of(&git_dir)));
    if own && in_repo { IdentitySource::Repository } else { IdentitySource::Elsewhere }
}

/// The common git dir of `git_dir` by git's layout: a linked worktree's git dir is
/// `<common>/worktrees/<name>`.
fn common_of(git_dir: &Path) -> PathBuf {
    match git_dir.parent() {
        Some(p) if p.file_name().is_some_and(|n| n == "worktrees") => {
            p.parent().map(Path::to_path_buf).unwrap_or_else(|| git_dir.to_path_buf())
        }
        _ => git_dir.to_path_buf(),
    }
}

/// `path` with `.` dropped and `..` taken back, by spelling (git may print a common dir it found
/// through a relative `commondir` file).
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// Removes user.name and user.email from the repository's own config, every value of each (one
/// that isn't set is fine: git exits 5); its user.signingkey stays.
pub fn clear_local(workdir: &Path, env: &[(&str, &str)]) -> Result<(), String> {
    for key in ["user.name", "user.email"] {
        let out = gitcli::output(workdir, &args(&["config", "--local", "--unset-all", key]), "", env, Some(gitcli::READ_TIMEOUT));
        if !out.ok && out.code != Some(5) {
            return Err(format!("Could not remove {key} from the repository's Git config: {}", out.combined()));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "repo_identity_tests.rs"]
mod tests;
