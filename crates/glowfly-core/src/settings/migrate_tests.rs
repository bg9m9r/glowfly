use super::*;
use crate::settings::paths::tests::{env_of, root};
use crate::settings::paths::{cache_dir_for, config_dir_for, log_dir_for};

/// A folder holding `file` with `text`.
fn seed(dir: &Path, file: &str, text: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(file), text).unwrap();
}

#[test]
fn old_dirs_per_platform() {
    let r = root("old");
    let (home, xdg) = (r.join("home"), r.join("xdg"));
    let env = env_of(vec![("HOME", home.clone()), ("APPDATA", r.join("appdata"))]);
    let appdata = r.join("appdata");
    assert_eq!(old_config_dirs_for(Os::Windows, &env), vec![appdata.join("Trellis"), appdata.join("Riffle")]);
    let dot = home.join(".config");
    let support = home.join("Library").join("Application Support");
    assert_eq!(old_config_dirs_for(Os::Mac, &env), vec![support.join("Trellis"), dot.join("Trellis"), dot.join("Riffle")]);
    assert_eq!(old_config_dirs_for(Os::Xdg, &env), vec![dot.join("trellis"), dot.join("Trellis"), dot.join("Riffle")]);

    let env = env_of(vec![("HOME", home.clone()), ("XDG_CONFIG_HOME", xdg.clone())]);
    assert_eq!(
        old_config_dirs_for(Os::Mac, &env),
        vec![support.join("Trellis"), xdg.join("Trellis"), dot.join("Trellis"), xdg.join("Riffle"), dot.join("Riffle")]
    );
    assert_eq!(old_config_dirs_for(Os::Xdg, &env), vec![xdg.join("trellis"), xdg.join("Trellis"), xdg.join("Riffle")]);
}

#[test]
fn trellis_settings_move_to_glowfly_on_every_platform() {
    for (os, tag) in [(Os::Windows, "win"), (Os::Mac, "mac"), (Os::Xdg, "xdg")] {
        let r = root(&format!("trellis-{tag}"));
        let _ = std::fs::remove_dir_all(&r);
        let env = env_of(vec![("HOME", r.join("home")), ("APPDATA", r.join("appdata")), ("LOCALAPPDATA", r.join("local"))]);
        let old = config_dir_named(os, TRELLIS, &env).unwrap();
        seed(&old, "settings.json", "{\"zoom\":1.5}");
        // A Riffle folder too: the newer Trellis one wins and Riffle's is left alone.
        let riffle = old_config_dirs_for(os, &env).into_iter().find(|d| d.ends_with("Riffle")).unwrap();
        seed(&riffle, "settings.json", "{}");
        migrate_all(os, &env).unwrap();
        let new = config_dir_for(os, &env).unwrap();
        assert_eq!(std::fs::read_to_string(new.join("settings.json")).unwrap(), "{\"zoom\":1.5}", "{tag}");
        assert!(!old.exists(), "{tag}");
        assert!(riffle.exists(), "{tag}");

        // Once Glowfly's folder exists, a Trellis folder that comes back is left alone.
        seed(&old, "settings.json", "{}");
        migrate_all(os, &env).unwrap();
        assert!(old.exists(), "{tag}");
        assert_eq!(std::fs::read_to_string(new.join("settings.json")).unwrap(), "{\"zoom\":1.5}", "{tag}");
        let _ = std::fs::remove_dir_all(&r);
    }
}

#[test]
fn riffle_settings_still_move_when_there_is_no_trellis_folder() {
    let r = root("riffle-only");
    let _ = std::fs::remove_dir_all(&r);
    let env = env_of(vec![("HOME", r.join("home")), ("APPDATA", r.join("appdata"))]);
    seed(&r.join("appdata").join("Riffle"), "recent.txt", "C:/repo");
    migrate_all(Os::Windows, &env).unwrap();
    assert_eq!(std::fs::read_to_string(r.join("appdata").join("Glowfly").join("recent.txt")).unwrap(), "C:/repo");
    assert!(!r.join("appdata").join("Riffle").exists());
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn macos_moves_the_old_dot_config_folder_once() {
    let r = root("mac");
    let _ = std::fs::remove_dir_all(&r);
    let env = env_of(vec![("HOME", r.join("home"))]);
    let old = r.join("home").join(".config").join("Trellis");
    seed(&old, "settings.json", "{}");
    let new = config_dir_for(Os::Mac, &env).unwrap();
    migrate_first(&old_config_dirs_for(Os::Mac, &env), &new).unwrap();
    assert!(!old.exists());
    assert_eq!(std::fs::read_to_string(new.join("settings.json")).unwrap(), "{}");

    // A second run, with a stale old folder back, leaves both alone.
    std::fs::create_dir_all(&old).unwrap();
    migrate_first(&old_config_dirs_for(Os::Mac, &env), &new).unwrap();
    assert!(old.exists());
    assert!(new.join("settings.json").exists());
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn trellis_cache_and_logs_move_too() {
    // Windows: the logs live in the cache folder and move with it.
    let r = root("local-win");
    let _ = std::fs::remove_dir_all(&r);
    let env = env_of(vec![("LOCALAPPDATA", r.join("local"))]);
    seed(&r.join("local").join("Trellis").join("avatars"), "a.png", "png");
    seed(&r.join("local").join("Trellis").join("logs"), "activity.log", "fetch");
    migrate_all(Os::Windows, &env).unwrap();
    assert_eq!(std::fs::read_to_string(cache_dir_for(Os::Windows, &env).unwrap().join("avatars").join("a.png")).unwrap(), "png");
    assert_eq!(std::fs::read_to_string(log_dir_for(Os::Windows, &env).unwrap().join("activity.log")).unwrap(), "fetch");
    assert!(!r.join("local").join("Trellis").exists());
    let _ = std::fs::remove_dir_all(&r);

    // XDG: separate cache and state folders.
    let r = root("local-xdg");
    let _ = std::fs::remove_dir_all(&r);
    let env = env_of(vec![("HOME", r.join("home"))]);
    seed(&r.join("home").join(".cache").join("trellis"), "a.png", "png");
    seed(&r.join("home").join(".local").join("state").join("trellis").join("logs"), "activity.log", "fetch");
    migrate_all(Os::Xdg, &env).unwrap();
    assert_eq!(std::fs::read_to_string(cache_dir_for(Os::Xdg, &env).unwrap().join("a.png")).unwrap(), "png");
    assert_eq!(std::fs::read_to_string(log_dir_for(Os::Xdg, &env).unwrap().join("activity.log")).unwrap(), "fetch");
    assert_eq!(local_dirs_for(Os::Mac, TRELLIS, &env).len(), 2);
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn a_move_across_volumes_copies_then_removes() {
    let base = root("exdev");
    let _ = std::fs::remove_dir_all(&base);
    let (old, new) = (base.join("old"), base.join("new").join("Glowfly"));
    std::fs::create_dir_all(old.join("avatars")).unwrap();
    std::fs::write(old.join("settings.json"), "{}").unwrap();
    std::fs::write(old.join("avatars").join("a.png"), "png").unwrap();
    std::fs::create_dir_all(new.parent().unwrap()).unwrap();
    let cross = |_: &Path, _: &Path| Err(std::io::Error::new(std::io::ErrorKind::CrossesDevices, "cross-device link"));
    move_dir_with(&old, &new, cross).unwrap();
    assert!(!old.exists());
    assert_eq!(std::fs::read_to_string(new.join("settings.json")).unwrap(), "{}");
    assert_eq!(std::fs::read_to_string(new.join("avatars").join("a.png")).unwrap(), "png");
    // A copy that can't finish leaves the original alone.
    let missing = base.join("not-there");
    assert!(move_dir_with(&missing, &base.join("elsewhere"), cross).is_err());
    assert!(!base.join("elsewhere").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn old_config_dir_moves_once() {
    let base = root("riffle");
    let _ = std::fs::remove_dir_all(&base);
    let (old, new) = (base.join("Riffle"), base.join("Glowfly"));
    seed(&old, "recent.txt", "C:/repo");
    move_dir_if_new_missing(&old, &new).unwrap();
    assert!(!old.exists());
    assert_eq!(std::fs::read_to_string(new.join("recent.txt")).unwrap(), "C:/repo");

    // An existing new folder is never overwritten.
    std::fs::create_dir_all(&old).unwrap();
    move_dir_if_new_missing(&old, &new).unwrap();
    assert!(old.exists());
    let _ = std::fs::remove_dir_all(&base);
}
