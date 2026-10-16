use super::*;

fn home() -> PathBuf {
    PathBuf::from(if cfg!(windows) { "C:/Users/ada" } else { "/home/ada" })
}

/// A fresh folder for one test (tests run in parallel).
fn temp(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("trellis-ssh-keys-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn parses_public_key_lines() {
    let p = |t: &str| parse_public_key(t);
    assert_eq!(p("ssh-ed25519 AAAA ada@laptop"), Some(("ssh-ed25519".into(), "ada@laptop".into())));
    assert_eq!(p("ssh-rsa AAAA my  work key\n"), Some(("ssh-rsa".into(), "my work key".into())));
    assert_eq!(p("ssh-ed25519 AAAA"), Some(("ssh-ed25519".into(), String::new())));
    assert_eq!(p("sk-ssh-ed25519@openssh.com AAAA k").map(|t| t.0), Some("sk-ssh-ed25519@openssh.com".into()));
    assert_eq!(p("ecdsa-sha2-nistp256 AAAA k").map(|t| t.0), Some("ecdsa-sha2-nistp256".into()));
    for bad in ["", "garbage", "ssh-ed25519", "no-pty ssh-rsa AAAA ada@x"] {
        assert_eq!(p(bad), None, "{bad:?}");
    }
    // Only the first line counts.
    assert_eq!(p("garbage\nssh-ed25519 AAAA x"), None);
}

#[test]
fn parses_fingerprints() {
    assert_eq!(parse_fingerprint("256 SHA256:abcDEF ada@x (ED25519)\n"), Some((Some(256), "SHA256:abcDEF".into(), "ED25519".into())));
    assert_eq!(parse_fingerprint("3072 SHA256:x no comment (RSA)"), Some((Some(3072), "SHA256:x".into(), "RSA".into())));
    for junk in ["", "junk", "ssh-keygen: not a key file", "256 abc ada (ED25519)", "256 SHA256:x ada"] {
        assert_eq!(parse_fingerprint(junk), None, "{junk:?}");
    }
}

#[test]
fn kinds_and_labels() {
    assert_eq!(kind_from_type("ssh-ed25519"), "ED25519");
    assert_eq!(kind_from_type("ssh-rsa"), "RSA");
    assert_eq!(kind_from_type("ecdsa-sha2-nistp384"), "ECDSA");
    assert_eq!(kind_from_type("sk-ssh-ed25519@openssh.com"), "ED25519-SK");
    assert_eq!(kind_from_type("sk-ecdsa-sha2-nistp256@openssh.com"), "ECDSA-SK");
    assert_eq!(kind_from_type("ssh-dss"), "DSA");
    assert_eq!(kind_from_type("ssh-new"), "SSH-NEW");
    let pair = |kind: &str, bits| SshKeyPair {
        private: "/k/id".into(),
        public: "/k/id.pub".into(),
        key_type: String::new(),
        kind: kind.into(),
        bits,
        fingerprint: None,
        comment: String::new(),
        public_key: String::new(),
    };
    assert_eq!(pair("RSA", Some(3072)).kind_label(), "RSA 3072");
    assert_eq!(pair("ED25519", None).kind_label(), "ED25519");
    assert_eq!(pair("ED25519", Some(256)).kind_label(), "ED25519", "one size only: no number");
    assert_eq!(pair("RSA", None).file_name(), "id");
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
fn keygen_candidate_order() {
    let path = std::env::join_paths([PathBuf::from("/a/bin"), PathBuf::from("/b/bin")]).unwrap();
    let exec = Path::new("/Git/mingw64/libexec/git-core");
    let got = keygen_candidates(Some(&path), ".exe", Some(exec), Some(Path::new("/Windows")));
    let want = vec![
        PathBuf::from("/a/bin").join("ssh-keygen.exe"),
        PathBuf::from("/b/bin").join("ssh-keygen.exe"),
        PathBuf::from("/Git").join("usr").join("bin").join("ssh-keygen.exe"),
        PathBuf::from("/Windows").join("System32").join("OpenSSH").join("ssh-keygen.exe"),
    ];
    assert_eq!(got, want);
    assert_eq!(keygen_candidates(Some(&path), "", None, None)[0], PathBuf::from("/a/bin").join("ssh-keygen"));
    assert!(keygen_candidates(None, "", None, None).is_empty());
}

#[test]
fn lists_only_complete_pairs() {
    let dir = temp("list");
    let write = |f: &str, t: &str| std::fs::write(dir.join(f), t).unwrap();
    write("id_ed25519", "private");
    write("id_ed25519.pub", "ssh-ed25519 AAAAC3Nz ada@laptop  \n");
    write("a_ed25519", "private");
    write("a_ed25519.pub", "ssh-ed25519 AAAAB other\n");
    write("id_rsa", "private, no pub");
    write("lone.pub", "ssh-ed25519 AAAA lone\n");
    write("junk", "private");
    write("junk.pub", "not a key\n");
    write("known_hosts", "host ssh-ed25519 AAAA\n");
    write("config", "Host *\n");
    let keys = list_key_pairs(&dir, None).unwrap();
    let names: Vec<String> = keys.iter().map(SshKeyPair::file_name).collect();
    assert_eq!(names, ["a_ed25519", "id_ed25519"], "sorted by name, complete pairs only");
    let k = &keys[1];
    assert_eq!(k.private, dir.join("id_ed25519"));
    assert_eq!(k.public, dir.join("id_ed25519.pub"));
    assert_eq!(k.comment, "ada@laptop");
    assert_eq!(k.public_key, "ssh-ed25519 AAAAC3Nz ada@laptop");
    assert_eq!((k.fingerprint.as_deref(), k.kind.as_str(), k.bits), (None, "ED25519", None));
    assert_eq!(list_key_pairs(&dir.join("missing"), None).unwrap(), vec![]);
    let _ = std::fs::remove_dir_all(&dir);
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
        std::fs::read_dir(&ssh).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains(".trellis-new")).collect();
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
