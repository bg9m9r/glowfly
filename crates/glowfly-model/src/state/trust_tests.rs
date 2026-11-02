use super::*;

#[test]
fn decisions_match_any_spelling_of_the_folder() {
    let mut s = Settings::default();
    record(&mut s, Path::new("work/tools"), RepoTrust::Trusted);
    assert_eq!(recorded(&s, Path::new("work/./tools/")), Some(RepoTrust::Trusted));
    assert_eq!(recorded(&s, Path::new("work/other")), None);
    // Another case is another repository as far as the saved decisions go: on a volume that
    // tells case apart it is one, and trust is never shared by guessing.
    assert_eq!(recorded(&s, Path::new("work/Tools")), None);
    #[cfg(windows)]
    {
        record(&mut s, Path::new(r"\\?\C:\x"), RepoTrust::Trusted);
        assert_eq!(recorded(&s, Path::new(r"c:\x")), Some(RepoTrust::Trusted));
        assert_eq!(recorded(&s, Path::new(r"c:\X")), None);
        // A new decision replaces the one saved under the older spelling.
        record(&mut s, Path::new(r"c:\x\"), RepoTrust::NotTrusted);
        assert_eq!(recorded(&s, Path::new(r"\\?\C:\x")), Some(RepoTrust::NotTrusted));
        assert_eq!(s.repo_trust.len(), 2);
    }
    record(&mut s, Path::new("work/tools/"), RepoTrust::NotTrusted);
    assert_eq!(recorded(&s, Path::new("work/tools")), Some(RepoTrust::NotTrusted));
    assert_eq!(s.repo_trust.keys().filter(|k| k.contains("tools")).count(), 1);
}

#[test]
fn standing_follows_the_decision_then_the_scan() {
    assert_eq!(standing_of(Some(RepoTrust::Trusted), false, true), Standing::Trusted);
    assert_eq!(standing_of(Some(RepoTrust::Trusted), false, false), Standing::Changed, "gained something since");
    assert_eq!(standing_of(Some(RepoTrust::NotTrusted), true, true), Standing::NotTrusted, "no stays no");
    assert_eq!(standing_of(Some(RepoTrust::NotTrusted), false, false), Standing::NotTrusted);
    assert_eq!(standing_of(None, true, true), Standing::Clean);
    assert_eq!(standing_of(None, false, true), Standing::Undecided);
}

#[test]
fn the_paused_note_says_what_is_held() {
    let (undecided, refused) = (paused_note(Standing::Undecided), paused_note(Standing::NotTrusted));
    let changed = paused_note(Standing::Changed);
    assert_ne!(undecided, refused);
    let start = |n: &str| n.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
    assert!(start(changed) != start(undecided) && start(changed) != start(refused), "{changed}");
    assert!(changed.contains("gained settings or hooks"), "{changed}");
    for note in [undecided, refused, changed] {
        assert!(note.contains("background"), "{note}");
        assert!(!note.contains("  "), "one space between sentences: {note}");
        assert!(note.contains("Pull, Push, Commit or any other git command you start"), "{note}");
        assert!(!note.contains("still run"), "{note}");
    }
    assert!(undecided.contains("settings or hooks make git run programs"));
    assert!(refused.starts_with("You chose not to trust"));
    assert!(paused_note(Standing::Trusted).is_empty() && paused_note(Standing::Clean).is_empty());
}

#[test]
fn labels_and_values_are_shortened() {
    assert_eq!(hooks_label(Path::new("C:/work/tools/.git/hooks"), Path::new("C:/work/tools")), ".git/hooks");
    assert_eq!(hooks_label(Path::new("D:/shared/hooks"), Path::new("C:/work/tools")), "D:/shared/hooks");
    assert_eq!(shown_value("a\u{1b}b"), "ab");
    let long = "x".repeat(200);
    let shown = shown_value(&long);
    assert_eq!(shown.chars().count(), VALUE_CHARS + 1);
    assert!(shown.ends_with('…'));
}
