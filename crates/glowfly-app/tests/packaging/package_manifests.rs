//! The package manager manifests `scripts/fill-manifests.sh` fills for each release: every
//! template's placeholders are ones the script fills, the Scoop manifest installs the release's
//! Windows zips (and updates itself from `SHA256SUMS`), the Homebrew cask tells how to open the
//! unsigned app, and the Installers workflow runs `scripts/fill-manifests-test.sh` on the script.

use std::collections::BTreeSet;
use std::path::Path;

fn workspace_file(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
    // A Windows checkout may have CRLF line endings.
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display())).replace("\r\n", "\n")
}

const TEMPLATES: [&str; 5] = [
    "packaging/homebrew/glowfly.rb.in",
    "packaging/scoop/glowfly.json.in",
    "packaging/winget/Glowfly.Glowfly.yaml.in",
    "packaging/winget/Glowfly.Glowfly.installer.yaml.in",
    "packaging/winget/Glowfly.Glowfly.locale.en-US.yaml.in",
];

/// The `@NAME@` placeholders in `text`.
fn placeholders(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut parts = text.split('@');
    parts.next();
    // Between an opening and a closing '@'; a name is upper-case letters, digits and '_'.
    let mut open = true;
    for part in parts {
        if open && !part.is_empty() && part.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
            out.insert(part.to_owned());
            open = false;
        } else {
            open = true;
        }
    }
    out
}

/// The placeholders fill-manifests.sh replaces (its `-e "s|@NAME@|...|g"` arguments).
fn filled() -> BTreeSet<String> {
    let script = workspace_file("scripts/fill-manifests.sh");
    script.split("\"s|@").skip(1).filter_map(|s| s.split_once("@|")).map(|(name, _)| name.to_owned()).collect()
}

#[test]
fn the_script_fills_every_template_and_every_placeholder() {
    let script = workspace_file("scripts/fill-manifests.sh");
    let filled = filled();
    for template in TEMPLATES {
        let target = Path::new(template).file_stem().and_then(|s| s.to_str()).unwrap();
        let dir = Path::new(template).parent().and_then(|p| p.to_str()).unwrap();
        let writes = if dir.ends_with("winget") {
            script.contains(&format!(" {target}")) && script.contains("fill \"$root/packaging/winget/$manifest.in\" \"$out/$manifest\"")
        } else {
            script.contains(&format!("fill \"$root/{template}\" \"$out/{target}\"\n"))
        };
        assert!(writes, "fill-manifests.sh writes {target}");
        let text = workspace_file(template);
        let found = placeholders(&text);
        assert!(!found.is_empty(), "{template} has placeholders");
        for name in found {
            assert!(filled.contains(&name), "fill-manifests.sh never fills @{name}@ in {template}");
        }
    }
}

/// The template with every placeholder filled the way the script would, for a v1.2.3 release.
fn scoop_manifest() -> serde_json::Value {
    let mut text = workspace_file("packaging/scoop/glowfly.json.in");
    for name in placeholders(&text) {
        let value = match name.as_str() {
            "VERSION" => "1.2.3".to_owned(),
            "TAG" => "v1.2.3".to_owned(),
            "REPO" => "owner/glowfly".to_owned(),
            other => format!("{:0>64}", other.len()),
        };
        text = text.replace(&format!("@{name}@"), &value);
    }
    serde_json::from_str(&text).expect("the filled Scoop manifest is JSON")
}

#[test]
fn the_scoop_manifest_installs_the_release_zips() {
    let m = scoop_manifest();
    assert_eq!(m["version"], "1.2.3");
    assert_eq!(m["homepage"], "https://github.com/owner/glowfly");
    assert_eq!(m["license"], "MIT|Apache-2.0", "Cargo.toml's MIT OR Apache-2.0, in Scoop's form");
    // package-windows.ps1 names the zip and its one top folder glowfly-<tag>-<target>.
    let packaging = workspace_file("scripts/package-windows.ps1");
    assert!(packaging.contains("$name = \"glowfly-$Tag-$Target\"\n") && packaging.contains("$zip = Join-Path $dist \"$name.zip\"\n"));
    let release = "https://github.com/owner/glowfly/releases/download";
    let update = &m["autoupdate"]["architecture"];
    for (arch, target, placeholder) in
        [("64bit", "x86_64-pc-windows-msvc", "SHA256_WIN_X64_ZIP"), ("arm64", "aarch64-pc-windows-msvc", "SHA256_WIN_ARM64_ZIP")]
    {
        let a = &m["architecture"][arch];
        assert_eq!(a["url"], format!("{release}/v1.2.3/glowfly-v1.2.3-{target}.zip"), "{arch}");
        assert_eq!(a["hash"], format!("{:0>64}", placeholder.len()), "{arch} uses @{placeholder}@");
        assert_eq!(a["extract_dir"], format!("glowfly-v1.2.3-{target}"), "{arch}");
        assert_eq!(update[arch]["url"], format!("{release}/v$version/glowfly-v$version-{target}.zip"), "{arch}");
        assert_eq!(update[arch]["extract_dir"], format!("glowfly-v$version-{target}"), "{arch}");
    }
    let script = workspace_file("scripts/fill-manifests.sh");
    assert!(script.contains("win_x64_zip=\"$(hash_of \"glowfly-$tag-x86_64-pc-windows-msvc.zip\")\"\n"));
    assert!(script.contains("win_arm_zip=\"$(hash_of \"glowfly-$tag-aarch64-pc-windows-msvc.zip\")\"\n"));
    assert!(script.contains("-e \"s|@SHA256_WIN_X64_ZIP@|$win_x64_zip|g\" -e \"s|@SHA256_WIN_ARM64_ZIP@|$win_arm_zip|g\""));
    assert_eq!(m["bin"], "glowfly.exe");
    assert_eq!(m["shortcuts"], serde_json::json!([["glowfly.exe", "Glowfly"]]));
    assert_eq!(m["checkver"], "github", "new versions come from the repository's latest release");
    // A later version's hashes come from its SHA256SUMS (`<hash>  <file>`, or `*<file>`).
    assert_eq!(m["autoupdate"]["hash"]["url"], format!("{release}/v$version/SHA256SUMS"));
    assert_eq!(m["autoupdate"]["hash"]["regex"], "$sha256\\s+\\*?$basename");
}

#[test]
fn the_cask_tells_how_to_open_the_unsigned_app() {
    let cask = workspace_file("packaging/homebrew/glowfly.rb.in");
    let caveats = cask.split_once("  caveats <<~EOS\n").map(|(_, c)| c).expect("a caveats block");
    let caveats = caveats.split_once("  EOS\n").map(|(c, _)| c).expect("the caveats end");
    for text in [
        "macOS 15 and later",
        "System Settings > Privacy & Security",
        "\"Open Anyway\"",
        "xattr -dr com.apple.quarantine #{appdir}/Glowfly.app",
    ] {
        assert!(caveats.contains(text), "the caveats say {text}");
    }
    // Homebrew deprecated --no-quarantine, so the cask never advises it.
    assert!(!cask.contains("no-quarantine"));
    assert!(cask.ends_with("  EOS\nend\n"), "caveats is the cask's last stanza");
}

#[test]
fn the_release_attaches_them_and_installers_tests_the_script() {
    let release = workspace_file(".github/workflows/release.yml");
    let job = release.split_once("\n  manifests:\n").map(|(_, j)| j).expect("a manifests job");
    let job = job.split_once("\n\n").map(|(j, _)| j).unwrap_or(job);
    assert!(
        job.contains("bash scripts/fill-manifests.sh \"$TAG\" \"$RUNNER_TEMP/sums/SHA256SUMS\" \"$RUNNER_TEMP/manifests\" \"$GH_REPO\"\n")
    );
    // Every file the script writes is attached, glowfly.json with the others.
    assert!(job.contains("find \"$RUNNER_TEMP/manifests\" -type f -print0 | xargs -0 -r gh release upload \"$TAG\" --clobber\n"));
    let installers = workspace_file(".github/workflows/installers.yml");
    assert!(installers.contains("        run: bash scripts/fill-manifests-test.sh\n"));
    assert!(installers.contains(" scripts/fill-manifests-test.sh\n"), "Installers shellchecks the test");
    let test = workspace_file("scripts/fill-manifests-test.sh");
    assert!(test.contains("./glowfly.json ./glowfly.rb"), "the test expects the Scoop manifest among the files");
}
