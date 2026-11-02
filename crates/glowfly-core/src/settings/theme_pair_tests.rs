//! The Match the system pair: card clicks and the check, resolution, and how it loads.

use super::super::tests::scratch_settings;
use super::*;
use crate::settings::ProfilePrefs;
use BuiltinTheme as B;

fn with(theme: ThemeChoice, pair: SystemPair) -> Settings {
    Settings { theme, system_pair: pair, ..Settings::default() }
}

const MIDNIGHT_PORCELAIN: SystemPair = SystemPair { dark: B::Midnight, light: B::Porcelain };

#[test]
fn a_card_sets_its_pair_member_with_the_check_on_and_the_theme_with_it_off() {
    let mut s = with(ThemeChoice::System, SystemPair::default());
    s.pick_card(B::Midnight);
    assert_eq!((&s.theme, s.system_pair), (&ThemeChoice::System, SystemPair { dark: B::Midnight, light: B::Porcelain }));
    s.pick_card(B::Light);
    assert_eq!((&s.theme, s.system_pair), (&ThemeChoice::System, SystemPair { dark: B::Midnight, light: B::Light }));

    let mut s = with(ThemeChoice::Graphite, MIDNIGHT_PORCELAIN);
    s.pick_card(B::Mist);
    assert_eq!((&s.theme, s.system_pair), (&ThemeChoice::Mist, MIDNIGHT_PORCELAIN));
    // A theme file is replaced by the card too.
    let mut s = with(ThemeChoice::Custom("ocean".into()), MIDNIGHT_PORCELAIN);
    s.pick_card(B::HighContrastDark);
    assert_eq!((&s.theme, s.system_pair), (&ThemeChoice::HighContrastDark, MIDNIGHT_PORCELAIN));
}

#[test]
fn turning_the_check_off_keeps_what_is_on_screen() {
    for os_dark in [true, false] {
        let mut s = with(ThemeChoice::System, MIDNIGHT_PORCELAIN);
        s.set_match_system(false, os_dark);
        assert_eq!(s.theme, MIDNIGHT_PORCELAIN.pick(os_dark).choice(), "os_dark {os_dark}");
        assert_eq!(s.system_pair, MIDNIGHT_PORCELAIN, "the pair stays saved");
    }
}

#[test]
fn turning_the_check_on_pairs_the_theme_on_screen_with_its_twin() {
    for (from, pair) in [
        (ThemeChoice::Midnight, SystemPair { dark: B::Midnight, light: B::Mist }),
        (ThemeChoice::Light, SystemPair { dark: B::Dark, light: B::Light }),
        (ThemeChoice::HighContrastLight, SystemPair { dark: B::HighContrastDark, light: B::HighContrastLight }),
        (ThemeChoice::Custom("ocean".into()), MIDNIGHT_PORCELAIN),
    ] {
        let mut s = with(from.clone(), MIDNIGHT_PORCELAIN);
        s.set_match_system(true, true);
        assert_eq!((&s.theme, s.system_pair), (&ThemeChoice::System, pair), "from {from:?}");
    }
}

#[test]
fn system_follows_the_pair_in_both_modes() {
    assert_eq!(ThemeChoice::System.builtin(true, MIDNIGHT_PORCELAIN), Some(B::Midnight));
    assert_eq!(ThemeChoice::System.builtin(false, MIDNIGHT_PORCELAIN), Some(B::Porcelain));
    assert_eq!(ThemeChoice::Mist.builtin(true, MIDNIGHT_PORCELAIN), Some(B::Mist), "a fixed choice ignores the pair");
}

#[test]
fn twins_pair_each_dark_theme_with_its_light_one() {
    for (i, b) in B::ALL.into_iter().enumerate() {
        assert_eq!(b.twin().twin(), b, "{b:?}");
        assert_eq!(b.twin(), B::ALL[(i + 4) % 8], "{b:?}");
        assert_ne!(b.twin().is_dark(), b.is_dark(), "{b:?}");
    }
}

/// The pair a settings file with `pair` (raw JSON, or none) loads, with its zoom.
fn load(name: &str, pair: Option<&str>) -> (SystemPair, u16) {
    let text = match pair {
        Some(p) => format!(r#"{{"system_pair": {p}, "zoom_percent": 125}}"#),
        None => r#"{"zoom_percent": 125}"#.to_owned(),
    };
    let (dir, file) = scratch_settings(&format!("pair-{name}"), text.as_bytes());
    let (s, problem) = Settings::load_from(&file);
    assert_eq!((problem, &s.retired_theme), (None, &None), "{name}: loads without a problem or a retired-theme toast");
    let _ = std::fs::remove_dir_all(&dir);
    (s.system_pair, s.zoom_percent)
}

#[test]
fn the_pair_loads_leniently() {
    assert_eq!(load("none", None), (SystemPair::default(), 125), "an older file shows Graphite and Porcelain");
    assert_eq!(load("written", Some(r#"{"dark":"midnight","light":"mist"}"#)).0, SystemPair { dark: B::Midnight, light: B::Mist });
    assert_eq!(load("wrong-kind", Some(r#"{"dark":"porcelain"}"#)).0, SystemPair::default());
    assert_eq!(load("retired", Some(r#"{"dark":"pine"}"#)).0, SystemPair { dark: B::Dark, light: B::Porcelain });
    assert_eq!(load("older-ids", Some(r#"{"dark":"dark","light":"light"}"#)).0, SystemPair { dark: B::Dark, light: B::Light });
    assert_eq!(load("garbage", Some(r#"{"dark":7,"light":"nope"}"#)), (SystemPair::default(), 125));
    assert_eq!(load("not-an-object", Some("[1]")), (SystemPair::default(), 125));
}

#[test]
fn the_pair_saves_as_ids_and_round_trips() {
    let s = with(ThemeChoice::System, SystemPair { dark: B::Dark, light: B::Mist });
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["system_pair"], serde_json::json!({ "dark": "dusk", "light": "mist" }));
    let (dir, file) = scratch_settings("pair-round-trip", b"{}");
    s.save_to(&file).unwrap();
    let (back, _) = Settings::load_from(&file);
    assert_eq!(back.system_pair, s.system_pair);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_profile_copy_without_the_pair_loads_the_default() {
    let mut prefs = serde_json::to_value(ProfilePrefs::from_settings(&with(ThemeChoice::System, MIDNIGHT_PORCELAIN))).unwrap();
    assert_eq!(prefs["system_pair"]["dark"], "midnight", "the pair goes with the profile");
    prefs.as_object_mut().unwrap().remove("system_pair");
    let back: ProfilePrefs = serde_json::from_value(prefs).unwrap();
    assert_eq!(back.system_pair, SystemPair::default());
    assert_eq!(back.theme, ThemeChoice::System, "the rest of the copy still loads");
}
