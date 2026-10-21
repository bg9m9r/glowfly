use super::*;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

#[test]
fn add_cmd_puts_the_url_after_double_dash() {
    let url = "https://h/o/lib.git";
    assert_eq!(add_cmd(url, "libs/lib", None), s(&["submodule", "add", "--", url, "libs/lib"]));
    assert_eq!(add_cmd(url, "libs/lib", Some("dev")), s(&["submodule", "add", "-b", "dev", "--", url, "libs/lib"]));
    // A URL that looks like an option is still the URL.
    let cmd = add_cmd("  git@h:o/lib.git ", "lib", None);
    let dash = cmd.iter().position(|a| a == "--").unwrap();
    assert_eq!(&cmd[dash + 1..], ["git@h:o/lib.git", "lib"]);
}

#[test]
fn file_protocol_is_allowed_only_for_local_urls() {
    for url in [r"C:\r\lib", "/srv/lib", "../lib", "./lib", "file:///x", "FILE:///x"] {
        let cmd = add_cmd(url, "lib", None);
        assert_eq!(&cmd[..2], ["-c", "protocol.file.allow=always"], "{url}");
        assert!(needs_file_protocol(url), "{url}");
    }
    for url in ["https://h/o/lib.git", "git@h:o/lib.git", "ssh://h/o/lib.git"] {
        assert_eq!(add_cmd(url, "lib", None)[0], "submodule", "{url}");
        assert!(!needs_file_protocol(url), "{url}");
    }
}

#[test]
fn deinit_and_rm_read_the_path_literally() {
    assert_eq!(deinit_cmd("lib[1]"), s(&["submodule", "deinit", "-f", "--", ":(literal)lib[1]"]));
    assert_eq!(rm_cmd("lib[1]"), s(&["--literal-pathspecs", "rm", "-f", "--", "lib[1]"]));
}

#[test]
fn url_checks() {
    assert_eq!(check_add_url("  "), Err(UrlProblem::Empty));
    assert_eq!(check_add_url("-x"), Err(UrlProblem::Dash));
    assert_eq!(check_add_url("a\tb"), Err(UrlProblem::Control));
    assert_eq!(check_add_url(" https://h/o/lib.git "), Ok(()));
    assert_eq!(add_url_message(&UrlProblem::Empty), "Enter the URL or path of the repository to add.");
    assert_eq!(add_url_message(&UrlProblem::Dash), "URLs can't start with a dash.");
}

#[test]
fn parse_add_path_table() {
    let work = crate::paths::temp_dir().join("glowfly-parse-add-path").join("repo");
    let sibling = work.with_file_name("other");
    let inside = work.join("libs").join("x");
    let (inside, root, sibling) = (inside.display().to_string(), work.display().to_string(), sibling.display().to_string());
    for windows in [true, false] {
        let p = |t: &str| parse_add_path_for(t, &work, windows);
        use AddPathProblem as P;
        assert_eq!(p(""), Err(P::Empty));
        assert_eq!(p("  "), Err(P::Empty));
        assert_eq!(p("."), Err(P::Empty));
        assert_eq!(p("libs/lib/"), Ok("libs/lib".into()));
        assert_eq!(p("./libs//lib"), Ok("libs/lib".into()));
        assert_eq!(p("a/../b"), Err(P::Outside));
        assert_eq!(p(".."), Err(P::Outside));
        assert_eq!(p(&inside), Ok("libs/x".into()), "{windows}");
        assert_eq!(p(&format!("{inside}/")), Ok("libs/x".into()), "{windows}");
        assert_eq!(p(&root), Err(P::Root));
        assert_eq!(p(&sibling), Err(P::Outside));
        assert_eq!(p(".git/x"), Err(P::InsideGitDir));
        assert_eq!(p("libs/GIT~1"), Err(P::InsideGitDir));
        assert_eq!(p("libs/.Git"), Err(P::InsideGitDir));
        if windows {
            assert!(matches!(p("libs/aux"), Err(P::BadName(n, _)) if n == "aux"));
            assert_eq!(p(r"a\b"), Ok("a/b".into()));
            assert_eq!(p(r".\a\\b\"), Ok("a/b".into()));
        } else {
            assert_eq!(p("libs/aux"), Ok("libs/aux".into()));
            assert_eq!(p(r"a\b"), Ok(r"a\b".into()));
        }
    }
}

#[test]
fn default_paths() {
    assert_eq!(default_path("https://h/o/lib.git"), "lib");
    assert_eq!(default_path("git@h:o/lib.git"), "lib");
    assert_eq!(default_path(r"C:\repos\lib"), "lib");
    assert_eq!(default_path("https://h/o/lib/"), "lib");
    assert_eq!(default_path("../tools.git"), "tools");
    assert_eq!(default_path("https://h/o/con"), "con-repo");
}

#[test]
fn module_dirs() {
    let common = Path::new("common");
    for bad in ["", "/x", "a/../b", "a//b", "./a", "a/.git", "a/"] {
        assert!(module_dir(common, bad).is_err(), "{bad:?}");
    }
    assert_eq!(module_dir(common, "libs/lib").unwrap(), common.join("modules").join("libs").join("lib"));
}

#[test]
fn messages() {
    use AddPathProblem as P;
    assert_eq!(
        P::InIndex { gitlink: false }.message("README.md"),
        "README.md is already tracked. Choose a folder that doesn't exist yet, or an empty one."
    );
    assert_eq!(P::Exists { file: false }.message("lib"), "lib already exists. Choose a folder that doesn't exist yet, or an empty one.");
    assert_eq!(
        P::ParentSpelling("the folder Libs already exists as libs; use that spelling".into()).message("Libs/x"),
        "The folder Libs already exists as libs; use that spelling."
    );
    assert!(
        P::ModuleExists(PathBuf::from("g").join("modules").join("lib"))
            .message("lib")
            .contains(&crate::native_path::shown(&PathBuf::from("g/modules/lib")))
    );
}
