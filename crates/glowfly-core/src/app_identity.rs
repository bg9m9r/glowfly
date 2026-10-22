//! Glowfly's identity with the operating system: the id its installers register and the Linux
//! desktop entry's name. The Flatpak exports its desktop file and icon under [`APP_ID`], so inside
//! it the desktop entry is that id ([`desktop_entry`]).

/// The Windows AppUserModelID on the MSI's shortcuts (packaging/windows/glowfly.wxs) and the
/// macOS CFBundleIdentifier (packaging/macos/Info.plist): what installed copies post desktop
/// notifications under.
pub const APP_ID: &str = "io.github.bg9m9r.glowfly";

/// packaging/linux/glowfly.desktop's name, the Wayland app id and the hicolor icon name.
pub const DESKTOP_ENTRY: &str = "glowfly";

/// The desktop entry (Wayland app id, X11 class, icon name) for a copy inside Flatpak or not.
pub fn desktop_entry_for(flatpak: bool) -> &'static str {
    if flatpak { APP_ID } else { DESKTOP_ENTRY }
}

/// This copy's desktop entry: [`APP_ID`] inside Flatpak, else [`DESKTOP_ENTRY`].
pub fn desktop_entry() -> &'static str {
    desktop_entry_for(crate::flatpak::in_flatpak())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file of the repository's `packaging/` folder.
    fn packaging(file: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging").join(file);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    #[test]
    fn the_bundle_id_is_glowfly_app_id() {
        let plist = packaging("macos/Info.plist");
        let lines: Vec<&str> = plist.lines().map(str::trim).collect();
        let key = lines.iter().position(|l| *l == "<key>CFBundleIdentifier</key>").expect("CFBundleIdentifier");
        assert_eq!(lines.get(key + 1).copied(), Some(format!("<string>{APP_ID}</string>").as_str()));
    }

    #[test]
    fn the_installer_shortcuts_carry_the_app_id() {
        let wxs = packaging("windows/glowfly.wxs");
        let property = format!("<ShortcutProperty Key=\"System.AppUserModel.ID\" Value=\"{APP_ID}\" />");
        let at: Vec<usize> = wxs.match_indices(&property).map(|(i, _)| i).collect();
        assert_eq!(at.len(), 2, "one on each shortcut");
        for (id, &i) in ["GlowflyShortcut", "GlowflyDesktopShortcut"].iter().zip(&at) {
            let open = wxs[..i].rfind("<Shortcut ").expect("inside a Shortcut");
            assert!(wxs[open..i].contains(&format!("Id=\"{id}\"")), "{id} carries the id");
            assert!(!wxs[open..i].contains("/>") && wxs[i..].find("</Shortcut>").is_some(), "{id} is still open");
        }
    }

    /// The `Key=` lines of a desktop file, for comparing two of them.
    fn lines_of(text: &str, key: &str) -> Vec<String> {
        text.lines().map(str::trim).filter(|l| l.starts_with(&format!("{key}="))).map(str::to_owned).collect()
    }

    #[test]
    fn the_flatpak_takes_the_app_id_as_its_entry() {
        assert_eq!(desktop_entry_for(false), DESKTOP_ENTRY);
        assert_eq!(desktop_entry_for(true), APP_ID);
    }

    #[test]
    fn the_flatpak_desktop_entry_is_the_app_id() {
        let entry = packaging(&format!("flatpak/{APP_ID}.desktop"));
        let lines: Vec<&str> = entry.lines().map(str::trim).collect();
        for want in
            [format!("Icon={APP_ID}"), format!("StartupWMClass={APP_ID}"), "Exec=glowfly %f".into(), "MimeType=inode/directory;".into()]
        {
            assert!(lines.contains(&want.as_str()), "{want}");
        }
        let linux = packaging(&format!("linux/{DESKTOP_ENTRY}.desktop"));
        for key in ["Name", "GenericName", "Comment", "Categories", "Keywords"] {
            assert!(!lines_of(&linux, key).is_empty(), "{key}");
            assert_eq!(lines_of(&entry, key), lines_of(&linux, key), "{key}");
        }
    }

    #[test]
    fn the_metainfo_names_the_app() {
        let xml = packaging(&format!("flatpak/{APP_ID}.metainfo.xml.in"));
        for want in [
            format!("<id>{APP_ID}</id>"),
            format!("<launchable type=\"desktop-id\">{APP_ID}.desktop</launchable>"),
            "<binary>glowfly</binary>".into(),
            "MIT OR Apache-2.0".into(),
            "@VERSION@".into(),
            "@DATE@".into(),
        ] {
            assert!(xml.contains(&want), "{want}");
        }
    }

    #[test]
    fn the_desktop_entry_is_glowfly() {
        let entry = packaging(&format!("linux/{DESKTOP_ENTRY}.desktop"));
        let lines: Vec<&str> = entry.lines().map(str::trim).collect();
        assert!(lines.contains(&format!("Icon={DESKTOP_ENTRY}").as_str()));
        assert!(lines.contains(&format!("StartupWMClass={DESKTOP_ENTRY}").as_str()));
    }
}
