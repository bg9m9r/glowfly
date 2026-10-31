//! The app icon: the window's (Windows and X11; macOS takes the bundle's icon, Wayland the
//! desktop entry's) and the welcome screen's mark beside the wordmark (`app_mark`).

use eframe::egui::IconData;
use std::sync::Arc;

/// Glowfly's icon at 256 px (`assets/icon`, drawn by `tests/app_icon.rs`).
pub fn app_icon() -> Arc<IconData> {
    let png = include_bytes!("../../../assets/icon/glowfly-256.png");
    Arc::new(eframe::icon_data::from_png_bytes(png).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_icon_decodes() {
        let icon = super::app_icon();
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }
}
