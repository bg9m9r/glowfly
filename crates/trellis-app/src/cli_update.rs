//! `trellis --version` and `trellis --check-update <folder>`: the command-line modes the
//! installer checks in CI use (scripts/update-feed-check.ps1). They exit before logging,
//! settings and the window.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use trellis_core::app_update::{self, CURRENT_PLATFORM, CURRENT_VERSION, Checked, Feed, InstallKind, Version, check_and_download};
use trellis_core::native_path::shown;

pub const VERSION_FLAG: &str = "--version";
pub const CHECK_FLAG: &str = "--check-update";
/// `--check-update`'s exit status when there is no newer release.
pub const EXIT_UP_TO_DATE: i32 = 3;
const USAGE: &str = "usage: trellis --check-update <absolute folder to download into>";

/// Prints `trellis <version>`; a closed stdout is ignored.
pub fn print_version() -> i32 {
    print_lines(&[format!("trellis {CURRENT_VERSION}")]);
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

/// The folder to download into: exactly one argument, an absolute path.
fn download_dir(args: &[OsString]) -> Result<PathBuf, ()> {
    let [arg] = args else { return Err(()) };
    let dir = PathBuf::from(arg);
    if dir.is_absolute() { Ok(dir) } else { Err(()) }
}

/// `--check-update <folder>`: checks the feed and downloads a newer release's file for this
/// copy into `<folder>`. Exits 0 when one was downloaded and verified, 3 when there is no newer
/// release, 1 when the check or download failed and 2 on a usage or configuration error.
pub fn check_update(args: &[OsString]) -> i32 {
    let Ok(dir) = download_dir(args) else {
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
            eprintln!("error: Couldn't find Trellis's own program: {e}");
            return 1;
        }
    };
    let result = check_and_download(&feed, &current, CURRENT_PLATFORM, kind, &dir);
    let (code, out, err) = report(&feed, &current, kind, &result);
    print_lines(&out);
    if let Some(err) = err {
        eprintln!("{err}");
    }
    code
}

/// The exit status, stdout lines and stderr line for a check's `result`.
fn report(feed: &Feed, current: &Version, kind: InstallKind, result: &Result<Checked, String>) -> (i32, Vec<String>, Option<String>) {
    let install = match kind {
        InstallKind::Installer => "installer",
        InstallKind::Portable => "portable",
    };
    let mut out = vec![format!("current {current}"), format!("install {install}"), format!("feed {}", feed.describe())];
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
        Err(e) => (1, out, Some(format!("error: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trellis_core::app_update::{Asset, Offer, Release, SumsSource};

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap_or_else(|| panic!("{s} parses"))
    }

    #[test]
    fn the_download_folder_is_one_absolute_path() {
        let dl = trellis_core::paths::temp_dir().join("dl");
        assert_eq!(download_dir(&[]), Err(()));
        assert_eq!(download_dir(&[dl.clone().into(), dl.clone().into()]), Err(()));
        assert_eq!(download_dir(&["dl".into()]), Err(()));
        assert_eq!(download_dir(&[dl.clone().into()]), Ok(dl));
    }

    #[test]
    fn reports_lines_and_exit_codes() {
        let dir = trellis_core::paths::temp_dir().join("trellis-feed");
        let feed = Feed::Folder(dir.clone());
        let head = |kind: &str| vec!["current 0.1.0".to_owned(), format!("install {kind}"), format!("feed folder {}", shown(&dir))];

        let (code, out, err) = report(&feed, &v("0.1.0"), InstallKind::Portable, &Ok(Checked::UpToDate { latest: None }));
        let mut want = head("portable");
        want.push("up-to-date".into());
        assert_eq!((code, out, err), (3, want, None));
        assert_eq!(EXIT_UP_TO_DATE, 3);

        let name = "trellis-v0.1.1-x86_64-pc-windows-msvc.msi";
        let asset = Asset { name: name.into(), size: 1, url: String::new() };
        let release = Release {
            tag: "v0.1.1".into(),
            name: String::new(),
            html_url: String::new(),
            body: String::new(),
            published_at: None,
            assets: vec![],
        };
        let offer = Offer { release, version: v("0.1.1"), asset: Some(asset), sums: SumsSource::None };
        let path = dir.join("dl").join(name);
        let (code, out, err) =
            report(&feed, &v("0.1.0"), InstallKind::Installer, &Ok(Checked::Downloaded { offer: Box::new(offer), path: path.clone() }));
        let mut want = head("installer");
        want.push(format!("offer 0.1.1 {name}"));
        want.push(format!("downloaded {}", shown(&path)));
        assert_eq!((code, out, err), (0, want, None));

        let (code, out, err) =
            report(&feed, &v("0.1.0"), InstallKind::Portable, &Err("This release has no download for this system.".into()));
        assert_eq!((code, out), (1, head("portable")));
        assert_eq!(err.as_deref(), Some("error: This release has no download for this system."));
    }
}
