//! Decoding commit text: encoding labels and aliases, the UTF-8 fallbacks, and Latin-1,
//! headerless and Shift-JIS commits read through the walk and the details panel.

use super::*;
use crate::repo::{self, LoadOptions, load};
use crate::testutil::*;
use std::borrow::Cow;

fn named(label: &str) -> Decoder {
    Decoder::named(Some(label))
}

#[test]
fn utf8_labels_and_no_header_decode_as_utf8_without_copying() {
    let utf8 = Decoder::named(None);
    for label in ["UTF-8", "utf8", " Utf-8 "] {
        assert_eq!(named(label), utf8, "{label}");
    }
    assert!(matches!(utf8.decode(b"plain"), Cow::Borrowed("plain")), "ASCII is borrowed");
    assert!(matches!(utf8.decode("Zoë".as_bytes()), Cow::Borrowed("Zoë")), "UTF-8 is borrowed");
    assert!(matches!(named("latin1").decode(b"plain"), Cow::Borrowed("plain")), "ASCII in Latin-1 is borrowed too");
}

#[test]
fn latin1_and_windows_1252_labels() {
    for label in ["ISO-8859-1", "iso8859-1", "latin1", "latin-1", "LATIN1"] {
        assert_eq!(named(label).decode(b"Ren\xe9 M\xfcller"), "René Müller", "{label}");
    }
    assert_eq!(named("windows-1252").decode(b"\x80 5"), "€ 5");
}

#[test]
fn japanese_labels_and_aliases() {
    for label in ["Shift_JIS", "SJIS", "cp932"] {
        assert_eq!(named(label).decode(b"\x93\xfa\x96\x7b\x8c\xea"), "日本語", "{label}");
    }
    for label in ["EUC-JP", "eucJP", "euc_jp"] {
        assert_eq!(named(label).decode(b"\xc6\xfc\xcb\xdc"), "日本", "{label}");
    }
}

#[test]
fn a_wrong_header_over_utf8_shows_the_utf8() {
    // E2 82 is malformed in EUC-JP; without the fallback this would hold U+FFFD.
    assert_eq!(named("EUC-JP").decode("Price 5 €".as_bytes()), "Price 5 €");
}

#[test]
fn bad_bytes_without_a_known_header_become_replacement_characters() {
    assert_eq!(Decoder::named(None).decode(b"Caf\xe9"), "Caf\u{FFFD}");
    for label in ["x-made-up", "UTF-16", "utf-16le", "utf-16be", "iso-2022-kr"] {
        let d = named(label);
        assert_eq!(d, Decoder::named(None), "{label} decodes as UTF-8");
        assert_eq!(d.decode(b"plain"), "plain", "{label}");
        assert_eq!(d.decode(b"Caf\xe9"), "Caf\u{FFFD}", "{label}");
    }
}

/// A root commit with an empty tree on `main`, made of raw bytes.
fn raw_on_main(dir: &std::path::Path, who: &[u8], encoding: Option<&str>, msg: &[u8]) -> git2::Oid {
    let repo = git2::Repository::open(dir).unwrap();
    let tree = repo.treebuilder(None).unwrap().write().unwrap();
    let oid = raw_commit(&repo, tree, None, who, encoding, msg);
    repo.reference("refs/heads/main", oid, true, "test").unwrap();
    oid
}

#[test]
fn latin1_commit_reads_in_the_walk_and_details() {
    let dir = scratch_repo("commit-text-latin1");
    let oid = raw_on_main(&dir, LATIN1_WHO, Some("ISO-8859-1"), LATIN1_MSG);
    let snap = load(&dir, &LoadOptions::all(10)).unwrap();
    let c = &snap.commits[0];
    assert_eq!((c.summary.as_str(), c.body.as_str()), ("Café au lait", "Crème brûlée for everyöne."));
    assert_eq!((c.author.name(), c.author.email()), ("René Müller", "rené@example.com"));
    assert_eq!(snap.head_message.as_deref(), Some("Café au lait\n\nCrème brûlée for everyöne."));

    let d = repo::commit_detail(&dir, oid).unwrap();
    assert_eq!(d.message, "Café au lait\n\nCrème brûlée for everyöne.");
    assert_eq!((d.author.as_str(), d.email.as_str()), ("René Müller", "rené@example.com"));
    assert_eq!((d.committer.as_str(), d.committer_email.as_str()), ("René Müller", "rené@example.com"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn invalid_utf8_without_a_header_reads_lossily() {
    let dir = scratch_repo("commit-text-lossy");
    let oid = raw_on_main(&dir, LATIN1_WHO, None, LATIN1_MSG);
    let snap = load(&dir, &LoadOptions::all(10)).unwrap();
    let c = &snap.commits[0];
    assert_eq!(c.summary, "Caf\u{FFFD} au lait");
    assert_eq!(c.author.name(), "Ren\u{FFFD} M\u{FFFD}ller");
    let d = repo::commit_detail(&dir, oid).unwrap();
    assert!(d.message.starts_with("Caf\u{FFFD} au lait"), "{:?}", d.message);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shift_jis_commit_reads_in_the_walk() {
    let dir = scratch_repo("commit-text-sjis");
    raw_on_main(&dir, b"\x93\xfa\x96\x7b <jp@example.jp>", Some("Shift_JIS"), b"\x93\xfa\x96\x7b\x8c\xea\n");
    let snap = load(&dir, &LoadOptions::all(10)).unwrap();
    let c = &snap.commits[0];
    assert_eq!((c.summary.as_str(), c.author.name(), c.author.email()), ("日本語", "日本", "jp@example.jp"));
    let _ = std::fs::remove_dir_all(&dir);
}
