//! Tests only: writes the generated parts of THIRD-PARTY-NOTICES.md from the crates in
//! `Cargo.lock`: which crates the release binaries compile in (`cargo tree` per release
//! target, normal dependencies only), each one's licence texts from its source in the cargo
//! registry (`cargo metadata`), grouped by identical text, plus libgit2's COPYING and egui's
//! built-in font licences. Runs only under `UPDATE_NOTICES=1` (cargo may download sources).

use super::spdx;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The release targets (`.github/workflows/release.yml`'s build matrix; a test keeps them in step).
pub const TARGETS: &[&str] = &[
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
];

/// Build dependencies whose output the binary embeds: glowfly-core's build script packs
/// two-face's syntax definitions into the binary.
const EMBEDDED_BUILD_OUTPUT: &[&str] = &["two-face"];

/// Licence files a crate keeps below its root, for code it compiles in: the zlib that
/// libz-sys builds when the system has none, and the code ring takes from other projects.
const EXTRA_FILES: &[(&str, &[&str])] =
    &[("libz-sys", &["src/zlib/LICENSE"]), ("ring", &["src/polyfill/once_cell/LICENSE-MIT", "third_party/fiat/LICENSE"])];

/// Licences a crate names whose texts are in another section: epaint_default_fonts' fonts.
const ELSEWHERE: &[(&str, &[&str])] = &[("epaint_default_fonts", &["OFL-1.1", "Ubuntu-font-1.0"])];

/// The licence files of egui's built-in fonts (epaint_default_fonts' `fonts/`), and their fonts.
pub const EGUI_FONT_FILES: &[(&str, &str)] = &[
    ("UFL.txt", "Ubuntu Light"),
    ("Hack-Regular.txt", "Hack"),
    ("OFL.txt", "Noto Emoji"),
    ("emoji-icon-font-mit-license.txt", "emoji-icon-font"),
];

/// A crate from `cargo metadata`.
pub struct Package {
    pub name: String,
    pub version: String,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub authors: Vec<String>,
    pub dir: PathBuf,
}

/// The crate `name` at `version`.
pub type Id = (String, String);

fn cargo(root: &Path, args: &[&str]) -> String {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo).args(args).current_dir(root).output().expect("run cargo");
    assert!(out.status.success(), "cargo {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).expect("cargo prints UTF-8")
}

/// Every registry crate in the workspace's dependency graph, by name and version.
pub fn packages(root: &Path) -> BTreeMap<Id, Package> {
    let json: Value = serde_json::from_str(&cargo(root, &["metadata", "--format-version", "1", "--locked"])).expect("metadata JSON");
    let mut out = BTreeMap::new();
    for p in json["packages"].as_array().expect("packages") {
        if p["source"].is_null() {
            continue; // a workspace member
        }
        let s = |k: &str| p[k].as_str().map(str::to_owned);
        let manifest = PathBuf::from(s("manifest_path").expect("manifest_path"));
        let pkg = Package {
            name: s("name").expect("name"),
            version: s("version").expect("version"),
            license: s("license"),
            license_file: s("license_file"),
            authors: p["authors"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
            dir: manifest.parent().expect("a crate folder").to_path_buf(),
        };
        out.insert((pkg.name.clone(), pkg.version.clone()), pkg);
    }
    out
}

/// The registry crates compiled into the release binaries for any release target.
pub fn shipped(root: &Path, packages: &BTreeMap<Id, Package>) -> BTreeSet<Id> {
    let mut out = BTreeSet::new();
    for target in TARGETS {
        let tree = cargo(root, &["tree", "--locked", "-e", "normal", "--target", target, "-p", "glowfly", "--prefix", "none", "-f", "{p}"]);
        for line in tree.lines() {
            let mut words = line.split_whitespace();
            let (Some(name), Some(version)) = (words.next(), words.next()) else { continue };
            let id = (name.to_owned(), version.trim_start_matches('v').to_owned());
            if packages.contains_key(&id) {
                out.insert(id);
            }
        }
    }
    for name in EMBEDDED_BUILD_OUTPUT {
        out.extend(packages.keys().filter(|(n, _)| n == name).cloned());
    }
    out
}

/// A licence text as written: LF line ends, no trailing spaces, no blank lines around it.
pub fn tidy(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    lines.join("\n").trim_matches('\n').to_owned()
}

fn read(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    tidy(&String::from_utf8_lossy(&bytes).replace('\u{feff}', ""))
}

/// A crate's licence files: files at its root named like one, and in a LICENSES folder.
fn licence_files(p: &Package) -> Vec<(String, String)> {
    let looks_like = |name: &str| {
        let n = name.to_lowercase();
        ["licen", "copying", "notice", "copyright", "unlicense"].iter().any(|w| n.starts_with(w))
            || n.contains("-license")
            || n.contains("_license")
    };
    let mut files = BTreeSet::new();
    for entry in std::fs::read_dir(&p.dir).expect("crate folder").flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        // A `.spdx` file is metadata, not a licence text.
        if path.is_file() && looks_like(&name) && !name.ends_with(".spdx") {
            files.insert(name);
        } else if path.is_dir() && name.eq_ignore_ascii_case("licenses") {
            for inner in std::fs::read_dir(&path).expect("LICENSES").flatten() {
                files.insert(format!("{name}/{}", inner.file_name().to_string_lossy()));
            }
        }
    }
    if let Some(f) = &p.license_file {
        files.insert(f.replace('\\', "/"));
    }
    files.into_iter().map(|f| (read(&p.dir.join(&f)), f)).collect()
}

/// The standard text of `id` with a copyright line from the crate's authors, for a crate that
/// ships no file of a licence it names.
fn standard_text(root: &Path, p: &Package, id: &str) -> String {
    let path = root.join("crates/glowfly-core/notices/spdx").join(format!("{id}.txt"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        return format!("MISSING STANDARD TEXT {id}: {} {} names it without a file; add {}", p.name, p.version, path.display());
    };
    let names: Vec<&str> = p.authors.iter().map(|a| a.split('<').next().unwrap_or(a).trim()).filter(|a| !a.is_empty()).collect();
    let holders = if names.is_empty() { format!("the {} authors", p.name) } else { names.join(", ") };
    format!("Copyright (c) {holders}\n\n{}", tidy(&text))
}

/// The texts (label, text) a crate's notice needs.
pub fn crate_texts(root: &Path, p: &Package) -> Vec<(String, String)> {
    let expr = spdx::parse(p.license.as_deref().unwrap_or_else(|| panic!("{} {} has no license field", p.name, p.version)))
        .unwrap_or_else(|e| panic!("{} {}: {e}", p.name, p.version));
    let files: Vec<(Option<&str>, String, String)> =
        licence_files(p).into_iter().map(|(text, f)| (spdx::classify(&text), f, text)).collect();
    let has_text = |id: &str| files.iter().any(|(c, _, text)| *c == Some(id) || (c.is_none() && spdx::holds(text, id)));
    let chosen = spdx::choose(&expr, &has_text);
    let elsewhere: &[&str] = ELSEWHERE.iter().find(|(n, _)| *n == p.name).map(|(_, ids)| *ids).unwrap_or(&[]);
    let mut out = Vec::new();
    for (class, file, text) in &files {
        match class {
            Some(id) if chosen.iter().any(|c| c == id) => out.push(((*id).to_owned(), text.clone())),
            Some(_) => {}
            None => out.push((file.clone(), text.clone())),
        }
    }
    for id in &chosen {
        if !elsewhere.contains(&id.as_str()) && !has_text(id) {
            out.push((id.clone(), standard_text(root, p, id)));
        }
    }
    for f in EXTRA_FILES.iter().filter(|(n, _)| *n == p.name).flat_map(|(_, f)| f.iter()) {
        let text = read(&p.dir.join(f));
        out.push((spdx::classify(&text).map_or_else(|| (*f).to_owned(), str::to_owned), text));
    }
    out
}

/// One `<details>` block.
pub fn block(summary: &str, text: &str) -> String {
    assert!(!text.lines().any(|l| l.trim_start().starts_with("```")), "{summary}: a line of the text starts a fence");
    format!("<details>\n<summary>{summary}</summary>\n\n````text\n{text}\n````\n</details>\n\n")
}

/// The crate section: one block per distinct text, labelled with its licence and the crates
/// that carry it (`MIT: anyhow 1.0.98, serde 1.0.219`).
pub fn crate_section(root: &Path, packages: &BTreeMap<Id, Package>, shipped: &BTreeSet<Id>) -> String {
    let mut groups: BTreeMap<String, (String, BTreeSet<Id>)> = BTreeMap::new();
    for id in shipped {
        for (label, text) in crate_texts(root, &packages[id]) {
            groups.entry(text).or_insert_with(|| (label, BTreeSet::new())).1.insert(id.clone());
        }
    }
    let mut blocks: Vec<(String, String)> = groups
        .into_iter()
        .map(|(text, (label, crates))| {
            let names: Vec<String> = crates.iter().map(|(n, v)| format!("{n} {v}")).collect();
            (format!("{label}: {}", names.join(", ")), text)
        })
        .collect();
    blocks.sort();
    let mut seen = BTreeMap::<String, usize>::new();
    let mut md = String::new();
    for (summary, text) in blocks {
        let n = seen.entry(summary.clone()).or_default();
        *n += 1;
        let summary = if *n == 1 { summary } else { format!("{summary} ({n})") };
        md.push_str(&block(&summary, &text));
    }
    let missing: Vec<&str> = md.lines().filter(|l| l.starts_with("MISSING STANDARD TEXT")).collect();
    assert!(
        missing.is_empty(),
        "{}",
        missing.join(
            "
"
        )
    );
    md
}

/// The one package called `name` (panics when there is none or several).
pub fn only<'a>(packages: &'a BTreeMap<Id, Package>, name: &str) -> &'a Package {
    let mut found = packages.values().filter(|p| p.name == name);
    let p = found.next().unwrap_or_else(|| panic!("no {name} in Cargo.lock"));
    assert!(found.next().is_none(), "two versions of {name}");
    p
}

/// libgit2's COPYING, verbatim, and where its source is.
pub fn libgit2_section(sys: &Package) -> String {
    let libgit2 = sys.version.split_once('+').map_or(sys.version.as_str(), |(_, v)| v);
    let copying = read(&sys.dir.join("libgit2/COPYING"));
    let source = format!(
        "Glowfly links libgit2 {libgit2} statically: the copy that the libgit2-sys {v} crate vendors,\n\
         built by that crate. The complete source of that libgit2 is in the libgit2-sys {v}\n\
         package on crates.io (https://crates.io/crates/libgit2-sys/{v}) and upstream at\n\
         https://github.com/libgit2/libgit2/tree/v{libgit2}. Glowfly's own complete source for each\n\
         release is at that release's tag (https://github.com/bg9m9r/glowfly/tags).\n\
         \n\
         You may rebuild Glowfly with a modified libgit2: point the libgit2-sys dependency at your\n\
         copy (for example with a [patch.crates-io] section in Glowfly's Cargo.toml) and run\n\
         cargo build --release.",
        v = sys.version
    );
    let mut md = block(&format!("libgit2 {libgit2}: COPYING (libgit2-sys {})", sys.version), &copying);
    md.push_str(&block(&format!("libgit2 {libgit2}: source and relinking (libgit2-sys {})", sys.version), &source));
    md
}

/// The licences of egui's built-in fonts, from epaint_default_fonts' `fonts/` folder.
pub fn egui_fonts_section(fonts: &Package) -> String {
    let dir = fonts.dir.join("fonts");
    let mut found: Vec<String> = std::fs::read_dir(&dir)
        .expect("fonts folder")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".txt"))
        .collect();
    found.sort();
    let known: Vec<&str> = EGUI_FONT_FILES.iter().map(|(f, _)| *f).collect();
    assert!(
        found.iter().all(|f| known.contains(&f.as_str())),
        "epaint_default_fonts has licence files {found:?}; name their fonts in EGUI_FONT_FILES"
    );
    let mut md = String::new();
    for (file, font) in EGUI_FONT_FILES {
        let summary = format!("{font} (epaint_default_fonts {}, fonts/{file})", fonts.version);
        md.push_str(&block(&summary, &read(&dir.join(file))));
    }
    md
}
