//! A log file that rotates by size: `errors.log` moves to `errors.1.log`, the older files up by
//! one, and the oldest is dropped. It runs inside the tracing writer and the panic hook, so it
//! never panics; errors come back from `write`.

use crate::persist;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The `n`th rotated name of `path`: `errors.log` -> `errors.1.log`; a name without an extension
/// gets `.1`.
pub fn rotated_path(path: &Path, n: usize) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = match path.extension() {
        Some(ext) => format!("{stem}.{n}.{}", ext.to_string_lossy()),
        None => format!("{stem}.{n}"),
    };
    path.with_file_name(name)
}

/// An append-only log of at most about `max_bytes` (a single write larger than that still goes
/// in whole), plus `keep` rotated files. Opened on its first write.
pub struct RotatingFile {
    path: PathBuf,
    max_bytes: u64,
    keep: usize,
    file: Option<File>,
    size: u64,
}

impl RotatingFile {
    /// Does no I/O: the file opens on the first write.
    pub fn new(path: PathBuf, max_bytes: u64, keep: usize) -> Self {
        Self { path, max_bytes, keep, file: None, size: 0 }
    }

    fn open(&mut self) -> io::Result<()> {
        let f = persist::open_append(&self.path)?;
        self.size = f.metadata().map(|m| m.len()).unwrap_or(0);
        self.file = Some(f);
        Ok(())
    }

    /// Moves the files up by one and starts an empty file. When a rename fails (another program
    /// holds a file), the current file is emptied instead, so it can't grow without bound.
    fn rotate(&mut self) -> io::Result<()> {
        self.file = None;
        let moved = (|| {
            if self.keep == 0 {
                return Err(io::Error::other("no rotated files are kept"));
            }
            for n in (1..self.keep).rev() {
                match std::fs::rename(rotated_path(&self.path, n), rotated_path(&self.path, n + 1)) {
                    Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                    _ => {}
                }
            }
            std::fs::rename(&self.path, rotated_path(&self.path, 1))
        })();
        if moved.is_ok() {
            return self.open();
        }
        let f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).open(&self.path)?;
        // Appending from here on, so another instance's writes don't overwrite ours.
        drop(f);
        self.open()?;
        self.size = 0;
        Ok(())
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.file.is_none() {
            self.open()?;
        }
        if self.size > 0 && self.size.saturating_add(buf.len() as u64) > self.max_bytes {
            self.rotate()?;
        }
        let Some(f) = self.file.as_mut() else { return Err(io::Error::other("the log file isn't open")) };
        // The whole buffer in one go, so a line from one event is never split.
        f.write_all(buf)?;
        self.size = self.size.saturating_add(buf.len() as u64);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.file.as_mut() {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = crate::paths::temp_dir().join(format!("glowfly-rotate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    #[test]
    fn rotated_path_names() {
        let dir = crate::paths::temp_dir();
        assert_eq!(rotated_path(&dir.join("errors.log"), 2), dir.join("errors.2.log"));
        assert_eq!(rotated_path(&dir.join("trace"), 1), dir.join("trace.1"));
    }

    #[test]
    fn rotation_keeps_at_most_keep_old_files() {
        let dir = scratch("keep");
        let path = dir.join("errors.log");
        let mut f = RotatingFile::new(path.clone(), 64, 3);
        for i in 0..20 {
            // 30 bytes each.
            f.write_all(format!("line {i:02} {}\n", "x".repeat(21)).as_bytes()).unwrap();
        }
        assert_eq!(names(&dir), ["errors.1.log", "errors.2.log", "errors.3.log", "errors.log"]);
        for name in names(&dir) {
            assert!(std::fs::metadata(dir.join(&name)).unwrap().len() <= 64, "{name}");
        }
        let all: String = ["errors.3.log", "errors.2.log", "errors.1.log", "errors.log"]
            .iter()
            .map(|n| std::fs::read_to_string(dir.join(n)).unwrap())
            .collect();
        assert!(std::fs::read_to_string(&path).unwrap().contains("line 19"));
        assert!(!all.contains("line 00"), "the oldest lines are gone");
        assert!(all.contains("line 12"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn size_counts_an_existing_file() {
        let dir = scratch("existing");
        let path = dir.join("errors.log");
        std::fs::write(&path, "y".repeat(60)).unwrap();
        let mut f = RotatingFile::new(path.clone(), 64, 3);
        f.write_all(b"0123456789").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "0123456789");
        assert_eq!(std::fs::read_to_string(dir.join("errors.1.log")).unwrap(), "y".repeat(60));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_first_write_larger_than_the_limit_is_written() {
        let dir = scratch("large");
        let path = dir.join("errors.log");
        let mut f = RotatingFile::new(path.clone(), 64, 3);
        let big = "z".repeat(100);
        f.write_all(big.as_bytes()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), big);
        assert_eq!(names(&dir), ["errors.log"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
