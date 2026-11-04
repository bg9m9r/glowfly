//! Bundles CHANGELOG.md for the release notes dialog (`state/release_notes.rs`), and passes the
//! target triple to About's Build row (`GLOWFLY_TARGET`, `state/menu_bar.rs`).

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    // release-please creates CHANGELOG.md with the first release; until then the missing file
    // makes Cargo run this script on every build, which is cheap.
    println!("cargo::rerun-if-changed=../../CHANGELOG.md");

    // Cargo reruns build scripts for a new target on its own; the triple is all that's passed.
    println!("cargo::rerun-if-env-changed=TARGET");
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo::rustc-env=GLOWFLY_TARGET={target}");

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let changelog = std::fs::read(Path::new("../../CHANGELOG.md")).unwrap_or_default();
    let bundled = out.join("CHANGELOG.md");
    // Rewritten only when it changed, so a rerun doesn't make the crate compile again.
    if std::fs::read(&bundled).ok().as_deref() != Some(changelog.as_slice()) {
        std::fs::write(&bundled, changelog).expect("write OUT_DIR/CHANGELOG.md");
    }
}
