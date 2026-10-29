//! The system font search: the language's variant, each system's order, the Unix folders and
//! their search, and which candidates `choose` keeps.

use super::candidates::{self, Variant};
use super::{Candidate, Script, choose, scripts_in};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const SUBSET: &[u8] = include_bytes!("../../../../../assets/fonts/test/NotoSansCJKsc-TestSubset.otf");
const NOTO_SANS: &[u8] = include_bytes!("../../../../../assets/fonts/NotoSans-Regular.ttf");

/// A fresh folder for one test.
fn folder(name: &str) -> PathBuf {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-system-fonts-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temp folder");
    dir
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("its folder");
    std::fs::write(path, bytes).expect("written");
}

fn file_names(c: &[Candidate]) -> Vec<String> {
    c.iter().map(|c| c.path.file_name().unwrap().to_string_lossy().into_owned()).collect()
}

#[test]
fn reads_the_variant_from_the_locale() {
    use Variant::*;
    for (tag, want) in [
        ("ja-JP", Some(Japanese)),
        ("ja", Some(Japanese)),
        ("ko_KR.UTF-8", Some(Korean)),
        ("zh-CN", Some(Simplified)),
        ("zh-Hans-HK", Some(Simplified)),
        ("zh-TW", Some(Traditional)),
        ("zh_HK", Some(Traditional)),
        ("zh-Hant", Some(Traditional)),
        ("zh", Some(Simplified)),
        ("en-US", None),
        ("", None),
    ] {
        assert_eq!(candidates::variant_of(tag), want, "{tag}");
    }
}

#[test]
fn windows_tries_the_languages_font_first() {
    let fonts = Path::new(r"C:\Windows\Fonts");
    let any = candidates::windows(fonts, None);
    assert_eq!(file_names(&any)[..3], ["msyh.ttc", "YuGothR.ttc", "malgun.ttf"]);
    assert!(any.iter().all(|c| c.path.parent() == Some(fonts) && c.index == 0));
    assert_eq!(file_names(&candidates::windows(fonts, Some(Variant::Korean)))[..2], ["malgun.ttf", "gulim.ttc"]);
    assert_eq!(file_names(&candidates::windows(fonts, Some(Variant::Japanese)))[0], "YuGothR.ttc");
    assert_eq!(file_names(&candidates::windows(fonts, Some(Variant::Traditional)))[0], "msjh.ttc");
}

#[test]
fn macos_looks_in_the_system_folders_then_home() {
    let home = Path::new("/Users/me");
    let list = candidates::macos(Some(home), None);
    assert_eq!(list[0].path, Path::new("/System/Library/Fonts/PingFang.ttc"));
    assert!(list.iter().any(|c| c.path == home.join("Library/Fonts/AppleSDGothicNeo.ttc")));
    let ja = candidates::macos(Some(home), Some(Variant::Japanese));
    assert!(file_names(&ja)[0].ends_with(" W3.ttc"), "Hiragino Sans first: {:?}", ja[0]);
    assert!(candidates::macos(None, None).iter().all(|c| c.path.starts_with("/System") || c.path.starts_with("/Library")));
}

#[test]
fn unix_folders_follow_fontconfigs_defaults() {
    let env =
        |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| OsString::from(v));
    let p = |v: &[&str]| v.iter().map(PathBuf::from).collect::<Vec<_>>();
    assert_eq!(
        candidates::unix_dirs(env(&[("HOME", "/home/me")])),
        p(&[
            "/home/me/.local/share/fonts",
            "/home/me/.fonts",
            "/usr/local/share/fonts",
            "/usr/share/fonts",
            "/run/host/fonts",
            "/run/host/user-fonts"
        ])
    );
    let dirs =
        candidates::unix_dirs(env(&[("HOME", "/home/me"), ("XDG_DATA_HOME", "/data"), ("XDG_DATA_DIRS", "/opt/share:rel:/usr/share")]));
    assert_eq!(dirs[..4], p(&["/data/fonts", "/home/me/.fonts", "/opt/share/fonts", "/usr/share/fonts"])[..], "relative parts ignored");
    assert_eq!(dirs.iter().filter(|d| d.ends_with("usr/share/fonts")).count(), 1, "listed once: {dirs:?}");
    let none = candidates::unix_dirs(env(&[("HOME", "relative"), ("XDG_DATA_HOME", "also")]));
    assert!(none.iter().all(|d| d.to_string_lossy().starts_with('/')), "{none:?}");
}

#[test]
fn unix_search_finds_known_files_in_rank_order() {
    let root = folder("unix");
    write(&root.join("share/fonts/truetype/wqy/wqy-zenhei.ttc"), b"x");
    write(&root.join("share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"), b"x");
    write(&root.join("share/fonts/opentype/noto/NotoSans-Regular.ttf"), b"x");
    write(&root.join("home/.fonts/NanumGothic.ttf"), b"x");
    let dirs = [root.join("home/.fonts"), root.join("share/fonts"), root.join("missing")];
    let any = candidates::unix(&dirs, None);
    assert_eq!(file_names(&any), ["NotoSansCJK-Regular.ttc", "wqy-zenhei.ttc", "NanumGothic.ttf"]);
    assert_eq!(any[0].index, 2, "the Simplified Chinese face of the collection");
    let ko = candidates::unix(&dirs, Some(Variant::Korean));
    assert_eq!(file_names(&ko)[0], "NanumGothic.ttf");
    assert_eq!(ko.iter().find(|c| c.path.ends_with("NotoSansCJK-Regular.ttc")).unwrap().index, 1, "its Korean face");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_test_font_has_every_script_and_noto_sans_none() {
    assert_eq!(scripts_in(SUBSET, 0), Some(vec![Script::Han, Script::Kana, Script::Hangul]));
    assert_eq!(scripts_in(NOTO_SANS, 0), Some(vec![]));
    assert_eq!(scripts_in(SUBSET, 3), None, "no such face");
    assert_eq!(scripts_in(b"not a font", 0), None);
}

#[test]
fn choose_keeps_only_fonts_that_add_a_script() {
    let root = folder("choose");
    let (garbage, latin, cjk, again) = (root.join("bad.ttf"), root.join("latin.ttf"), root.join("cjk.otf"), root.join("again.otf"));
    write(&garbage, &[7u8; 4096]);
    write(&latin, NOTO_SANS);
    write(&cjk, SUBSET);
    write(&again, SUBSET);
    let at = |p: &PathBuf| Candidate { path: p.clone(), index: 0 };
    let list = [Candidate { path: root.join("missing.ttc"), index: 0 }, at(&garbage), at(&latin), at(&cjk), at(&again)];
    let kept = choose(&list);
    assert_eq!(kept.len(), 1, "{:?}", kept.iter().map(|f| &f.path).collect::<Vec<_>>());
    assert_eq!(kept[0].path, cjk);
    assert_eq!(kept[0].name, "system-cjk.otf-0");
    assert_eq!(kept[0].data, SUBSET, "the file's bytes, mapped");
    assert!(choose(&[at(&latin)]).is_empty(), "a font without these scripts isn't kept");
}
