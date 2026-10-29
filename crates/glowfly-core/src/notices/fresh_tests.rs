//! THIRD-PARTY-NOTICES.md is current: its generated sections match two-face's syntax
//! licences, and every registry crate in Cargo.lock is either in its crate section (at the
//! locked version) or listed in `crates/glowfly-core/notices/not-shipped.txt` as not compiled
//! into the release binaries. The check needs no network. Run with `UPDATE_NOTICES=1` to
//! regenerate the sections and the list after `Cargo.lock` or two-face changes (cargo may then
//! download the sources of other targets' crates).

use super::generate;
use super::{APPIMAGE, CRATES, EGUI_FONTS, LIBGIT2, SECTIONS, SYNTAX, Section, section};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const NOT_SHIPPED: &str = "crates/glowfly-core/notices/not-shipped.txt";
const FIX: &str = "run UPDATE_NOTICES=1 cargo test -p glowfly-core notice";

/// The workspace root, from the manifest folder at run time: a binary built in another
/// worktree (shared target dir) keeps that worktree's compile-time path.
fn root() -> PathBuf {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into());
    Path::new(&dir).join("../..")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())).replace("\r\n", "\n")
}

/// The byte range between section `s`'s BEGIN line and its END marker.
fn body(md: &str, s: &Section) -> std::ops::Range<usize> {
    let begin = md.find(s.begin).unwrap_or_else(|| panic!("the notice has {}", s.begin)) + s.begin.len() + 1;
    let end = md.find(s.end).unwrap_or_else(|| panic!("the notice has {}", s.end));
    assert!(begin <= end, "{} comes before {}", s.begin, s.end);
    begin..end
}

fn replaced(md: &str, s: &Section, content: &str) -> String {
    let r = body(md, s);
    format!("{}{content}{}", &md[..r.start], &md[r.end..])
}

fn syntax_md() -> String {
    let mut md = String::new();
    for license in two_face::acknowledgement::listing().for_syntaxes() {
        license.write_md(&mut md);
    }
    md.replace("\r\n", "\n")
}

/// The registry crates in Cargo.lock.
fn locked(root: &Path) -> BTreeSet<generate::Id> {
    let lock: toml::Table = toml::from_str(&read(&root.join("Cargo.lock"))).expect("Cargo.lock parses");
    let packages = lock["package"].as_array().expect("[[package]]");
    let s = |p: &toml::Value, k: &str| p.get(k).and_then(|v| v.as_str()).map(str::to_owned);
    packages.iter().filter(|p| s(p, "source").is_some()).map(|p| (s(p, "name").expect("name"), s(p, "version").expect("version"))).collect()
}

/// The crates a crate-section summary names (`MIT: anyhow 1.0.98, serde 1.0.219 (2)`).
fn crates_of(summary: &str) -> Vec<generate::Id> {
    let list = summary.split_once(": ").map_or("", |(_, l)| l);
    list.split(", ")
        .filter_map(|c| {
            let mut w = c.split_whitespace();
            Some((w.next()?.to_owned(), w.next()?.to_owned()))
        })
        .collect()
}

fn not_shipped(text: &str) -> BTreeSet<generate::Id> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut w = l.split_whitespace();
            (w.next().expect("a name").to_owned(), w.next().unwrap_or_else(|| panic!("{l}: no version")).to_owned())
        })
        .collect()
}

fn update(root: &Path, md: &str) -> (String, String) {
    let packages = generate::packages(root);
    let shipped = generate::shipped(root, &packages);
    let mut md = replaced(md, &SYNTAX, &syntax_md());
    md = replaced(&md, &EGUI_FONTS, &generate::egui_fonts_section(generate::only(&packages, "epaint_default_fonts")));
    md = replaced(&md, &LIBGIT2, &generate::libgit2_section(generate::only(&packages, "libgit2-sys")));
    md = replaced(&md, &CRATES, &generate::crate_section(root, &packages, &shipped));
    let mut list = String::from(
        "# The registry crates in Cargo.lock that the release binaries don't compile in: build-only,\n\
         # test-only or for other platforms. Generated with THIRD-PARTY-NOTICES.md's crate section\n\
         # (UPDATE_NOTICES=1 cargo test -p glowfly-core notice); src/notices/fresh_tests.rs checks both.\n",
    );
    for (name, version) in locked(root).difference(&shipped) {
        list.push_str(&format!("{name} {version}\n"));
    }
    (md, list)
}

#[test]
fn third_party_notices_are_current() {
    let root = root();
    let path = root.join("THIRD-PARTY-NOTICES.md");
    let list_path = root.join(NOT_SHIPPED);
    if std::env::var_os("UPDATE_NOTICES").is_some() {
        let (md, list) = update(&root, &read(&path));
        std::fs::write(&path, md).expect("write THIRD-PARTY-NOTICES.md");
        std::fs::write(&list_path, list).expect("write not-shipped.txt");
    }
    let md = read(&path);
    let starts: Vec<usize> = SECTIONS.iter().map(|s| body(&md, s).start).collect();
    assert!(starts.is_sorted(), "the sections are in SECTIONS order");
    assert!(md.contains("PowerShell.sublime-syntax"), "the notice names Glowfly's own grammar");
    assert!(md[body(&md, &SYNTAX)] == syntax_md(), "the syntax licenses are out of date: {FIX}");

    let lock = locked(&root);
    let version = |name: &str| lock.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone()).unwrap_or_else(|| panic!("{name} is locked"));
    let git2 = format!("(libgit2-sys {})", version("libgit2-sys"));
    let libgit2 = section(&md, &LIBGIT2);
    assert!(libgit2.len() == 2 && libgit2.iter().all(|n| n.source.contains(&git2)), "libgit2's COPYING is not that of {git2}: {FIX}");
    let fonts = format!("(epaint_default_fonts {},", version("epaint_default_fonts"));
    let egui = section(&md, &EGUI_FONTS);
    assert!(
        egui.len() == generate::EGUI_FONT_FILES.len() && egui.iter().all(|n| n.source.contains(&fonts)),
        "egui's font licences are not those of {fonts}: {FIX}"
    );

    let listed: BTreeSet<generate::Id> = section(&md, &CRATES).iter().flat_map(|n| crates_of(&n.source)).collect();
    let skipped = not_shipped(&read(&list_path));
    let both: Vec<_> = listed.intersection(&skipped).collect();
    assert!(both.is_empty(), "crates both listed and not shipped: {both:?}: {FIX}");
    let known: BTreeSet<generate::Id> = listed.union(&skipped).cloned().collect();
    let missing: Vec<_> = lock.difference(&known).collect();
    let stale: Vec<_> = known.difference(&lock).collect();
    assert!(missing.is_empty() && stale.is_empty(), "crates without a notice: {missing:?}; no longer locked: {stale:?}: {FIX}");
}

#[test]
fn notice_targets_are_the_release_targets() {
    let workflow = read(&root().join(".github/workflows/release.yml"));
    let targets: Vec<&str> = workflow
        .lines()
        .filter_map(|l| l.split_once("target: "))
        .filter_map(|(_, r)| r.split_whitespace().next())
        .filter(|t| t.contains('-'))
        .collect();
    assert_eq!(targets, generate::TARGETS, "the notice generator's TARGETS follow release.yml's build matrix");
}

#[test]
fn appimage_notices_name_the_pinned_runtime() {
    let script = read(&root().join("scripts/package-linux.sh"));
    let pinned = script.lines().find_map(|l| l.strip_prefix("RUNTIME_VERSION=")).expect("RUNTIME_VERSION").trim_matches('"');
    let md = read(&root().join("THIRD-PARTY-NOTICES.md"));
    let notices = section(&md, &APPIMAGE);
    assert!(
        !notices.is_empty() && notices[0].source.contains(pinned),
        "the AppImage notices are for type2-runtime {pinned} (scripts/package-linux.sh)"
    );
}

#[test]
fn notice_summaries_name_their_crates() {
    let id = |n: &str, v: &str| (n.to_owned(), v.to_owned());
    assert_eq!(crates_of("MIT: anyhow 1.0.98, serde 1.0.219"), [id("anyhow", "1.0.98"), id("serde", "1.0.219")]);
    assert_eq!(crates_of("LICENSE: ring 0.17.14 (2)"), [id("ring", "0.17.14")]);
    assert_eq!(crates_of("Apache-2.0 WITH LLVM-exception: rustix 1.0.0"), [id("rustix", "1.0.0")]);
    assert_eq!(not_shipped("# head\ncc 1.2.0\n\nwinresource 0.1.20\n"), BTreeSet::from([id("cc", "1.2.0"), id("winresource", "0.1.20")]));
}
