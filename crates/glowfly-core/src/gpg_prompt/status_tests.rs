use super::status::{SignResult, classify};

#[test]
fn a_created_signature() {
    let err = "[GNUPG:] KEY_CONSIDERED 0123456789ABCDEF0123456789ABCDEF01234567 2\n\
               [GNUPG:] BEGIN_SIGNING H10\n\
               [GNUPG:] SIG_CREATED D 1 10 00 1750000000 0123456789ABCDEF0123456789ABCDEF01234567\n";
    assert_eq!(classify(true, err), SignResult::Created);
    assert_eq!(classify(false, err), SignResult::Failed, "a failed exit is not a signature");
}

#[test]
fn passphrase_outcomes() {
    let no_pinentry = "gpg: signing failed: No pinentry\n[GNUPG:] FAILURE sign 67108949\n";
    assert_eq!(classify(false, no_pinentry), SignResult::NeedsPassphrase);
    assert_eq!(classify(false, "[GNUPG:] BAD_PASSPHRASE ABCD\n"), SignResult::BadPassphrase);
    assert_eq!(classify(false, "gpg: signing failed: Bad passphrase\n[GNUPG:] FAILURE sign 67108875\n"), SignResult::BadPassphrase);
    assert_eq!(classify(false, "[GNUPG:] FAILURE sign 67108963\n"), SignResult::Cancelled);
    assert_eq!(classify(false, "[GNUPG:] ERROR pkdecrypt_failed 67108924\n"), SignResult::Unsupported, "low bits 60");
}

#[test]
fn old_gnupg_and_other_failures() {
    assert_eq!(classify(false, "gpg: invalid option \"--pinentry-mode\"\n"), SignResult::Unsupported);
    assert_eq!(classify(false, "gpg: unknown option 'pinentry-mode'\n"), SignResult::Unsupported);
    let missing = "gpg: skipped \"K\": No secret key\n[GNUPG:] INV_SGNR 9 K\n[GNUPG:] FAILURE sign 17\n\
                   gpg: signing failed: No secret key\n";
    assert_eq!(classify(false, missing), SignResult::Failed);
    assert_eq!(classify(false, ""), SignResult::Failed);
}
