//! The machine's real processor, which can differ from the one this build is for: Windows on
//! Arm runs x64 programs emulated, and Apple silicon runs x86_64 ones under Rosetta. An update
//! is offered for the real one.

use super::{Arch, CURRENT_PLATFORM, Platform};
use std::sync::OnceLock;

/// The processor a Windows `IMAGE_FILE_MACHINE` value names, when Glowfly is built for it.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn arch_for_machine(machine: u16) -> Option<Arch> {
    match machine {
        0x8664 => Some(Arch::X86_64),
        0xAA64 => Some(Arch::Aarch64),
        _ => None,
    }
}

/// The processor of a Mac given this build's and `sysctl.proc_translated` (1 under Rosetta, 0
/// when native, missing on Intel Macs).
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
pub fn mac_arch(build: Option<Arch>, translated: Option<i32>) -> Option<Arch> {
    if translated == Some(1) { Some(Arch::Aarch64) } else { build }
}

fn build_arch() -> Option<Arch> {
    CURRENT_PLATFORM.map(|p| p.arch)
}

#[cfg(windows)]
fn host_arch() -> Option<Arch> {
    use windows::Win32::System::SystemInformation::IMAGE_FILE_MACHINE;
    use windows::Win32::System::Threading::{GetCurrentProcess, IsWow64Process2};
    let (mut process, mut native) = (IMAGE_FILE_MACHINE::default(), IMAGE_FILE_MACHINE::default());
    // SAFETY: the pseudo handle of this process needs no closing, and both pointers are to
    // locals that outlive the call.
    let asked = unsafe { IsWow64Process2(GetCurrentProcess(), &raw mut process, Some(&raw mut native)) };
    match asked {
        Ok(()) => arch_for_machine(native.0).or_else(build_arch),
        Err(_) => build_arch(),
    }
}

#[cfg(target_os = "macos")]
fn host_arch() -> Option<Arch> {
    let mut value: libc::c_int = 0;
    let mut size: libc::size_t = std::mem::size_of::<libc::c_int>();
    // SAFETY: the name is a NUL-terminated literal, `value` and `size` are locals that outlive
    // the call and `size` holds `value`'s size; nothing is written to the system (null, 0).
    let status =
        unsafe { libc::sysctlbyname(c"sysctl.proc_translated".as_ptr(), (&raw mut value).cast(), &raw mut size, std::ptr::null_mut(), 0) };
    mac_arch(build_arch(), (status == 0).then_some(value))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn host_arch() -> Option<Arch> {
    build_arch()
}

/// This machine's platform: the build's system with the real processor (asked once).
pub fn current_platform() -> Option<Platform> {
    static PLATFORM: OnceLock<Option<Platform>> = OnceLock::new();
    *PLATFORM.get_or_init(|| CURRENT_PLATFORM.map(|p| Platform { arch: host_arch().unwrap_or(p.arch), ..p }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_machines_map_to_processors() {
        assert_eq!(arch_for_machine(0x8664), Some(Arch::X86_64));
        assert_eq!(arch_for_machine(0xAA64), Some(Arch::Aarch64));
        assert_eq!(arch_for_machine(0x014C), None, "x86");
        assert_eq!(arch_for_machine(0), None);
    }

    #[test]
    fn rosetta_means_apple_silicon() {
        assert_eq!(mac_arch(Some(Arch::X86_64), Some(1)), Some(Arch::Aarch64));
        assert_eq!(mac_arch(Some(Arch::X86_64), Some(0)), Some(Arch::X86_64));
        assert_eq!(mac_arch(Some(Arch::Aarch64), None), Some(Arch::Aarch64));
    }

    #[test]
    fn the_platform_keeps_the_builds_system() {
        assert_eq!(current_platform().map(|p| p.os), CURRENT_PLATFORM.map(|p| p.os));
        assert_eq!(current_platform().is_some(), CURRENT_PLATFORM.is_some());
    }
}
