//! When building for Windows, embeds the app icon and version details in glowfly.exe. (The
//! release notes' CHANGELOG.md is bundled by glowfly-model's build.rs.)

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=../../assets/icon/glowfly.ico");

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
