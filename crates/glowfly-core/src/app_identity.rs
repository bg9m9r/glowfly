//! Glowfly's identity with the operating system: the id its installers register and the Linux
//! desktop entry's name.

/// The Windows AppUserModelID on the MSI's shortcuts (packaging/windows/glowfly.wxs) and the
/// macOS CFBundleIdentifier (packaging/macos/Info.plist): what installed copies post desktop
/// notifications under.
pub const APP_ID: &str = "io.github.bg9m9r.glowfly";

/// packaging/linux/glowfly.desktop's name, the Wayland app id and the hicolor icon name.
pub const DESKTOP_ENTRY: &str = "glowfly";

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

    #[test]
    fn the_desktop_entry_is_glowfly() {
        let entry = packaging(&format!("linux/{DESKTOP_ENTRY}.desktop"));
        let lines: Vec<&str> = entry.lines().map(str::trim).collect();
        assert!(lines.contains(&format!("Icon={DESKTOP_ENTRY}").as_str()));
        assert!(lines.contains(&format!("StartupWMClass={DESKTOP_ENTRY}").as_str()));
    }
}
