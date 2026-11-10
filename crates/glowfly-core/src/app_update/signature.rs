//! The minisign signature over a release's `SHA256SUMS` (`SHA256SUMS.minisig`), made by the
//! Release workflow with the project's key. No checksum in `SHA256SUMS` is trusted until the
//! signature verifies against one of the public keys pinned here and its trusted comment names
//! the release being installed (`Glowfly <tag>: SHA256SUMS`), so a download matches what the
//! project signed for that release, not only what the release page lists, and an older signed
//! list republished under a newer tag is refused (docs/RELEASING.md, Signing).

use super::{Asset, Release, UpdateError};
use minisign_verify::{PublicKey, Signature};

/// The signature's file name among the release's files.
pub(super) const SIGNATURE_FILE: &str = "SHA256SUMS.minisig";

/// The project's minisign public keys (base64, the second line of a `.pub` file). A signature
/// by any of them is accepted, so a key change ships the next key here one release before the
/// release secret switches to it, and drops the old one a release later.
pub(super) const KEYS: &[&str] = &[
    // 43C59A9629C2A686, since 2026-11-05.
    "RWSGpsIplprFQ/toKF1uFX1qZOySLsmrcaXgRecNj/C5j+CYKPZYMB1K",
];

/// The test key (C61175B0AC05E8D3). Its secret half is committed on purpose
/// (`minisign_fixtures/test-only.key`), so the tests and the installer checks can sign local
/// feeds. It is trusted only for a feed folder (GLOWFLY_UPDATE_FEED), and only in glowfly-core's
/// tests and builds with the `installer-check` feature (feed.rs, `Feed::TestKeyFolder`). A release
/// build never trusts it.
pub(super) const TEST_KEY: &str = "RWTT6AWssHURxjR/C+5C0lcSHiG4EvwQ/guMhtYrkrb3zLIErX70zLDk";

/// The largest signature file read (minisign's are about 300 bytes).
pub(super) const MAX_SIGNATURE: u64 = 4 * 1024;

/// The release's signature file, when it publishes one.
pub(super) fn signature_asset(release: &Release) -> Option<Asset> {
    release.assets.iter().find(|a| a.name == SIGNATURE_FILE).cloned()
}

/// The trusted comment the Release workflow signs a release's `SHA256SUMS` with (release.yml's
/// `minisign -S -t "Glowfly $TAG: SHA256SUMS"`).
pub(super) fn release_comment(tag: &str) -> String {
    format!("Glowfly {tag}: SHA256SUMS")
}

/// The trusted comment of `signature` (a `.minisig` file's text) when it is a valid signature of
/// `sums` by one of `keys`. Only pre-hashed signatures count (what minisign makes by default); the
/// legacy kind is refused. A key that doesn't parse is skipped. The comment is covered by the
/// signature, so it is only read once that verifies.
pub(super) fn signed_comment(sums: &[u8], signature: &[u8], keys: &[&str]) -> Result<String, UpdateError> {
    let text = std::str::from_utf8(signature).map_err(|_| UpdateError::BadSignature)?;
    let signature = Signature::decode(text).map_err(|_| UpdateError::BadSignature)?;
    let signed = keys.iter().filter_map(|k| PublicKey::from_base64(k).ok()).any(|key| key.verify(sums, &signature, false).is_ok());
    if signed { Ok(signature.trusted_comment().to_owned()) } else { Err(UpdateError::BadSignature) }
}

/// Whether `signature` is a valid signature of `sums` by one of `keys` ([`signed_comment`]) made
/// for the release `tag`: its trusted comment must be exactly [`release_comment`] of `tag` (byte
/// for byte, nothing trimmed). An empty `tag` is always refused.
pub(super) fn verify(sums: &[u8], signature: &[u8], keys: &[&str], tag: &str) -> Result<(), UpdateError> {
    let comment = signed_comment(sums, signature, keys)?;
    if !tag.is_empty() && comment == release_comment(tag) {
        return Ok(());
    }
    Err(UpdateError::SignedForAnotherRelease { tag: shown(tag), signed: signed_tag(&comment).map(shown) })
}

/// The tag a trusted comment of the form `Glowfly <tag>: SHA256SUMS` names, when it has that
/// form with a non-empty tag.
fn signed_tag(comment: &str) -> Option<&str> {
    comment.strip_prefix("Glowfly ")?.strip_suffix(": SHA256SUMS").filter(|t| !t.is_empty())
}

/// An untrusted tag as an error shows it: printable, at most 100 characters.
fn shown(s: &str) -> String {
    crate::text::printable(s).chars().take(100).collect()
}
