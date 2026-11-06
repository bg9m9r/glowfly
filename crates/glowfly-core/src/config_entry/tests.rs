//! Reading config entries with and without values: bare keys, empty values, bytes that aren't
//! UTF-8 and multivars with a bare entry among real ones.

use super::*;
use crate::testutil::scratch_repo;
use git2::Repository;
use std::io::Write;
use std::path::Path;

fn append(dir: &Path, bytes: &[u8]) {
    let mut f = std::fs::OpenOptions::new().append(true).open(dir.join(".git").join("config")).unwrap();
    f.write_all(bytes).unwrap();
}

/// A `[t]` section with a bare key, an empty value, text, a byte that isn't UTF-8 and a
/// multivar with a bare entry in the middle.
fn fixture(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    let mut text = b"[t]\n\tbare\n\tempty =\n\ttext = hello\n\tbytes = ".to_vec();
    text.push(0xFF);
    text.extend_from_slice(b"\n\tmulti = a\n\tmulti\n\tmulti = b\n");
    append(&dir, &text);
    dir
}

#[test]
fn entries_with_and_without_values() {
    let dir = fixture("config-entry-reads");
    let cfg = Repository::open(&dir).unwrap().config().unwrap().snapshot().unwrap();
    let entry = |k: &str| cfg.get_entry(k).unwrap();
    assert_eq!(raw(&entry("t.bare")), None);
    assert_eq!(raw(&entry("t.empty")), Some(&b""[..]));
    assert_eq!(utf8(&entry("t.text")), Some("hello"));
    assert_eq!(utf8(&entry("t.bytes")), None);
    assert_eq!(lossy(&entry("t.bytes")).as_deref(), Some("\u{FFFD}"));
    assert_eq!(raw(&entry("t.bytes")), Some(&[0xFF][..]));
    let mut multi = Vec::new();
    let mut entries = cfg.multivar("t.multi", None).unwrap();
    while let Some(Ok(e)) = entries.next() {
        multi.push(utf8(e).map(str::to_owned));
    }
    assert_eq!(multi, [Some("a".to_owned()), None, Some("b".to_owned())]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bool_or_word_reads_a_bare_key_as_true() {
    let dir = fixture("config-entry-bool-or-word");
    let read = |k: &str| bool_or_word(&Repository::open(&dir).unwrap().config().unwrap(), k);
    assert_eq!(read("t.bare").as_deref(), Some("true"));
    assert_eq!(read("t.empty").as_deref(), Some(""));
    assert_eq!(read("t.text").as_deref(), Some("hello"));
    assert_eq!(read("t.unset"), None);
    // The last value counts, as with get_string.
    append(&dir, b"[t]\n\tbare = no\n");
    assert_eq!(read("t.bare").as_deref(), Some("no"));
    let _ = std::fs::remove_dir_all(&dir);
}
