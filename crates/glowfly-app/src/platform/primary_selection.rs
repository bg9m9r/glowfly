//! The PRIMARY selection of Linux desktops: the terminal's last mouse selection is offered to
//! other programs there, and a middle click pastes it. On Wayland it goes through the
//! compositor's primary-selection protocol (smithay-clipboard), on X11 through the PRIMARY atom
//! (arboard); both crates are the ones the window's own clipboard already uses (egui-winit), so
//! nothing new is linked. Windows and macOS have no PRIMARY: setting it does nothing there, and
//! a paste from it falls back to the clipboard.

/// Where a middle click's paste comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteFrom {
    Primary,
    Clipboard,
}

/// What a middle click pastes: the PRIMARY selection where the display has one, else the
/// clipboard (a display whose PRIMARY couldn't be opened).
pub fn middle_click_reads(has_primary: bool) -> PasteFrom {
    if has_primary { PasteFrom::Primary } else { PasteFrom::Clipboard }
}

/// The window's PRIMARY selection, when the display has one.
#[derive(Default)]
pub struct PrimarySelection {
    #[cfg(target_os = "linux")]
    backend: Option<linux::Backend>,
}

impl PrimarySelection {
    /// The PRIMARY selection of the display the window `cc` opened on: Wayland's when the window
    /// is a Wayland one, else X11's. None elsewhere, or when neither can be opened.
    pub fn install(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(target_os = "linux")]
        {
            use raw_window_handle::HasDisplayHandle;
            let display = cc.display_handle().ok().map(|h| h.as_raw());
            Self { backend: linux::Backend::open(display) }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = cc;
            Self::default()
        }
    }

    /// Whether there is a PRIMARY selection to set and read.
    pub fn available(&self) -> bool {
        #[cfg(target_os = "linux")]
        return self.backend.is_some();
        #[cfg(not(target_os = "linux"))]
        false
    }

    /// Offers `text` as the PRIMARY selection; nothing where there is none.
    pub fn set(&mut self, text: String) {
        #[cfg(target_os = "linux")]
        if let Some(b) = &mut self.backend {
            b.set(text);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = text;
    }

    /// The PRIMARY selection's text: None when nothing (or no text) is selected anywhere, or
    /// there is no PRIMARY. It is read on the UI thread, as the window's clipboard paste is.
    pub fn get(&mut self) -> Option<String> {
        #[cfg(target_os = "linux")]
        return self.backend.as_mut().and_then(linux::Backend::get).filter(|t| !t.is_empty());
        #[cfg(not(target_os = "linux"))]
        None
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use arboard::{GetExtLinux, LinuxClipboardKind, SetExtLinux};
    use raw_window_handle::RawDisplayHandle;

    pub(super) enum Backend {
        Wayland(smithay_clipboard::Clipboard),
        X11(arboard::Clipboard),
    }

    impl Backend {
        pub(super) fn open(display: Option<RawDisplayHandle>) -> Option<Backend> {
            if let Some(RawDisplayHandle::Wayland(w)) = display {
                // SAFETY: the pointer is the window's live wl_display, which outlives the app:
                // the shell drops this in `on_exit`, before eframe closes the window.
                return Some(Backend::Wayland(unsafe { smithay_clipboard::Clipboard::new(w.display.as_ptr()) }));
            }
            match arboard::Clipboard::new() {
                Ok(c) => Some(Backend::X11(c)),
                Err(e) => {
                    tracing::warn!("terminal: no PRIMARY selection: {e}");
                    None
                }
            }
        }

        pub(super) fn set(&mut self, text: String) {
            match self {
                Backend::Wayland(c) => c.store_primary(text),
                Backend::X11(c) => {
                    if let Err(e) = c.set().clipboard(LinuxClipboardKind::Primary).text(text) {
                        tracing::debug!("terminal: setting PRIMARY failed: {e}");
                    }
                }
            }
        }

        pub(super) fn get(&mut self) -> Option<String> {
            match self {
                Backend::Wayland(c) => c.load_primary().ok(),
                Backend::X11(c) => c.get().clipboard(LinuxClipboardKind::Primary).text().ok(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_click_reads_primary_where_there_is_one() {
        assert_eq!(middle_click_reads(true), PasteFrom::Primary);
        assert_eq!(middle_click_reads(false), PasteFrom::Clipboard, "no PRIMARY could be opened");
    }

    #[test]
    fn without_a_display_there_is_no_primary() {
        let mut p = PrimarySelection::default();
        assert!(!p.available());
        p.set("ignored".into());
        assert_eq!(p.get(), None);
        assert_eq!(middle_click_reads(p.available()), PasteFrom::Clipboard);
    }
}
