use super::*;
use crate::app_update::sha256;

/// A fixture: a folder with a "program" (standing in for the running exe), a temp folder for
/// staging, and a downloads folder.
struct Fixture {
    root: PathBuf,
    exe: PathBuf,
    temp: PathBuf,
    downloads: PathBuf,
}

fn fixture(tag: &str) -> Fixture {
    let root = crate::paths::temp_dir().join(format!("glowfly-restart-{tag}-{}", std::process::id()));
    let _ = persist::remove_tree(&root);
    let (app, temp, downloads) = (root.join("app"), root.join("temp"), root.join("downloads"));
    for d in [&app, &temp, &downloads] {
        std::fs::create_dir_all(d).unwrap();
    }
    let exe = app.join("glowfly.exe");
    std::fs::write(&exe, "old program").unwrap();
    Fixture { root, exe, temp, downloads }
}

fn release_zip() -> Vec<u8> {
    use std::io::Cursor;
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    w.start_file("glowfly-v0.3.0-x86_64-pc-windows-msvc/glowfly.exe", opts).unwrap();
    w.write_all(b"new program").unwrap();
    w.finish().unwrap().into_inner()
}

fn request(f: &Fixture, file: &Path, bytes: &[u8], method: InstallMethod) -> RestartRequest {
    std::fs::write(file, bytes).unwrap();
    RestartRequest {
        file: file.to_owned(),
        sha256: sha256(bytes),
        method,
        version: "0.3.0".into(),
        exe: f.exe.clone(),
        target: f.exe.clone(),
        places: vec![f.temp.clone()],
        os: TargetOs::Windows,
    }
}

fn staging_folders(f: &Fixture) -> Vec<PathBuf> {
    std::fs::read_dir(&f.temp).unwrap().map(|e| e.unwrap().path()).collect()
}

#[test]
fn a_portable_swap_is_prepared_and_can_be_abandoned() {
    let f = fixture("swap");
    let req = request(&f, &f.downloads.join("glowfly-v0.3.0-x86_64-pc-windows-msvc.zip"), &release_zip(), InstallMethod::SwapPortable);
    let p = prepare_with(&req, &|_| Ok(())).unwrap();
    assert_eq!(std::fs::read(&f.exe).unwrap(), b"new program");
    let bak = f.exe.with_file_name("glowfly.exe.bak");
    assert_eq!(std::fs::read(&bak).unwrap(), b"old program");
    assert_eq!(staging_folders(&f), std::slice::from_ref(&p.staging));
    assert_eq!(p.helper, p.staging.join("glowfly-update.exe"));
    assert_eq!(std::fs::read(&p.helper).unwrap(), b"old program", "the helper is the running program");
    let plan = Plan::read(&p.plan_path).unwrap();
    let record = f.exe.with_file_name("glowfly.exe.bak.record");
    assert!(super::super::backups::recorded(&bak), "the backup is recorded");
    assert_eq!((plan.remove.clone(), plan.relaunch.clone(), plan.msi.clone()), (vec![bak.clone()], f.exe.clone(), None));
    assert!(record.is_file(), "the record is beside the backup");
    assert_eq!((plan.version.as_str(), plan.staging.as_path()), ("0.3.0", p.staging.as_path()));
    p.abandon().unwrap();
    assert_eq!(std::fs::read(&f.exe).unwrap(), b"old program");
    assert!(!bak.exists() && !record.exists());
    assert!(staging_folders(&f).is_empty());
    let _ = persist::remove_tree(&f.root);
}

#[test]
fn an_msi_is_staged_without_touching_the_program() {
    let f = fixture("msi");
    let req = request(&f, &f.downloads.join("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi"), b"pretend msi", InstallMethod::RunMsi);
    let p = prepare_with(&req, &|_| Ok(())).unwrap();
    assert_eq!(std::fs::read(&f.exe).unwrap(), b"old program");
    let copy = p.staging.join("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi");
    assert_eq!(std::fs::read(&copy).unwrap(), b"pretend msi");
    let plan = Plan::read(&p.plan_path).unwrap();
    assert_eq!(plan.msi, Some(MsiStep { file: copy, log: p.staging.join("install.log") }));
    assert!(plan.remove.is_empty());
    p.abandon().unwrap();
    assert!(staging_folders(&f).is_empty());
    let _ = persist::remove_tree(&f.root);
}

#[test]
fn a_changed_file_prepares_nothing() {
    let f = fixture("changed");
    let mut req = request(&f, &f.downloads.join("glowfly-v0.3.0-x86_64-pc-windows-msvc.zip"), &release_zip(), InstallMethod::SwapPortable);
    req.sha256 = sha256(b"something else");
    let err = prepare_with(&req, &|_| Ok(())).unwrap_err();
    assert!(err.starts_with("The downloaded file changed after it was checked"), "{err}");
    assert_eq!(std::fs::read(&f.exe).unwrap(), b"old program");
    assert!(staging_folders(&f).is_empty());
    let _ = persist::remove_tree(&f.root);
}

#[test]
fn a_failed_swap_leaves_nothing_behind() {
    let f = fixture("noprogram");
    // An archive without the program.
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    w.start_file("README.md", zip::write::SimpleFileOptions::default()).unwrap();
    w.write_all(b"readme").unwrap();
    let bytes = w.finish().unwrap().into_inner();
    let req = request(&f, &f.downloads.join("glowfly.zip"), &bytes, InstallMethod::SwapPortable);
    assert_eq!(prepare_with(&req, &|_| Ok(())).unwrap_err(), "The archive has no glowfly.exe.");
    assert_eq!(std::fs::read(&f.exe).unwrap(), b"old program");
    assert!(staging_folders(&f).is_empty());
    // A method that doesn't install in place.
    let req = request(&f, &f.downloads.join("glowfly.dmg"), b"dmg", InstallMethod::OpenAndQuit);
    assert_eq!(prepare_with(&req, &|_| Ok(())).unwrap_err(), "This update can't be installed in place.");
    assert!(staging_folders(&f).is_empty());
    let _ = persist::remove_tree(&f.root);
}

/// The start of a type-2 AppImage, then some payload.
const APPIMAGE: &[u8] = b"\x7fELF\x02\x01\x01\0AI\x02\0\0\0\0\0new appimage";

/// A Linux AppImage copy: the program runs from the image's mount, and the image sits in
/// `apps`.
fn appimage_request(f: &Fixture, bytes: &[u8]) -> RestartRequest {
    let (mount, apps) = (f.root.join("mount").join("usr").join("bin"), f.root.join("apps"));
    std::fs::create_dir_all(&mount).unwrap();
    std::fs::create_dir_all(&apps).unwrap();
    let exe = mount.join("glowfly");
    std::fs::write(&exe, "old program").unwrap();
    let target = apps.join("Glowfly.AppImage");
    std::fs::write(&target, "old appimage").unwrap();
    let mut req = request(f, &f.downloads.join("glowfly-v0.3.0-x86_64-unknown-linux-gnu.AppImage"), bytes, InstallMethod::SwapAppImage);
    (req.exe, req.target, req.os) = (exe, target, TargetOs::Linux);
    req
}

#[test]
fn an_appimage_is_swapped_in_place() {
    let f = fixture("appimage");
    let req = appimage_request(&f, APPIMAGE);
    let p = prepare_with(&req, &|_| Ok(())).unwrap();
    assert_eq!(std::fs::read(&req.target).unwrap(), APPIMAGE);
    let bak = req.target.with_file_name("Glowfly.AppImage.bak");
    assert_eq!(std::fs::read(&bak).unwrap(), b"old appimage");
    assert_eq!(p.helper, p.staging.join("glowfly-update"));
    assert_eq!(std::fs::read(&p.helper).unwrap(), b"old program", "the helper is the running program, not the image");
    assert_eq!(std::fs::read(&req.exe).unwrap(), b"old program");
    let plan = Plan::read(&p.plan_path).unwrap();
    let record = req.target.with_file_name("Glowfly.AppImage.bak.record");
    assert_eq!((plan.remove.clone(), plan.relaunch.clone(), plan.msi.clone()), (vec![bak.clone()], req.target.clone(), None));
    assert!(record.is_file(), "the record is beside the backup");
    p.abandon().unwrap();
    assert_eq!(std::fs::read(&req.target).unwrap(), b"old appimage");
    assert!(!bak.exists() && !record.exists());
    assert!(staging_folders(&f).is_empty());
    let _ = persist::remove_tree(&f.root);
}

#[test]
fn a_download_that_isnt_an_appimage_prepares_nothing() {
    let f = fixture("notappimage");
    let req = appimage_request(&f, b"not an appimage");
    assert_eq!(prepare_with(&req, &|_| Ok(())).unwrap_err(), "The download isn't an AppImage.");
    assert_eq!(std::fs::read(&req.target).unwrap(), b"old appimage");
    assert!(!req.target.with_file_name("Glowfly.AppImage.bak").exists());
    assert!(staging_folders(&f).is_empty());
    let _ = persist::remove_tree(&f.root);
}

#[test]
fn a_temp_folder_the_helper_cant_run_from_stages_it_beside_the_program() {
    let f = fixture("noexec");
    let mut req = request(&f, &f.downloads.join("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi"), b"pretend msi", InstallMethod::RunMsi);
    let app = f.exe.parent().unwrap().to_owned();
    req.places = vec![f.temp.clone(), app.clone()];
    let noexec = |helper: &Path| {
        if helper.starts_with(&f.temp) { Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)) } else { Ok(()) }
    };
    let p = prepare_with(&req, &noexec).unwrap();
    assert_eq!(p.staging.parent(), Some(app.as_path()));
    assert!(staging_folders(&f).is_empty(), "nothing is left in the temp folder");
    let plan = Plan::read(&p.plan_path).unwrap();
    assert_eq!(plan.staging, p.staging);
    p.abandon().unwrap();
    assert_eq!(std::fs::read_dir(&app).unwrap().count(), 1, "only the program is left");
    let _ = persist::remove_tree(&f.root);
}
