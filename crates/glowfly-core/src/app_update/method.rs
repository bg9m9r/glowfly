//! What Install and restart does with a downloaded file: a pure table of its kind, how this copy
//! was installed and the system.

use super::{InstallKind, TargetOs, split_extension};

/// How a downloaded update is put in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallMethod {
    /// An MSI for a copy the MSI installed: msiexec runs it once Glowfly has quit, then Glowfly
    /// starts again.
    RunMsi,
    /// An archive for a portable copy: its program replaces this one, then Glowfly starts again.
    SwapPortable,
    /// An installer that can't run in place (a disk image, a .deb, an MSI for a portable copy):
    /// it is opened, and Glowfly quits without starting again.
    OpenAndQuit,
    /// Anything else: the file is only shown.
    ShowOnly,
}

impl InstallMethod {
    /// Whether Glowfly starts again by itself afterwards.
    pub fn restarts(self) -> bool {
        matches!(self, Self::RunMsi | Self::SwapPortable)
    }

    /// Whether Install does anything beyond showing the file.
    pub fn acts(self) -> bool {
        self != Self::ShowOnly
    }
}

/// How the downloaded `file_name` is installed for a `kind` copy on `os`. Extensions are
/// compared without regard to case.
pub fn install_method_for(file_name: &str, kind: InstallKind, os: TargetOs) -> InstallMethod {
    use InstallKind::{Installer, Portable};
    use InstallMethod::{OpenAndQuit, RunMsi, ShowOnly, SwapPortable};
    let lower = file_name.to_ascii_lowercase();
    match (split_extension(&lower).1, os, kind) {
        (".msi", TargetOs::Windows, Installer) => RunMsi,
        (".msi", TargetOs::Windows, Portable) => OpenAndQuit,
        (".dmg", TargetOs::Mac, _) | (".deb", TargetOs::Linux, _) => OpenAndQuit,
        (".zip", TargetOs::Windows, Portable) => SwapPortable,
        (".tar.gz", TargetOs::Linux | TargetOs::Mac, Portable) => SwapPortable,
        _ => ShowOnly,
    }
}

#[cfg(test)]
mod tests {
    use super::InstallMethod::*;
    use super::*;
    use InstallKind::{Installer, Portable};
    use TargetOs::{Linux, Mac, Windows};

    #[test]
    fn the_method_follows_the_file_the_copy_and_the_system() {
        let cases = [
            ("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi", Installer, Windows, RunMsi),
            ("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi", Portable, Windows, OpenAndQuit),
            ("GLOWFLY.MSI", Installer, Windows, RunMsi),
            ("glowfly.msi", Installer, Linux, ShowOnly),
            ("glowfly.msi", Portable, Mac, ShowOnly),
            ("glowfly-v0.3.0-aarch64-apple-darwin.dmg", Installer, Mac, OpenAndQuit),
            ("glowfly-v0.3.0-aarch64-apple-darwin.dmg", Portable, Mac, OpenAndQuit),
            ("glowfly.dmg", Installer, Windows, ShowOnly),
            ("glowfly-v0.3.0-x86_64-unknown-linux-gnu.deb", Installer, Linux, OpenAndQuit),
            ("glowfly.deb", Portable, Mac, ShowOnly),
            ("glowfly-v0.3.0-x86_64-pc-windows-msvc.zip", Portable, Windows, SwapPortable),
            ("glowfly-v0.3.0-x86_64-pc-windows-msvc.zip", Installer, Windows, ShowOnly),
            ("glowfly.zip", Portable, Linux, ShowOnly),
            ("glowfly-v0.3.0-x86_64-unknown-linux-gnu.tar.gz", Portable, Linux, SwapPortable),
            ("glowfly-v0.3.0-aarch64-apple-darwin.tar.gz", Portable, Mac, SwapPortable),
            ("glowfly-v0.3.0-x86_64-unknown-linux-gnu.tar.gz", Installer, Linux, ShowOnly),
            ("glowfly.TAR.GZ", Portable, Linux, SwapPortable),
            ("glowfly.tar.gz", Portable, Windows, ShowOnly),
            ("glowfly.exe", Portable, Windows, ShowOnly),
            ("glowfly", Portable, Linux, ShowOnly),
        ];
        for (name, kind, os, want) in cases {
            assert_eq!(install_method_for(name, kind, os), want, "{name} {kind:?} {os:?}");
        }
    }

    #[test]
    fn what_each_method_does() {
        let all = [RunMsi, SwapPortable, OpenAndQuit, ShowOnly];
        assert_eq!(all.map(InstallMethod::restarts), [true, true, false, false]);
        assert_eq!(all.map(InstallMethod::acts), [true, true, true, false]);
    }
}
