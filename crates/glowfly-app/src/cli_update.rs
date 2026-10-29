//! `glowfly --version` and `glowfly --check-update <folder>`: the command-line modes the
//! installer checks in CI use (scripts/update-feed-check.ps1); and `glowfly --finish-update
//! <plan>`, the helper Install and restart starts. They exit before logging, settings and the
//! window.

use glowfly_core::app_update::{self, CURRENT_VERSION, Checked, Feed, InstallKind, Version, check_and_download};
use glowfly_core::native_path::shown;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const VERSION_FLAG: &str = app_update::VERSION_FLAG;
pub const CHECK_FLAG: &str = "--check-update";
pub const FINISH_FLAG: &str = app_update::FINISH_FLAG;
/// `--check-update`'s exit status when there is no newer release.
pub const EXIT_UP_TO_DATE: i32 = 3;
/// `--check-update`'s exit status inside Flatpak when there is a newer release, which Flatpak
/// installs (nothing is downloaded).
pub const EXIT_UPDATE_BY_FLATPAK: i32 = 4;
const USAGE: &str = "usage: glowfly --check-update <absolute folder to download into>";
const FINISH_USAGE: &str = "usage: glowfly --finish-update <absolute path of the update plan>";

/// Prints `glowfly <version>`; a closed stdout is ignored.
pub fn print_version() -> i32 {
    print_lines(&[format!("glowfly {CURRENT_VERSION}")]);
    0
}

fn print_lines(lines: &[String]) {
    let mut out = std::io::stdout().lock();
    for line in lines {
        if writeln!(out, "{line}").is_err() {
            return;
        }
    }
    let _ = out.flush();
}

/// The one argument of a mode that takes a folder or file: an absolute path.
fn one_absolute_path(args: &[OsString]) -> Result<PathBuf, ()> {
    let [arg] = args else { return Err(()) };
    let dir = PathBuf::from(arg);
    if dir.is_absolute() { Ok(dir) } else { Err(()) }
}

/// `--finish-update <plan>`: the update helper, which waits for Glowfly to quit on its stdin
/// and then carries out the plan. Exits 2 on a usage error.
pub fn finish_update(args: &[OsString]) -> i32 {
    let Ok(plan) = one_absolute_path(args) else {
        eprintln!("{FINISH_USAGE}");
        return 2;
    };
    app_update::finish_update(&plan, std::io::stdin())
}

/// `--check-update <folder>`: checks the feed and downloads a newer release's file for this
/// copy into `<folder>`. Exits 0 when one was downloaded and verified, 3 when there is no newer
/// release, 4 when there is one that Flatpak installs (a Flatpak copy writes nothing to
/// `<folder>`), 1 when the check or download failed and 2 on a usage or configuration error.
pub fn check_update(args: &[OsString]) -> i32 {
    let Ok(dir) = one_absolute_path(args) else {
        eprintln!("{USAGE}");
        return 2;
    };
    let feed = match Feed::from_env() {
        Ok(feed) => feed,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let Some(current) = Version::parse(CURRENT_VERSION) else {
        eprintln!("error: this build's version {CURRENT_VERSION} isn't a version");
        return 1;
    };
    let kind = match std::env::current_exe() {
        Ok(exe) => app_update::install_kind(&exe),
        Err(e) => {
            eprintln!("error: Couldn't find Glowfly's own program: {e}");
            return 1;
        }
    };
    let result = check_and_download(&feed, &current, app_update::current_platform(), kind, &dir);
    let (code, out, err) = report(&feed, &current, kind, &result);
    print_lines(&out);
    if let Some(err) = err {
        eprintln!("{err}");
    }
    code
}

/// The exit status, stdout lines and stderr line for a check's `result`.
fn report(feed: &Feed, current: &Version, kind: InstallKind, result: &Result<Checked, String>) -> (i32, Vec<String>, Option<String>) {
    let mut out = vec![format!("current {current}"), format!("install {}", kind.label()), format!("feed {}", feed.describe())];
    match result {
        Ok(Checked::UpToDate { .. }) => {
            out.push("up-to-date".to_owned());
            (EXIT_UP_TO_DATE, out, None)
        }
        Ok(Checked::Downloaded { offer, path }) => {
            let name = offer.asset.as_ref().map_or("", |a| a.name.as_str());
            out.push(format!("offer {} {name}", offer.version));
            out.push(format!("downloaded {}", shown(Path::new(path))));
            (0, out, None)
        }
        Ok(Checked::UpdateWithFlatpak { offer }) => {
            out.push(format!("offer {}", offer.version));
            out.push("update-with flatpak".to_owned());
            (EXIT_UPDATE_BY_FLATPAK, out, None)
        }
        Err(e) => (1, out, Some(format!("error: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::app_update::{Asset, Offer, Release, SumsSource};

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap_or_else(|| panic!("{s} parses"))
    }

    #[test]
    fn the_download_folder_is_one_absolute_path() {
        let dl = glowfly_core::paths::temp_dir().join("dl");
        assert_eq!(one_absolute_path(&[]), Err(()));
        assert_eq!(one_absolute_path(&[dl.clone().into(), dl.clone().into()]), Err(()));
        assert_eq!(one_absolute_path(&["dl".into()]), Err(()));
        assert_eq!(one_absolute_path(&[dl.clone().into()]), Ok(dl));
    }

    #[test]
    fn the_update_plan_is_one_absolute_path() {
        let plan = glowfly_core::paths::temp_dir().join("plan.json");
        assert_eq!(finish_update(&[]), 2);
        assert_eq!(finish_update(&[plan.clone().into(), plan.into()]), 2);
        assert_eq!(finish_update(&["plan.json".into()]), 2);
    }

    #[test]
    fn reports_lines_and_exit_codes() {
        let dir = glowfly_core::paths::temp_dir().join("glowfly-feed");
        let feed = Feed::Folder(dir.clone());
        let head = |kind: &str| vec!["current 0.1.0".to_owned(), format!("install {kind}"), format!("feed folder {}", shown(&dir))];

        let (code, out, err) = report(&feed, &v("0.1.0"), InstallKind::Portable, &Ok(Checked::UpToDate { latest: None }));
        let mut want = head("portable");
        want.push("up-to-date".into());
        assert_eq!((code, out, err), (3, want, None));
        assert_eq!(EXIT_UP_TO_DATE, 3);

        let name = "glowfly-v0.1.1-x86_64-pc-windows-msvc.msi";
        let asset = Asset { name: name.into(), size: 1, url: String::new() };
        let release = Release {
            tag: "v0.1.1".into(),
            name: String::new(),
            html_url: String::new(),
            body: String::new(),
            published_at: None,
            assets: vec![],
        };
        let offer = Offer { release, version: v("0.1.1"), asset: Some(asset), sums: SumsSource::None, install: InstallKind::Installer };
        let path = dir.join("dl").join(name);
        let (code, out, err) = report(
            &feed,
            &v("0.1.0"),
            InstallKind::Installer,
            &Ok(Checked::Downloaded { offer: Box::new(offer.clone()), path: path.clone() }),
        );
        let mut want = head("installer");
        want.push(format!("offer 0.1.1 {name}"));
        want.push(format!("downloaded {}", shown(&path)));
        assert_eq!((code, out, err), (0, want, None));

        let flatpak = Offer { asset: None, install: InstallKind::Flatpak, ..offer.clone() };
        let (code, out, err) =
            report(&feed, &v("0.1.0"), InstallKind::Flatpak, &Ok(Checked::UpdateWithFlatpak { offer: Box::new(flatpak) }));
        let mut want = head("flatpak");
        want.extend(["offer 0.1.1".to_owned(), "update-with flatpak".to_owned()]);
        assert_eq!((code, out, err), (4, want, None));
        assert_eq!(EXIT_UPDATE_BY_FLATPAK, 4);

        let (code, out, err) =
            report(&feed, &v("0.1.0"), InstallKind::Portable, &Err("This release has no download for this system.".into()));
        assert_eq!((code, out), (1, head("portable")));
        assert_eq!(err.as_deref(), Some("error: This release has no download for this system."));

        for (kind, word) in [(InstallKind::RpmPackage, "rpm"), (InstallKind::AppImage, "appimage"), (InstallKind::Flatpak, "flatpak")] {
            let (code, out, _) = report(&feed, &v("0.1.0"), kind, &Ok(Checked::UpToDate { latest: None }));
            assert_eq!((code, out[1].as_str()), (3, format!("install {word}").as_str()));
        }
    }
}
