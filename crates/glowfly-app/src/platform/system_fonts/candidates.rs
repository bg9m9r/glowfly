//! Where each system keeps its CJK and Hangul fonts, as a list of files to try in order:
//! `%SystemRoot%\Fonts` on Windows, the system, local and user font folders on macOS, and on
//! Linux and the BSDs the folders fontconfig reads by default (the XDG data folders' `fonts`,
//! `~/.fonts`, and the host's fonts inside Flatpak), searched for known file names. The fonts
//! made for the system language's script variant come first (Simplified or Traditional
//! Chinese, Japanese, Korean), so Han characters take the shapes its readers expect.
//! Everything but the folder listing is pure, so the order is tested on every system.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// A face of a font file to try.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub path: PathBuf,
    pub index: u32,
}

/// The script variant a font is made for, and the system language's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Simplified,
    Traditional,
    Japanese,
    Korean,
}

use Variant::{Japanese, Korean, Simplified, Traditional};

/// Which face of a collection to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Faces {
    First,
    /// A pan-CJK collection (Noto Sans CJK, Source Han Sans): Japanese, Korean, Simplified,
    /// Traditional, Hong Kong; each has every script, with its variant's Han shapes.
    PanCjk,
}

/// A known font file.
struct Known {
    file: &'static str,
    made_for: Option<Variant>,
    faces: Faces,
}

const fn k(file: &'static str, made_for: Option<Variant>, faces: Faces) -> Known {
    Known { file, made_for, faces }
}

/// Windows' own: Microsoft YaHei and JhengHei, Yu Gothic, Meiryo, Malgun Gothic, MS Gothic,
/// SimSun, MingLiU and Gulim.
const WINDOWS: &[Known] = &[
    k("msyh.ttc", Some(Simplified), Faces::First),
    k("YuGothR.ttc", Some(Japanese), Faces::First),
    k("malgun.ttf", Some(Korean), Faces::First),
    k("msjh.ttc", Some(Traditional), Faces::First),
    k("meiryo.ttc", Some(Japanese), Faces::First),
    k("msgothic.ttc", Some(Japanese), Faces::First),
    k("simsun.ttc", Some(Simplified), Faces::First),
    k("mingliu.ttc", Some(Traditional), Faces::First),
    k("gulim.ttc", Some(Korean), Faces::First),
];

/// Stands for Hiragino Sans's file, whose name is Japanese (`hiragino_sans_file`).
const HIRAGINO_SANS: &str = "Hiragino Sans W3";

/// The file name of Hiragino Sans W3: "Hiragino Kaku Gothic W3.ttc" in katakana and kanji.
/// Built from code points, so the glyph check, which reads string literals as text the app
/// shows, doesn't count a file name.
fn hiragino_sans_file() -> String {
    const NAME: [u32; 9] = [0x30D2, 0x30E9, 0x30AE, 0x30CE, 0x89D2, 0x30B4, 0x30B7, 0x30C3, 0x30AF];
    NAME.iter().filter_map(|c| char::from_u32(*c)).chain(" W3.ttc".chars()).collect()
}

/// macOS's: PingFang, Hiragino Sans, Hiragino Sans GB, Apple SD Gothic Neo, then the older
/// Heiti, AppleGothic and Arial Unicode.
const MACOS: &[Known] = &[
    k("PingFang.ttc", Some(Simplified), Faces::First),
    k(HIRAGINO_SANS, Some(Japanese), Faces::First),
    k("Hiragino Sans GB.ttc", Some(Simplified), Faces::First),
    k("AppleSDGothicNeo.ttc", Some(Korean), Faces::First),
    k("STHeiti Light.ttc", Some(Simplified), Faces::First),
    k("AppleGothic.ttf", Some(Korean), Faces::First),
    k("Arial Unicode.ttf", None, Faces::First),
];

/// The usual Linux packages: Noto Sans CJK (the collection, then the per-language files, then
/// the variable collection, whose default instance egui draws), Source Han Sans, WenQuanYi,
/// Droid Sans Fallback, IPA and Takao Gothic, Nanum and Un Dotum.
const LINUX: &[Known] = &[
    k("NotoSansCJK-Regular.ttc", None, Faces::PanCjk),
    k("NotoSansCJKsc-Regular.otf", Some(Simplified), Faces::First),
    k("NotoSansCJKtc-Regular.otf", Some(Traditional), Faces::First),
    k("NotoSansCJKjp-Regular.otf", Some(Japanese), Faces::First),
    k("NotoSansCJKkr-Regular.otf", Some(Korean), Faces::First),
    k("SourceHanSans-Regular.ttc", None, Faces::PanCjk),
    k("NotoSansCJK-VF.ttc", None, Faces::PanCjk),
    k("wqy-microhei.ttc", Some(Simplified), Faces::First),
    k("wqy-zenhei.ttc", Some(Simplified), Faces::First),
    k("DroidSansFallbackFull.ttf", None, Faces::First),
    k("DroidSansFallback.ttf", None, Faces::First),
    k("ipagp.ttf", Some(Japanese), Faces::First),
    k("ipaexg.ttf", Some(Japanese), Faces::First),
    k("TakaoPGothic.ttf", Some(Japanese), Faces::First),
    k("NanumGothic.ttf", Some(Korean), Faces::First),
    k("UnDotum.ttf", Some(Korean), Faces::First),
];

/// The variant a locale like "ja-JP", "zh-Hant-TW", "zh_HK.UTF-8" or "ko" reads.
pub fn variant_of(locale: &str) -> Option<Variant> {
    let lower = locale.to_ascii_lowercase().replace('_', "-");
    let mut parts = lower.split(['-', '.', '@']);
    match parts.next()? {
        "ja" => Some(Japanese),
        "ko" => Some(Korean),
        "zh" => {
            let rest: Vec<&str> = parts.collect();
            let traditional = rest.iter().any(|p| matches!(*p, "hant" | "tw" | "hk" | "mo"));
            let simplified = rest.contains(&"hans");
            Some(if traditional && !simplified { Traditional } else { Simplified })
        }
        _ => None,
    }
}

/// `table` with the fonts made for `variant` first, otherwise in the table's order.
fn ranked(table: &'static [Known], variant: Option<Variant>) -> Vec<&'static Known> {
    let mut out: Vec<&Known> = table.iter().collect();
    if let Some(v) = variant {
        out.sort_by_key(|k| k.made_for != Some(v));
    }
    out
}

fn index(faces: Faces, variant: Option<Variant>) -> u32 {
    match (faces, variant) {
        (Faces::First, _) => 0,
        (Faces::PanCjk, Some(Japanese)) => 0,
        (Faces::PanCjk, Some(Korean)) => 1,
        (Faces::PanCjk, Some(Simplified) | None) => 2,
        (Faces::PanCjk, Some(Traditional)) => 3,
    }
}

/// Windows: each known file in `fonts` (`%SystemRoot%\Fonts`).
pub fn windows(fonts: &Path, variant: Option<Variant>) -> Vec<Candidate> {
    ranked(WINDOWS, variant).into_iter().map(|k| Candidate { path: fonts.join(k.file), index: index(k.faces, variant) }).collect()
}

/// macOS: each known file in each font folder, the system's first, then `home`'s.
pub fn macos(home: Option<&Path>, variant: Option<Variant>) -> Vec<Candidate> {
    let mut dirs: Vec<PathBuf> =
        ["/System/Library/Fonts", "/System/Library/Fonts/Supplemental", "/Library/Fonts"].iter().map(PathBuf::from).collect();
    dirs.extend(home.map(|h| h.join("Library").join("Fonts")));
    let mut out = Vec::new();
    for k in ranked(MACOS, variant) {
        let file = if k.file == HIRAGINO_SANS { hiragino_sans_file() } else { k.file.to_owned() };
        out.extend(dirs.iter().map(|d| Candidate { path: d.join(&file), index: index(k.faces, variant) }));
    }
    out
}

/// The folders fontconfig reads by default: `$XDG_DATA_HOME/fonts` (else `~/.local/share/fonts`),
/// `~/.fonts`, `fonts` in each of `$XDG_DATA_DIRS` (else `/usr/local/share` and `/usr/share`),
/// and the host's folders Flatpak shows the sandbox. Values that don't start with `/` are
/// ignored (checked as text, so the rule is the same when tested on Windows).
pub fn unix_dirs(var: impl Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    let abs = |v: &OsString| v.to_string_lossy().starts_with('/');
    let home = var("HOME").filter(abs).map(PathBuf::from);
    let mut out = Vec::new();
    match var("XDG_DATA_HOME").filter(abs) {
        Some(data) => out.push(PathBuf::from(data).join("fonts")),
        None => out.extend(home.as_ref().map(|h| h.join(".local").join("share").join("fonts"))),
    }
    out.extend(home.as_ref().map(|h| h.join(".fonts")));
    let shared = var("XDG_DATA_DIRS").map(|v| v.to_string_lossy().into_owned()).filter(|v| !v.is_empty());
    let shared = shared.unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    // A part that isn't UTF-8 (shown with U+FFFD) is skipped rather than guessed at.
    let parts = shared.split(':').filter(|p| p.starts_with('/') && !p.contains(char::REPLACEMENT_CHARACTER));
    out.extend(parts.map(|p| PathBuf::from(p).join("fonts")));
    out.extend(["/usr/share/fonts", "/run/host/fonts", "/run/host/user-fonts"].map(PathBuf::from));
    let mut seen = Vec::new();
    out.retain(|p| {
        let new = !seen.contains(p);
        seen.push(p.clone());
        new
    });
    out
}

/// Linux and the BSDs: the known files found under `dirs` (`unix_dirs`), in rank order.
pub fn unix(dirs: &[PathBuf], variant: Option<Variant>) -> Vec<Candidate> {
    let names: Vec<&str> = LINUX.iter().map(|k| k.file).collect();
    let found = walk(dirs, &names);
    let mut out = Vec::new();
    for k in ranked(LINUX, variant) {
        out.extend(found.iter().filter(|(n, _)| *n == k.file).map(|(_, p)| Candidate { path: p.clone(), index: index(k.faces, variant) }));
    }
    out
}

/// How deep and how far the folder search goes: font folders are a few levels deep.
const MAX_DEPTH: usize = 6;
const MAX_ENTRIES: usize = 50_000;

/// Each file under `dirs` whose name is one of `names`, as (name, path), in the order found.
fn walk(dirs: &[PathBuf], names: &[&str]) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut seen = 0;
    let mut stack: Vec<(PathBuf, usize)> = dirs.iter().rev().map(|d| (d.clone(), 0)).collect();
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            seen += 1;
            if seen > MAX_ENTRIES {
                return out;
            }
            let name = entry.file_name();
            let Some(text) = name.to_str() else { continue };
            let path = entry.path();
            // Follows links: font folders often link files or whole folders (depth stops loops).
            if path.is_dir() {
                if depth < MAX_DEPTH {
                    subdirs.push((path, depth + 1));
                }
            } else if names.contains(&text) {
                out.push((text.to_owned(), path));
            }
        }
        subdirs.sort();
        stack.extend(subdirs.into_iter().rev());
    }
    out
}

/// The candidates for the running system, in the order to try them.
pub fn for_this_system() -> Vec<Candidate> {
    let variant = sys_locale::get_locale().as_deref().and_then(variant_of);
    if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").filter(|r| !r.is_empty()).map(PathBuf::from);
        root.map(|r| windows(&r.join("Fonts"), variant)).unwrap_or_default()
    } else if cfg!(target_os = "macos") {
        let home = std::env::var_os("HOME").map(PathBuf::from).filter(|h| h.is_absolute());
        macos(home.as_deref(), variant)
    } else {
        unix(&unix_dirs(|k| std::env::var_os(k)), variant)
    }
}
