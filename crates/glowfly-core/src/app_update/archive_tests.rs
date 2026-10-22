use super::*;
use std::io::Write;

const WIN_ROOT: &str = "glowfly-v0.3.0-x86_64-pc-windows-msvc";
const LINUX_ROOT: &str = "glowfly-v0.3.0-x86_64-unknown-linux-gnu";

fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in files {
        if name.ends_with('/') {
            w.add_directory(*name, opts).unwrap();
        } else {
            w.start_file(*name, opts).unwrap();
            w.write_all(bytes).unwrap();
        }
    }
    w.finish().unwrap().into_inner()
}

/// One tar entry: a raw name (written straight into the header, so `..` and `/` get through),
/// its type and its bytes.
fn tar_gz_of(files: &[(&str, tar::EntryType, &[u8])]) -> Vec<u8> {
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut b = tar::Builder::new(gz);
    for (name, ty, bytes) in files {
        let mut h = tar::Header::new_old();
        h.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
        h.set_entry_type(*ty);
        h.set_size(bytes.len() as u64);
        h.set_mode(0o755);
        if ty.is_symlink() {
            h.as_old_mut().linkname[..4].copy_from_slice(b"/bin");
        }
        h.set_cksum();
        b.append(&h, *bytes).unwrap();
    }
    b.into_inner().unwrap().finish().unwrap()
}

fn names(entries: &[Entry]) -> Vec<(&str, &[u8])> {
    entries.iter().map(|e| (e.name.as_str(), e.bytes.as_slice())).collect()
}

const ZIP: &str = "glowfly-v0.3.0-x86_64-pc-windows-msvc.zip";
const TGZ: &str = "glowfly-v0.3.0-x86_64-unknown-linux-gnu.tar.gz";

#[test]
fn a_release_zip_yields_its_program_and_docs_without_the_root() {
    for sep in ["/", "\\"] {
        let exe = format!("{WIN_ROOT}{sep}glowfly.exe");
        let readme = format!("{WIN_ROOT}{sep}README.md");
        let zip = zip_of(&[(&exe, b"new"), (&readme, b"readme"), (&format!("{WIN_ROOT}{sep}notes.txt"), b"x")]);
        let got = read(ZIP, &zip, wanted(TargetOs::Windows)).unwrap();
        assert_eq!(names(&got), [("glowfly.exe", &b"new"[..]), ("README.md", b"readme")], "separator {sep}");
    }
    // With the root folder's own entry, and upper-case extension.
    let zip = zip_of(&[(&format!("{WIN_ROOT}/"), b""), (&format!("{WIN_ROOT}/glowfly.exe"), b"new")]);
    assert_eq!(names(&read("GLOWFLY.ZIP", &zip, wanted(TargetOs::Windows)).unwrap()), [("glowfly.exe", &b"new"[..])]);
}

#[test]
fn a_release_tar_gz_yields_its_program() {
    use tar::EntryType::{Directory, Regular};
    let tgz = tar_gz_of(&[
        (&format!("{LINUX_ROOT}/"), Directory, b""),
        (&format!("{LINUX_ROOT}/glowfly"), Regular, b"new"),
        (&format!("{LINUX_ROOT}/LICENSE-MIT"), Regular, b"mit"),
    ]);
    let got = read(TGZ, &tgz, wanted(TargetOs::Linux)).unwrap();
    assert_eq!(names(&got), [("glowfly", &b"new"[..]), ("LICENSE-MIT", b"mit")]);
}

#[test]
fn top_level_entries_are_read_without_a_root() {
    let zip = zip_of(&[("glowfly.exe", b"new"), ("CHANGELOG.md", b"log")]);
    assert_eq!(names(&read(ZIP, &zip, wanted(TargetOs::Windows)).unwrap()), [("glowfly.exe", &b"new"[..]), ("CHANGELOG.md", b"log")]);
    let tgz = tar_gz_of(&[("./glowfly", tar::EntryType::Regular, b"new")]);
    assert_eq!(names(&read(TGZ, &tgz, wanted(TargetOs::Linux)).unwrap()), [("glowfly", &b"new"[..])]);
}

#[test]
fn unwanted_and_nested_entries_are_ignored() {
    let zip = zip_of(&[
        (&format!("{WIN_ROOT}/sub/glowfly.exe"), b"deep"),
        (&format!("{WIN_ROOT}/Glowfly.exe"), b"case"),
        (&format!("{WIN_ROOT}/other.exe"), b"x"),
    ]);
    assert_eq!(read(ZIP, &zip, wanted(TargetOs::Windows)).unwrap(), []);
    // The Linux program isn't the Windows one.
    let tgz = tar_gz_of(&[(&format!("{LINUX_ROOT}/glowfly"), tar::EntryType::Regular, b"new")]);
    assert_eq!(read(TGZ, &tgz, wanted(TargetOs::Windows)).unwrap(), []);
}

#[test]
fn duplicates_and_links_are_refused() {
    let zip = zip_of(&[(&format!("{WIN_ROOT}/glowfly.exe"), b"a"), (&format!("{WIN_ROOT}\\glowfly.exe"), b"b")]);
    assert_eq!(read(ZIP, &zip, wanted(TargetOs::Windows)), Err("The archive has two copies of glowfly.exe.".to_owned()));
    let tgz = tar_gz_of(&[
        (&format!("{LINUX_ROOT}/glowfly"), tar::EntryType::Regular, b"a"),
        (&format!("{LINUX_ROOT}/glowfly"), tar::EntryType::Regular, b"b"),
    ]);
    assert_eq!(read(TGZ, &tgz, wanted(TargetOs::Linux)), Err("The archive has two copies of glowfly.".to_owned()));
    let tgz = tar_gz_of(&[(&format!("{LINUX_ROOT}/glowfly"), tar::EntryType::Symlink, b"")]);
    assert_eq!(read(TGZ, &tgz, wanted(TargetOs::Linux)), Err("glowfly in the archive isn't a regular file.".to_owned()));
    let tgz = tar_gz_of(&[(&format!("{LINUX_ROOT}/glowfly"), tar::EntryType::Link, b"")]);
    assert!(read(TGZ, &tgz, wanted(TargetOs::Linux)).unwrap_err().contains("isn't a regular file"));
}

#[test]
fn names_that_leave_the_folder_never_yield_an_entry() {
    use tar::EntryType::Regular;
    for name in ["../glowfly", "/glowfly", "root/../glowfly", "./../glowfly"] {
        let tgz = tar_gz_of(&[(name, Regular, b"evil")]);
        assert_eq!(read(TGZ, &tgz, wanted(TargetOs::Linux)).unwrap(), [], "{name}");
    }
    for name in ["../glowfly.exe", "/glowfly.exe", "C:/glowfly.exe", "..\\glowfly.exe"] {
        let zip = zip_of(&[(name, b"evil")]);
        assert_eq!(read(ZIP, &zip, wanted(TargetOs::Windows)).unwrap(), [], "{name}");
    }
}

#[test]
fn a_corrupt_archive_says_so() {
    let e = read(ZIP, b"not a zip at all", wanted(TargetOs::Windows)).unwrap_err();
    assert!(e.starts_with(&format!("Couldn't read the archive {ZIP}: ")), "{e}");
    let e = read(TGZ, b"not gzip", wanted(TargetOs::Linux)).unwrap_err();
    assert!(e.starts_with(&format!("Couldn't read the archive {TGZ}: ")), "{e}");
    assert!(read("glowfly.msi", b"", wanted(TargetOs::Windows)).unwrap_err().starts_with("Couldn't read the archive glowfly.msi"));
}

#[test]
fn the_root_and_entry_names() {
    assert_eq!(common_root(["r/a", "r/b", "r/"]), Some("r".to_owned()));
    assert_eq!(common_root(["./r/a", "r/b"]), Some("r".to_owned()));
    assert_eq!(common_root(["r/a", "s/b"]), None);
    assert_eq!(common_root(["r/a", "b"]), None);
    assert_eq!(common_root(["/r/a"]), None);
    assert_eq!(common_root(["../a"]), None);
    assert_eq!(common_root(["C:/a"]), None);
    assert_eq!(common_root(Vec::<&str>::new()), None);

    let cases: [(&str, Option<&str>, Option<&str>); 9] = [
        ("r/glowfly", Some("r"), Some("glowfly")),
        ("./r/glowfly", Some("r"), Some("glowfly")),
        ("glowfly", None, Some("glowfly")),
        ("r/sub/glowfly", Some("r"), None),
        ("r/", Some("r"), None),
        ("r/..", Some("r"), None),
        ("rr/glowfly", Some("r"), None),
        ("a\\b", None, None),
        ("/glowfly", None, None),
    ];
    for (raw, root, want) in cases {
        assert_eq!(entry_name(raw, root).as_deref(), want, "{raw} in {root:?}");
    }
}

#[test]
fn the_wanted_names() {
    assert_eq!(wanted(TargetOs::Windows)[0], "glowfly.exe");
    assert_eq!(program_name(TargetOs::Linux), "glowfly");
    assert_eq!(program_name(TargetOs::Mac), "glowfly");
    assert_eq!(&wanted(TargetOs::Mac)[1..], DOC_FILES);
}
