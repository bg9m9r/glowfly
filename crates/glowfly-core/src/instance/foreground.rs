//! Letting the running window come to the front.

/// Windows gives the foreground only to the process the user last used: this process lets
/// `pid` take it. Failure only means the window may flash in the taskbar instead.
#[cfg(windows)]
pub(super) fn allow(pid: u32) {
    use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;
    // SAFETY: the call takes a plain process id and touches no memory of ours.
    let _ = unsafe { AllowSetForegroundWindow(pid) };
}

/// Other systems need no permission from the sender.
#[cfg(not(windows))]
pub(super) fn allow(_pid: u32) {}
