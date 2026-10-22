//! Before the window opens: an installed Windows copy takes the AppUserModelID its Start menu
//! shortcut carries (packaging/windows/glowfly.wxs), so its window groups with that shortcut and
//! its notifications show Glowfly's name and icon.
//!
//! Portable copies keep the id Windows derives: no shortcut registers Glowfly's id for them, so
//! taking it would split their taskbar button from a pinned copy and make their toasts vanish.

/// Takes Glowfly's app id when this is the installed copy. Called once, before any window exists;
/// does nothing elsewhere.
pub fn claim() {
    #[cfg(windows)]
    {
        use glowfly_core::app_update::{InstallKind, install_kind};
        use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
        use windows::core::HSTRING;
        if !std::env::current_exe().is_ok_and(|e| install_kind(&e) == InstallKind::Installer) {
            return;
        }
        // SAFETY: a plain call with a valid, NUL-terminated wide string that outlives it.
        if let Err(e) = unsafe { SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(glowfly_core::app_identity::APP_ID)) } {
            tracing::warn!("couldn't take Glowfly's app id; the taskbar may not group with its shortcut: {e}");
        }
    }
}
