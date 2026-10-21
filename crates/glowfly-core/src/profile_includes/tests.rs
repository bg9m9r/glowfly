//! Folder-rule patterns, include files, and syncs against a scratch global config.

use super::*;
use crate::settings::Profile;
use crate::testutil::{empty_global_config, scratch_repo};

const WORK: ProfileId = ProfileId(1);

fn rule_at(path: &Path, ignore_case: bool) -> FolderRule {
    FolderRule { path: paths::encode(path), repository: false, ignore_case }
}

fn id(name: &str, email: &str, key: Option<&str>) -> Identity {
    Identity { name: name.into(), email: email.into(), signing_key: key.map(str::to_owned) }
}

fn borrowed(env: &[(String, String)]) -> Vec<(&str, &str)> {
    env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
}

/// `git config <a…>`'s value in `dir`, None when unset.
fn get(dir: &Path, env: &[(&str, &str)], a: &[&str]) -> Option<String> {
    let mut all = vec!["config"];
    all.extend_from_slice(a);
    let out = gitcli::output(dir, &args(&all), "", env, None);
    out.ok.then(|| out.stdout.trim_end_matches(['\r', '\n']).to_owned())
}

#[test]
fn patterns_end_in_a_slash_and_choose_the_case_form() {
    let base = paths::temp_dir().join("glowfly-pattern");
    let plain = gitdir_pattern(&rule_at(&base, false)).unwrap();
    assert!(plain.starts_with("gitdir:") && plain.ends_with("glowfly-pattern/"), "{plain}");
    let folded = gitdir_pattern(&rule_at(&base.join("x/"), true)).unwrap();
    assert!(folded.starts_with("gitdir/i:") && folded.ends_with("glowfly-pattern/x/") && !folded.ends_with("//"), "{folded}");
    let globs = gitdir_pattern(&rule_at(&base.join("a[1]*b?"), false)).unwrap();
    assert!(globs.ends_with(r"a\[1\]\*b\?/"), "{globs}");
    let control = base.join("line\nbreak");
    assert_eq!(gitdir_pattern(&rule_at(&control, false)), Err(NOT_TEXT.to_owned()));
}

#[cfg(unix)]
#[test]
fn unix_backslashes_are_escaped_and_other_bytes_refused() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let p = gitdir_pattern(&rule_at(Path::new(r"/srv/a\b"), false)).unwrap();
    assert_eq!(p, r"gitdir:/srv/a\\b/");
    let bytes = Path::new(OsStr::from_bytes(b"/srv/caf\xe9"));
    assert_eq!(gitdir_pattern(&rule_at(bytes, false)), Err(NOT_TEXT.to_owned()));
}

#[cfg(windows)]
#[test]
fn windows_paths_use_forward_slashes() {
    let p = gitdir_pattern(&rule_at(Path::new(r"C:\Users\ada\work"), true)).unwrap();
    assert_eq!(p, "gitdir/i:C:/Users/ada/work/");
    let unc = gitdir_pattern(&rule_at(Path::new(r"\\server\share\team"), false)).unwrap();
    assert_eq!(unc, "gitdir://server/share/team/");
    assert_eq!(config_path_value(Path::new(r"C:\a\b.gitconfig")).unwrap(), "C:/a/b.gitconfig");
}

#[test]
fn include_files_read_back_exactly() {
    let dir = paths::temp_dir().join(format!("glowfly-include-text-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("profile.gitconfig");
    let name = r#"  Ada "the" \ Count # ; "#;
    std::fs::write(&file, file_text(&id(name, "ada@work.example", None))).unwrap();
    let f = file.to_str().unwrap();
    assert_eq!(get(&dir, &[], &["--file", f, "--get", "user.name"]).as_deref(), Some(name));
    assert_eq!(get(&dir, &[], &["--file", f, "--get", "user.email"]).as_deref(), Some("ada@work.example"));
    assert_eq!(get(&dir, &[], &["--file", f, "--get", "user.signingkey"]), None, "no key, no line");
    let blank = file_text(&id(" ", "", Some("ABCD")));
    assert!(!blank.contains("name") && !blank.contains("email") && blank.contains("signingkey = \"ABCD\""), "{blank}");
    assert_eq!(file_text(&Identity::default()), format!("{FILE_HEADER}\n"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A scratch repository without its own identity, its global config, and the include folder.
fn setup(name: &str) -> (PathBuf, Vec<(String, String)>, PathBuf) {
    let repo = scratch_repo(name);
    let env = empty_global_config(&repo);
    let git_env = borrowed(&env);
    for key in ["user.name", "user.email"] {
        assert!(gitcli::output(&repo, &args(&["config", "--local", "--unset", key]), "", &git_env, None).ok);
    }
    let includes = repo.with_file_name(format!("{}-profiles", repo.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&includes);
    (repo, env, includes)
}

fn work_plan(folder: &Path) -> IncludePlan {
    let mut s = Settings::default();
    s.profiles.push(Profile {
        id: WORK,
        name: "Work".into(),
        author_name: "Ada".into(),
        author_email: "ada@work.example".into(),
        ..Profile::default()
    });
    s.add_folder_rule(WORK, check_folder(folder, false).unwrap()).unwrap();
    plan(&s)
}

fn cleanup(repo: &Path, includes: &Path) {
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(includes);
}

/// Glowfly's sections in the global file (`--get-regexp` lines).
fn include_count(repo: &Path, env: &[(&str, &str)]) -> usize {
    get(repo, env, &["--global", "--get-regexp", r"^includeif\."]).map_or(0, |t| t.lines().count())
}

#[test]
fn a_synced_rule_gives_the_folder_the_profile() {
    let (repo, env, includes) = setup("includes-sync");
    let env = borrowed(&env);
    sync(&repo, &includes, &work_plan(&repo), true, &env).unwrap();
    assert_eq!(get(&repo, &env, &["user.email"]).as_deref(), Some("ada@work.example"));
    assert_eq!(get(&repo, &env, &["user.name"]).as_deref(), Some("Ada"));
    assert_eq!(get(&repo, &env, &["--global", "user.email"]), None, "the global identity is untouched");
    cleanup(&repo, &includes);
}

#[test]
fn placing_puts_the_entries_after_a_later_user_section() {
    let (repo, env, includes) = setup("includes-order");
    let env = borrowed(&env);
    let plan = work_plan(&repo);
    sync(&repo, &includes, &plan, true, &env).unwrap();
    // A user section appended after the entries (none existed), as `git config --global` does.
    assert!(gitcli::output(&repo, &args(&["config", "--global", "user.email", "ada@home.example"]), "", &env, None).ok);
    assert_eq!(get(&repo, &env, &["user.email"]).as_deref(), Some("ada@home.example"));
    sync(&repo, &includes, &plan, true, &env).unwrap();
    assert_eq!(get(&repo, &env, &["user.email"]).as_deref(), Some("ada@work.example"));
    assert_eq!(get(&repo, &env, &["--global", "user.email"]).as_deref(), Some("ada@home.example"));
    cleanup(&repo, &includes);
}

#[test]
fn syncs_are_idempotent_and_leave_other_sections_alone() {
    let (repo, env, includes) = setup("includes-again");
    let env = borrowed(&env);
    // The user's own include, a section shared with a key of theirs, and a dotted folder name.
    let dotted = repo.join("v1.2");
    std::fs::create_dir_all(&dotted).unwrap();
    let theirs = ["config", "--global", "--add", "includeIf.gitdir:~/oss/.path", "~/.gitconfig-oss"];
    assert!(gitcli::output(&repo, &args(&theirs), "", &env, None).ok);
    let mut s = Settings::default();
    s.profiles.push(Profile { id: WORK, name: "Work".into(), author_email: "ada@work.example".into(), ..Profile::default() });
    s.add_folder_rule(WORK, check_folder(&repo, false).unwrap()).unwrap();
    s.add_folder_rule(WORK, check_folder(&dotted, false).unwrap()).unwrap();
    let plan = plan(&s);
    assert_eq!(plan.entries.len(), 2);
    assert!(plan.entries[1].0.ends_with("v1.2/"), "shallow first: {:?}", plan.entries);
    sync(&repo, &includes, &plan, true, &env).unwrap();
    sync(&repo, &includes, &plan, true, &env).unwrap();
    assert_eq!(include_count(&repo, &env), 3, "one per entry, plus the user's");
    // A user key in one of Glowfly's sections keeps that section where it is.
    let shared = format!("includeIf.{}.condition", plan.entries[0].0);
    assert!(gitcli::output(&repo, &args(&["config", "--global", &shared, "mine"]), "", &env, None).ok);
    let empty = IncludePlan { files: Vec::new(), entries: Vec::new() };
    sync(&repo, &includes, &empty, true, &env).unwrap();
    let left = get(&repo, &env, &["--global", "--get-regexp", r"^includeif\."]).unwrap_or_default();
    assert!(left.contains("~/.gitconfig-oss") && left.contains("mine") && !left.contains("v1.2"), "{left}");
    assert!(!includes.join(file_name(WORK)).exists(), "the stale include file is gone");
    sync(&repo, &includes, &plan, true, &env).unwrap();
    sync(&repo, &includes, &plan, true, &env).unwrap();
    assert_eq!(include_count(&repo, &env), 4, "the shared section isn't added again: {left}");
    cleanup(&repo, &includes);
}

#[test]
fn without_placing_only_the_files_change() {
    let (repo, env, includes) = setup("includes-files");
    let global = PathBuf::from(&env[0].1);
    let env = borrowed(&env);
    let mut plan = work_plan(&repo);
    sync(&repo, &includes, &plan, true, &env).unwrap();
    let before = std::fs::read(&global).unwrap();
    plan.files[0].1.email = "ada@new.example".into();
    sync(&repo, &includes, &plan, false, &env).unwrap();
    assert_eq!(std::fs::read(&global).unwrap(), before);
    assert_eq!(get(&repo, &env, &["user.email"]).as_deref(), Some("ada@new.example"));
    cleanup(&repo, &includes);
}

#[test]
fn checking_a_folder() {
    assert!(check_folder(Path::new("relative/folder"), false).is_err());
    let base = paths::temp_dir();
    assert!(check_folder(&base.join(format!("glowfly-missing-{}", std::process::id())), false).is_err());
    let rule = check_folder(&std::env::temp_dir(), true).unwrap();
    assert_eq!(paths::decode(&rule.path), base, "resolved");
    assert!(rule.repository && gitdir_pattern(&rule).is_ok());
}

#[test]
fn the_case_probe_flips_the_deepest_part_with_letters() {
    let base = paths::temp_dir();
    assert_eq!(pattern::flip(&base.join("Repo").join("123")), Some(base.join("rEPO").join("123")));
    assert_eq!(pattern::flip(Path::new("/")), None);
}

#[test]
fn include_files_are_known_by_name() {
    let dir = paths::temp_dir().join("glowfly-profiles");
    assert_eq!(profile_of_file(&dir, &dir.join("profile-12.gitconfig")), Some(ProfileId(12)));
    assert_eq!(profile_of_file(&dir, &dir.join("profile-x.gitconfig")), None);
    assert_eq!(profile_of_file(&dir, &dir.join("sub").join("profile-1.gitconfig")), None);
    assert_eq!(profile_of_file(&dir, &paths::temp_dir().join("profile-1.gitconfig")), None);
}
