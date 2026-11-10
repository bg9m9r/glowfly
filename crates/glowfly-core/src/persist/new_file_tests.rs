//! A new file's write: a name taken meanwhile is never replaced, names that stay taken are
//! refused, a failed write leaves nothing, and temporary names read back.

use super::super::{WriteOptions, is_temp_of, temp_sibling, temp_target};
use super::{NAME_TRIES, write_streamed_new};
use std::cell::Cell;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

/// A fresh, empty folder under the temp folder.
fn scratch(tag: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-new-file-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn a_name_taken_while_writing_is_never_replaced() {
    let dir = scratch("taken");
    let first = dir.join("a.bin");
    let fill = |f: &mut std::fs::File| {
        f.write_all(b"new")?;
        // Another program saves a file under the same name while this one is written.
        std::fs::write(dir.join("a.bin"), "other")
    };
    let got = write_streamed_new(&first, WriteOptions::default(), || dir.join("a (1).bin"), fill).unwrap();
    assert_eq!(got, dir.join("a (1).bin"));
    assert_eq!(std::fs::read_to_string(&got).unwrap(), "new");
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "other", "the file that appeared is kept");
    assert_eq!(names(&dir), ["a (1).bin", "a.bin"], "no temporary file is left");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn names_that_stay_taken_are_refused() {
    let dir = scratch("refused");
    let taken = ["a.bin", "b.bin", "c.bin", "d.bin", "e.bin"];
    for name in taken {
        std::fs::write(dir.join(name), name).unwrap();
    }
    let asked = Cell::new(0);
    let next = || {
        asked.set(asked.get() + 1);
        dir.join(taken[asked.get() % taken.len()])
    };
    let err = write_streamed_new(&dir.join("a.bin"), WriteOptions::default(), next, |f| f.write_all(b"new")).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::AlreadyExists, "{err}");
    assert_eq!(asked.get(), NAME_TRIES - 1);
    for name in taken {
        assert_eq!(std::fs::read_to_string(dir.join(name)).unwrap(), name, "{name} is unchanged");
    }
    assert_eq!(names(&dir), taken, "no .a.bin.*.tmp is left");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_fill_leaves_nothing_and_never_asks_for_a_name() {
    let dir = scratch("failed");
    let fill = |f: &mut std::fs::File| {
        f.write_all(b"half")?;
        Err(std::io::Error::other("the download stopped"))
    };
    let err = write_streamed_new(&dir.join("a.bin"), WriteOptions::default(), || panic!("no name is asked for"), fill).unwrap_err();
    assert_eq!(err.to_string(), "the download stopped");
    assert!(names(&dir).is_empty(), "{:?}", names(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_name_in_another_folder_is_refused() {
    let dir = scratch("elsewhere");
    let (here, there) = (dir.join("here"), dir.join("there"));
    std::fs::create_dir_all(&here).unwrap();
    std::fs::create_dir_all(&there).unwrap();
    std::fs::write(here.join("a.bin"), "taken").unwrap();
    let err =
        write_streamed_new(&here.join("a.bin"), WriteOptions::default(), || there.join("a.bin"), |f| f.write_all(b"new")).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidInput, "{err}");
    assert!(names(&there).is_empty(), "nothing written there");
    assert_eq!(names(&here), ["a.bin"], "no temporary file is left");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn temp_target_reads_back_temp_sibling_names() {
    let dir = scratch("names");
    for name in ["glowfly-v0.1.1-x86_64-pc-windows-msvc.msi", "x (1).tar.gz", "notes.txt"] {
        let tmp = temp_sibling(&dir.join(name));
        let tmp = tmp.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(temp_target(&tmp), Some(name), "{tmp}");
        assert!(is_temp_of(name, &tmp), "{tmp}");
    }
    for bad in [".x.tmp", ".x.1.tmp", "x.1.2.tmp", "..1.2.tmp", ".x.a.2.tmp", ".x.1.2.tmp.bak", ".x.1..tmp"] {
        assert_eq!(temp_target(bad), None, "{bad}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn a_private_new_file_is_0600() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("private");
    let opts = WriteOptions { private: true, ..WriteOptions::default() };
    let got = write_streamed_new(&dir.join("a.bin"), opts, || dir.join("a (1).bin"), |f| f.write_all(b"secret")).unwrap();
    let mode = std::fs::metadata(&got).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    let _ = std::fs::remove_dir_all(&dir);
}
