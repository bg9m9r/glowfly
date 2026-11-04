//! The sandbox's own checks: whatever a test runs through `runner::testing` (one effect, or the
//! whole-app tests' runner) uses the stand-ins, never the real trash, sequence editor or config
//! folder.

use super::*;
use glowfly_model::effect::DeleteMode;
use glowfly_model::effect::SaveEffect;

/// Whether `s` is the tests' stand-ins (its kind, and `cp` as the editor). That the stand-in
/// trash is what a delete reaches is checked by running one
/// (`a_delete_run_by_a_test_goes_to_the_stand_in_bin`).
fn stand_ins(s: &Seams) -> bool {
    s.kind == SeamsKind::StandIn && matches!((s.sequence_editor)(), SequenceEditor::Copy)
}

#[test]
fn the_stand_ins_are_not_the_real_trash_or_editor() {
    assert!(stand_ins(&SEAMS));
    assert!(!stand_ins(&Seams::REAL), "the app's seams are the real ones");
    assert!(matches!((Seams::REAL.sequence_editor)(), SequenceEditor::Helper(_)), "the app installs todos itself");
    assert!(SEAMS.chunk > Seams::REAL.chunk);
}

#[test]
fn the_whole_app_tests_runner_uses_the_stand_ins() {
    let (tx, _rx) = std::sync::mpsc::channel();
    let runner = sandboxed(tx, Waker::none(), glowfly_core::paths::temp_dir());
    assert!(stand_ins(&runner.seams));
    assert!(runner.sandbox.is_some());
    let (tx, _rx) = std::sync::mpsc::channel();
    assert!(!stand_ins(&Runner::new(tx, Waker::none()).seams), "the app's runner uses the real ones");
}

#[test]
fn a_delete_run_by_a_test_goes_to_the_stand_in_bin() {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-testing-trash-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    glowfly_core::testkit::git(&dir, &["init", "-q", "-b", "main"]);
    let contents = format!("only in the stand-in bin {}\n", std::process::id());
    std::fs::write(dir.join("gone.txt"), &contents).unwrap();
    let op = FileOp::Delete { path: "gone.txt".into(), mode: DeleteMode::Trash };
    let msg = execute(Effect::Files(glowfly_model::effect::FilesEffect::Work { workdir: dir.clone(), op }));
    assert!(msg.is_some());
    assert!(!dir.join("gone.txt").exists());
    let binned = std::fs::read_dir(test_bin()).unwrap().flatten().any(|e| std::fs::read_to_string(e.path()).is_ok_and(|t| t == contents));
    assert!(binned, "the file is in the stand-in bin");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Whether `dir` is inside the user's config folder (as the environment names it now).
fn in_config_folder(dir: &Path) -> bool {
    glowfly_core::settings::config_dir().is_some_and(|c| dir.starts_with(c))
}

#[test]
fn saves_run_by_a_test_land_in_the_test_folder_not_the_config_folder() {
    let marker = 4_000_000 + std::process::id();
    let settings = glowfly_core::settings::Settings { auto_fetch_minutes: marker, ..Default::default() };
    assert!(execute(Effect::Save(SaveEffect::Settings(Box::new(settings)))).is_none(), "the save worked");
    let repo = glowfly_core::paths::temp_dir().join(format!("glowfly-own-files-{marker}"));
    assert!(execute(Effect::Save(SaveEffect::Recent(vec![repo.clone()]))).is_none());
    assert!(execute(Effect::Save(SaveEffect::Tabs { paths: vec![repo.clone()], labels: vec![None], active: 0 })).is_none());
    let dir = own_files();
    assert!(!in_config_folder(&dir), "{} is in the config folder", dir.display());
    let saved = std::fs::read_to_string(dir.join("settings.json")).unwrap();
    assert!(saved.contains(&format!("\"auto_fetch_minutes\": {marker}")), "{saved}");
    for file in ["recent.txt", "tabs.txt"] {
        let text = std::fs::read_to_string(dir.join(file)).unwrap();
        assert!(text.contains(&format!("glowfly-own-files-{marker}")), "{file}: {text}");
    }
    // A runner given the stand-ins saves there too, on its save thread. (One test, since both
    // write the same files.)
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut runner = Runner::new(tx, Waker::none());
    runner.seams = SEAMS;
    let repo = glowfly_core::paths::temp_dir().join(format!("glowfly-runner-own-files-{}", std::process::id()));
    runner.run(Effect::Save(SaveEffect::Recent(vec![repo.clone()])));
    let file = own_files().join("recent.txt");
    let wanted = repo.display().to_string();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !std::fs::read_to_string(&file).is_ok_and(|t| t.contains(&wanted)) {
        assert!(std::time::Instant::now() < deadline, "recent.txt was not written into {}", own_files().display());
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn effects_reaching_outside_the_test_are_refused() {
    use glowfly_model::effect::{ConfigWrite, PrefsEffect, UpdateEffect};
    use glowfly_model::git_env::GitEnvEffect;
    use glowfly_model::onboarding::OnboardingEffect;
    use glowfly_model::profile::ProfileEffect;
    use glowfly_model::ssh::SshEffect;
    let here = glowfly_core::paths::temp_dir();
    let identity = glowfly_core::identity::Identity { name: "Ada".into(), email: "ada@example.invalid".into(), signing_key: None };
    let global = |key: &str| ConfigWrite { global: true, key: key.into(), value: Some("x".into()) };
    let plan = glowfly_core::profile_includes::IncludePlan { files: Vec::new(), entries: Vec::new() };
    let outside = [
        // The user's global git config.
        Effect::Onboarding(OnboardingEffect::WriteIdentity { cwd: here.clone(), identity }),
        Effect::Prefs(PrefsEffect::SetGitConfig { cwd: here.clone(), global: true, key: "user.name".into(), value: None }),
        Effect::Prefs(PrefsEffect::WriteGitConfig { cwd: here.clone(), ticket: 1, writes: vec![global("user.email")] }),
        Effect::GitEnv(GitEnvEffect::AddSafeDirectory(here.clone())),
        Effect::Ssh(SshEffect::SetKeyChoice { repo: None, key: None }),
        Effect::Profile(ProfileEffect::SyncIncludes { cwd: here.clone(), plan, place: true }),
        // Glowfly's own folders.
        Effect::Theme(glowfly_model::themes::ThemeEffect::Load),
        Effect::Prefs(PrefsEffect::ReadSettings),
        Effect::Updates(UpdateEffect::Tidy),
        Effect::FetchAvatars(vec!["ada@example.invalid".into()]),
    ];
    for effect in &outside {
        assert!(refused(effect), "{} is refused", effect.name());
    }
    let inside = [
        Effect::Prefs(PrefsEffect::SetGitConfig { cwd: here.clone(), global: false, key: "user.name".into(), value: None }),
        Effect::Ssh(SshEffect::ReadChoice { generation: 1, repo: None }),
        Effect::Ssh(SshEffect::SetKeyChoice { repo: Some(here.clone()), key: None }),
        Effect::Save(SaveEffect::Settings(Box::default())),
    ];
    for effect in &inside {
        assert!(!refused(effect), "{} runs", effect.name());
    }
    // A trust re-check around an effect doesn't change whether it runs.
    let checked = |effect: Effect| {
        let check = TrustCheck {
            workdir: here.clone(),
            real: here.clone(),
            saved: None,
            sealed: glowfly_model::effect::Sealed::Read,
            accepted: None,
            lang: Default::default(),
        };
        Effect::Checked(Box::new(Checked { effect, check }))
    };
    let allowed = checked(Effect::Ssh(SshEffect::ReadChoice { generation: 1, repo: None }));
    assert!(!refused(&allowed), "a checked {} runs", allowed.name());
    let global =
        checked(Effect::Prefs(PrefsEffect::SetGitConfig { cwd: here.clone(), global: true, key: "user.name".into(), value: None }));
    assert!(refused(&global), "a checked {} is refused", global.name());
}

/// Proven with an effect that only reads Glowfly's theme folder, so nothing changes even if the
/// refusal ever stopped working.
#[test]
#[should_panic(expected = "reaches outside the test")]
fn running_an_effect_that_reaches_outside_fails_the_test() {
    execute(Effect::Theme(glowfly_model::themes::ThemeEffect::Load));
}
