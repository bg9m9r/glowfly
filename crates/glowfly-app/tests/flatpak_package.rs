//! The Flatpak manifest (packaging/flatpak/io.github.bg9m9r.glowfly.yml) and the workflows that
//! build and check its bundle, read line by line: the runtime, the sandbox permissions, the
//! pinned git source, an offline build, the icons the .deb installs too, git's licences and
//! notice (GIT-NOTICE.md) with its source attached to the release, and the release and
//! installer jobs.

use std::path::Path;

fn workspace_file(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
    // A Windows checkout may have CRLF line endings.
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display())).replace("\r\n", "\n")
}

fn manifest() -> String {
    workspace_file("packaging/flatpak/io.github.bg9m9r.glowfly.yml")
}

fn trimmed(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).collect()
}

/// The `- ` items of the top-level list `key:`, up to the next top-level key.
fn list<'a>(text: &'a str, key: &str) -> Vec<&'a str> {
    let mut lines = text.lines().skip_while(|l| *l != format!("{key}:")).skip(1);
    let mut out = Vec::new();
    for line in lines.by_ref() {
        if !line.starts_with(' ') && !line.is_empty() {
            break;
        }
        if let Some(item) = line.trim().strip_prefix("- ") {
            out.push(item);
        }
    }
    out
}

#[test]
fn the_manifest_names_the_app_and_its_runtime() {
    let m = manifest();
    let lines = trimmed(&m);
    for want in [
        "id: io.github.bg9m9r.glowfly",
        "runtime: org.freedesktop.Platform",
        "runtime-version: '25.08'",
        "sdk: org.freedesktop.Sdk",
        "- org.freedesktop.Sdk.Extension.rust-stable",
        "command: glowfly",
    ] {
        assert!(lines.contains(&want), "{want}");
    }
}

#[test]
fn the_sandbox_gets_exactly_these_permissions() {
    let m = manifest();
    let args = list(&m, "finish-args");
    let want = [
        "--share=ipc",
        "--socket=wayland",
        "--socket=fallback-x11",
        "--device=dri",
        "--share=network",
        "--filesystem=host",
        "--filesystem=xdg-run/gnupg:ro",
        "--socket=ssh-auth",
        "--talk-name=org.freedesktop.Notifications",
        "--talk-name=org.freedesktop.secrets",
    ];
    assert_eq!(args, want);
    assert!(!m.contains("org.freedesktop.Flatpak"), "no way out of the sandbox");
    assert!(!m.contains("--socket=x11"), "X11 only as the fallback");
}

#[test]
fn git_comes_from_a_pinned_tarball_and_glowfly_builds_offline() {
    let m = manifest();
    let lines = trimmed(&m);
    let url = lines.iter().find_map(|l| l.strip_prefix("url: ")).expect("the git archive's url");
    assert!(url.starts_with("https://www.kernel.org/pub/software/scm/git/git-") && url.ends_with(".tar.xz"), "{url}");
    let sha = lines.iter().find_map(|l| l.strip_prefix("sha256: ")).expect("its sha256");
    assert!(sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)), "{sha}");
    let build = lines.iter().find(|l| l.contains("cargo ") && l.contains(" build")).expect("the cargo build");
    assert!(build.contains("--offline") && build.contains("--locked"), "{build}");
}

#[test]
fn the_flatpak_installs_the_icon_sizes_the_deb_does() {
    let cargo: toml::Table = workspace_file("crates/glowfly-app/Cargo.toml").parse().unwrap();
    let assets = cargo["package"]["metadata"]["deb"]["assets"].as_array().expect("deb assets");
    let deb: Vec<String> = assets
        .iter()
        .filter_map(|a| a.as_array()?.get(1)?.as_str())
        .filter_map(|dest| dest.strip_prefix("usr/share/icons/hicolor/")?.split_once('x').map(|(s, _)| s.to_owned()))
        .collect();
    assert!(!deb.is_empty());
    let m = manifest();
    let sizes = m.lines().find_map(|l| l.trim().strip_prefix("- for size in ")).and_then(|l| l.split_once("; do")).map(|(s, _)| s);
    let flatpak: Vec<String> = sizes.expect("the icon loop").split_whitespace().map(str::to_owned).collect();
    assert_eq!(flatpak, deb);
    assert!(m.contains("/app/share/icons/hicolor/${size}x${size}/apps/io.github.bg9m9r.glowfly.png"));
}

#[test]
fn the_workflows_build_and_check_the_bundle() {
    let release = workspace_file(".github/workflows/release.yml");
    let needs = release.lines().skip_while(|l| l.trim() != "checksums:").find_map(|l| l.trim().strip_prefix("needs: "));
    assert_eq!(needs, Some("[prepare, build, flatpak]"));
    let job = release.split_once("\n  flatpak:\n").map(|(_, rest)| rest).expect("a flatpak job");
    let job = job.split_once("\n  checksums:").map_or(job, |(j, _)| j);
    assert!(job.contains("dist/*.flatpak"), "the job uploads the bundle");
    let installers = workspace_file(".github/workflows/installers.yml");
    assert!(installers.contains("bash scripts/installer-check-flatpak.sh "));
}

#[test]
fn the_bundle_carries_gits_licences_and_its_notice_names_the_source() {
    let m = manifest();
    let lines = trimmed(&m);
    for want in [
        "- install -Dm644 -t /app/share/doc/git COPYING LGPL-2.1",
        "- install -Dm644 reftable/LICENSE /app/share/doc/git/reftable-LICENSE",
        "- install -Dm644 sha1dc/LICENSE.txt /app/share/doc/git/sha1dc-LICENSE.txt",
    ] {
        assert!(lines.contains(&want), "{want}");
    }
    let docs = lines.iter().find(|l| l.contains("/app/share/doc/glowfly ")).expect("Glowfly's doc files");
    for f in ["LICENSE-MIT", "LICENSE-APACHE", "THIRD-PARTY-NOTICES.md", "packaging/flatpak/GIT-NOTICE.md"] {
        assert!(docs.split_whitespace().any(|w| w == f), "{f} in {docs}");
    }
    // The notice names the archive the manifest builds: its version, address and sha256.
    let url = lines.iter().find_map(|l| l.strip_prefix("url: ")).expect("the git archive's url");
    let sha = lines.iter().find_map(|l| l.strip_prefix("sha256: ")).expect("its sha256");
    let file = url.rsplit('/').next().unwrap();
    let version = file.strip_prefix("git-").and_then(|f| f.strip_suffix(".tar.xz")).unwrap();
    let notice = workspace_file("packaging/flatpak/GIT-NOTICE.md");
    for want in [url, sha, file, &format!("git {version}")] {
        assert!(notice.contains(want), "GIT-NOTICE.md doesn't name {want}");
    }
}

#[test]
fn the_release_attaches_gits_source_beside_the_bundle() {
    let release = workspace_file(".github/workflows/release.yml");
    let job = release.split_once("\n  flatpak:\n").map(|(_, rest)| rest).expect("a flatpak job");
    let job = job.split_once("\n  checksums:").map_or(job, |(j, _)| j);
    assert!(job.contains("dist/git-*.tar.xz"), "the job uploads the git source");
    let sums = release.split_once("\n  checksums:\n").map(|(_, rest)| rest).expect("a checksums job");
    let sums = sums.split_once("\n  manifests:").map_or(sums, |(j, _)| j);
    assert!(sums.contains("--pattern 'git-*.tar.xz'"), "SHA256SUMS covers the git source");
    assert!(sums.contains("bash scripts/release-check-git-source.sh dist"), "the release checks it is there");
    let script = workspace_file("scripts/package-flatpak.sh");
    assert!(script.contains("  export_git_source\n") && script.contains("  check_docs\n"));
    let installers = workspace_file(".github/workflows/installers.yml");
    assert!(installers.contains("bash scripts/release-check-git-source.sh dist"));
}
