//! The .rpm's file list (`[package.metadata.generate-rpm]` in this crate's Cargo.toml) matches
//! what the Linux install checks expect (scripts/installer-lib-linux.sh's `PATHS`): every file
//! the package installs is one the checks look for, and none is marked `doc`, which a system
//! set to skip documentation (dnf's tsflags=nodocs, as in Fedora's container images) leaves
//! out while still listing it as the package's.

use std::path::Path;

fn workspace_file(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn rpm_assets() -> Vec<toml::Table> {
    let manifest: toml::Table = workspace_file("crates/glowfly-app/Cargo.toml").parse().unwrap();
    manifest["package"]["metadata"]["generate-rpm"]["assets"]
        .as_array()
        .expect("generate-rpm assets")
        .iter()
        .map(|a| a.as_table().expect("an asset table").clone())
        .collect()
}

/// The paths the install checks require, with the icon loop's sizes expanded.
fn checked_paths() -> Vec<String> {
    let lib = workspace_file("scripts/installer-lib-linux.sh");
    let block = lib.split_once("PATHS=(").and_then(|(_, rest)| rest.split_once(')')).map(|(block, _)| block).expect("a PATHS=( ... ) list");
    let mut paths: Vec<String> = block.lines().map(str::trim).filter(|l| l.starts_with('/')).map(str::to_owned).collect();
    let sizes =
        lib.lines().find_map(|l| l.trim().strip_prefix("for size in ")).and_then(|l| l.strip_suffix("; do")).expect("the icon sizes loop");
    assert!(lib.contains(r#"PATHS+=("/usr/share/icons/hicolor/${size}x${size}/apps/glowfly.png")"#));
    for size in sizes.split_whitespace() {
        paths.push(format!("/usr/share/icons/hicolor/{size}x{size}/apps/glowfly.png"));
    }
    paths
}

#[test]
fn rpm_assets_are_never_marked_doc() {
    for asset in rpm_assets() {
        assert_ne!(
            asset.get("doc").and_then(|d| d.as_bool()),
            Some(true),
            "{} is marked doc, so a nodocs system wouldn't install it",
            asset["dest"]
        );
    }
}

#[test]
fn rpm_installs_exactly_the_files_the_install_checks_expect() {
    let mut dests: Vec<String> = rpm_assets().iter().map(|a| a["dest"].as_str().expect("an asset dest").to_owned()).collect();
    let mut checked = checked_paths();
    dests.sort();
    checked.sort();
    assert_eq!(dests, checked);
}
