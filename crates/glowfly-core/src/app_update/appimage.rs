//! Running from an AppImage: the AppImage runtime mounts (or extracts) the image to a folder,
//! sets `APPDIR` to it and `APPIMAGE` to the image file's own path, and runs the program from
//! inside that folder. The image file is what an update replaces and what starts again.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The AppImage file a program at `exe` runs from, given the `APPIMAGE` and `APPDIR` values:
/// Some only when both are absolute and `exe` lies inside `APPDIR` (by whole components), so a
/// variable inherited by an unrelated program is ignored.
pub fn appimage_from(exe: &Path, appimage: Option<&OsStr>, appdir: Option<&OsStr>) -> Option<PathBuf> {
    let (image, dir) = (Path::new(appimage?), Path::new(appdir?));
    let absolute = |p: &Path| !p.as_os_str().is_empty() && p.is_absolute();
    (absolute(image) && absolute(dir) && exe.starts_with(dir)).then(|| image.to_owned())
}

/// The AppImage file the program at `exe` runs from, read from the environment. Always None
/// outside Linux.
pub fn appimage_file(exe: &Path) -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    appimage_from(exe, std::env::var_os("APPIMAGE").as_deref(), std::env::var_os("APPDIR").as_deref())
}

/// Whether `bytes` start like a type-2 AppImage: the ELF magic, then `AI` and 2 at byte 8.
pub fn is_appimage(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x7f, b'E', b'L', b'F']) && bytes.get(8..11) == Some(&[b'A', b'I', 0x02][..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_image_counts_only_when_the_program_runs_inside_it() {
        let root = crate::paths::temp_dir();
        let image = root.join("apps").join("Glowfly.AppImage");
        let mount = root.join(".mount_ab");
        let exe = mount.join("usr").join("bin").join("glowfly");
        let os = |p: &Path| Some(p.as_os_str().to_owned());
        let from =
            |exe: &Path, a: Option<std::ffi::OsString>, d: Option<std::ffi::OsString>| appimage_from(exe, a.as_deref(), d.as_deref());
        assert_eq!(from(&exe, os(&image), os(&mount)), Some(image.clone()));
        // Elsewhere: an inherited variable.
        assert_eq!(from(&root.join("bin").join("glowfly"), os(&image), os(&mount)), None);
        // A string prefix that isn't a component prefix.
        let longer = root.join(".mount_abc").join("usr").join("bin").join("glowfly");
        assert_eq!(from(&longer, os(&image), os(&mount)), None);
        // Relative, empty or missing values.
        assert_eq!(from(&exe, Some("Glowfly.AppImage".into()), os(&mount)), None);
        assert_eq!(from(Path::new("usr/bin/glowfly"), os(&image), Some("usr".into())), None);
        assert_eq!(from(&exe, Some("".into()), os(&mount)), None);
        assert_eq!(from(&exe, os(&image), Some("".into())), None);
        assert_eq!(from(&exe, None, os(&mount)), None);
        assert_eq!(from(&exe, os(&image), None), None);
    }

    #[test]
    fn an_appimage_is_known_by_its_magic() {
        let mut image = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0, b'A', b'I', 2, 0];
        assert!(is_appimage(&image));
        assert!(is_appimage(&image[..11]));
        image[10] = 1;
        assert!(!is_appimage(&image), "type 1");
        assert!(!is_appimage(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0]), "a plain ELF program");
        assert!(!is_appimage(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, b'A']), "too short");
        assert!(!is_appimage(b"PK\x03\x04\x14\0\0\0AI\x02"), "a zip");
        assert!(!is_appimage(&[]));
    }
}
