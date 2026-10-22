//! Which file each platform and install kind is offered, and how a copy was installed.

use super::tests::{WIN, full_release, release};
use super::*;
use InstallKind::{AppImage, Installer, Portable, RpmPackage};

const WIN_ARM: Platform = Platform { os: TargetOs::Windows, arch: Arch::Aarch64 };
const MAC_ARM: Platform = Platform { os: TargetOs::Mac, arch: Arch::Aarch64 };
const MAC_X86: Platform = Platform { os: TargetOs::Mac, arch: Arch::X86_64 };
const LINUX: Platform = Platform { os: TargetOs::Linux, arch: Arch::X86_64 };
const LINUX_ARM: Platform = Platform { os: TargetOs::Linux, arch: Arch::Aarch64 };

fn picked(r: &Release, p: Platform, k: InstallKind) -> Option<String> {
    pick(r, p, k).map(|a| a.name)
}

#[test]
fn the_file_for_each_platform_and_install() {
    let r = full_release();
    let cases = [
        (WIN, Installer, "x86_64-pc-windows-msvc.msi"),
        (WIN, Portable, "x86_64-pc-windows-msvc.zip"),
        (WIN_ARM, Installer, "aarch64-pc-windows-msvc.msi"),
        (WIN_ARM, Portable, "aarch64-pc-windows-msvc.zip"),
        (MAC_ARM, Installer, "aarch64-apple-darwin.dmg"),
        (MAC_ARM, Portable, "aarch64-apple-darwin.tar.gz"),
        (MAC_X86, Installer, "x86_64-apple-darwin.dmg"),
        (MAC_X86, Portable, "x86_64-apple-darwin.tar.gz"),
        (LINUX, Installer, "x86_64-unknown-linux-gnu.deb"),
        (LINUX, RpmPackage, "x86_64-unknown-linux-gnu.rpm"),
        (LINUX, AppImage, "x86_64-unknown-linux-gnu.AppImage"),
        (LINUX, Portable, "x86_64-unknown-linux-gnu.tar.gz"),
        (LINUX_ARM, Installer, "aarch64-unknown-linux-gnu.deb"),
        (LINUX_ARM, RpmPackage, "aarch64-unknown-linux-gnu.rpm"),
        (LINUX_ARM, AppImage, "aarch64-unknown-linux-gnu.AppImage"),
        (LINUX_ARM, Portable, "aarch64-unknown-linux-gnu.tar.gz"),
    ];
    for (platform, kind, want) in cases {
        assert_eq!(picked(&r, platform, kind), Some(format!("glowfly-v0.3.0-{want}")), "{platform:?} {kind:?}");
    }
}

#[test]
fn arm_falls_back_to_x86_64_on_windows_and_macos() {
    let names = [
        "glowfly-v0.3.0-x86_64-pc-windows-msvc.msi",
        "glowfly-v0.3.0-x86_64-pc-windows-msvc.zip",
        "glowfly-v0.3.0-x86_64-apple-darwin.dmg",
        "glowfly-v0.3.0-x86_64-apple-darwin.tar.gz",
        "glowfly-v0.3.0-x86_64-unknown-linux-gnu.deb",
        "glowfly-v0.3.0-x86_64-unknown-linux-gnu.tar.gz",
    ];
    let x86_only = release("v0.3.0", &names);
    assert_eq!(picked(&x86_only, WIN_ARM, Installer).as_deref(), Some(names[0]));
    assert_eq!(picked(&x86_only, WIN_ARM, Portable).as_deref(), Some(names[1]));
    assert_eq!(picked(&x86_only, MAC_ARM, Installer).as_deref(), Some(names[2]));
    assert_eq!(picked(&x86_only, MAC_ARM, Portable).as_deref(), Some(names[3]));
    for kind in [Installer, RpmPackage, AppImage, Portable] {
        assert_eq!(picked(&x86_only, LINUX_ARM, kind), None, "Linux on Arm never takes x86_64 ({kind:?})");
    }
    // With both, the Arm file wins.
    let both = release("v0.3.0", &[names[0], "glowfly-v0.3.0-aarch64-pc-windows-msvc.msi"]);
    assert_eq!(picked(&both, WIN_ARM, Installer).as_deref(), Some("glowfly-v0.3.0-aarch64-pc-windows-msvc.msi"));
    // The format decides before the processor.
    let mixed = release("v0.3.0", &["glowfly-v0.3.0-aarch64-pc-windows-msvc.zip", names[0]]);
    assert_eq!(picked(&mixed, WIN_ARM, Installer).as_deref(), Some(names[0]));
    assert_eq!(picked(&mixed, WIN_ARM, Portable).as_deref(), Some("glowfly-v0.3.0-aarch64-pc-windows-msvc.zip"));
}

#[test]
fn linux_kinds_fall_back_to_the_archive() {
    let tar = "glowfly-v0.3.0-x86_64-unknown-linux-gnu.tar.gz";
    let archive_only = release("v0.3.0", &[tar, "glowfly-v0.3.0-x86_64-unknown-linux-gnu.deb"]);
    assert_eq!(picked(&archive_only, LINUX, RpmPackage).as_deref(), Some(tar));
    assert_eq!(picked(&archive_only, LINUX, AppImage).as_deref(), Some(tar));
    let others = release("v0.3.0", &["glowfly-v0.3.0-x86_64-unknown-linux-gnu.rpm", "glowfly-v0.3.0-x86_64-unknown-linux-gnu.AppImage"]);
    assert_eq!(picked(&others, LINUX, Installer), None, "a .deb copy takes neither");
    // The extension is matched as published.
    let lower = release("v0.3.0", &["glowfly-v0.3.0-x86_64-unknown-linux-gnu.appimage"]);
    assert_eq!(picked(&lower, LINUX, AppImage), None);
}

#[test]
fn picking_falls_back_and_ignores_other_files() {
    let zip_only = release("v0.3.0", &["glowfly-v0.3.0-x86_64-pc-windows-msvc.zip", "glowfly-v0.3.0-x86_64-pc-windows-msvc.msi.sha256"]);
    assert_eq!(picked(&zip_only, WIN, Installer).as_deref(), Some("glowfly-v0.3.0-x86_64-pc-windows-msvc.zip"));
    let deb_only = release("v0.3.0", &["glowfly-v0.3.0-x86_64-unknown-linux-gnu.deb"]);
    assert_eq!(picked(&deb_only, LINUX, Portable).as_deref(), Some("glowfly-v0.3.0-x86_64-unknown-linux-gnu.deb"));
    // Checksums, other tags and other triples never count.
    let other = release(
        "v0.3.0",
        &[
            "SHA256SUMS",
            "glowfly-v0.3.0-x86_64-unknown-linux-gnu.deb.sha256",
            "glowfly-v0.2.0-x86_64-unknown-linux-gnu.deb",
            "glowfly-v0.3.0-aarch64-unknown-linux-gnu.deb",
            "glowfly-v0.3.0-x86_64-unknown-linux-gnu.rpm",
        ],
    );
    assert_eq!(picked(&other, LINUX, Installer), None);
    assert_eq!(picked(&other, LINUX, Portable), None);
}

#[test]
fn files_named_for_trellis_are_picked_too() {
    let old = release("v0.3.0", &["trellis-v0.3.0-x86_64-pc-windows-msvc.msi", "trellis-v0.3.0-x86_64-pc-windows-msvc.zip"]);
    assert_eq!(picked(&old, WIN, Installer).as_deref(), Some("trellis-v0.3.0-x86_64-pc-windows-msvc.msi"));
    // The new name wins when a release has both.
    let both = release("v0.3.0", &["trellis-v0.3.0-x86_64-pc-windows-msvc.msi", "glowfly-v0.3.0-x86_64-pc-windows-msvc.msi"]);
    assert_eq!(picked(&both, WIN, Installer).as_deref(), Some("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi"));
}

#[test]
fn how_a_copy_was_installed() {
    let win = [
        (r"C:\Users\ada\AppData\Local\Programs\Glowfly\glowfly.exe", Installer),
        (r"c:\users\ada\appdata\local\PROGRAMS\glowfly\GLOWFLY.EXE", Installer),
        ("C:/Users/ada/AppData/Local/Programs/Glowfly/glowfly.exe", Installer),
        (r"D:\tools\glowfly\glowfly.exe", Portable),
        (r"C:\Programs\Other\glowfly.exe", Portable),
        ("glowfly.exe", Portable),
    ];
    for (exe, want) in win {
        assert_eq!(install_kind_for(exe, TargetOs::Windows), want, "{exe}");
    }
    let mac = [
        ("/Applications/Glowfly.app/Contents/MacOS/glowfly", Installer),
        ("/Users/ada/Applications/Glowfly.app/Contents/MacOS/glowfly", Installer),
        ("/Users/ada/bin/glowfly", Portable),
        ("/Users/ada/.app/Contents/MacOS/glowfly", Portable),
    ];
    for (exe, want) in mac {
        assert_eq!(install_kind_for(exe, TargetOs::Mac), want, "{exe}");
    }
    assert_eq!(install_kind_for("/usr/bin/glowfly", TargetOs::Linux), Installer);
    assert_eq!(install_kind_for("/home/u/.local/bin/glowfly", TargetOs::Linux), Portable);
    assert_eq!(install_kind_for("/usr/local/bin/glowfly", TargetOs::Linux), Portable);
    // Each OS's rule only applies on that OS.
    assert_eq!(install_kind_for("/usr/bin/glowfly", TargetOs::Mac), Portable);
}

#[test]
fn linux_hints_name_the_package_and_the_appimage() {
    let format = |f: Option<&'static str>| InstallHints { appimage: false, package_format: f };
    let linux = |exe: &str, hints| install_kind_with(exe, TargetOs::Linux, hints);
    assert_eq!(linux("/usr/bin/glowfly", format(Some("rpm"))), RpmPackage);
    assert_eq!(linux("/usr/bin/glowfly", format(Some("rpm\n"))), RpmPackage);
    for other in [Some("deb"), Some("deb\n"), Some("other"), None] {
        assert_eq!(linux("/usr/bin/glowfly", format(other)), Installer, "{other:?}");
    }
    assert_eq!(linux("/home/u/bin/glowfly", format(Some("rpm"))), Portable, "only the packaged program");
    let image = InstallHints { appimage: true, package_format: None };
    for exe in ["/tmp/.mount_ab/usr/bin/glowfly", "/usr/bin/glowfly", "/home/u/bin/glowfly"] {
        assert_eq!(linux(exe, image), AppImage, "{exe}");
    }
    let both = InstallHints { appimage: true, package_format: Some("rpm") };
    let windows = r"C:\Users\ada\AppData\Local\Programs\Glowfly\glowfly.exe";
    assert_eq!(install_kind_with(windows, TargetOs::Windows, both), Installer, "ignored on Windows");
    assert_eq!(install_kind_with(r"D:\tools\glowfly.exe", TargetOs::Windows, both), Portable);
    assert_eq!(install_kind_with("/Applications/Glowfly.app/Contents/MacOS/glowfly", TargetOs::Mac, both), Installer, "and on macOS");
    assert_eq!(install_kind_with("/usr/bin/glowfly", TargetOs::Mac, both), Portable);
}

#[test]
fn platforms_triples_fallbacks_and_labels() {
    let triples = [
        (WIN, "x86_64-pc-windows-msvc"),
        (WIN_ARM, "aarch64-pc-windows-msvc"),
        (MAC_X86, "x86_64-apple-darwin"),
        (MAC_ARM, "aarch64-apple-darwin"),
        (LINUX, "x86_64-unknown-linux-gnu"),
        (LINUX_ARM, "aarch64-unknown-linux-gnu"),
    ];
    for (p, want) in triples {
        assert_eq!(p.triple(), Some(want), "{p:?}");
    }
    assert_eq!(WIN_ARM.fallback(), Some(WIN));
    assert_eq!(MAC_ARM.fallback(), Some(MAC_X86));
    for p in [WIN, MAC_X86, LINUX, LINUX_ARM] {
        assert_eq!(p.fallback(), None, "{p:?}");
    }
    assert_eq!([Installer, RpmPackage, AppImage, Portable].map(InstallKind::label), ["installer", "rpm", "appimage", "portable"]);
}
