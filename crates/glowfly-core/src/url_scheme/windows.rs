//! The per-user `trellis` class key in the registry. Every function takes the classes key's
//! path under `HKEY_CURRENT_USER`, so tests use a scratch key and never touch the real one.

use super::SchemeStatus;
use crate::native_path::{native, wide};
use ::windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use ::windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteKeyW,
    RegDeleteTreeW, RegGetValueW, RegSetValueExW,
};
use ::windows::core::PCWSTR;
use std::ffi::OsStr;
use std::path::Path;

/// Where per-user classes live.
pub(super) const CLASSES: &str = r"Software\Classes";

pub(super) fn status(exe: &Path) -> Result<SchemeStatus, String> {
    status_at(CLASSES, exe)
}

pub(super) fn register(exe: &Path) -> Result<SchemeStatus, String> {
    register_at(CLASSES, exe)
}

pub(super) fn unregister(exe: &Path) -> Result<SchemeStatus, String> {
    unregister_at(CLASSES)?;
    status_at(CLASSES, exe)
}

/// An open key, closed when dropped.
struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the handle came from RegCreateKeyExW and is closed exactly once.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

fn create(path: &str) -> Result<Key, String> {
    let p = wide(OsStr::new(path));
    let mut key = HKEY::default();
    // SAFETY: `p` is NUL-terminated and outlives the call; `key` receives the handle.
    let r = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(p.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        )
    };
    if r.is_err() {
        return Err(format!("Couldn't create the registry key HKEY_CURRENT_USER\\{path} (error {}).", r.0));
    }
    Ok(Key(key))
}

/// Sets the string value `name` (the key's default value for None).
fn set(key: &Key, name: Option<&str>, value: &str) -> Result<(), String> {
    let data: Vec<u8> = wide(OsStr::new(value)).iter().flat_map(|u| u.to_le_bytes()).collect();
    let name_w = name.map(|n| wide(OsStr::new(n)));
    let name_ptr = name_w.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr()));
    // SAFETY: the name is NUL-terminated (or null for the default value) and `data` holds the
    // NUL-terminated UTF-16 string; both outlive the call.
    let r = unsafe { RegSetValueExW(key.0, name_ptr, None, REG_SZ, Some(&data)) };
    if r.is_err() {
        return Err(format!("Couldn't write a registry value for trellis:// links (error {}).", r.0));
    }
    Ok(())
}

/// The default string value of the key at `path`; None when the key or value is missing.
fn default_value(path: &str) -> Result<Option<String>, String> {
    let p = wide(OsStr::new(path));
    let mut size = 0u32;
    // SAFETY: `p` is NUL-terminated; a null value name reads the default value; only the size
    // is asked for.
    let r = unsafe { RegGetValueW(HKEY_CURRENT_USER, PCWSTR(p.as_ptr()), PCWSTR::null(), RRF_RT_REG_SZ, None, None, Some(&mut size)) };
    if r == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if r.is_err() {
        return Err(format!("Couldn't read the registry key HKEY_CURRENT_USER\\{path} (error {}).", r.0));
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2) + 1];
    let mut bytes = (buf.len() * 2) as u32;
    // SAFETY: `buf` is writable for `bytes` bytes, as passed.
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(p.as_ptr()),
            PCWSTR::null(),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    if r == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if r.is_err() {
        return Err(format!("Couldn't read the registry key HKEY_CURRENT_USER\\{path} (error {}).", r.0));
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Ok(Some(String::from_utf16_lossy(&buf[..len])))
}

/// Deletes the key at `path` and everything under it; a missing key is fine.
pub(super) fn delete_tree(path: &str) -> Result<(), String> {
    let p = wide(OsStr::new(path));
    // SAFETY: `p` is NUL-terminated and outlives both calls.
    let tree = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(p.as_ptr())) };
    // SAFETY: as above.
    let key = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, PCWSTR(p.as_ptr())) };
    match [tree, key].into_iter().find(|r| r.is_err() && *r != ERROR_FILE_NOT_FOUND) {
        Some(r) => Err(format!("Couldn't remove the registry key HKEY_CURRENT_USER\\{path} (error {}).", r.0)),
        None => Ok(()),
    }
}

pub(super) fn status_at(root: &str, exe: &Path) -> Result<SchemeStatus, String> {
    let command = default_value(&format!(r"{root}\trellis\shell\open\command"))?;
    Ok(super::decide_windows(command.as_deref(), exe))
}

pub(super) fn register_at(root: &str, exe: &Path) -> Result<SchemeStatus, String> {
    let base = format!(r"{root}\trellis");
    let key = create(&base)?;
    set(&key, None, "URL:Trellis link")?;
    set(&key, Some("URL Protocol"), "")?;
    set(&create(&format!(r"{base}\DefaultIcon"))?, None, &format!("\"{}\",0", native(exe).display()))?;
    set(&create(&format!(r"{base}\shell\open\command"))?, None, &super::windows_command(exe))?;
    status_at(root, exe)
}

pub(super) fn unregister_at(root: &str) -> Result<(), String> {
    delete_tree(&format!(r"{root}\trellis"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deletes the scratch key even when an assertion fails.
    struct Scratch(String);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = delete_tree(&self.0);
        }
    }

    #[test]
    fn register_and_unregister_under_a_scratch_key() {
        let token = crate::askpass::protocol::new_token().unwrap();
        let root = Scratch(format!(r"Software\Trellis-tests-{token}"));
        let exe = crate::paths::temp_dir().join("Trellis Test").join("glowfly.exe");
        let other = crate::paths::temp_dir().join("Other").join("other.exe");
        assert_eq!(status_at(&root.0, &exe), Ok(SchemeStatus::NotRegistered));
        assert_eq!(register_at(&root.0, &exe), Ok(SchemeStatus::Registered));
        assert_eq!(default_value(&format!(r"{}\trellis", root.0)), Ok(Some("URL:Trellis link".to_owned())));
        register_at(&root.0, &other).unwrap();
        assert_eq!(status_at(&root.0, &exe), Ok(SchemeStatus::OtherProgram(super::super::windows_command(&other))));
        unregister_at(&root.0).unwrap();
        assert_eq!(status_at(&root.0, &exe), Ok(SchemeStatus::NotRegistered));
        unregister_at(&root.0).unwrap();
    }
}
