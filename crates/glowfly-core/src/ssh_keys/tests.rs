use super::*;
use std::path::{Path, PathBuf};

/// A fresh folder for one test (tests run in parallel).
pub(super) fn temp(name: &str) -> PathBuf {
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
fn read_key_pair_reads_a_pair_anywhere() {
    let dir = temp("pick");
    let write = |f: &str, t: &str| std::fs::write(dir.join(f), t).unwrap();
    write("deploy", "private");
    write("deploy.pub", "ssh-ed25519 AAAAdeploy deploy@ci\n");
    let by_private = read_key_pair(&dir.join("deploy"), None).unwrap();
    let by_public = read_key_pair(&dir.join("deploy.pub"), None).unwrap();
    assert_eq!(by_private.private, dir.join("deploy"));
    assert_eq!(by_public, by_private, "a .pub stands for the private key next to it");
    assert_eq!((by_private.comment.as_str(), by_private.public_key.as_str()), ("deploy@ci", "ssh-ed25519 AAAAdeploy deploy@ci"));

    write("lone", "private");
    assert_eq!(read_key_pair(&dir.join("lone"), None), Err(KeyFileProblem::NoPublicKey));
    write("junk", "private");
    write("junk.pub", "not a key\n");
    assert_eq!(read_key_pair(&dir.join("junk"), None), Err(KeyFileProblem::NotAPublicKey));
    std::fs::create_dir_all(dir.join("folder")).unwrap();
    assert_eq!(read_key_pair(&dir.join("folder"), None), Err(KeyFileProblem::NotAFile));
    assert_eq!(read_key_pair(&dir.join("missing"), None), Err(KeyFileProblem::NotAFile));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn list_appends_extra_keys() {
    let home = temp("extra");
    let ssh = home.join(".ssh");
    let outside = home.join("keys");
    std::fs::create_dir_all(&ssh).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(ssh.join("id_ed25519"), "private").unwrap();
    std::fs::write(ssh.join("id_ed25519.pub"), "ssh-ed25519 AAAAa ada\n").unwrap();
    std::fs::write(outside.join("work_key"), "private").unwrap();
    std::fs::write(outside.join("work_key.pub"), "ssh-ed25519 AAAAw work\n").unwrap();
    let extra = [outside.join("work_key"), ssh.join("id_ed25519"), outside.join("missing")];
    let got = list(&home, &home, &extra).unwrap();
    let names: Vec<String> = got.keys.iter().map(SshKeyPair::file_name).collect();
    assert_eq!(names, ["id_ed25519", "work_key"], "the duplicate and the missing extra are skipped");
    assert_eq!(got.keys[1].private, outside.join("work_key"));

    let plain = list(&home, &home, &[]).unwrap();
    assert_eq!(plain.keys, list_key_pairs(&ssh, plain.keygen.as_deref()).unwrap(), "no extras: the folder's pairs");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn default_identity_names() {
    let dir = PathBuf::from("/h/.ssh");
    let at = |folder: &Path, name: &str| SshKeyPair {
        private: folder.join(name),
        public: folder.join(format!("{name}.pub")),
        key_type: String::new(),
        kind: String::new(),
        bits: None,
        fingerprint: None,
        comment: String::new(),
        public_key: String::new(),
    };
    assert!(is_default_identity(&at(&dir, "id_ed25519"), &dir));
    assert!(is_default_identity(&at(&dir, "id_rsa"), &dir));
    assert!(!is_default_identity(&at(Path::new("/elsewhere"), "id_ed25519"), &dir), "another folder");
    for name in ["id_ED25519", "id_dsa", "id_ed25519_work"] {
        assert!(!is_default_identity(&at(&dir, name), &dir), "{name}");
    }
}
