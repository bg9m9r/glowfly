//! Bundles CHANGELOG.md for the release notes dialog (`state/release_notes.rs`) and, when
//! building for Windows, embeds the app icon and version details in glowfly.exe.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    // release-please creates CHANGELOG.md with the first release; until then the missing file
    // makes Cargo run this script on every build, which is cheap.
    println!("cargo::rerun-if-changed=../../CHANGELOG.md");
    println!("cargo::rerun-if-changed=../../assets/icon/glowfly.ico");

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let changelog = std::fs::read(Path::new("../../CHANGELOG.md")).unwrap_or_default();
    let bundled = out.join("CHANGELOG.md");
    // Rewritten only when it changed, so a rerun doesn't make the crate compile again.
    if std::fs::read(&bundled).ok().as_deref() != Some(changelog.as_slice()) {
        std::fs::write(&bundled, changelog).expect("write OUT_DIR/CHANGELOG.md");
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windows_resource();
    }
}

/// The icon Explorer, the taskbar and Alt+Tab show, and the file's description. A machine
/// without the Windows SDK's resource compiler still builds, with a warning.
#[cfg(windows)]
fn windows_resource() {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icon/glowfly.ico")
        .set("FileDescription", "Glowfly")
        .set("ProductName", "Glowfly")
        .set("LegalCopyright", "Copyright 2026 Brett Gamerson and the Glowfly contributors. MIT or Apache-2.0.");
    if let Err(e) = res.compile() {
        println!("cargo::warning=glowfly.exe gets no icon: the resource compiler failed ({e})");
    }
}

/// Cross-compiling for Windows from elsewhere: no resource compiler to run.
#[cfg(not(windows))]
fn windows_resource() {
    println!("cargo::warning=glowfly.exe gets no icon when built on another system");
}
