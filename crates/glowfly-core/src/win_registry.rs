//! Reading single values from the Windows registry: a DWORD or a string, None when the key or
//! value is missing (or has another type).

use crate::native_path::wide;
use std::ffi::OsStr;
use windows::Win32::System::Registry::{HKEY, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW};
use windows::core::PCWSTR;

/// A DWORD from the registry, or None when the key or value isn't there.
pub fn dword(hkey: HKEY, key: &str, value: &str) -> Option<u32> {
    let (key, value) = (wide(OsStr::new(key)), wide(OsStr::new(value)));
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: both strings are NUL-terminated; `data` and `size` are valid for a DWORD.
    let r = unsafe {
        RegGetValueW(
            hkey,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut data).cast()),
            Some(&mut size),
        )
    };
    r.is_ok().then_some(data)
}

/// A REG_SZ string from the registry, or None when the key or value isn't there.
pub fn string(hkey: HKEY, key: &str, value: &str) -> Option<String> {
    let (key, value) = (wide(OsStr::new(key)), wide(OsStr::new(value)));
    let mut size = 0u32;
    // SAFETY: both strings are NUL-terminated; only the size is asked for.
    let r = unsafe { RegGetValueW(hkey, PCWSTR(key.as_ptr()), PCWSTR(value.as_ptr()), RRF_RT_REG_SZ, None, None, Some(&mut size)) };
    if r.is_err() {
        return None;
    }
    // One spare unit: the value may grow between the two calls.
    let mut buf = vec![0u16; (size as usize).div_ceil(2) + 1];
    let mut bytes = (buf.len() * 2) as u32;
    // SAFETY: as above, and `buf` is writable for `bytes` bytes, as passed.
    let r = unsafe {
        RegGetValueW(
            hkey,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    if r.is_err() {
        return None;
    }
    let units = (bytes as usize / 2).min(buf.len());
    let text = &buf[..units];
    let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    String::from_utf16(&text[..end]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    const VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

    #[test]
    fn string_reads_the_build_number() {
        let build = string(HKEY_LOCAL_MACHINE, VERSION, "CurrentBuildNumber").expect("every Windows has it");
        assert!(!build.is_empty() && build.chars().all(|c| c.is_ascii_digit()), "{build:?}");
        assert_eq!(string(HKEY_LOCAL_MACHINE, VERSION, "GlowflyNoSuchValue"), None);
    }

    #[test]
    fn a_missing_value_is_none() {
        assert_eq!(dword(HKEY_CURRENT_USER, r"Software\GlowflyNoSuchKey", "Value"), None);
        assert_eq!(dword(HKEY_LOCAL_MACHINE, VERSION, "GlowflyNoSuchValue"), None);
        // A string isn't a DWORD.
        assert_eq!(dword(HKEY_LOCAL_MACHINE, VERSION, "CurrentBuildNumber"), None);
    }
}
