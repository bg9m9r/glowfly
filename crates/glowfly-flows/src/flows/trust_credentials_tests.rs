//! Signing in while a repository that isn't trusted is open (Connect in Preferences, the fork
//! dialogs' service calls): git's credential helpers are asked from a folder outside the
//! repository, so the repository's own `credential.helper` never runs; the same request in the
//! repository would run it.

use super::trust_held_tests::held_repo;
use super::trust_tests::run;
use glowfly_core::forge::{Account, Provider};
use glowfly_core::testkit::git;
use glowfly_model::effect::{AccountEffect, Effect};
use glowfly_model::msg::{ForgeMsg, Msg};
use glowfly_model::remote::{RemoteEffect, RemoteMsg};
use std::path::Path;

/// `git credential fill` for github.com in `cwd`, as Connect asks it, with only the
/// repository's own settings (no global or system config, no prompt).
fn fill_in(cwd: &Path, scratch: &Path) {
    let global = scratch.with_file_name(format!("{}-gitconfig", scratch.file_name().unwrap().to_string_lossy()));
    std::fs::write(&global, "").unwrap();
    let env = [
        ("GIT_CONFIG_GLOBAL", global.to_str().unwrap()),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_ASKPASS", ""),
        ("SSH_ASKPASS", ""),
        ("GCM_INTERACTIVE", "never"),
    ];
    let a = glowfly_core::gitcli::args(&["credential", "fill"]);
    let _ = glowfly_core::gitcli::run_with_input(cwd, &a, "protocol=https\nhost=github.com\n\n", &env);
    let _ = std::fs::remove_file(&global);
}

#[test]
fn connecting_while_an_untrusted_repository_is_open_never_runs_its_credential_helper() {
    let (mut st, dir) = held_repo("trust-credentials");
    git(&dir, &["config", "credential.helper", "!echo ran >ran.txt"]);
    let account = Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() };
    let fx = run(&mut st, Msg::Forge(ForgeMsg::Connect { account, user: None, token: None }));
    let cwd = fx
        .iter()
        .find_map(|e| match e {
            Effect::Account(AccountEffect::Connect { cwd, .. }) => Some(cwd.clone()),
            _ => None,
        })
        .expect("Connect asks the credential manager");
    assert!(!glowfly_core::paths::same_spelling(&cwd, &dir), "asked outside the repository: {}", cwd.display());
    fill_in(&cwd, &dir);
    assert!(!dir.join("ran.txt").exists(), "the repository's credential helper never ran");
    // The same request in the repository runs it: what Connect stays out of.
    fill_in(&dir, &dir);
    assert!(dir.join("ran.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_fork_dialogs_never_run_an_untrusted_repositorys_credential_helper() {
    let (mut st, dir) = held_repo("trust-credentials-fork");
    git(&dir, &["config", "credential.helper", "!echo ran >ran.txt"]);
    st.snap.as_mut().unwrap().remote_info.push(glowfly_core::repo::RemoteInfo {
        name: "origin".into(),
        urls: vec!["https://github.com/team/app.git".into()],
        push_urls: Vec::new(),
        skip_fetch_all: false,
    });
    for msg in [RemoteMsg::ForkDialog("origin".into()), RemoteMsg::ForkOnService("origin".into())] {
        st.modal = None;
        let fx = run(&mut st, Msg::Remote(msg));
        let cwd = match &fx[..] {
            [Effect::Remote(RemoteEffect::ListForks(r) | RemoteEffect::FindMyFork(r))] => r.cwd.clone(),
            _ => panic!("one service call: {fx:?}"),
        };
        assert!(!glowfly_core::paths::same_spelling(&cwd, &dir), "asked outside the repository: {}", cwd.display());
        fill_in(&cwd, &dir);
        assert!(!dir.join("ran.txt").exists(), "the repository's credential helper never ran");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
