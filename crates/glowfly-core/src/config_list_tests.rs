use super::*;
use crate::gitcli::{self, args};
use crate::testutil::{empty_global_config, scratch_repo};
use std::path::Path;

fn e(k: &str, v: Option<&str>) -> Entry {
    (k.to_owned(), v.map(str::to_owned))
}

fn set(k: &str, v: &str) -> Entry {
    e(k, Some(v))
}

#[test]
fn parse_z_reads_values_newlines_and_valueless_keys() {
    let out = "difftool.My.Tool.cmd\na \"$LOCAL\"\nb\0difftool.My.Tool.trustexitcode\0diff.tool\nMy.Tool\0";
    assert_eq!(
        parse_z(out),
        [set("difftool.My.Tool.cmd", "a \"$LOCAL\"\nb"), e("difftool.My.Tool.trustexitcode", None), set("diff.tool", "My.Tool")]
    );
    assert!(parse_z("").is_empty());
}

#[test]
fn keys_match_like_git() {
    assert!(same_key("core.longpaths", "Core.LongPaths"));
    assert!(same_key("url.Foo.insteadof", "URL.Foo.insteadOf"));
    assert!(!same_key("url.Foo.insteadof", "url.foo.insteadof"));
    assert!(!same_key("core.longpaths", "core.autocrlf"));
    assert!(same_key("Flag", "flag"));
    assert_eq!(split("includeif.gitdir:~/a.b/.path"), ("includeif", Some("gitdir:~/a.b/"), "path"));
    assert!(same_key("includeif.gitdir:~/a.b/.path", "IncludeIf.gitdir:~/a.b/.PATH"));
    assert!(!same_key("includeif.gitdir:~/a.b/.path", "includeif.gitdir:~/A.b/.path"));
    assert_eq!(split("core.longpaths"), ("core", None, "longpaths"));
}

#[test]
fn lookups_from_text() {
    let c = ConfigList::parse("user.name\nA\0x.flag\0user.name\nB\0multi.v\na\0x.empty\n\0multi.v\nb\0");
    assert_eq!(c.last("User.Name"), Some(Some("B")));
    assert_eq!(c.last("x.flag"), Some(None));
    assert_eq!(c.text("x.flag"), Some(""));
    assert_eq!(c.text("x.empty"), Some(""));
    assert_eq!(c.last("not.set"), None);
    assert_eq!(c.text("not.set"), None);
    assert_eq!(c.all("user.name"), [Some("A"), Some("B")]);
    assert_eq!(c.all("multi.v"), [Some("a"), Some("b")]);
    assert_eq!(c.all("x.flag"), [None]);
    assert_eq!(c.entries().len(), 6);
    assert_eq!(c.bool("x.flag"), Some(true));
    assert_eq!(c.bool("x.empty"), Some(false));
    assert_eq!(c.bool("user.name"), None);
    assert_eq!(c.bool("not.set"), None);
}

/// Value, git's reading: Some(bool) or None when refused.
const TABLE: &[(&str, Option<bool>)] = &[
    ("yes", Some(true)),
    ("On", Some(true)),
    ("TRUE", Some(true)),
    ("1", Some(true)),
    ("2", Some(true)),
    ("-1", Some(true)),
    ("+1", Some(true)),
    ("010", Some(true)),
    ("0x10", Some(true)),
    ("0X1F", Some(true)),
    ("1k", Some(true)),
    ("1K", Some(true)),
    ("1m", Some(true)),
    ("1g", Some(true)),
    ("2147483647", Some(true)),
    (" 1", Some(true)),
    ("", Some(false)),
    ("0", Some(false)),
    ("00", Some(false)),
    ("0x0", Some(false)),
    ("-0", Some(false)),
    ("0k", Some(false)),
    ("no", Some(false)),
    ("OFF", Some(false)),
    ("false", Some(false)),
    ("1x", None),
    ("1 ", None),
    (" yes", None),
    ("yes ", None),
    ("08", None),
    ("k", None),
    ("1 k", None),
    ("2g", None),
    ("3g", None),
    ("2147483648", None),
    ("99999999999", None),
    ("maybe", None),
    ("auto", None),
    ("input", None),
];

#[test]
fn booleans_read_as_git_reads_them() {
    assert_eq!(parse_bool(None), Some(true), "a valueless key");
    for (v, want) in TABLE {
        assert_eq!(parse_bool(Some(v)), *want, "{v:?}");
    }
}

#[test]
fn booleans_match_git_type_bool() {
    let dir = scratch_repo("config-list-bools");
    let file = dir.join("bools.config");
    for (v, _) in TABLE {
        // None of the values holds a quote or a backslash.
        std::fs::write(&file, format!("[a]\n\tb = \"{v}\"\n")).unwrap();
        let out = gitcli::read(&dir, &args(&["config", "--file", &file.display().to_string(), "--type=bool", "--get", "a.b"]));
        let git = out.ok.then(|| out.stdout.trim() == "true");
        assert_eq!(parse_bool(Some(v)), git, "{v:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A repository, a global config (with an include) and the environment that uses it.
fn fixture(name: &str) -> (std::path::PathBuf, Vec<(String, String)>) {
    let dir = scratch_repo(name);
    // The repository's own name would come after the global ones.
    crate::testutil::git(&dir, &["config", "--local", "--unset", "user.name"]);
    let env = empty_global_config(&dir);
    let global = Path::new(&env[0].1).to_path_buf();
    let inc = dir.with_file_name(format!("{name}-included"));
    std::fs::write(&inc, "[user]\n\tname = Included\n").unwrap();
    let inc_path = inc.display().to_string().replace(std::path::MAIN_SEPARATOR, "/");
    std::fs::write(
        &global,
        format!(
            "[user]\n\tname = Early\n[Core]\n\tLongPaths = yes\n[x]\n\tflag\n\tempty =\n[url \"Foo\"]\n\tinsteadOf = one\n\
             [url \"foo\"]\n\tinsteadOf = two\n[multi]\n\tv = a\n\tv = b\n[include]\n\tpath = {inc_path}\n"
        ),
    )
    .unwrap();
    (dir, env)
}

fn run(dir: &Path, env: &[(String, String)], a: &[&str]) -> gitcli::GitOutput {
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    gitcli::output(dir, &args(a), "", &env, None)
}

fn list(dir: &Path, env: &[(String, String)], scope: &[&str]) -> ConfigList {
    let a: Vec<&str> = ["config"].iter().chain(scope).chain(&["--list", "-z"]).copied().collect();
    let out = run(dir, env, &a);
    assert!(out.ok, "{}", out.combined());
    ConfigList::parse(&out.stdout)
}

#[test]
fn real_list_output() {
    let (dir, env) = fixture("config-list-real");
    let c = list(&dir, &env, &[]);
    assert_eq!(c.last("user.name"), Some(Some("Included")), "the include comes after the earlier value");
    let pos = |key: &str, v: &str| c.entries().iter().position(|(k, val)| k == key && val.as_deref() == Some(v)).unwrap();
    let include = c.entries().iter().position(|(k, _)| k == "include.path").unwrap();
    assert!(pos("user.name", "Early") < include && include < pos("user.name", "Included"));
    assert_eq!(c.all("multi.v"), [Some("a"), Some("b")]);
    assert_eq!(c.text("url.Foo.insteadof"), Some("one"));
    assert_eq!(c.text("url.foo.insteadof"), Some("two"));
    assert_eq!(c.last("x.flag"), Some(None));
    assert_eq!(c.bool("x.flag"), Some(true));
    assert_eq!(c.text("x.flag"), Some(""));
    assert_eq!(c.bool("x.empty"), Some(false));
    assert_eq!(c.bool("core.longpaths"), Some(true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn list_answers_match_get() {
    let (dir, env) = fixture("config-list-get");
    let c = list(&dir, &env, &[]);
    for key in ["core.longpaths", "user.name", "url.Foo.insteadOf", "url.foo.insteadof", "multi.v", "x.flag", "x.empty", "not.set"] {
        let get = run(&dir, &env, &["config", "--get", key]);
        let want = get.ok.then(|| get.stdout.trim_end_matches(['\r', '\n']).to_owned());
        assert_eq!(c.text(key).map(str::to_owned), want, "{key}");
        let b = run(&dir, &env, &["config", "--type=bool", "--get", key]);
        if b.ok {
            assert_eq!(c.bool(key), Some(b.stdout.trim() == "true"), "{key} as a boolean");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scoped_list_skips_includes() {
    let (dir, env) = fixture("config-list-scoped");
    let c = list(&dir, &env, &["--global"]);
    assert!(c.last("include.path").is_some(), "the include line is listed");
    assert_eq!(c.all("user.name"), [Some("Early")], "but what it includes is not");
    let _ = std::fs::remove_dir_all(&dir);
}
