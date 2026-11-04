//! The system's reduce-motion, high-contrast and text-size settings (`docs/PLATFORMS.md`,
//! "Accessibility settings"), read for the shell into a `SystemA11y`:
//! - Windows: `SPI_GETCLIENTAREAANIMATION` (animations off is less motion), `SPI_GETHIGHCONTRAST`
//!   (a contrast theme) and `TextScaleFactor` under `HKCU\Software\Microsoft\Accessibility`
//!   (Settings > Accessibility > Text size, 100 to 225).
//! - macOS: `NSWorkspace`'s `accessibilityDisplayShouldReduceMotion` and
//!   `accessibilityDisplayShouldIncreaseContrast`; macOS has no system text size for apps, so
//!   the text-size follow check is hidden there (`text_size_followable`).
//! - Linux: gsettings `org.gnome.desktop.interface enable-animations` and `text-scaling-factor`,
//!   and `org.gnome.desktop.a11y.interface high-contrast`. The text size counts only on Wayland:
//!   on X11 GNOME already puts it in `Xft.dpi`, which winit's scale factor includes.
//!
//! A `Watch` runs its reader (the OS's, or a stand-in in tests) when the shell asks: inline on
//! Windows and macOS, where the reads are cheap system calls, and on its own thread on Linux,
//! where gsettings is a child process. The shell takes the result at the start of a frame.

use eframe::egui;
use glowfly_model::state::system_a11y::SystemA11y;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, Sender};

/// Reads the system's settings when asked and hands them to the shell.
pub struct Watch {
    read: fn() -> SystemA11y,
    tx: Sender<SystemA11y>,
    rx: Receiver<SystemA11y>,
    /// A read runs on its thread: another ask waits for it rather than starting a second.
    reading: Arc<AtomicBool>,
}

impl Watch {
    /// A watch that reads with `read` (`read_system` in the app).
    pub fn new(read: fn() -> SystemA11y) -> Self {
        let (tx, rx) = mpsc::channel();
        Self { read, tx, rx, reading: Arc::new(AtomicBool::new(false)) }
    }

    /// Reads the settings again, waking `ctx` once they are in.
    pub fn refresh(&self, ctx: &egui::Context) {
        if !cfg!(target_os = "linux") {
            let _ = self.tx.send((self.read)());
            ctx.request_repaint();
            return;
        }
        if self.reading.swap(true, Relaxed) {
            return;
        }
        let (read, tx, reading, ctx) = (self.read, self.tx.clone(), self.reading.clone(), ctx.clone());
        let started = std::thread::Builder::new().name("system-a11y".into()).spawn(move || {
            let _ = tx.send(read());
            reading.store(false, Relaxed);
            ctx.request_repaint();
        });
        if let Err(e) = started {
            self.reading.store(false, Relaxed);
            tracing::debug!("could not read the system's accessibility settings: {e}");
        }
    }

    /// The latest settings read since the last call.
    pub fn take(&self) -> Option<SystemA11y> {
        self.rx.try_iter().last()
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
/// The text size Windows stores as a percent (`TextScaleFactor`, 100 to 225) as a factor; 1.0
/// when it is missing or out of range.
fn windows_text_scale(percent: Option<u32>) -> f32 {
    match percent {
        Some(p @ 100..=225) => p as f32 / 100.0,
        _ => 1.0,
    }
}

#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
/// A gsettings boolean as printed (`true` or `false`).
fn gsettings_bool(out: &str) -> Option<bool> {
    match out.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
/// GNOME's `text-scaling-factor` as printed (`1.25`), kept to 0.5 to 3, as GNOME's own range.
fn gsettings_scale(out: &str) -> Option<f32> {
    out.trim().parse::<f32>().ok().filter(|s| s.is_finite() && (0.5..=3.0).contains(s))
}

/// The system's settings, read from the OS.
pub fn read_system() -> SystemA11y {
    os::read()
}

#[cfg(windows)]
mod os {
    use super::SystemA11y;
    use std::ffi::c_void;
    use windows::Win32::System::Registry::HKEY_CURRENT_USER;
    use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    use windows::core::BOOL;

    pub(super) fn read() -> SystemA11y {
        let scale = glowfly_core::win_registry::dword(HKEY_CURRENT_USER, r"Software\Microsoft\Accessibility", "TextScaleFactor");
        SystemA11y {
            reduce_motion: !animations_on(),
            high_contrast: high_contrast_on(),
            text_scale: super::windows_text_scale(scale),
            text_size_followable: true,
        }
    }

    /// Settings > Accessibility > Visual effects > Animation effects (on when it can't be read).
    fn animations_on() -> bool {
        let mut on = BOOL::from(true);
        // SAFETY: `on` is a BOOL, writable for the call, as SPI_GETCLIENTAREAANIMATION expects.
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some((&raw mut on).cast::<c_void>()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .is_ok();
        !ok || on.as_bool()
    }

    /// Whether a Windows high-contrast (contrast) theme is on.
    pub(crate) fn high_contrast_on() -> bool {
        let size = std::mem::size_of::<HIGHCONTRASTW>() as u32;
        let mut hc = HIGHCONTRASTW { cbSize: size, ..Default::default() };
        // SAFETY: `hc` is a HIGHCONTRASTW with its size set, writable for the call.
        let ok = unsafe {
            SystemParametersInfoW(SPI_GETHIGHCONTRAST, size, Some((&raw mut hc).cast::<c_void>()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0))
        }
        .is_ok();
        ok && (hc.dwFlags & HCF_HIGHCONTRASTON) == HCF_HIGHCONTRASTON
    }
}

#[cfg(windows)]
pub(crate) use os::high_contrast_on;

#[cfg(target_os = "macos")]
mod os {
    use super::SystemA11y;
    use objc2_app_kit::NSWorkspace;

    pub(super) fn read() -> SystemA11y {
        let ws = NSWorkspace::sharedWorkspace();
        SystemA11y {
            reduce_motion: ws.accessibilityDisplayShouldReduceMotion(),
            high_contrast: ws.accessibilityDisplayShouldIncreaseContrast(),
            text_scale: 1.0,
            text_size_followable: false,
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod os {
    use super::{SystemA11y, gsettings_bool, gsettings_scale};
    use std::process::Command;

    const INTERFACE: &str = "org.gnome.desktop.interface";

    pub(super) fn read() -> SystemA11y {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
        let scale = if wayland { get(INTERFACE, "text-scaling-factor").as_deref().and_then(gsettings_scale) } else { None };
        SystemA11y {
            reduce_motion: get(INTERFACE, "enable-animations").as_deref().and_then(gsettings_bool) == Some(false),
            high_contrast: get("org.gnome.desktop.a11y.interface", "high-contrast").as_deref().and_then(gsettings_bool) == Some(true),
            text_scale: scale.unwrap_or(1.0),
            text_size_followable: true,
        }
    }

    /// `gsettings get <schema> <key>`'s output; None when gsettings is missing, the schema isn't
    /// installed (outside GNOME) or it fails.
    fn get(schema: &str, key: &str) -> Option<String> {
        let mut cmd = Command::new("gsettings");
        cmd.args(["get", schema, key]);
        glowfly_core::env_path::apply(&mut cmd);
        let out = cmd.output().ok().filter(|o| o.status.success())?;
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_text_size_is_a_percent_in_its_range() {
        assert_eq!(windows_text_scale(None), 1.0);
        assert_eq!(windows_text_scale(Some(100)), 1.0);
        assert_eq!(windows_text_scale(Some(150)), 1.5);
        assert_eq!(windows_text_scale(Some(225)), 2.25);
        assert_eq!(windows_text_scale(Some(99)), 1.0, "out of range");
        assert_eq!(windows_text_scale(Some(400)), 1.0, "out of range");
    }

    #[test]
    fn gsettings_values_parse_as_printed() {
        assert_eq!(gsettings_bool("true\n"), Some(true));
        assert_eq!(gsettings_bool("false\n"), Some(false));
        assert_eq!(gsettings_bool("No such schema"), None);
        assert_eq!(gsettings_scale("1.25\n"), Some(1.25));
        assert_eq!(gsettings_scale("1.0"), Some(1.0));
        assert_eq!(gsettings_scale("9.0"), None, "out of range");
        assert_eq!(gsettings_scale("nan"), None);
        assert_eq!(gsettings_scale(""), None);
    }

    #[test]
    fn a_watch_hands_over_what_its_reader_read() {
        let watch = Watch::new(|| SystemA11y { reduce_motion: true, high_contrast: true, text_scale: 1.5, text_size_followable: false });
        assert_eq!(watch.take(), None, "nothing read yet");
        let ctx = egui::Context::default();
        watch.refresh(&ctx);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let got = loop {
            if let Some(v) = watch.take() {
                break Some(v);
            }
            if std::time::Instant::now() > deadline {
                break None;
            }
            std::thread::yield_now();
        };
        assert_eq!(got, Some(SystemA11y { reduce_motion: true, high_contrast: true, text_scale: 1.5, text_size_followable: false }));
        assert_eq!(watch.take(), None, "taken once");
    }
}
