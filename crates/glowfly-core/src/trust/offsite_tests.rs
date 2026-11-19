//! Places a repository's settings and hooks name that the scan doesn't look at: a
//! `core.hooksPath`, a program a setting runs and a hook link at `testutil::net_place()` (never
//! touched) are listed in `TrustScan::offsite` and nothing is read from them, in the repository
//! and in a submodule; the same on this computer is read as before. Only the hook link needs a
//! link, and skips on Windows where the account may not make one. The Windows spellings of a
//! share with a `?` in them are read as the share they name, never as a pattern.

use super::tests::hook;
use super::*;
use crate::testutil::*;

fn shown(p: &Path) -> String {
    crate::native_path::shown(p)
}

#[test]
fn a_hooks_path_at_a_network_place_is_not_listed() {
    let dir = scratch_repo("trust-offsite-hooks");
    hook(&dir.join(".git/hooks"), "pre-commit");
    let place = net_place().join("hooks");
    git(&dir, &["config", "core.hooksPath", &git_spelling(&place)]);
    for s in [scan_at(&dir).unwrap(), scan_top(&Repository::open(&dir).unwrap())] {
        assert_eq!(s.config, [ConfigCommand { key: "core.hooksPath".into(), value: git_spelling(&place) }], "the setting is shown");
        assert_eq!(s.offsite, [shown(Path::new(&git_spelling(&place)))]);
        assert_eq!((s.hooks_dir.as_ref(), s.hooks.len(), s.hook_digests.len()), (None, 0, 0), "nothing is listed or read there");
        assert!(s.follows.is_empty(), "{:?}", s.follows);
        // The stamp looks at nothing there either.
        let repo = Repository::open(&dir).unwrap();
        assert_eq!(stamp(&repo, &s), stamp(&repo, &s));
    }
    // Never part of the print: a decision can't cover what wasn't read.
    let mut seen = scan_at(&dir).unwrap();
    let print = seen.print();
    seen.offsite.clear();
    assert_eq!(seen.print(), print);
    // On this computer it is read as before.
    hook(&dir.join("hooks-here"), "pre-push");
    git(&dir, &["config", "core.hooksPath", "hooks-here"]);
    let s = scan_at(&dir).unwrap();
    assert_eq!((s.hooks, s.offsite), (vec!["pre-push".to_owned()], Vec::<String>::new()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_program_at_a_network_place_is_not_looked_at() {
    let dir = scratch_repo("trust-offsite-program");
    let tool = git_spelling(&net_place().join("tools/fsm.sh"));
    let ca = git_spelling(&net_place().join("certs/ca.pem"));
    git(&dir, &["config", "core.fsmonitor", &tool]);
    git(&dir, &["config", "filter.x.clean", &format!("sh {tool} --clean")]);
    git(&dir, &["config", "http.sslCAInfo", &ca]);
    let s = scan_at(&dir).unwrap();
    assert_eq!(s.config.len(), 3, "each setting is shown: {:?}", s.config);
    assert_eq!(s.offsite, [shown(Path::new(&tool)), shown(Path::new(&ca))]);
    assert!(s.follows.is_empty(), "{:?}", s.follows);
    // One in the working tree is followed as before.
    std::fs::create_dir_all(dir.join("tools")).unwrap();
    write(&dir, "tools/fsm.sh", "#!/bin/sh\n");
    git(&dir, &["config", "core.fsmonitor", "./tools/fsm.sh"]);
    let s = scan_at(&dir).unwrap();
    assert!(s.follows.iter().any(|f| f.path == "tools/fsm.sh"), "{:?}", s.follows);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_submodules_places_are_named_after_its_path() {
    let (parent, sub) = with_submodule("trust-offsite-sub");
    let place = git_spelling(&net_place().join("hooks"));
    git(&sub, &["config", "core.hooksPath", &place]);
    let s = scan_at(&parent).unwrap();
    assert_eq!(s.submodules.len(), 1);
    assert_eq!((s.submodules[0].scan.hooks_dir.as_ref(), s.submodules[0].scan.hooks.len()), (None, 0));
    assert_eq!(s.offsite, [format!("libs/lib: {}", shown(Path::new(&place)))]);
    assert!(scan_top(&Repository::open(&parent).unwrap()).offsite.is_empty(), "a refresh reads the repository alone");
    let _ = std::fs::remove_dir_all(&parent);
    let _ = std::fs::remove_dir_all(parent.with_file_name(format!("{}-lib", parent.file_name().unwrap().to_string_lossy())));
}

#[test]
fn a_hook_linked_to_a_network_place_is_listed_by_name_alone() {
    let dir = scratch_repo("trust-offsite-link");
    let hooks = dir.join(".git/hooks");
    hook(&hooks, "pre-commit");
    assert_eq!(scan_at(&dir).unwrap().offsite, Vec::<String>::new());
    if !link_to(&net_place().join("x.sh"), &hooks.join("pre-push"), false) {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let s = scan_at(&dir).unwrap();
    assert_eq!(s.hooks, ["pre-commit", "pre-push"]);
    assert_eq!(s.hook_digests.keys().collect::<Vec<_>>(), ["pre-commit"], "the linked one is never read");
    assert_eq!(s.offsite, [shown(&hooks.join("pre-push"))]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_share_spelled_with_a_question_mark_is_not_looked_at() {
    if !cfg!(windows) {
        // These name local folders elsewhere.
        return;
    }
    let host = "glowfly-test.invalid";
    let spellings = [
        format!("//?/UNC/{host}/share/h/x"),
        format!(r"\\?\UNC\{host}\share\h\x"),
        format!("//./UNC/{host}/share/h/x"),
        format!(r"\\?\UNC/{host}/share\h/x"),
        format!("//{host}/share/h?oks/x"),
    ];
    for (i, place) in spellings.iter().enumerate() {
        for key in ["core.hooksPath", "core.fsmonitor", "http.sslCAInfo"] {
            let dir = scratch_repo(&format!("trust-offsite-spelled-{i}-{}", key.len()));
            git(&dir, &["config", key, place]);
            let s = scan_at(&dir).unwrap();
            assert_eq!(s.config.len(), 1, "{key} = {place}: {:?}", s.config);
            assert!(!s.offsite.is_empty(), "{key} = {place}: noted");
            assert!(s.offsite.iter().all(|p| on_net_place(Path::new(p))), "{key} = {place}: {:?}", s.offsite);
            // The hooks folder is the repository's own unless the setting names one.
            assert_eq!((s.hooks_dir.is_none(), s.hooks.len()), (key == "core.hooksPath", 0), "{key} = {place}");
            assert!(s.follows.iter().all(|f| !f.path.contains(host)), "{key} = {place}: {:?}", s.follows);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
