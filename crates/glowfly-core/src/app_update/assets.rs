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
    /// The target triple the Release workflow builds for this platform.
    pub fn triple(self) -> Option<&'static str> {
        Some(match (self.os, self.arch) {
            (TargetOs::Windows, Arch::X86_64) => "x86_64-pc-windows-msvc",
            (TargetOs::Windows, Arch::Aarch64) => "aarch64-pc-windows-msvc",
            (TargetOs::Mac, Arch::Aarch64) => "aarch64-apple-darwin",
            (TargetOs::Mac, Arch::X86_64) => "x86_64-apple-darwin",
            (TargetOs::Linux, Arch::X86_64) => "x86_64-unknown-linux-gnu",
            (TargetOs::Linux, Arch::Aarch64) => "aarch64-unknown-linux-gnu",
        })
    }

    /// The platform whose files also run here when a release has none for this one: x86_64 on
    /// Windows and macOS on Arm (both run x86_64 programs). Linux on Arm has none.
    pub fn fallback(self) -> Option<Platform> {
        match (self.os, self.arch) {
            (TargetOs::Windows | TargetOs::Mac, Arch::Aarch64) => Some(Platform { os: self.os, arch: Arch::X86_64 }),
            _ => None,
        }
    }
}

/// How this copy of Glowfly was installed, which decides the kind of file it is offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallKind {
    /// By the MSI, the .app bundle or the .deb package.
    Installer,
    /// By the .rpm package: `/usr/bin/glowfly` with [`PACKAGE_FORMAT_FILE`] reading `rpm`.
    RpmPackage,
    /// Running from an AppImage.
    AppImage,
    /// Unpacked from a zip or tar.gz anywhere else.
    Portable,
}

impl InstallKind {
    /// The word `glowfly --check-update` prints for this kind.
    pub fn label(self) -> &'static str {
        match self {
            Self::Installer => "installer",
            Self::RpmPackage => "rpm",
            Self::AppImage => "appimage",
            Self::Portable => "portable",
        }
    }
}

/// The file the Linux packages install to say which package format installed them (`deb` or
/// `rpm`). A package without it (from before it existed) is a .deb.
pub const PACKAGE_FORMAT_FILE: &str = "/usr/share/glowfly/package-format";

/// What else is known about a Linux copy besides its program's path.
#[derive(Clone, Copy, Debug, Default)]
pub struct InstallHints<'a> {
    /// The program runs from inside an AppImage.
    pub appimage: bool,
    /// The text of [`PACKAGE_FORMAT_FILE`], when it was read.
    pub package_format: Option<&'a str>,
}

/// How a copy whose program is at `exe` was installed on `os`; a pure function of the spelling
/// and, on Linux, of `hints` (ignored elsewhere).
pub fn install_kind_with(exe: &str, os: TargetOs, hints: InstallHints) -> InstallKind {
    if os == TargetOs::Linux && hints.appimage {
        return InstallKind::AppImage;
    }
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
    match (installer, os) {
        (false, _) => InstallKind::Portable,
        // Any other marker text (or none) is the .deb.
        (true, TargetOs::Linux) if hints.package_format.is_some_and(|f| f.trim() == "rpm") => InstallKind::RpmPackage,
        (true, _) => InstallKind::Installer,
    }
}

/// How a copy whose program is at `exe` was installed on `os`, from the path alone.
pub fn install_kind_for(exe: &str, os: TargetOs) -> InstallKind {
    install_kind_with(exe, os, InstallHints::default())
}

/// How the copy at `exe` (the running program) was installed. On Linux this reads the
/// AppImage variables and, for the packaged program, [`PACKAGE_FORMAT_FILE`].
pub fn install_kind(exe: &Path) -> InstallKind {
    let os = super::CURRENT_PLATFORM.map_or(TargetOs::Linux, |p| p.os);
    let spelled = exe.to_string_lossy();
    if os != TargetOs::Linux {
        return install_kind_for(&spelled, os);
    }
    let format = if spelled == crate::url_scheme::PACKAGED_EXE { std::fs::read_to_string(PACKAGE_FORMAT_FILE).ok() } else { None };
    let hints = InstallHints { appimage: super::appimage::appimage_file(exe).is_some(), package_format: format.as_deref() };
    install_kind_with(&spelled, os, hints)
}

/// The extensions a `kind` copy on `os` takes, preferred first: its installer (msi, dmg, deb,
/// rpm or AppImage, matched exactly as published) and the archive (zip or tar.gz). A portable
/// copy prefers the archive.
fn formats(os: TargetOs, kind: InstallKind) -> [&'static str; 2] {
    let installer = match (os, kind) {
        (TargetOs::Windows, _) => "msi",
        (TargetOs::Mac, _) => "dmg",
        (TargetOs::Linux, InstallKind::RpmPackage) => "rpm",
        (TargetOs::Linux, InstallKind::AppImage) => "AppImage",
        (TargetOs::Linux, _) => "deb",
    };
    let archive = if os == TargetOs::Windows { "zip" } else { "tar.gz" };
    if kind == InstallKind::Portable { [archive, installer] } else { [installer, archive] }
}

/// The file of `release` to fetch on `platform` for a `kind` copy: an installed copy prefers
/// its installer (msi, dmg, deb, rpm or AppImage) and falls back to the archive, a portable
/// copy the other way round. Each format is looked for in this platform's build first and then,
/// on Windows and macOS on Arm, in the x86_64 build (both run it); so the format decides before
/// the processor. None when the release has none of these. Files are named
/// `glowfly-<tag>-<triple>.<ext>`; a release that still names them `trellis-...` (the app's name
/// before Glowfly) is read too.
pub fn pick(release: &Release, platform: Platform, kind: InstallKind) -> Option<Asset> {
    let triples: Vec<&str> = [Some(platform), platform.fallback()].into_iter().flatten().filter_map(Platform::triple).collect();
    formats(platform.os, kind).iter().find_map(|ext| {
        triples.iter().find_map(|triple| {
            ["glowfly", "trellis"].iter().find_map(|name| {
                let want = format!("{name}-{}-{triple}.{ext}", release.tag);
                release.assets.iter().find(|a| a.name == want).cloned()
            })
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
