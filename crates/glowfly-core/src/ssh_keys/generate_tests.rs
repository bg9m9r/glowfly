//! Generating keys, where they go, and moving a new pair into place.

use super::generate::install_pair;
use super::tests::temp;
use super::*;
use crate::error::Error;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

fn home() -> PathBuf {
    PathBuf::from(if cfg!(windows) { "C:/Users/ada" } else { "/home/ada" })
}

#[test]
fn resolves_locations() {
    let h = home();
    let r = |t: &str| resolve_location(t, &h);
    assert_eq!(r(""), Err(LocationProblem::Empty));
    assert_eq!(r("   "), Err(LocationProblem::Empty));
    assert_eq!(r("keys/id"), Err(LocationProblem::Relative));
    assert_eq!(r("~"), Err(LocationProblem::NotAFile));
    assert_eq!(r("~/.ssh/"), Err(LocationProblem::NotAFile));
    assert_eq!(r("~/.ssh/work"), Ok(h.join(".ssh").join("work")));
    assert_eq!(r("~\\.ssh\\work"), Ok(h.join(".ssh").join("work")));
    let abs = if cfg!(windows) { "C:/keys/id" } else { "/keys/id" };
    assert_eq!(r(&format!("{abs}.PUB")), Err(LocationProblem::PublicFile));
    assert_eq!(r(&format!("  {abs}  ")), Ok(PathBuf::from(abs)));
    assert!(LocationProblem::Relative.message().contains("full path"));
    // A name Windows can't use would open a device (con) or lose its trailing dot.
    for bad in ["~/.ssh/con", "~/.ssh/NUL.key", "~/.ssh/id.", "~/.ssh/a|b", "~/.ssh/COM\u{b9}"] {
        assert_eq!(resolve_location_for(bad, &h, true), Err(LocationProblem::WindowsName), "{bad}");
        assert!(resolve_location_for(bad, &h, false).is_ok(), "{bad}");
    }
    assert!(resolve_location_for("~/.ssh/console_key", &h, true).is_ok());
    assert!(resolve_location_for(abs, &h, true).is_ok(), "a drive letter isn't the file name");
    assert_eq!(comment_problem("a\nb"), Some("The comment can't contain line breaks or control characters."));
    assert_eq!(comment_problem("ada@example.com"), None);
    assert_eq!(default_location(&h), h.join(".ssh").join("id_ed25519"));
}

#[test]
fn keygen_arguments_and_redaction() {
    let out = Path::new("/k/.id.tmp");
    let key = NewKey { path: "/k/id".into(), comment: "ada@example.com".into(), passphrase: "secret".into() };
    let a: Vec<OsString> =
        ["-q", "-t", "ed25519", "-N", "secret", "-C", "ada@example.com", "-f", "/k/.id.tmp"].into_iter().map(OsString::from).collect();
    assert_eq!(keygen_args(&key, out), a);
    let bare = NewKey { comment: String::new(), passphrase: String::new(), ..key.clone() };
    let a: Vec<OsString> = ["-q", "-t", "ed25519", "-N", "", "-f", "/k/.id.tmp"].into_iter().map(OsString::from).collect();
    assert_eq!(keygen_args(&bare, out), a, "an empty passphrase argument, and no -C");
    let shown = format!("{:?}", NewKey { passphrase: "s3cret".into(), ..key });
    assert!(!shown.contains("s3cret") && shown.contains("<redacted>"), "{shown}");
    assert!(format!("{bare:?}").contains("passphrase: \"\""));
}

#[test]
fn generates_and_replaces_keys() {
    let dir = temp("generate");
    let Some(keygen) = find_keygen(&dir) else {
        eprintln!("ssh-keygen not found, skipping");
        return;
    };
    let ssh = dir.join("nested").join(".ssh");
    let path = ssh.join("id_test");
    let key = NewKey { path: path.clone(), comment: "ada@example.com".into(), passphrase: "s3cret pass".into() };
    let pair = match generate(&keygen, &key, false) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ssh-keygen could not generate a key here, skipping: {e}");
            return;
        }
    };
    assert!(pair.fingerprint.as_deref().is_some_and(|f| f.starts_with("SHA256:")), "{pair:?}");
    assert_eq!((pair.kind.as_str(), pair.comment.as_str()), ("ED25519", "ada@example.com"));
    assert!(path.is_file() && pub_path(&path).is_file());

    let before = (std::fs::read(&path).unwrap(), std::fs::read(pub_path(&path)).unwrap());
    assert!(matches!(generate(&keygen, &key, false), Err(Error::WouldOverwrite(_))));
    assert_eq!(before, (std::fs::read(&path).unwrap(), std::fs::read(pub_path(&path)).unwrap()), "untouched");

    let again = generate(&keygen, &NewKey { passphrase: String::new(), ..key.clone() }, true).unwrap();
    assert_ne!(again.public_key, pair.public_key, "a new key replaced the old one");
    let leftovers: Vec<_> =
        std::fs::read_dir(&ssh).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains(".glowfly-new")).collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    let folder = generate(&keygen, &NewKey { path: ssh.clone(), ..key }, true).unwrap_err();
    assert!(folder.to_string().contains("is a folder"), "{folder}");

    let listed = list_key_pairs(&ssh, Some(&keygen)).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].fingerprint, again.fingerprint);
    assert!(listed[0].fingerprint.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_install_keeps_the_old_pair() {
    let dir = temp("install-fails");
    let (tmp, tmp_pub) = (dir.join(".id.new"), dir.join(".id.new.pub"));
    let (private, public) = (dir.join("id"), dir.join("id.pub"));
    std::fs::write(&tmp, "NEW PRIVATE").unwrap();
    std::fs::write(&tmp_pub, "NEW PUBLIC").unwrap();
    std::fs::write(&private, "OLD PRIVATE").unwrap();
    // A non-empty folder where the public key goes: no rename can replace it, on any system.
    std::fs::create_dir_all(public.join("inside")).unwrap();
    assert!(install_pair([&tmp_pub, &tmp], [&public, &private]).is_err());
    assert_eq!(std::fs::read_to_string(&private).unwrap(), "OLD PRIVATE", "the old private key survives");
    assert!(public.join("inside").is_dir());
    let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, [".id.new", ".id.new.pub", "id", "id.pub"], "no backups left; the caller removes the new files");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_install_replaces_both_files_and_leaves_no_backups() {
    let dir = temp("install-ok");
    let (tmp, tmp_pub) = (dir.join(".id.new"), dir.join(".id.new.pub"));
    let (private, public) = (dir.join("id"), dir.join("id.pub"));
    for (p, text) in [(&tmp, "NEW PRIVATE"), (&tmp_pub, "NEW PUBLIC"), (&private, "OLD PRIVATE"), (&public, "OLD PUBLIC")] {
        std::fs::write(p, text).unwrap();
    }
    install_pair([&tmp_pub, &tmp], [&public, &private]).unwrap();
    assert_eq!(std::fs::read_to_string(&private).unwrap(), "NEW PRIVATE");
    assert_eq!(std::fs::read_to_string(&public).unwrap(), "NEW PUBLIC");
    let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, ["id", "id.pub"]);
    let _ = std::fs::remove_dir_all(&dir);
}
