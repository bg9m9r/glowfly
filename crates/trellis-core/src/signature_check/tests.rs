use super::*;
use crate::testutil::{empty_global_config, git, scratch_repo, ssh_signed_commit, write};

const FPR: &str = "0123456789ABCDEF0123456789ABCDEF01234567";

fn gpg(stdout: &str) -> SignatureCheck {
    parse_check(SigningFormat::OpenPgp, stdout, "").unwrap()
}

#[test]
fn good_gpg_signature() {
    let out = format!("G\0Ada <ada@example.com>\0ABCDEF0123456789\0{FPR}\0ultimate\0gpg: Good signature from \"Ada\"\n");
    let c = gpg(&out);
    assert_eq!(c.status, SigStatus::Good);
    assert_eq!(c.verdict(), Verdict::Verified);
    assert_eq!(c.signer, "Ada <ada@example.com>");
    assert_eq!(c.key, "ABCDEF0123456789");
    assert_eq!(c.fingerprint, FPR);
    assert_eq!(c.trust.as_deref(), Some("ultimate"));
    assert!(c.note.is_empty(), "no note for a good signature");
    assert_eq!(c.reason(), "Good signature from a key you trust.");
    let d = c.details();
    assert_eq!(d.iter().map(|(k, _)| *k).collect::<Vec<_>>(), ["Signer", "Key", "Fingerprint", "Trust"]);
}

#[test]
fn every_letter_maps_to_a_status_and_verdict() {
    let cases = [
        ('G', SigStatus::Good, Verdict::Verified),
        ('U', SigStatus::GoodUntrusted, Verdict::Signed),
        ('X', SigStatus::ExpiredSignature, Verdict::Expired),
        ('Y', SigStatus::ExpiredKey, Verdict::Expired),
        ('R', SigStatus::RevokedKey, Verdict::Revoked),
        ('B', SigStatus::Bad, Verdict::Bad),
        ('E', SigStatus::CannotCheck, Verdict::Signed),
        ('N', SigStatus::NotChecked, Verdict::Signed),
    ];
    for (letter, status, verdict) in cases {
        let c = gpg(&format!("{letter}\0Ada\0ABCDEF0123456789\0{FPR}\0marginal\0gpg: something\n"));
        assert_eq!((c.status, c.verdict()), (status, verdict), "{letter}");
    }
    assert_eq!(gpg("X\0\0\0\0\0\n").reason(), "The signature has expired.");
    assert_eq!(gpg("Y\0\0\0\0\0\n").reason(), "The key that made the signature has expired.");
    assert_eq!(gpg("R\0\0\0\0\0\n").reason(), "The key that made the signature has been revoked.");
    assert!(gpg("B\0\0\0\0never\0Could not verify\n").reason().starts_with("The signature doesn't match this commit"));
    assert_eq!(Verdict::Bad.label(), "Bad signature");
}

#[test]
fn missing_public_key() {
    let out = "E\0\0ABCDEF0123456789\0\0undefined\0gpg: Signature made Mon\ngpg: Can't check signature: No public key\n";
    let c = gpg(out);
    assert_eq!(c.status, SigStatus::CannotCheck);
    assert_eq!(c.note, "gpg: Can't check signature: No public key");
    assert_eq!(c.reason(), "The signer's public key isn't in your keyring, so the signature can't be checked.");
    let ssh = parse_check(SigningFormat::Ssh, "E\0\0\0\0\0ssh-keygen: broken\n", "").unwrap();
    assert_eq!(ssh.reason(), "Git couldn't check the signature: ssh-keygen: broken");
}

#[test]
fn no_allowed_signers_file() {
    let out = "N\0\0\0\0undefined\0\n";
    let stderr = "error: gpg.ssh.allowedSignersFile needs to be configured and exist for ssh signature verification\n";
    let c = parse_check(SigningFormat::Ssh, out, stderr).unwrap();
    assert_eq!(c.status, SigStatus::NoAllowedSigners);
    assert_eq!(c.verdict(), Verdict::Signed);
    assert!(c.reason().contains("Preferences > Commit signing"), "{}", c.reason());
    assert!(c.reason().contains("gpg.ssh.allowedSignersFile"));
    assert!(c.details().is_empty());
}

#[test]
fn signing_program_missing() {
    let c = parse_check(SigningFormat::OpenPgp, "N\0\0\0\0undefined\0\n", "error: cannot run gpg: No such file or directory\n").unwrap();
    assert_eq!(c.status, SigStatus::NotChecked);
    assert_eq!(c.note, "cannot run gpg: No such file or directory");
    assert_eq!(c.reason(), "Git couldn't check the signature: cannot run gpg: No such file or directory");
}

#[test]
fn odd_output() {
    assert!(parse_check(SigningFormat::OpenPgp, "", "").is_err());
    assert!(parse_check(SigningFormat::OpenPgp, "\n", "").is_err());
    let c = gpg("Q\0\0\0\0\0\n");
    assert_eq!(c.status, SigStatus::NotChecked, "an unknown letter");
    assert_eq!(c.reason(), "Git couldn't check the signature.");
    assert_eq!(gpg("U\0Ada\0K\0K\0undefined\0x\n").trust, None);
    assert_eq!(gpg("U\0Ada\0K\0K\0\0x\n").trust, None);
    // Control and bidi characters in the signer are dropped.
    let c = gpg("U\0Ada\u{202E}\u{7}Evil\0K\0K\0marginal\0x\n");
    assert_eq!(c.signer, "AdaEvil");
    // Only the one trailing newline goes; %GG's own lines stay, and the note is its last line.
    let c = gpg("U\0\0K\0K\0undefined\0Good \"git\" signature with ED25519 key SHA256:x\nNo principal matched.\n\n");
    assert_eq!(c.note, "No principal matched.");
}

#[test]
fn untrusted_reasons_differ_by_format() {
    let out = "U\0\0SHA256:abc\0SHA256:abc\0undefined\0Good \"git\" signature\nNo principal matched.\n";
    let ssh = parse_check(SigningFormat::Ssh, out, "").unwrap();
    let pgp = parse_check(SigningFormat::OpenPgp, out, "").unwrap();
    assert!(ssh.reason().contains("allowed signers file"), "{}", ssh.reason());
    assert!(pgp.reason().contains("trusted in GPG"), "{}", pgp.reason());
    // For SSH the fingerprint is the key: shown once. The empty signer is left out.
    assert_eq!(ssh.details(), vec![("Key", "SHA256:abc".to_owned())]);
    let ssh_good = parse_check(SigningFormat::Ssh, "G\0ada@example.com\0SHA256:abc\0SHA256:abc\0fully\0ok\n", "").unwrap();
    assert!(ssh_good.reason().contains("allowed signers file lists"));
    assert!(ssh_good.details().iter().all(|(k, _)| *k != "Trust"), "trust is shown for OpenPGP only");
}

#[test]
fn armor_formats() {
    assert_eq!(format_of(b"-----BEGIN PGP SIGNATURE-----\n\nabc"), SigningFormat::OpenPgp);
    assert_eq!(format_of(b"-----BEGIN PGP MESSAGE-----\n"), SigningFormat::OpenPgp);
    assert_eq!(format_of(b"-----BEGIN SSH SIGNATURE-----\nU1NIU0lH"), SigningFormat::Ssh);
    assert_eq!(format_of(b"-----BEGIN SIGNED MESSAGE-----\n"), SigningFormat::X509);
}

#[test]
fn unsigned_commit_has_no_signature() {
    let dir = scratch_repo("sig-unsigned");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let repo = git2::Repository::open(&dir).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    assert_eq!(signature_format(&repo, head), None);
    assert_eq!(crate::repo::commit_detail(&dir, head).unwrap().signature, None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Runs the check with the developer's global and system config left out.
fn check(dir: &Path, oid: Oid) -> SignatureCheck {
    let env = empty_global_config(dir);
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    verify_commit_with(dir, oid, SigningFormat::Ssh, &env).unwrap()
}

#[test]
fn ssh_signatures_checked_by_git() {
    let dir = scratch_repo("sig-ssh");
    let Some((oid, public)) = ssh_signed_commit(&dir, "a.txt", "signed") else { return };
    let repo = git2::Repository::open(&dir).unwrap();
    assert_eq!(signature_format(&repo, oid), Some(SigningFormat::Ssh));
    assert_eq!(crate::repo::commit_detail(&dir, oid).unwrap().signature, Some(SigningFormat::Ssh));
    let key = std::fs::read_to_string(&public).unwrap();

    // (a) No allowed signers file.
    let c = check(&dir, oid);
    assert_eq!(c.status, SigStatus::NoAllowedSigners, "{c:?}");

    // (b) The file lists only another key.
    let allowed = dir.with_file_name(format!("{}-allowed", dir.file_name().unwrap().to_string_lossy()));
    let other = allowed.with_file_name(format!("{}-other-key", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_file(&other);
    let made = std::process::Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-f"]).arg(&other).output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let other_key = std::fs::read_to_string(format!("{}.pub", other.display())).unwrap();
    std::fs::write(&allowed, format!("test@example.com {other_key}")).unwrap();
    git(&dir, &["config", "gpg.ssh.allowedSignersFile", &allowed.display().to_string()]);
    let c = check(&dir, oid);
    assert_eq!(c.status, SigStatus::GoodUntrusted, "{c:?}");
    assert!(c.key.starts_with("SHA256:"), "{c:?}");

    // (c) The file lists this key for the signer's email.
    std::fs::write(&allowed, format!("test@example.com {key}")).unwrap();
    let c = check(&dir, oid);
    assert_eq!(c.status, SigStatus::Good, "{c:?}");
    assert_eq!(c.signer, "test@example.com");

    // (d) log.showSignature doesn't get in the way.
    git(&dir, &["config", "log.showSignature", "true"]);
    assert_eq!(check(&dir, oid).status, SigStatus::Good);

    // (e) A changed copy of the commit no longer matches its signature.
    let (ok, raw) = crate::gitcli::run(&dir, &crate::gitcli::args(&["cat-file", "commit", &oid.to_string()]));
    assert!(ok, "{raw}");
    let tampered = raw.replace("\n\nsigned", "\n\ntampered");
    assert_ne!(tampered, raw);
    let a = crate::gitcli::args(&["hash-object", "-t", "commit", "-w", "--stdin"]);
    let (ok, out) = crate::gitcli::run_with_input(&dir, &a, &tampered, &[]);
    assert!(ok, "{out}");
    let bad = Oid::from_str(out.trim()).unwrap();
    assert_eq!(check(&dir, bad).status, SigStatus::Bad);

    let _ = std::fs::remove_dir_all(&dir);
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    for f in ["key", "key.pub", "gitconfig", "allowed", "other-key", "other-key.pub"] {
        let _ = std::fs::remove_file(dir.with_file_name(format!("{name}-{f}")));
    }
}
