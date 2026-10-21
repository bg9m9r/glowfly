use super::*;
use crate::testutil::alias_of;

fn dir(name: &str) -> PathBuf {
    let d = temp_dir().join(format!("glowfly-paths-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    resolve(&d)
}

#[test]
fn resolve_follows_a_link_and_keeps_a_missing_tail() {
    let root = dir("link");
    let target = root.join("target");
    std::fs::create_dir_all(&target).unwrap();
    let link = alias_of(&target).expect("made a link");
    assert_eq!(resolve(&link), target);
    assert_eq!(resolve(&link.join("not-yet").join("deeper")), target.join("not-yet").join("deeper"));
    // `..` after a part that doesn't exist yet is applied, not left in.
    assert_eq!(resolve(&link.join("not-yet").join("..").join("other")), target.join("other"));
    assert_eq!(resolve(&link.join("a").join("b").join("..").join("..")), target);
    let _ = std::fs::remove_dir(&link).or_else(|_| std::fs::remove_file(&link));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn resolve_leaves_what_it_cannot_find() {
    let p = Path::new("no-such-relative-folder-for-glowfly");
    assert_eq!(resolve(p), p);
    // Relative paths are never resolved against the working folder.
    let rel = Path::new("src").join("no-such-file");
    assert_eq!(resolve(&rel), rel);
}

/// Pairs that are, and aren't, spelled as one path, on every OS.
#[test]
fn spelling_rules() {
    let same: &[(&str, &str)] = &[("work/repo/", "work/./repo"), ("/w/a", "/w/a/"), ("/w/./a/.", "/w/a")];
    let different: &[(&str, &str)] = &[
        ("work/repo", "work/other"),
        // Case and Unicode form are the volume's business, never the spelling's.
        ("/w/Repo", "/w/repo"),
        ("/w/caf\u{e9}", "/w/cafe\u{301}"),
        // `..` isn't resolved lexically: `/w/link/..` needn't be `/w`.
        ("/w/a/../b", "/w/b"),
        ("/w/a", "w/a"),
    ];
    for (a, b) in same {
        assert!(same_spelling(Path::new(a), Path::new(b)), "{a} = {b}");
    }
    for (a, b) in different {
        assert!(!same_spelling(Path::new(a), Path::new(b)), "{a} != {b}");
    }
    assert!(is_within(Path::new("/w/a/b"), Path::new("/w/a/")));
    assert!(is_within(Path::new("/w/a"), Path::new("/w/a")));
    assert!(!is_within(Path::new("/w/ab"), Path::new("/w/a")));
    assert!(!is_within(Path::new("/w/A/b"), Path::new("/w/a")));
    assert!(!is_within(Path::new("/w/a"), Path::new("")));
}

#[cfg(windows)]
#[test]
fn windows_spelling_rules() {
    let same: &[(&str, &str)] = &[
        (r"C:\work\repo", "c:/work/repo/"),
        (r"\\?\C:\work\repo", r"C:\work\repo"),
        (r"\\.\C:\work\repo", r"c:\work\repo"),
        (r"\\?\UNC\server\share\repo", r"\\Server\SHARE\repo"),
        (r"\\wsl$\Ubuntu\home", r"\\wsl.localhost\ubuntu\home"),
    ];
    let different: &[(&str, &str)] =
        &[(r"C:\Work\repo", r"C:\work\repo"), (r"\\server\share\Repo", r"\\server\share\repo"), (r"C:\x", r"C:x"), (r"C:\x", r"D:\x")];
    for (a, b) in same {
        assert!(same_spelling(Path::new(a), Path::new(b)), "{a} = {b}");
    }
    for (a, b) in different {
        assert!(!same_spelling(Path::new(a), Path::new(b)), "{a} != {b}");
    }
    assert!(is_within(Path::new(r"c:\work\repo\src"), Path::new(r"\\?\C:\work\repo")));
}

#[cfg(windows)]
#[test]
fn plain_drops_the_verbatim_prefix_only_when_it_means_the_same() {
    assert_eq!(plain(Path::new(r"\\?\C:\work\repo")), Path::new(r"C:\work\repo"));
    assert_eq!(plain(Path::new(r"\\?\UNC\server\share\repo")), Path::new(r"\\server\share\repo"));
    assert_eq!(plain(Path::new(r"C:\work")), Path::new(r"C:\work"));
    for kept in [r"\\?\C:\work\name.", r"\\?\C:\work\name \x", r"\\?\C:\work\NUL", r"\\?\C:\aux.txt\x", r"\\?\C:\w\COM¹"] {
        assert_eq!(plain(Path::new(kept)), Path::new(kept), "{kept}");
    }
    assert_eq!(plain(Path::new(r"\\?\C:\w\COM10")), Path::new(r"C:\w\COM10"));
}

/// The short 8.3 alias of a folder, when the volume makes them (CI runners do; many developer
/// machines have them turned off).
#[cfg(windows)]
fn short_name(p: &Path) -> Option<PathBuf> {
    let out = std::process::Command::new("cmd").arg("/c").arg(format!("for %I in (\"{}\") do @echo %~sI", p.display())).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    let short = PathBuf::from(s);
    (short != p && short.exists()).then_some(short)
}

#[cfg(windows)]
#[test]
fn resolve_expands_short_names() {
    let root = dir("short");
    let long = root.join("a folder with a long name");
    std::fs::create_dir_all(&long).unwrap();
    match short_name(&long) {
        Some(short) => {
            assert_eq!(resolve(&short), long);
            assert_eq!(resolve(&short.join("gone")), long.join("gone"));
            assert!(same_folder(&short, &long));
            assert_eq!(FolderId::of(&short), FolderId::of(&long));
        }
        None => eprintln!("note: this volume makes no 8.3 names; nothing to expand"),
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_folder_is_itself_under_a_link_and_not_its_neighbour() {
    let root = dir("identity");
    let (a, b) = (root.join("a"), root.join("b"));
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    let alias = alias_of(&a).expect("made a link");
    assert!(FolderId::of(&a).is_some());
    assert_eq!(FolderId::of(&alias), FolderId::of(&a));
    assert_ne!(FolderId::of(&a), FolderId::of(&b));
    assert!(same_folder(&alias, &a) && same_folder(&a, &a.join(".")));
    assert!(!same_folder(&alias, &b));
    // Paths that don't exist compare by their resolved spelling.
    assert_eq!(FolderId::of(&root.join("gone")), None);
    assert!(same_folder(&alias.join("gone"), &a.join("gone")));
    assert!(!same_folder(&a.join("gone"), &b.join("gone")));
    let _ = std::fs::remove_dir(&alias).or_else(|_| std::fs::remove_file(&alias));
    let _ = std::fs::remove_dir_all(&root);
}

/// `App` then `app` in `parent`: two folders where the volume tells case apart, one folder under
/// two spellings where it doesn't. Either way the identity follows the disk. Returns whether the
/// volume told them apart.
pub(crate) fn case_twins_follow_the_volume(parent: &Path) -> bool {
    let (upper, lower) = (parent.join("App"), parent.join("app"));
    std::fs::create_dir_all(&upper).unwrap();
    let apart = match std::fs::create_dir(&lower) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(e) => panic!("could not make {}: {e}", lower.display()),
    };
    let ids = (FolderId::of(&upper), FolderId::of(&lower));
    assert!(ids.0.is_some() && ids.1.is_some(), "{ids:?}");
    if apart {
        assert_ne!(ids.0, ids.1, "two folders");
        assert!(!same_folder(&upper, &lower));
        assert!(!same_spelling(&resolve(&upper), &resolve(&lower)));
    } else {
        assert_eq!(ids.0, ids.1, "one folder");
        assert!(same_folder(&upper, &lower));
    }
    apart
}

#[test]
fn case_twins_on_the_temp_volume() {
    let root = dir("case");
    let apart = case_twins_follow_the_volume(&root);
    // ext4 on the Linux runners keeps them apart.
    if cfg!(target_os = "linux") {
        assert!(apart, "App and app are two folders on ext4");
    }
    eprintln!("note: the temp volume {} case", if apart { "tells apart" } else { "ignores" });
    let _ = std::fs::remove_dir_all(&root);
}

/// A real case-sensitive volume or folder (CI: a case-sensitive APFS image on macOS, a folder
/// with `fsutil file setCaseSensitiveInfo` on Windows), named by `GLOWFLY_TEST_CS_DIR`.
#[test]
fn case_twins_on_a_case_sensitive_volume() {
    let Some(base) = std::env::var_os("GLOWFLY_TEST_CS_DIR").map(PathBuf::from) else {
        eprintln!("note: GLOWFLY_TEST_CS_DIR is not set; skipping the case-sensitive volume check");
        return;
    };
    let root = base.join(format!("glowfly-paths-cs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    assert!(case_twins_follow_the_volume(&root), "{} tells case apart", base.display());
    // A link to one twin is that twin, not the other.
    let alias = alias_of(&root.join("App")).expect("made a link");
    assert!(same_folder(&alias, &root.join("App")) && !same_folder(&alias, &root.join("app")));
    let _ = std::fs::remove_dir(&alias).or_else(|_| std::fs::remove_file(&alias));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn saved_paths_round_trip() {
    let plain_paths = [r"C:\work\repo", "/home/me/café", "/w/~enc:literal"];
    for p in plain_paths {
        assert_eq!(decode(&encode(Path::new(p))), Path::new(p), "{p}");
    }
    // Plain text is saved as it is, so files from earlier versions read the same.
    assert_eq!(encode(Path::new("/home/me/café")), "/home/me/café");
    assert_eq!(decode("C:/old/entry/"), Path::new("C:/old/entry/"));
    // One line per path, whatever is in it.
    for odd in ["/w/two\nlines", "/w/tab\there", "~enc:/w/start", "/w/100%"] {
        let saved = encode(Path::new(odd));
        assert!(!saved.contains('\n') && !saved.contains('\t'), "{saved}");
        assert_eq!(decode(&saved), Path::new(odd), "{saved}");
    }
}

#[cfg(unix)]
#[test]
fn saved_paths_keep_bytes_that_are_not_utf8() {
    use std::os::unix::ffi::OsStrExt;
    let p = Path::new(OsStr::from_bytes(b"/w/caf\xe9/x"));
    let saved = encode(p);
    assert!(saved.is_ascii() && saved.starts_with("~enc:"), "{saved}");
    assert_eq!(decode(&saved), p);
}

#[cfg(windows)]
#[test]
fn saved_paths_keep_lone_surrogates() {
    use std::os::windows::ffi::OsStringExt;
    let wide: Vec<u16> = r"C:\w\".encode_utf16().chain([0xD800, b'x' as u16]).chain("😀".encode_utf16()).collect();
    let p = PathBuf::from(OsString::from_wide(&wide));
    let saved = encode(&p);
    assert!(saved.starts_with("~enc:"), "{saved}");
    assert_eq!(decode(&saved), p);
}

#[test]
fn typed_folders_expand_home_and_must_be_absolute() {
    let home = temp_dir().join("home");
    assert_eq!(expand_home(" ~ ", Some(&home)), home);
    assert_eq!(expand_home("~/src/x", Some(&home)), home.join("src/x"));
    assert_eq!(expand_home("~user/src", Some(&home)), Path::new("~user/src"), "another user's home isn't guessed");
    assert_eq!(expand_home("~/src", None), Path::new("~/src"));
    assert_eq!(expand_home("rel/dir", Some(&home)), Path::new("rel/dir"));
    assert!(typed_is_absolute("~") && typed_is_absolute("~/src"));
    assert!(typed_is_absolute(&home.display().to_string()));
    for relative in ["src", "./src", "~user", "~x/y", ""] {
        assert!(!typed_is_absolute(relative), "{relative}");
    }
    assert_eq!(typed_is_absolute(r"~\src"), cfg!(windows));
    if cfg!(windows) {
        assert_eq!(expand_home(r"~\src", Some(&home)), home.join("src"));
        assert!(!typed_is_absolute(r"\src"), "a root without a drive is relative to the current drive");
    }
}
