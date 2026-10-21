//! Which of a release's files suits this system and this copy of Glowfly, and where its checksum
//! is published. File names follow the Release workflow: `glowfly-<tag>-<target triple>.<ext>`.

use super::{Asset, Release};
use std::path::Path;

/// The operating systems Glowfly is built for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetOs {
    Windows,
    Mac,
    Linux,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
}

/// An operating system and processor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Platform {
    pub os: TargetOs,
    pub arch: Arch,
}

const fn current_os() -> Option<TargetOs> {
    if cfg!(windows) {
        Some(TargetOs::Windows)
    } else if cfg!(target_os = "macos") {
        Some(TargetOs::Mac)
    } else if cfg!(target_os = "linux") {
        Some(TargetOs::Linux)
    } else {
        None
    }
}

const fn current_arch() -> Option<Arch> {
    if cfg!(target_arch = "x86_64") {
        Some(Arch::X86_64)
    } else if cfg!(target_arch = "aarch64") {
        Some(Arch::Aarch64)
    } else {
        None
    }
}

/// The platform this build runs on, when it is one Glowfly knows.
pub const CURRENT_PLATFORM: Option<Platform> = match (current_os(), current_arch()) {
    (Some(os), Some(arch)) => Some(Platform { os, arch }),
    _ => None,
};

impl Platform {
    /// The target triple the Release workflow builds for this platform, if any.
    pub fn triple(self) -> Option<&'static str> {
        match (self.os, self.arch) {
            (TargetOs::Windows, Arch::X86_64) => Some("x86_64-pc-windows-msvc"),
            (TargetOs::Mac, Arch::Aarch64) => Some("aarch64-apple-darwin"),
            (TargetOs::Mac, Arch::X86_64) => Some("x86_64-apple-darwin"),
            (TargetOs::Linux, Arch::X86_64) => Some("x86_64-unknown-linux-gnu"),
            (TargetOs::Windows | TargetOs::Linux, Arch::Aarch64) => None,
        }
    }
}

/// How this copy of Glowfly was installed, which decides the kind of file it is offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallKind {
    /// By the MSI, the .app bundle or the .deb package.
    Installer,
    /// Unpacked from a zip or tar.gz anywhere else.
    Portable,
}

/// How a copy whose program is at `exe` was installed on `os`; a pure function of the spelling.
pub fn install_kind_for(exe: &str, os: TargetOs) -> InstallKind {
    let installer = match os {
        TargetOs::Windows => {
            let parts: Vec<&str> = exe.split(['\\', '/']).filter(|p| !p.is_empty()).collect();
            // Windows paths ignore case: ...\Programs\Glowfly\glowfly.exe.
            parts.len() >= 3
                && parts[parts.len() - 3].eq_ignore_ascii_case("Programs")
                && parts[parts.len() - 2].eq_ignore_ascii_case("Glowfly")
        }
        TargetOs::Mac => {
            let parts: Vec<&str> = exe.split('/').filter(|p| !p.is_empty()).collect();
            parts.len() >= 4
                && parts[parts.len() - 2] == "MacOS"
                && parts[parts.len() - 3] == "Contents"
                && parts[parts.len() - 4].len() > ".app".len()
                && parts[parts.len() - 4].ends_with(".app")
        }
        TargetOs::Linux => exe == crate::url_scheme::PACKAGED_EXE,
    };
    if installer { InstallKind::Installer } else { InstallKind::Portable }
}

/// How the copy at `exe` (the running program) was installed.
pub fn install_kind(exe: &Path) -> InstallKind {
    let os = super::CURRENT_PLATFORM.map_or(TargetOs::Linux, |p| p.os);
    install_kind_for(&exe.to_string_lossy(), os)
}

fn installer_ext(os: TargetOs) -> &'static str {
    match os {
        TargetOs::Windows => "msi",
        TargetOs::Mac => "dmg",
        TargetOs::Linux => "deb",
    }
}

fn archive_ext(os: TargetOs) -> &'static str {
    if os == TargetOs::Windows { "zip" } else { "tar.gz" }
}

/// The file of `release` to fetch on `platform`: an installer copy prefers the installer
/// (msi, dmg or deb) and falls back to the archive, a portable copy the other way round. None
/// when the release has neither for this platform, or there is no build for it. Files are named
/// `glowfly-<tag>-<triple>.<ext>`; a release that still names them `trellis-...` (the app's name
/// before Glowfly) is read too.
pub fn pick(release: &Release, platform: Platform, kind: InstallKind) -> Option<Asset> {
    let triple = platform.triple()?;
    let (installer, archive) = (installer_ext(platform.os), archive_ext(platform.os));
    let order = match kind {
        InstallKind::Installer => [installer, archive],
        InstallKind::Portable => [archive, installer],
    };
    order.iter().find_map(|ext| {
        ["glowfly", "trellis"].iter().find_map(|name| {
            let want = format!("{name}-{}-{triple}.{ext}", release.tag);
            release.assets.iter().find(|a| a.name == want).cloned()
        })
    })
}

/// Where the checksum of a release's file is published.
#[derive(Clone, Debug, PartialEq)]
pub enum SumsSource {
    /// `SHA256SUMS`, covering every file.
    Combined(Asset),
    /// `<file>.sha256`, covering that file.
    Single(Asset),
    /// Nowhere: the file isn't downloaded.
    None,
}

/// The checksum file for `asset`: `SHA256SUMS` when the release has one, else `<name>.sha256`.
pub fn sums_source(release: &Release, asset: &Asset) -> SumsSource {
    let find = |name: &str| release.assets.iter().find(|a| a.name == name).cloned();
    if let Some(all) = find("SHA256SUMS") {
        return SumsSource::Combined(all);
    }
    match find(&format!("{}.sha256", asset.name)) {
        Some(one) => SumsSource::Single(one),
        None => SumsSource::None,
    }
}
