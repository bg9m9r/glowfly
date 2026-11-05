//! The minisign signature over `SHA256SUMS`: the committed fixtures (made once with the minisign
//! CLI) verify and refuse as they should, `sign` (the tests' signer for local feeds, with the
//! committed test key) makes what minisign makes, and a download goes ahead only with a signed
//! list that names the file with its digest. No network: feeds are folders or the mock server.

use super::feed_tests::{MSI, is_empty, offer_for, scratch, sum_line, write_feed, write_feed_signed};
use super::signature::{KEYS, SIGNATURE_FILE, TEST_KEY, verify};
use super::*;
use crate::forge::mock::{Mock, Reply};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use blake2::{Blake2b512, Digest};

const SUMS: &[u8] = include_bytes!("minisign_fixtures/SHA256SUMS");
const SIGNED: &[u8] = include_bytes!("minisign_fixtures/SHA256SUMS.minisig");
const BY_ANOTHER_KEY: &[u8] = include_bytes!("minisign_fixtures/SHA256SUMS.other-key.minisig");
/// The public half of the key `SHA256SUMS.other-key.minisig` was made with (its secret half was
/// thrown away).
const ANOTHER_KEY: &str = "RWR/SI3TjcZ6R5dt2pHd7WY9da2KCrFEV2n8JpBIZga6h9lXCsulRt5L";
/// TEST KEY ONLY: the committed secret key the tests and the installer checks sign local feeds
/// with. Glowfly trusts it for a feed folder, never for the hosting service.
const TEST_SECRET: &str = include_str!("minisign_fixtures/test-only.key");
const TEST_PUBLIC: &str = include_str!("minisign_fixtures/test-only.pub");
const MSI_BYTES: &[u8] = b"pretend msi bytes";

/// The key number and Ed25519 seed of an unencrypted minisign secret key file (`minisign -G
/// -W`): its second line is base64 of the algorithm, KDF and checksum names, the unused KDF
/// salt and limits, then the key number, the secret key and a checksum.
fn secret_key(file: &str) -> ([u8; 8], ed25519_compact::KeyPair) {
    let line = file.lines().nth(1).expect("a key line");
    let bin = STANDARD.decode(line.trim()).expect("base64");
    assert_eq!((&bin[0..2], &bin[2..4], bin.len()), (&b"Ed"[..], &[0u8, 0][..], 158), "an unencrypted Ed25519 key");
    let keynum: [u8; 8] = bin[54..62].try_into().unwrap();
    let seed = ed25519_compact::Seed::from_slice(&bin[62..94]).unwrap();
    (keynum, ed25519_compact::KeyPair::from_seed(seed))
}

/// A minisign signature of `bytes` by the test key, as `minisign -S` writes it: Ed25519 over
/// the BLAKE2b-512 of the bytes (`prehashed`; false gives the legacy kind, over the bytes
/// themselves), then over that signature and the trusted comment.
fn sign_as(bytes: &[u8], prehashed: bool) -> String {
    let (keynum, pair) = secret_key(TEST_SECRET);
    let (alg, signature) =
        if prehashed { (b"ED", pair.sk.sign(Blake2b512::digest(bytes), None)) } else { (b"Ed", pair.sk.sign(bytes, None)) };
    let comment = "file:SHA256SUMS test feed";
    let global = pair.sk.sign([&signature[..], comment.as_bytes()].concat(), None);
    let first = STANDARD.encode([&alg[..], &keynum, &signature[..]].concat());
    format!("untrusted comment: TEST KEY ONLY\n{first}\ntrusted comment: {comment}\n{}\n", STANDARD.encode(&global[..]))
}

/// A minisign signature of `bytes` by the test key.
pub(super) fn sign(bytes: &[u8]) -> String {
    sign_as(bytes, true)
}

#[test]
fn the_pinned_keys_are_minisign_keys() {
    let ids: Vec<String> = KEYS.iter().map(|k| key_id(k)).collect();
    assert_eq!(ids, ["43C59A9629C2A686"], "the project's key");
    assert_eq!(key_id(TEST_KEY), "C61175B0AC05E8D3");
    assert_eq!(TEST_PUBLIC.lines().nth(1), Some(TEST_KEY), "the committed test key's public half");
    assert!(TEST_SECRET.starts_with("untrusted comment: TEST KEY ONLY"), "labelled as a test key");
    assert!(!KEYS.contains(&TEST_KEY));
    // The manual check in the docs, and the key the release job checks its signature with, are
    // the key the app pins.
    let docs = [
        ("README.md", include_str!("../../../../README.md")),
        ("RELEASING.md", include_str!("../../../../docs/RELEASING.md")),
        ("release-notes-header.md", include_str!("../../../../packaging/release-notes-header.md")),
    ];
    for (doc, text) in docs {
        assert!(KEYS.iter().all(|k| text.contains(&format!("minisign -Vm SHA256SUMS -P {k}"))), "{doc} shows the manual check");
    }
    let public = include_str!("../../../../packaging/glowfly-minisign.pub");
    let line = public.lines().nth(1).expect("a key line");
    assert!(KEYS.contains(&line), "packaging/glowfly-minisign.pub is a pinned key");
    let comment = public.lines().next().unwrap_or("");
    assert_eq!(comment, format!("untrusted comment: minisign public key {}", key_id(line)));
}

/// A public key's ID as minisign prints it.
fn key_id(key: &str) -> String {
    assert!(minisign_verify::PublicKey::from_base64(key).is_ok(), "{key}");
    let bin = STANDARD.decode(key).unwrap();
    bin[2..10].iter().rev().map(|b| format!("{b:02X}")).collect()
}

#[test]
fn a_good_signature_verifies() {
    assert_eq!(verify(SUMS, SIGNED, &[TEST_KEY]), Ok(()), "the minisign CLI's signature");
    assert_eq!(verify(SUMS, SIGNED, &[ANOTHER_KEY, TEST_KEY]), Ok(()), "by any of the keys");
    assert_eq!(verify(SUMS, sign(SUMS).as_bytes(), &[TEST_KEY]), Ok(()), "the tests' signer");
    let crlf = String::from_utf8(SIGNED.to_vec()).unwrap().replace('\n', "\r\n");
    assert_eq!(verify(SUMS, crlf.as_bytes(), &[TEST_KEY]), Ok(()), "a signature with CRLF lines");
}

#[test]
fn a_tampered_sha256sums_is_refused() {
    let mut changed = SUMS.to_vec();
    changed[0] = b'3';
    assert_eq!(verify(&changed, SIGNED, &[TEST_KEY]), Err(UpdateError::BadSignature));
    assert_eq!(
        verify(&[SUMS, b"3333333333333333333333333333333333333333333333333333333333333333  extra.msi\n"].concat(), SIGNED, &[TEST_KEY]),
        Err(UpdateError::BadSignature),
        "a line added"
    );
    // A changed trusted comment breaks the second signature.
    let text = String::from_utf8(SIGNED.to_vec()).unwrap().replace("test fixture", "something else");
    assert_eq!(verify(SUMS, text.as_bytes(), &[TEST_KEY]), Err(UpdateError::BadSignature));
    for garbage in [&b""[..], b"not a signature", b"\xff\xfe", &SIGNED[..SIGNED.len() / 2]] {
        assert_eq!(verify(SUMS, garbage, &[TEST_KEY]), Err(UpdateError::BadSignature), "{garbage:?}");
    }
    assert_eq!(verify(SUMS, sign_as(SUMS, false).as_bytes(), &[TEST_KEY]), Err(UpdateError::BadSignature), "the legacy kind");
}

#[test]
fn a_signature_from_a_different_key_is_refused() {
    assert_eq!(verify(SUMS, BY_ANOTHER_KEY, &[ANOTHER_KEY]), Ok(()), "valid, by its own key");
    assert_eq!(verify(SUMS, BY_ANOTHER_KEY, &[TEST_KEY]), Err(UpdateError::BadSignature));
    assert_eq!(verify(SUMS, BY_ANOTHER_KEY, KEYS), Err(UpdateError::BadSignature));
    assert_eq!(verify(SUMS, SIGNED, KEYS), Err(UpdateError::BadSignature), "the test key isn't one of the project's");
    assert_eq!(verify(SUMS, SIGNED, &[]), Err(UpdateError::BadSignature), "no keys");
    assert_eq!(verify(SUMS, SIGNED, &["not a key"]), Err(UpdateError::BadSignature));
}

#[test]
fn only_a_feed_folder_trusts_the_test_key() {
    assert_eq!(Feed::Hosted(REPOSITORY.to_owned()).trusted_keys(), KEYS);
    let folder = Feed::Folder(crate::paths::temp_dir().join("feed")).trusted_keys();
    assert_eq!(folder, [KEYS, &[TEST_KEY]].concat());
}

/// A feed folder in `dir` with the MSI and `sums` signed as `signature` (None: no signature),
/// and the download folder beside it.
fn signed_feed(dir: &std::path::Path, sums: &str, signature: Option<String>) -> (Feed, std::path::PathBuf) {
    let feed_dir = dir.join("feed");
    std::fs::create_dir_all(&feed_dir).unwrap();
    write_feed_signed(&feed_dir, "v0.1.1", &[(MSI, MSI_BYTES)], Some(sums.to_owned()), signature);
    (Feed::Folder(feed_dir), dir.join("dl"))
}

#[test]
fn a_feed_signed_with_the_test_key_downloads() {
    let dir = scratch("signed");
    let sums = sum_line(MSI, MSI_BYTES);
    let (feed, dl) = signed_feed(&dir, &sums, Some(sign(sums.as_bytes())));
    let o = offer_for(&feed, InstallKind::Installer);
    assert!(o.release.assets.iter().any(|a| a.name == SIGNATURE_FILE));
    assert_eq!(std::fs::read(download(&feed, &o, &dl).unwrap()).unwrap(), MSI_BYTES);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_feed_with_a_bad_signature_downloads_nothing() {
    let dir = scratch("badsig");
    let sums = sum_line(MSI, MSI_BYTES);
    // Signed, then changed: another digest for the same file.
    let (feed, dl) = signed_feed(&dir, &sum_line(MSI, b"the real msi"), Some(sign(sums.as_bytes())));
    let err = download(&feed, &offer_for(&feed, InstallKind::Installer), &dl).unwrap_err();
    assert_eq!(err, UpdateError::BadSignature);
    assert_eq!(err.to_string(), "This release's checksums aren't signed with Glowfly's key, so nothing was downloaded.");
    assert!(is_empty(&dl));
    // Signed by a key the feed doesn't trust.
    let (feed, dl) = signed_feed(&dir, &sums, Some(String::from_utf8(BY_ANOTHER_KEY.to_vec()).unwrap()));
    assert_eq!(download(&feed, &offer_for(&feed, InstallKind::Installer), &dl), Err(UpdateError::BadSignature));
    assert!(is_empty(&dl));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_signature_downloads_nothing() {
    let dir = scratch("unsigned");
    let sums = sum_line(MSI, MSI_BYTES);
    let (feed, dl) = signed_feed(&dir, &sums, None);
    let o = offer_for(&feed, InstallKind::Installer);
    assert!(matches!(&o.sums, SumsSource::Combined(_)), "SHA256SUMS alone");
    let err = download(&feed, &o, &dl).unwrap_err();
    assert_eq!(err, UpdateError::Unsigned);
    assert_eq!(err.to_string(), "This release's checksums aren't signed, so nothing was downloaded.");
    assert!(is_empty(&dl));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_asset_not_in_the_signed_list_downloads_nothing() {
    let dir = scratch("unlisted");
    let sums = sum_line("glowfly-v0.1.1-x86_64-pc-windows-msvc.zip", b"zip");
    let (feed, dl) = signed_feed(&dir, &sums, Some(sign(sums.as_bytes())));
    let err = download(&feed, &offer_for(&feed, InstallKind::Installer), &dl).unwrap_err().to_string();
    assert!(err.contains("doesn't publish a checksum") && err.contains(MSI), "{err}");
    // Listed with another digest than the file has: the signed list wins.
    let sums = sum_line(MSI, b"what the project signed");
    let (feed, dl) = signed_feed(&dir, &sums, Some(sign(sums.as_bytes())));
    assert_eq!(download(&feed, &offer_for(&feed, InstallKind::Installer), &dl), Err(UpdateError::ChecksumMismatch));
    assert!(is_empty(&dl));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_hosted_feed_refuses_the_test_key_before_fetching_the_file() {
    let sums = sum_line(MSI, MSI_BYTES);
    let signature = sign(sums.as_bytes());
    let mock = Mock::start(move |req| match req.path.rsplit('/').next().unwrap_or("") {
        "SHA256SUMS" => Reply::text(&sums),
        "SHA256SUMS.minisig" => Reply::text(&signature),
        _ => Reply::text(std::str::from_utf8(MSI_BYTES).unwrap()),
    });
    let release = super::tests::release("v0.1.1", &[MSI, "SHA256SUMS", SIGNATURE_FILE]);
    let o = offer(&release, &Version::parse("0.1.0").unwrap(), Some(super::tests::WIN), InstallKind::Installer).expect("newer");
    let dir = scratch("hosted");
    let dl = dir.join("dl");
    assert_eq!(download(&Feed::Hosted(REPOSITORY.to_owned()), &o, &dl), Err(UpdateError::BadSignature));
    let paths: Vec<String> = mock.requests().iter().map(|r| r.path.rsplit('/').next().unwrap_or("").to_owned()).collect();
    assert_eq!(paths, ["SHA256SUMS", SIGNATURE_FILE], "the file itself was never asked for");
    assert!(mock.requests().iter().all(|r| r.host == "github.com" && r.header("Authorization").is_none()));
    assert!(is_empty(&dl));
    // Published, but the signature can't be fetched.
    drop(mock);
    let _mock = Mock::start(|req| if req.path.ends_with("/SHA256SUMS") { Reply::text("x") } else { Reply::not_found() });
    let err = download(&Feed::Hosted(REPOSITORY.to_owned()), &o, &dl).unwrap_err();
    assert!(matches!(&err, UpdateError::SignatureUnreadable(_)), "{err:?}");
    assert!(err.to_string().starts_with("Couldn't read the checksums' signature: "), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_feed_signs_with_the_test_key() {
    let dir = scratch("helper");
    let sums = sum_line(MSI, MSI_BYTES);
    write_feed(&dir, "v0.1.1", &[(MSI, MSI_BYTES)], Some(sums.clone()));
    let signature = std::fs::read(dir.join(SIGNATURE_FILE)).unwrap();
    assert_eq!(verify(sums.as_bytes(), &signature, &[TEST_KEY]), Ok(()));
    let _ = std::fs::remove_dir_all(&dir);
}
