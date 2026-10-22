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
        temp: f.temp.clone(),
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
    let p = prepare(&req).unwrap();
    assert_eq!(std::fs::read(&f.exe).unwrap(), b"new program");
    let bak = f.exe.with_file_name("glowfly.exe.bak");
    assert_eq!(std::fs::read(&bak).unwrap(), b"old program");
    assert_eq!(staging_folders(&f), std::slice::from_ref(&p.staging));
    assert_eq!(p.helper, p.staging.join("glowfly-update.exe"));
    assert_eq!(std::fs::read(&p.helper).unwrap(), b"old program", "the helper is the running program");
    let plan = Plan::read(&p.plan_path).unwrap();
    assert_eq!((plan.remove.clone(), plan.relaunch.clone(), plan.msi.clone()), (vec![bak.clone()], f.exe.clone(), None));
    assert_eq!((plan.version.as_str(), plan.staging.as_path()), ("0.3.0", p.staging.as_path()));
    p.abandon().unwrap();
    assert_eq!(std::fs::read(&f.exe).unwrap(), b"old program");
    assert!(!bak.exists());
    assert!(staging_folders(&f).is_empty());
    let _ = persist::remove_tree(&f.root);
}

#[test]
fn an_msi_is_staged_without_touching_the_program() {
    let f = fixture("msi");
    let req = request(&f, &f.downloads.join("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi"), b"pretend msi", InstallMethod::RunMsi);
    let p = prepare(&req).unwrap();
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
    let err = prepare(&req).unwrap_err();
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
    assert_eq!(prepare(&req).unwrap_err(), "The archive has no glowfly.exe.");
    assert_eq!(std::fs::read(&f.exe).unwrap(), b"old program");
    assert!(staging_folders(&f).is_empty());
    // A method that doesn't install in place.
    let req = request(&f, &f.downloads.join("glowfly.dmg"), b"dmg", InstallMethod::OpenAndQuit);
    assert_eq!(prepare(&req).unwrap_err(), "This update can't be installed in place.");
    assert!(staging_folders(&f).is_empty());
    let _ = persist::remove_tree(&f.root);
}
