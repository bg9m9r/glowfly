use super::*;
use crate::state::prefs::{Autocrlf, GitConfigValues};

fn flash_at(start: Instant) -> SavedFlash {
    let mut f = SavedFlash::default();
    f.mark();
    f.tick(start, false);
    f
}

#[test]
fn the_tick_shows_then_fades_then_hides() {
    let start = Instant::now();
    let mut f = flash_at(start);
    assert!(f.showing() && !f.pending);
    assert_eq!(f.alpha, 1.0);
    f.tick(start + SHOW_FOR - Duration::from_millis(1), false);
    assert_eq!(f.alpha, 1.0, "full strength while showing");
    f.tick(start + SHOW_FOR + FADE_FOR / 2, false);
    assert!((f.alpha - 0.5).abs() < 0.01, "half way through the fade: {}", f.alpha);
    f.tick(start + SHOW_FOR + FADE_FOR, false);
    assert!(!f.showing());
    assert_eq!(f.alpha, 0.0);
}

#[test]
fn with_reduce_motion_the_tick_does_not_fade() {
    let start = Instant::now();
    let mut f = flash_at(start);
    f.tick(start + SHOW_FOR + FADE_FOR / 2, true);
    assert_eq!(f.alpha, 1.0);
    f.tick(start + SHOW_FOR + FADE_FOR, true);
    assert!(!f.showing());
}

#[test]
fn marking_again_restarts_the_tick() {
    let start = Instant::now();
    let mut f = flash_at(start);
    let later = start + SHOW_FOR + FADE_FOR / 2;
    f.mark();
    f.tick(later, false);
    assert_eq!(f.since, Some(later));
    assert_eq!(f.alpha, 1.0);
}

fn prefs_on(page: PrefPage) -> PrefsState {
    let mut p = PrefsState::new(page);
    let git = GitConfigValues { repo_name: "Ada".into(), hooks_path: ".hooks".into(), ..Default::default() };
    p.git = Some(git.clone());
    p.git_saved = Some(git);
    p
}

#[test]
fn identity_counts_trimmed_edits_only() {
    let s = Settings::default();
    let mut p = prefs_on(PrefPage::RepoIdentity);
    assert!(!unsaved(&p, &s, None));
    p.git.as_mut().unwrap().repo_name = " Ada ".into();
    assert!(!unsaved(&p, &s, None), "spaces around the name aren't an edit");
    p.git.as_mut().unwrap().repo_email = "ada@example.com".into();
    assert!(unsaved(&p, &s, None));
}

#[test]
fn hooks_and_general_track_their_own_fields() {
    let s = Settings::default();
    let mut p = prefs_on(PrefPage::RepoHooks);
    p.git.as_mut().unwrap().autocrlf = Autocrlf::Input;
    assert!(!unsaved(&p, &s, None), "line endings belong to General");
    p.git.as_mut().unwrap().hooks_path = "hooks".into();
    assert!(unsaved(&p, &s, None));

    let mut p = prefs_on(PrefPage::General);
    assert!(!unsaved(&p, &s, None));
    p.git.as_mut().unwrap().longpaths = true;
    assert!(unsaved(&p, &s, None));
}

#[test]
fn pages_without_tracked_fields_are_never_unsaved() {
    let s = Settings::default();
    let mut p = prefs_on(PrefPage::Ui);
    p.git.as_mut().unwrap().hooks_path = "hooks".into();
    assert!(!unsaved(&p, &s, None));
}

#[test]
fn repo_pages_are_the_four_that_write_the_repository_config() {
    use PrefPage::*;
    let all = [
        General,
        Git,
        Profile,
        Integrations,
        Ui,
        Language,
        DiffViewer,
        Keyboard,
        Tools,
        Terminal,
        Signing,
        Ssh,
        RepoIdentity,
        RepoHooks,
        RepoTracker,
        RepoLfs,
        RepoGitFlow,
        RepoSparse,
    ];
    let repo: Vec<_> = all.into_iter().filter(|p| repo_page(*p)).collect();
    assert_eq!(repo, [RepoIdentity, RepoHooks, RepoGitFlow, RepoSparse]);
}
