//! Writing commit messages in `i18n.commitEncoding`: encoding and refusals, the config read
//! through includes, strict labels (ISO-8859-1, US-ASCII and others WHATWG widens), commit and
//! amend round trips in a windows-1252 repository (#242), an ISO-8859-1 one, older UTF-8
//! commits under a single-byte label, and a rebase's reword files.

use super::*;
use crate::rebase::{self, Outcome, RebaseAction, RebaseBase, SequenceEditor, StartRequest, TodoStep};
use crate::repo::{LoadOptions, load};
use crate::testutil::*;
use std::path::Path;

fn cp1252() -> MessageEncoding {
    MessageEncoding::named(Some("windows-1252"))
}

#[test]
fn labels_name_the_encoding_and_utf8_stays_utf8() {
    assert_eq!(MessageEncoding::named(Some(" windows-1252 ")), cp1252(), "trimmed");
    assert!(!MessageEncoding::named(Some("latin-1")).is_utf8(), "an iconv alias");
    for label in [None, Some("UTF-8"), Some("utf8"), Some("UTF-16"), Some("x-made-up")] {
        assert!(MessageEncoding::named(label).is_utf8(), "{label:?}");
    }
    assert_eq!(MessageEncoding::named(Some("cp932")).name(), "cp932", "the label as configured");
    assert_eq!(MessageEncoding::UTF8.name(), "UTF-8");
}

#[test]
fn encoding_borrows_what_needs_no_change() {
    assert!(matches!(MessageEncoding::UTF8.encode("Zoë ✓"), Ok(Cow::Borrowed(b)) if b == "Zoë ✓".as_bytes()));
    assert!(matches!(cp1252().encode("plain"), Ok(Cow::Borrowed(b"plain"))));
    assert_eq!(cp1252().encode("Café € 5").unwrap().as_ref(), b"Caf\xe9 \x80 5");
    let sjis = MessageEncoding::named(Some("Shift_JIS"));
    assert_eq!(sjis.encode("日本語").unwrap().as_ref(), b"\x93\xfa\x96\x7b\x8c\xea");
}

#[test]
fn characters_the_encoding_lacks_are_named_never_replaced() {
    let e = cp1252().encode("✓ done: 日本 ✓ Café").unwrap_err();
    assert_eq!(e, Unencodable { encoding: "windows-1252".into(), chars: vec!['✓', '日', '本'] });
    assert_eq!(e.listed(), "\"✓\" \"日\" \"本\"");
    assert!(e.to_string().contains("windows-1252"), "{e}");
    let many: String = ('Ａ'..='Ｚ').collect();
    assert_eq!(cp1252().encode(&many).unwrap_err().chars.len(), MAX_NAMED, "only the first few");
    assert!(cp1252().check(["fine", "Café"]).is_ok());
    assert!(cp1252().check(["fine", "ok ✓"]).is_err());
}

#[test]
fn labels_wider_in_whatwg_refuse_what_only_the_wider_page_has() {
    // WHATWG reads these as windows-1252; git records the label and iconv reads it strictly.
    for label in ["ISO-8859-1", "latin1", "l1", "cp819", "latin-1"] {
        let e = MessageEncoding::named(Some(label));
        assert_eq!(e.encode("Café ü ÿ").unwrap().as_ref(), b"Caf\xe9 \xfc \xff", "{label}");
        let refused = e.encode("“Café” – 5 € ’").unwrap_err();
        assert_eq!(refused.chars, ['“', '”', '–', '€', '’'], "{label}");
        assert_eq!(refused.encoding, label, "the configured label is named");
    }
    assert!(MessageEncoding::named(Some("ISO-8859-1")).encode("5 €").unwrap_err().to_string().contains("ISO-8859-1"));
    for label in ["US-ASCII", "ascii", "ANSI_X3.4-1968"] {
        let e = MessageEncoding::named(Some(label));
        assert!(matches!(e.encode("plain"), Ok(Cow::Borrowed(b"plain"))), "{label}");
        assert_eq!(e.encode("Café").unwrap_err().chars, ['é'], "{label}");
    }
    // ISO-8859-9 (windows-1254 in WHATWG) and TIS-620 / ISO-8859-11 (windows-874).
    let turkish = MessageEncoding::named(Some("ISO-8859-9"));
    assert_eq!(turkish.encode("Ağ").unwrap().as_ref(), b"A\xf0");
    assert_eq!(turkish.encode("5 €").unwrap_err().chars, ['€']);
    assert_eq!(MessageEncoding::named(Some("windows-1254")).encode("5 €").unwrap().as_ref(), b"5 \x80");
    assert_eq!(MessageEncoding::named(Some("TIS-620")).encode("ก").unwrap().as_ref(), b"\xa1");
    assert_eq!(MessageEncoding::named(Some("TIS-620")).encode("a\u{a0}…").unwrap_err().chars, ['\u{a0}', '…']);
    assert_eq!(MessageEncoding::named(Some("ISO-8859-11")).encode("a\u{a0}…").unwrap_err().chars, ['…']);
    assert_eq!(MessageEncoding::named(Some("windows-874")).encode("…").unwrap().as_ref(), b"\x85");
}

/// An ISO-8859-1 repository: typographic punctuation is refused, Latin-1 letters are written
/// as their bytes, and git's own strict reading shows them as typed.
#[test]
fn an_iso_8859_1_repository_commits_only_latin1() {
    let dir = scratch_repo("enc-latin1");
    set_config(&dir, &[("i18n.commitEncoding", "ISO-8859-1")]);
    let snap = load(&dir, &LoadOptions::all(10)).unwrap();
    assert!(snap.commit_encoding.encode("“Café” – 5 €").is_err());
    commit_like_glowfly(&dir, "Café", false);
    let repo = git2::Repository::open(&dir).unwrap();
    let c = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!((c.message_encoding(), c.message_bytes()), (Some("ISO-8859-1"), &b"Caf\xe9\n"[..]));
    let shown = crate::gitcli::run(&dir, &crate::gitcli::args(&["-c", "i18n.logOutputEncoding=UTF-8", "log", "-1", "--format=%B"])).1;
    assert_eq!(shown.trim_end(), "Café");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_setting_is_read_through_includes() {
    let dir = scratch_repo("enc-include");
    assert!(MessageEncoding::of(&git2::Repository::open(&dir).unwrap()).is_utf8(), "unset");
    std::fs::write(dir.join(".git/legacy.cfg"), "[i18n]\n\tcommitEncoding = windows-1252\n").unwrap();
    set_config(&dir, &[("include.path", "legacy.cfg")]);
    assert_eq!(MessageEncoding::of(&git2::Repository::open(&dir).unwrap()), cp1252());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_files_writes_none_when_one_does_not_fit() {
    let dir = scratch_repo("enc-files");
    let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
    let refused = cp1252().write_files(&[(a.clone(), "Café\n".into()), (b.clone(), "✓\n".into())]);
    assert!(matches!(refused, Err(Error::Unsupported(ref t)) if t.contains("\"✓\"")), "{refused:?}");
    assert!(!a.exists() && !b.exists(), "nothing written");
    cp1252().write_files(&[(a.clone(), "Café\n".into())]).unwrap();
    assert_eq!(std::fs::read(&a).unwrap(), b"Caf\xe9\n");
    assert_eq!(cp1252().decode(&std::fs::read(&a).unwrap()), "Café\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// What the commit box does: the message encoded with the loaded snapshot's encoding, written
/// to a file, then `git commit -F` (plus `--amend`).
fn commit_like_glowfly(dir: &Path, message: &str, amend: bool) {
    let snap = load(dir, &LoadOptions::all(10)).unwrap();
    let file = snap.git_dir.join("GLOWFLY_COMMIT_EDITMSG");
    std::fs::write(&file, snap.commit_encoding.encode(&format!("{message}\n")).unwrap()).unwrap();
    let path = file.display().to_string();
    let mut a = vec!["commit", "-q", "--allow-empty", "-F", &path];
    if amend {
        a.push("--amend");
    }
    let (ok, out) = crate::gitcli::run(dir, &crate::gitcli::args(&a));
    assert!(ok, "{out}");
}

fn head_message(dir: &Path) -> String {
    load(dir, &LoadOptions::all(10)).unwrap().head_message.unwrap()
}

fn head_bytes(dir: &Path) -> Vec<u8> {
    let repo = git2::Repository::open(dir).unwrap();
    let c = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(c.message_encoding(), Some("windows-1252"));
    c.message_bytes().to_vec()
}

#[test]
fn amending_twice_in_a_windows_1252_repository_keeps_the_message() {
    let dir = scratch_repo("enc-amend");
    set_config(&dir, &[("i18n.commitEncoding", "windows-1252")]);
    let typed = "Café für alle\n\nCrème brûlée, 5 €.";
    commit_like_glowfly(&dir, typed, false);
    assert_eq!(head_bytes(&dir), b"Caf\xe9 f\xfcr alle\n\nCr\xe8me br\xfbl\xe9e, 5 \x80.\n", "real windows-1252 bytes");
    assert_eq!(head_message(&dir), typed);
    for round in 1..=2 {
        // The amend draft is prefilled from HEAD and committed unchanged.
        commit_like_glowfly(&dir, &head_message(&dir), true);
        assert_eq!(head_message(&dir), typed, "after amend {round}");
    }
    let shown = crate::gitcli::run(&dir, &crate::gitcli::args(&["-c", "i18n.logOutputEncoding=UTF-8", "log", "-1", "--format=%B"])).1;
    assert_eq!(shown.trim_end(), typed, "git reads it the same way");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_older_utf8_commit_under_the_label_reads_as_typed_and_amends_cleanly() {
    let dir = scratch_repo("enc-older");
    set_config(&dir, &[("i18n.commitEncoding", "windows-1252")]);
    // What Glowfly wrote before #242 (and git writes for `commit -m`): UTF-8 under the label.
    let repo = git2::Repository::open(&dir).unwrap();
    let tree = repo.treebuilder(None).unwrap().write().unwrap();
    let oid = raw_commit(&repo, tree, None, b"Test <test@example.com>", Some("windows-1252"), "Café ✓\n".as_bytes());
    repo.reference("refs/heads/main", oid, true, "test").unwrap();
    assert_eq!(head_message(&dir), "Café ✓", "UTF-8 wins over a single-byte label");
    // "✓" has no windows-1252 byte, so that amend is refused; without it the amend re-encodes.
    assert!(MessageEncoding::of(&repo).encode(&head_message(&dir)).is_err());
    commit_like_glowfly(&dir, "Café", true);
    assert_eq!(head_bytes(&dir), b"Caf\xe9\n");
    assert_eq!(head_message(&dir), "Café");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn multi_byte_labels_keep_their_own_reading() {
    // C6 A1 is valid UTF-8 ("ơ") and also EUC-JP: the header wins there.
    let (euc, _) = encoding_rs::EUC_JP.decode_without_bom_handling(b"\xc6\xa1");
    assert_ne!(euc, "ơ");
    assert_eq!(Decoder::named(Some("EUC-JP")).decode(b"\xc6\xa1"), euc);
    assert_eq!(Decoder::named(Some("windows-1252")).decode("ơ".as_bytes()), "ơ");
    assert_eq!(Decoder::named(Some("windows-1252")).decode(b"\xc3\xa9t\xe9"), "Ã©té", "not UTF-8: windows-1252");
}

fn reword_request(dir: &Path, base: git2::Oid, row: usize, action: RebaseAction, message: &str) -> StartRequest {
    let p = rebase::plan(dir, &RebaseBase::FromCommit(base)).unwrap();
    let mut items: Vec<TodoStep> = p.steps;
    let item = items.iter_mut().filter_map(TodoStep::commit_mut).nth(row).unwrap();
    item.action = action;
    if action == RebaseAction::Reword {
        item.new_message = Some(message.into());
    }
    StartRequest { head: p.head, upstream: p.upstream, items, autostash: false }
}

#[test]
fn a_rebase_reword_is_written_in_the_encoding_and_read_back() {
    let dir = scratch_repo("enc-rebase");
    set_config(&dir, &[("i18n.commitEncoding", "windows-1252")]);
    for n in ["c1", "c2", "c3"] {
        git(&dir, &["commit", "-q", "--allow-empty", "-m", n]);
    }
    let c2 = git2::Repository::open(&dir).unwrap().revparse_single("HEAD~1").unwrap().id();

    // Refused before anything runs.
    let bad = reword_request(&dir, c2, 1, RebaseAction::Reword, "c3 ✓");
    assert!(matches!(rebase::start(&dir, &bad, &SequenceEditor::Copy), Err(Error::Unsupported(t)) if t.contains("\"✓\"")));
    assert_eq!(git2::Repository::open(&dir).unwrap().state(), git2::RepositoryState::Clean);

    // c2 stops for an edit; c3's reword waits in its message file.
    let mut req = reword_request(&dir, c2, 1, RebaseAction::Reword, "c3, café");
    req.items.iter_mut().find_map(TodoStep::commit_mut).unwrap().action = RebaseAction::Edit;
    let outcome = rebase::start(&dir, &req, &SequenceEditor::Copy).unwrap();
    assert!(matches!(outcome, Outcome::Stopped { ok: true, .. }), "{outcome:?}");
    let left = rebase::remaining(&dir).unwrap();
    let typed = left.steps.iter().filter_map(TodoStep::commit).find_map(|i| i.new_message.clone());
    assert_eq!(typed.as_deref(), Some("c3, café"), "the file is decoded from windows-1252");
    let (ok, out) = crate::gitcli::run(&dir, &crate::gitcli::args(&["rebase", "--continue"]));
    assert!(ok, "{out}");
    assert_eq!(head_bytes(&dir), b"c3, caf\xe9\n");
    assert_eq!(head_message(&dir), "c3, café");
    let _ = std::fs::remove_dir_all(&dir);
}
