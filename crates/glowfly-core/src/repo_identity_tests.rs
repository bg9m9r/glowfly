//! Reading where an identity comes from (pure parsing of git's output, then real reads on a
//! scratch repository) and clearing a repository's own identity.

use super::*;
use crate::profile_includes::{self, IncludePlan};
use crate::settings::{Profile, Settings};
use crate::testutil::{empty_global_config, scratch_repo};

const WORK: ProfileId = ProfileId(3);

fn base() -> PathBuf {
    paths::temp_dir().join("trellis-identity")
}

/// `--show-origin -z` output for (origin, key, value) entries.
fn output(entries: &[(&str, &str, &str)]) -> Vec<u8> {
    entries.iter().map(|(o, k, v)| format!("file:{o}\0{k}\n{v}\0")).collect::<String>().into_bytes()
}

fn parsed(entries: &[(&str, &str, &str)]) -> RepoIdentity {
    let repo = base().join("repo");
    parse(&output(entries), &repo, &repo.join(".git"), Some(&base().join("git-profiles")))
}

#[test]
fn origins_tell_the_source() {
    let include = base().join("git-profiles").join("profile-3.gitconfig");
    let include = include.to_str().unwrap();
    let global = base().join(".gitconfig");
    let global = global.to_str().unwrap();
    let from_rule = parsed(&[(global, "user.name", "Ada"), (include, "user.name", "Ada L"), (include, "user.email", "ada@work.example")]);
    assert_eq!(from_rule.source, IdentitySource::Profile(WORK));
    assert_eq!((from_rule.name.as_str(), from_rule.email.as_str()), ("Ada L", "ada@work.example"), "the last value wins");
    assert_eq!((from_rule.local_name, from_rule.local_email), (None, None));
    let local = parsed(&[
        (global, "user.email", "ada@home.example"),
        (".git/config", "user.name", "Me"),
        (".git/config", "user.email", "me@x.example"),
    ]);
    assert_eq!(local.source, IdentitySource::Repository);
    assert_eq!((local.local_name.as_deref(), local.local_email.as_deref()), (Some("Me"), Some("me@x.example")));
    assert_eq!(parsed(&[(global, "user.email", "ada@home.example")]).source, IdentitySource::Elsewhere);
    assert_eq!(parsed(&[]), RepoIdentity::default());
    assert_eq!(RepoIdentity::default().source, IdentitySource::Unset);
}

#[test]
fn a_partial_local_identity_counts() {
    let global = base().join(".gitconfig");
    let global = global.to_str().unwrap();
    let id = parsed(&[(global, "user.email", "ada@home.example"), (".git/config", "user.name", "Me")]);
    assert_eq!(id.source, IdentitySource::Elsewhere, "the email decides");
    assert_eq!((id.local_name.as_deref(), id.local_email), (Some("Me"), None));
    let name_only = parsed(&[(".git/config", "user.name", "Me")]);
    assert_eq!(name_only.source, IdentitySource::Repository, "the name decides without an email");
}

#[test]
fn a_worktree_reads_the_common_config_as_its_own() {
    let main = base().join("main").join(".git");
    let git_dir = main.join("worktrees").join("wt");
    let common = main.join("worktrees").join("wt").join("..").join("..").join("config");
    let bytes = output(&[(common.to_str().unwrap(), "user.email", "me@x.example")]);
    let id = parse(&bytes, &base().join("wt"), &git_dir, None);
    assert_eq!(id.source, IdentitySource::Repository);
}

fn borrowed(env: &[(String, String)]) -> Vec<(&str, &str)> {
    env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
}

#[test]
fn reading_and_clearing_a_real_repository() {
    let repo = scratch_repo("repo-identity");
    let env = empty_global_config(&repo);
    let env = borrowed(&env);
    let git_dir = repo.join(".git");
    let includes = repo.with_file_name(format!("{}-profiles", repo.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&includes);
    let mut s = Settings::default();
    s.profiles.push(Profile {
        id: WORK,
        name: "Work".into(),
        author_name: "Ada".into(),
        author_email: "ada@work.example".into(),
        ..Profile::default()
    });
    s.add_folder_rule(WORK, profile_includes::check_folder(&repo, false).unwrap()).unwrap();
    let rules = s.rules();
    assert!(gitcli::output(&repo, &args(&["config", "--local", "user.signingkey", "LOCALKEY"]), "", &env, None).ok);

    let own = read(&repo, &git_dir, Some(&includes), &rules, &env).unwrap();
    assert_eq!(own.source, IdentitySource::Repository);
    assert_eq!((own.local_email.as_deref(), own.expected), (Some("test@example.com"), Some(WORK)));

    clear_local(&repo, &env).unwrap();
    let plan: IncludePlan = profile_includes::plan(&s);
    profile_includes::sync(&repo, &includes, &plan, true, &env).unwrap();
    let ruled = read(&repo, &git_dir, Some(&includes), &rules, &env).unwrap();
    assert_eq!(ruled.source, IdentitySource::Profile(WORK));
    assert_eq!((ruled.name.as_str(), ruled.email.as_str(), ruled.expected), ("Ada", "ada@work.example", Some(WORK)));
    let key = gitcli::output(&repo, &args(&["config", "--local", "--get", "user.signingkey"]), "", &env, None);
    assert_eq!(key.stdout.trim(), "LOCALKEY", "the signing key stays");
    clear_local(&repo, &env).unwrap();

    profile_includes::sync(&repo, &includes, &IncludePlan { files: Vec::new(), entries: Vec::new() }, true, &env).unwrap();
    assert_eq!(read(&repo, &git_dir, Some(&includes), &[], &env).unwrap(), RepoIdentity::default(), "nothing set");
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&includes);
}
